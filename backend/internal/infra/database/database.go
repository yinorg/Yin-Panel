package database

import (
	"errors"
	"fmt"
	"log"
	"os"
	"sun-panel/internal/biz/repository"
	"sun-panel/internal/constant"
	"sun-panel/internal/global"
	"sun-panel/internal/util"
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
	if err := ensurePersonalSpaces(db); err != nil {
		return err
	}
	return backfillPanelSpaces(db)
}

// ensurePersonalSpaces is idempotent and prepares legacy users for the space
// model without moving existing panel data yet.
func ensurePersonalSpaces(db *gorm.DB) error {
	var users []repository.User
	if err := db.Find(&users).Error; err != nil {
		return err
	}
	for _, user := range users {
		var space repository.Space
		if err := db.Where("type = ? AND owner_user_id = ?", repository.SpaceTypePersonal, user.ID).First(&space).Error; err == nil {
			continue
		} else if !errors.Is(err, gorm.ErrRecordNotFound) {
			return err
		}
		space = repository.Space{Type: repository.SpaceTypePersonal, Name: user.Name, OwnerUserID: user.ID}
		if err := db.Create(&space).Error; err != nil {
			return err
		}
		member := repository.SpaceMember{SpaceID: space.ID, UserID: user.ID, Role: repository.SpaceRoleAdmin, JoinedAt: time.Now()}
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
		mail := "admin@sun.cc"
		mUser.Mail = mail
		mUser.Username = mail
		mUser.Name = mail
		mUser.Status = 1
		mUser.Role = 1
		mUser.Password = util.PasswordEncryption("12345678")
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
