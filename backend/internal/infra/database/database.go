package database

import (
	"encoding/json"
	"errors"
	"fmt"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/constant"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/util"
	"log"
	"os"
	"time"

	_ "gorm.io/driver/mysql"
	_ "gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/logger"
)

const (
	MYSQL  = "mysql"
	SQLITE = "sqlite"
)

type DbClient interface {
	Connect() (db *gorm.DB, err error)
}

func DbInit(dbClient DbClient) (db *gorm.DB, dbErr error) {
	db, dbErr = dbClient.Connect()
	if dbErr != nil {
		return
	}

	dbErr = initDatabase(db)
	if dbErr != nil {
		return nil, fmt.Errorf("database CreateDatabase error, %w", dbErr)
	}

	return
}

func initDatabase(db *gorm.DB) (err error) {
	if db.Dialector.Name() == SQLITE {
		var check string
		if err := db.Raw("PRAGMA quick_check").Scan(&check).Error; err != nil {
			return err
		}
		if check != "ok" {
			return fmt.Errorf("sqlite integrity check failed: %s", check)
		}
		log.Printf("SQLite integrity check: %s", check)
	}
	if err := PrepareMigrationBackup(db); err != nil {
		return fmt.Errorf("prepare migration backup: %w", err)
	}
	// 创建数据表
	err = db.AutoMigrate(
		&repository.User{},
		&repository.OAuthIdentity{},
		&repository.Space{},
		&repository.SpaceMember{},
		&repository.Team{},
		&repository.SpaceOIDCGroup{},
		&repository.SystemSetting{},
		&repository.ItemIcon{},
		&repository.UserConfig{},
		&repository.File{},
		&repository.ItemIconGroup{},
		&repository.ModuleConfig{},
	)

	if err != nil {
		return err
	}
	// Backfill email accounts from the legacy username column before the
	// application stops reading username.
	if db.Migrator().HasColumn("user", "username") {
		if err := db.Exec("UPDATE user SET mail = username WHERE (mail IS NULL OR mail = '') AND username IS NOT NULL AND username <> ''").Error; err != nil {
			return err
		}
	}
	if err := normalizeEmptyPublicIDs(db); err != nil {
		return err
	}
	if err := EnsurePersonalSpaces(db); err != nil {
		return err
	}
	// SQLite unique indexes reject multiple empty strings; NULL clears legacy
	// public links while allowing every user to remain link-free.
	if err := db.Exec("UPDATE user SET publiccode = NULL WHERE publiccode <> ''").Error; err != nil {
		return err
	}
	if !global.Config.Base.EnableMonitor {
		if err := disableUserMonitorConfigs(db); err != nil {
			return err
		}
	}
	if err := backfillPanelSpaces(db); err != nil {
		return err
	}
	if err := mergeDuplicatePersonalSpaces(db); err != nil {
		return err
	}
	if err := cleanupOrphanSpaceData(db); err != nil {
		return err
	}
	if err := ensureSpacePairs(db); err != nil {
		return err
	}
	return RunMigration(db)
}

func cleanupOrphanSpaceData(db *gorm.DB) error {
	var groups []repository.ItemIconGroup
	if err := db.Find(&groups).Error; err != nil {
		return err
	}
	for _, group := range groups {
		var count int64
		if err := db.Model(&repository.Space{}).Where("id = ?", group.SpaceID).Count(&count).Error; err != nil {
			return err
		}
		if count == 0 {
			if err := db.Where("item_icon_group_id = ?", group.ID).Delete(&repository.ItemIcon{}).Error; err != nil {
				return err
			}
			if err := db.Delete(&group).Error; err != nil {
				return err
			}
		}
	}
	return nil
}

// mergeDuplicatePersonalSpaces keeps the oldest personal Yin space and folds
// later accidental accounts/spaces into its pair before normal pairing runs.
func mergeDuplicatePersonalSpaces(db *gorm.DB) error {
	var spaces []repository.Space
	if err := db.Where("type = ? AND (side = ? OR side = '' OR side IS NULL)", repository.SpaceTypePersonal, "yin").Order("owner_user_id, created_at, id").Find(&spaces).Error; err != nil {
		return err
	}
	canonical := make(map[uint]repository.Space)
	return db.Transaction(func(tx *gorm.DB) error {
		for _, duplicate := range spaces {
			keep, ok := canonical[duplicate.OwnerUserID]
			if !ok {
				canonical[duplicate.OwnerUserID] = duplicate
				continue
			}
			if err := mergeSpaceData(tx, keep.ID, duplicate.ID); err != nil {
				return err
			}
		}
		return nil
	})
}

func mergeSpaceData(tx *gorm.DB, targetID, duplicateID uint) error {
	var groups []repository.ItemIconGroup
	if err := tx.Where("space_id = ?", duplicateID).Find(&groups).Error; err != nil {
		return err
	}
	for _, group := range groups {
		oldID := group.ID
		group.ID = 0
		group.SpaceID = targetID
		var count int64
		if err := tx.Model(&repository.ItemIconGroup{}).Where("space_id = ? AND title = ?", targetID, group.Title).Count(&count).Error; err != nil {
			return err
		}
		if count > 0 {
			group.Title = group.Title + " (merged)"
		}
		if err := tx.Create(&group).Error; err != nil {
			return err
		}
		if err := tx.Model(&repository.ItemIcon{}).Where("space_id = ? AND item_icon_group_id = ?", duplicateID, oldID).Updates(map[string]any{"space_id": targetID, "item_icon_group_id": group.ID}).Error; err != nil {
			return err
		}
		if err := tx.Delete(&repository.ItemIconGroup{}, oldID).Error; err != nil {
			return err
		}
	}
	if err := tx.Model(&repository.ItemIcon{}).Where("space_id = ?", duplicateID).Update("space_id", targetID).Error; err != nil {
		return err
	}
	var members []repository.SpaceMember
	if err := tx.Where("space_id = ?", duplicateID).Find(&members).Error; err != nil {
		return err
	}
	for _, member := range members {
		var existing repository.SpaceMember
		err := tx.Where("space_id = ? AND user_id = ?", targetID, member.UserID).First(&existing).Error
		if errors.Is(err, gorm.ErrRecordNotFound) {
			member.ID = 0
			member.SpaceID = targetID
			if err := tx.Create(&member).Error; err != nil {
				return err
			}
		} else if err != nil {
			return err
		} else if roleRank(member.Role) > roleRank(existing.Role) {
			if err := tx.Model(&existing).Updates(map[string]any{"role": member.Role, "source": member.Source}).Error; err != nil {
				return err
			}
		}
		if err := tx.Where("space_id = ? AND user_id = ?", duplicateID, member.UserID).Delete(&repository.SpaceMember{}).Error; err != nil {
			return err
		}
	}
	var rules []repository.SpaceOIDCGroup
	if err := tx.Where("space_id = ?", duplicateID).Find(&rules).Error; err != nil {
		return err
	}
	for _, rule := range rules {
		var existing repository.SpaceOIDCGroup
		err := tx.Where("space_id = ? AND provider = ? AND group_name = ?", targetID, rule.Provider, rule.GroupName).First(&existing).Error
		if errors.Is(err, gorm.ErrRecordNotFound) {
			rule.ID = 0
			rule.SpaceID = targetID
			if err := tx.Create(&rule).Error; err != nil {
				return err
			}
		} else if err != nil {
			return err
		} else if roleRank(rule.Role) > roleRank(existing.Role) {
			if err := tx.Model(&existing).Update("role", rule.Role).Error; err != nil {
				return err
			}
		}
	}
	if err := tx.Where("space_id = ?", duplicateID).Delete(&repository.SpaceOIDCGroup{}).Error; err != nil {
		return err
	}
	var duplicate, target repository.Space
	if err := tx.First(&duplicate, duplicateID).Error; err != nil {
		return err
	}
	if err := tx.First(&target, targetID).Error; err != nil {
		return err
	}
	updates := map[string]any{}
	if target.PublicID == nil && duplicate.PublicID != nil {
		updates["public_id"] = duplicate.PublicID
	}
	if !target.PublicEnabled && duplicate.PublicEnabled {
		updates["public_enabled"] = true
	}
	if target.PublicMode == "direct" && duplicate.PublicMode != "direct" {
		updates["public_mode"] = duplicate.PublicMode
	}
	if target.PublicCodeHash == "" && duplicate.PublicCodeHash != "" {
		updates["public_code_hash"] = duplicate.PublicCodeHash
	}
	if len(updates) > 0 {
		if err := tx.Model(&target).Updates(updates).Error; err != nil {
			return err
		}
	}
	var duplicateYang repository.Space
	if tx.Where("pair_id = ? AND side = ?", duplicateID, "yang").First(&duplicateYang).Error == nil {
		var targetYang repository.Space
		if tx.Where("pair_id = ? AND side = ?", targetID, "yang").First(&targetYang).Error == nil {
			if err := mergeSpaceData(tx, targetYang.ID, duplicateYang.ID); err != nil {
				return err
			}
			if err := tx.Delete(&repository.Space{}, duplicateYang.ID).Error; err != nil {
				return err
			}
		}
	}
	return tx.Delete(&repository.Space{}, duplicateID).Error
}

func roleRank(role string) int {
	switch role {
	case repository.SpaceRoleAdmin:
		return 3
	case repository.SpaceRoleEditor:
		return 2
	case repository.SpaceRoleViewer:
		return 1
	default:
		return 0
	}
}

// ensureSpacePairs upgrades legacy spaces in place. PairID always points at
// the Yin row, so the original space ID remains stable for existing clients.
func ensureSpacePairs(db *gorm.DB) error {
	var spaces []repository.Space
	if err := db.Where("side = ? OR side = '' OR side IS NULL", "yin").Find(&spaces).Error; err != nil {
		return err
	}
	for _, yin := range spaces {
		if yin.PairID == 0 {
			if err := db.Model(&repository.Space{}).Where("id = ?", yin.ID).Updates(map[string]any{"pair_id": yin.ID, "side": "yin"}).Error; err != nil {
				return err
			}
		}
		var yang repository.Space
		if err := db.Where("pair_id = ? AND side = ?", yin.ID, "yang").First(&yang).Error; err == nil {
			continue
		} else if !errors.Is(err, gorm.ErrRecordNotFound) {
			return err
		}
		yang = repository.Space{Type: yin.Type, Name: yin.Name + "-B", OwnerUserID: yin.OwnerUserID, TeamID: yin.TeamID, PairID: yin.ID, Side: "yang"}
		if err := db.Create(&yang).Error; err != nil {
			return err
		}
		var members []repository.SpaceMember
		if err := db.Where("space_id = ?", yin.ID).Find(&members).Error; err != nil {
			return err
		}
		for _, m := range members {
			m.ID = 0
			m.SpaceID = yang.ID
			if err := db.Create(&m).Error; err != nil {
				return err
			}
		}
		if err := db.Create(&repository.ItemIconGroup{Title: "APP", Icon: "material-symbols:apps", UserId: yin.OwnerUserID, SpaceID: yang.ID}).Error; err != nil {
			return err
		}
	}
	return nil
}

// Empty public IDs must be NULL so the unique index only applies to configured links.
func normalizeEmptyPublicIDs(db *gorm.DB) error {
	return db.Model(&repository.Space{}).
		Where("public_id = ?", "").
		UpdateColumn("public_id", nil).Error
}

func disableUserMonitorConfigs(db *gorm.DB) error {
	var configs []repository.UserConfig
	if err := db.Find(&configs).Error; err != nil {
		return err
	}
	for _, config := range configs {
		var panel map[string]any
		if err := json.Unmarshal([]byte(config.PanelJson), &panel); err != nil {
			continue
		}
		panel["systemMonitorShow"] = false
		panel["systemMonitorShowTitle"] = false
		data, err := json.Marshal(panel)
		if err != nil {
			return err
		}
		if err := db.Model(&repository.UserConfig{}).Where("user_id = ?", config.UserId).Update("panel_json", string(data)).Error; err != nil {
			return err
		}
	}
	return nil
}

// ensurePersonalSpaces is idempotent and prepares legacy users for the space
// model without moving existing panel data yet.
func EnsurePersonalSpaces(db *gorm.DB) error {
	var users []repository.User
	if err := db.Find(&users).Error; err != nil {
		return err
	}
	for _, user := range users {
		var space repository.Space
		if err := db.Where("type = ? AND owner_user_id = ?", repository.SpaceTypePersonal, user.ID).First(&space).Error; err != nil {
			if !errors.Is(err, gorm.ErrRecordNotFound) {
				return err
			}
			space = repository.Space{Type: repository.SpaceTypePersonal, Name: user.Name, OwnerUserID: user.ID}
			if err := db.Create(&space).Error; err != nil {
				return err
			}
		}
		var member repository.SpaceMember
		if err := db.Where("space_id = ? AND user_id = ?", space.ID, user.ID).First(&member).Error; err == nil {
			continue
		} else if !errors.Is(err, gorm.ErrRecordNotFound) {
			return err
		}
		member = repository.SpaceMember{SpaceID: space.ID, UserID: user.ID, Role: repository.SpaceRoleAdmin, JoinedAt: time.Now()}
		if err := db.Create(&member).Error; err != nil {
			return err
		}
	}
	return nil
}

func backfillPanelSpaces(db *gorm.DB) error {
	if err := db.Model(&repository.Space{}).Where("type = ?", "team").Update("type", repository.SpaceTypeShared).Error; err != nil {
		return err
	}
	var spaces []repository.Space
	if err := db.Where("type = ?", repository.SpaceTypePersonal).Find(&spaces).Error; err != nil {
		return err
	}
	for _, space := range spaces {
		if err := db.Model(&repository.ItemIconGroup{}).Where("user_id = ? AND (space_id = 0 OR space_id IS NULL)", space.OwnerUserID).Update("space_id", space.ID).Error; err != nil {
			return err
		}
		if err := db.Model(&repository.ItemIcon{}).Where("user_id = ? AND (space_id = 0 OR space_id IS NULL)", space.OwnerUserID).Update("space_id", space.ID).Error; err != nil {
			return err
		}
	}
	var teamSpaces []repository.Space
	if err := db.Where("type = ?", repository.SpaceTypeTeam).Find(&teamSpaces).Error; err != nil {
		return err
	}
	for _, space := range teamSpaces {
		var count int64
		if err := db.Model(&repository.ItemIconGroup{}).Where("space_id = ?", space.ID).Count(&count).Error; err != nil {
			return err
		}
		if count == 0 {
			if err := db.Create(&repository.ItemIconGroup{Title: "APP", Icon: "material-symbols:apps", Sort: 0, UserId: space.OwnerUserID, SpaceID: space.ID}).Error; err != nil {
				return err
			}
		}
	}
	return nil
}

func CreateDefaultUser() error {
	count, err := global.UserRepo.Count()
	if err != nil {
		return err
	}

	if count == 0 {
		mUser := repository.User{}
		mail := "admin@yiniot.com"
		mUser.Mail = mail
		mUser.Name = mail
		mUser.Status = 1
		mUser.Role = 1
		mUser.Password = util.PasswordEncryption("admin@yiniot.com")
		mUser.OauthProvider = constant.OAuthProviderBuildin
		if errCreate := global.UserService.CreateUser(&mUser); errCreate != nil {
			return errCreate
		}
	}

	return nil
}

func GetLogger() logger.Interface {
	return logger.New(
		log.New(os.Stdout, "\r\n", log.LstdFlags), // io writer（日志输出的目标，前缀和日志包含的内容——译者注）
		logger.Config{
			SlowThreshold:             time.Second, // 慢 SQL 阈值
			LogLevel:                  logger.Warn, // 日志级别
			IgnoreRecordNotFoundError: true,        // 忽略ErrRecordNotFound（记录未找到）错误
			Colorful:                  true,        // 彩色打印
		},
	)

}
