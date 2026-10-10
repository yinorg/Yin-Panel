package database

import (
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/theme"

	"gorm.io/gorm"
	"gorm.io/gorm/clause"
)

// This file implements the schema-upgrade steps around the new unique
// indexes: duplicate cleanup that must run before AutoMigrate, mail
// normalization, the OAuth identity backfill, automatic merging of duplicate
// login accounts and the unique mail index. Every step is idempotent and runs
// on both SQLite and MySQL.

const (
	userMailIndexName = "uk_user_mail"
	orphanMailDomain  = "local.invalid"
	builtinAuthMarker = "buildin"
)

// prepareSchemaUpgrade removes duplicate rows that would otherwise make the
// unique indexes added by AutoMigrate fail. It must run before AutoMigrate.
func prepareSchemaUpgrade(db *gorm.DB) error {
	if err := deduplicateUserConfigs(db); err != nil {
		return fmt.Errorf("deduplicate user configs: %w", err)
	}
	if err := deduplicateModuleConfigs(db); err != nil {
		return fmt.Errorf("deduplicate module configs: %w", err)
	}
	if err := deduplicateSystemSettings(db); err != nil {
		return fmt.Errorf("deduplicate system settings: %w", err)
	}
	return nil
}

// finalizeSchemaUpgrade runs after AutoMigrate: it normalizes the login mail,
// repairs identities and merges duplicate accounts before the unique mail
// index is created.
func finalizeSchemaUpgrade(db *gorm.DB) error {
	if err := normalizeUserMails(db); err != nil {
		return fmt.Errorf("normalize user mails: %w", err)
	}
	if err := backfillOAuthIdentities(db); err != nil {
		return fmt.Errorf("backfill oauth identities: %w", err)
	}
	if err := mergeDuplicateUserAccounts(db); err != nil {
		return fmt.Errorf("merge duplicate user accounts: %w", err)
	}
	if err := finalizeLegacyThemePreferences(db); err != nil {
		return fmt.Errorf("finalize legacy theme preferences: %w", err)
	}
	// dropLegacySchema rebuilds the user/space tables on SQLite, which also
	// drops the manually created mail index; recreate it afterwards.
	if err := dropLegacySchema(db); err != nil {
		return fmt.Errorf("drop legacy schema: %w", err)
	}
	if err := ensureUserMailIndex(db); err != nil {
		return fmt.Errorf("ensure user mail index: %w", err)
	}
	return nil
}

// deduplicateUserConfigs keeps the last scanned row per user. The table has no
// primary key, so the survivors are rewritten after a full delete.
func deduplicateUserConfigs(db *gorm.DB) error {
	if !db.Migrator().HasTable(&repository.UserConfig{}) {
		return nil
	}
	var rows []repository.UserConfig
	if err := db.Find(&rows).Error; err != nil {
		return err
	}
	keep := make(map[uint]repository.UserConfig, len(rows))
	for _, row := range rows {
		keep[row.UserId] = row
	}
	if len(keep) == len(rows) {
		return nil
	}
	return db.Transaction(func(tx *gorm.DB) error {
		if err := tx.Exec("DELETE FROM user_config").Error; err != nil {
			return err
		}
		for _, row := range keep {
			if err := tx.Create(&row).Error; err != nil {
				return err
			}
		}
		return nil
	})
}

// deduplicateModuleConfigs keeps the newest row (highest id) per user and name.
func deduplicateModuleConfigs(db *gorm.DB) error {
	if !db.Migrator().HasTable(&repository.ModuleConfig{}) {
		return nil
	}
	var rows []repository.ModuleConfig
	if err := db.Order("id").Find(&rows).Error; err != nil {
		return err
	}
	type moduleKey struct {
		UserID uint
		Name   string
	}
	keep := make(map[moduleKey]uint, len(rows))
	for _, row := range rows {
		key := moduleKey{UserID: row.UserId, Name: row.Name}
		if row.ID > keep[key] {
			keep[key] = row.ID
		}
	}
	duplicates := make([]uint, 0)
	for _, row := range rows {
		key := moduleKey{UserID: row.UserId, Name: row.Name}
		if keep[key] != row.ID {
			duplicates = append(duplicates, row.ID)
		}
	}
	if len(duplicates) == 0 {
		return nil
	}
	return db.Transaction(func(tx *gorm.DB) error {
		return tx.Delete(&repository.ModuleConfig{}, duplicates).Error
	})
}

// deduplicateSystemSettings keeps the newest row (highest id) per config name.
func deduplicateSystemSettings(db *gorm.DB) error {
	if !db.Migrator().HasTable(&repository.SystemSetting{}) {
		return nil
	}
	var rows []repository.SystemSetting
	if err := db.Order("id").Find(&rows).Error; err != nil {
		return err
	}
	keep := make(map[string]uint, len(rows))
	for _, row := range rows {
		if row.ID > keep[row.ConfigName] {
			keep[row.ConfigName] = row.ID
		}
	}
	duplicates := make([]uint, 0)
	for _, row := range rows {
		if keep[row.ConfigName] != row.ID {
			duplicates = append(duplicates, row.ID)
		}
	}
	if len(duplicates) == 0 {
		return nil
	}
	return db.Transaction(func(tx *gorm.DB) error {
		return tx.Delete(&repository.SystemSetting{}, duplicates).Error
	})
}

// normalizeUserMails lower-cases and trims the login identity and gives users
// without an address a unique placeholder so the unique index can be created.
// The comparison happens in Go because MySQL's case-insensitive collation would
// otherwise consider mixed-case addresses already normalized.
func normalizeUserMails(db *gorm.DB) error {
	var users []repository.User
	if err := db.Select("id", "mail").Find(&users).Error; err != nil {
		return err
	}
	for _, user := range users {
		normalized := strings.ToLower(strings.TrimSpace(user.Mail))
		if normalized == "" {
			normalized = fmt.Sprintf("orphan-%d@%s", user.ID, orphanMailDomain)
		}
		if normalized == user.Mail {
			continue
		}
		if err := db.Model(&repository.User{}).Where("id = ?", user.ID).Update("mail", normalized).Error; err != nil {
			return err
		}
	}
	return nil
}

// legacyOAuthIdentityRow reads the retired provider columns that still exist on
// old databases.
type legacyOAuthIdentityRow struct {
	ID       uint
	Provider string
	OauthID  string
	Mail     string
}

// backfillOAuthIdentities copies legacy provider columns into the canonical
// identity table before duplicate accounts are merged or the columns retire.
func backfillOAuthIdentities(db *gorm.DB) error {
	if !db.Migrator().HasColumn("user", "oauth_provider") || !db.Migrator().HasColumn("user", "oauth_id") {
		return nil
	}
	var users []legacyOAuthIdentityRow
	if err := db.Table("user").
		Select("id, oauth_provider AS provider, oauth_id, mail").
		Where("oauth_provider IS NOT NULL AND oauth_provider <> '' AND oauth_provider <> ? AND oauth_id IS NOT NULL AND oauth_id <> ''", builtinAuthMarker).
		Find(&users).Error; err != nil {
		return err
	}
	for _, user := range users {
		identity := repository.OAuthIdentity{UserID: user.ID, Provider: user.Provider, Subject: user.OauthID, Email: user.Mail}
		if err := db.Where("provider = ? AND subject = ?", user.Provider, user.OauthID).FirstOrCreate(&identity).Error; err != nil {
			return err
		}
	}
	return nil
}

// mergeDuplicateUserAccounts folds every account that shares a login mail into
// the oldest account with that address.
func mergeDuplicateUserAccounts(db *gorm.DB) error {
	var mails []string
	if err := db.Raw("SELECT mail FROM " + quoteIdent(db, "user") + " GROUP BY mail HAVING count(*) > 1").Scan(&mails).Error; err != nil {
		return err
	}
	for _, mail := range mails {
		var users []repository.User
		if err := db.Where("mail = ?", mail).Order("id").Find(&users).Error; err != nil {
			return err
		}
		if len(users) < 2 {
			continue
		}
		canonical := users[0]
		for _, duplicate := range users[1:] {
			if err := db.Transaction(func(tx *gorm.DB) error {
				return mergeUserAccount(tx, canonical, duplicate)
			}); err != nil {
				return fmt.Errorf("merge user %d into %d: %w", duplicate.ID, canonical.ID, err)
			}
		}
	}
	return nil
}

// mergeUserAccount moves every reference of the duplicate account to the
// canonical account and removes the duplicate. The canonical row wins when
// both accounts carry the same configuration key.
func mergeUserAccount(tx *gorm.DB, canonical, duplicate repository.User) error {
	if canonical.ID == 0 || duplicate.ID == 0 || canonical.ID == duplicate.ID {
		return nil
	}
	if err := tx.Model(&repository.Space{}).Where("owner_user_id = ?", duplicate.ID).Update("owner_user_id", canonical.ID).Error; err != nil {
		return err
	}
	if err := mergeUserSpaceMemberships(tx, canonical.ID, duplicate.ID); err != nil {
		return err
	}
	if err := tx.Model(&repository.ItemIcon{}).Where("user_id = ?", duplicate.ID).Update("user_id", canonical.ID).Error; err != nil {
		return err
	}
	if err := tx.Model(&repository.ItemIconGroup{}).Where("user_id = ?", duplicate.ID).Update("user_id", canonical.ID).Error; err != nil {
		return err
	}
	if err := tx.Model(&repository.File{}).Where("user_id = ?", duplicate.ID).Update("user_id", canonical.ID).Error; err != nil {
		return err
	}
	if err := tx.Model(&repository.OAuthIdentity{}).Where("user_id = ?", duplicate.ID).Update("user_id", canonical.ID).Error; err != nil {
		return err
	}
	if err := tx.Model(&theme.AuditRecord{}).Where("actor_id = ?", duplicate.ID).Update("actor_id", canonical.ID).Error; err != nil {
		return err
	}
	if err := tx.Model(&theme.WebWallpaperRecord{}).Where("owner_id = ?", duplicate.ID).Update("owner_id", canonical.ID).Error; err != nil {
		return err
	}
	if err := mergeUserConfig(tx, canonical.ID, duplicate.ID); err != nil {
		return err
	}
	if err := mergeUserModuleConfigs(tx, canonical.ID, duplicate.ID); err != nil {
		return err
	}
	if err := mergeUserThemePreference(tx, canonical.ID, duplicate.ID); err != nil {
		return err
	}
	if err := mergeUserSpaceThemePreferences(tx, canonical.ID, duplicate.ID); err != nil {
		return err
	}
	if err := mergeUserThemeGrants(tx, canonical.ID, duplicate.ID); err != nil {
		return err
	}
	if err := mergeUserThemeSettings(tx, canonical.ID, duplicate.ID); err != nil {
		return err
	}
	if err := mergeUserLegacyPreferences(tx, canonical.ID, duplicate.ID); err != nil {
		return err
	}
	if canonical.Password == "" && duplicate.Password != "" {
		if err := tx.Model(&repository.User{}).Where("id = ?", canonical.ID).Update("password", duplicate.Password).Error; err != nil {
			return err
		}
	}
	return tx.Delete(&repository.User{}, duplicate.ID).Error
}

func mergeUserSpaceMemberships(tx *gorm.DB, canonicalID, duplicateID uint) error {
	var members []repository.SpaceMember
	if err := tx.Where("user_id = ?", duplicateID).Order("id").Find(&members).Error; err != nil {
		return err
	}
	for _, member := range members {
		var existing repository.SpaceMember
		err := tx.Where("space_id = ? AND user_id = ?", member.SpaceID, canonicalID).First(&existing).Error
		switch {
		case errors.Is(err, gorm.ErrRecordNotFound):
			if err := tx.Model(&repository.SpaceMember{}).Where("id = ?", member.ID).Update("user_id", canonicalID).Error; err != nil {
				return err
			}
		case err != nil:
			return err
		default:
			if roleRank(member.Role) > roleRank(existing.Role) {
				updates := map[string]any{"role": member.Role, "source": member.Source}
				if err := tx.Model(&repository.SpaceMember{}).Where("id = ?", existing.ID).Updates(updates).Error; err != nil {
					return err
				}
			}
			if err := tx.Delete(&repository.SpaceMember{}, member.ID).Error; err != nil {
				return err
			}
		}
	}
	return nil
}

func mergeUserConfig(tx *gorm.DB, canonicalID, duplicateID uint) error {
	var config repository.UserConfig
	err := tx.Where("user_id = ?", canonicalID).First(&config).Error
	if errors.Is(err, gorm.ErrRecordNotFound) {
		return tx.Model(&repository.UserConfig{}).Where("user_id = ?", duplicateID).Update("user_id", canonicalID).Error
	}
	if err != nil {
		return err
	}
	return tx.Where("user_id = ?", duplicateID).Delete(&repository.UserConfig{}).Error
}

func mergeUserModuleConfigs(tx *gorm.DB, canonicalID, duplicateID uint) error {
	var configs []repository.ModuleConfig
	if err := tx.Where("user_id = ?", duplicateID).Order("id").Find(&configs).Error; err != nil {
		return err
	}
	for _, config := range configs {
		var existing repository.ModuleConfig
		err := tx.Where("user_id = ? AND name = ?", canonicalID, config.Name).First(&existing).Error
		switch {
		case errors.Is(err, gorm.ErrRecordNotFound):
			if err := tx.Model(&repository.ModuleConfig{}).Where("id = ?", config.ID).Update("user_id", canonicalID).Error; err != nil {
				return err
			}
		case err != nil:
			return err
		default:
			if err := tx.Delete(&repository.ModuleConfig{}, config.ID).Error; err != nil {
				return err
			}
		}
	}
	return nil
}

func mergeUserThemePreference(tx *gorm.DB, canonicalID, duplicateID uint) error {
	var preference theme.UserThemePreferenceV2
	err := tx.First(&preference, "user_id = ?", canonicalID).Error
	if errors.Is(err, gorm.ErrRecordNotFound) {
		return tx.Model(&theme.UserThemePreferenceV2{}).Where("user_id = ?", duplicateID).Update("user_id", canonicalID).Error
	}
	if err != nil {
		return err
	}
	return tx.Where("user_id = ?", duplicateID).Delete(&theme.UserThemePreferenceV2{}).Error
}

func mergeUserSpaceThemePreferences(tx *gorm.DB, canonicalID, duplicateID uint) error {
	var preferences []theme.UserSpaceThemePreferenceV2
	if err := tx.Where("user_id = ?", duplicateID).Find(&preferences).Error; err != nil {
		return err
	}
	for _, preference := range preferences {
		var existing theme.UserSpaceThemePreferenceV2
		err := tx.Where("user_id = ? AND space_id = ?", canonicalID, preference.SpaceID).First(&existing).Error
		switch {
		case errors.Is(err, gorm.ErrRecordNotFound):
			if err := tx.Model(&theme.UserSpaceThemePreferenceV2{}).
				Where("user_id = ? AND space_id = ?", duplicateID, preference.SpaceID).
				Update("user_id", canonicalID).Error; err != nil {
				return err
			}
		case err != nil:
			return err
		default:
			if err := tx.Where("user_id = ? AND space_id = ?", duplicateID, preference.SpaceID).
				Delete(&theme.UserSpaceThemePreferenceV2{}).Error; err != nil {
				return err
			}
		}
	}
	return nil
}

func mergeUserThemeGrants(tx *gorm.DB, canonicalID, duplicateID uint) error {
	var grants []theme.GrantRecordV2
	if err := tx.Where("user_id = ?", duplicateID).Find(&grants).Error; err != nil {
		return err
	}
	for _, grant := range grants {
		var existing theme.GrantRecordV2
		err := tx.Where("user_id = ? AND revision_id = ? AND execution_mode = ?", canonicalID, grant.RevisionID, grant.ExecutionMode).First(&existing).Error
		switch {
		case errors.Is(err, gorm.ErrRecordNotFound):
			if err := tx.Model(&theme.GrantRecordV2{}).
				Where("user_id = ? AND revision_id = ? AND execution_mode = ?", duplicateID, grant.RevisionID, grant.ExecutionMode).
				Update("user_id", canonicalID).Error; err != nil {
				return err
			}
		case err != nil:
			return err
		default:
			if err := tx.Where("user_id = ? AND revision_id = ? AND execution_mode = ?", duplicateID, grant.RevisionID, grant.ExecutionMode).
				Delete(&theme.GrantRecordV2{}).Error; err != nil {
				return err
			}
		}
	}
	return nil
}

func mergeUserThemeSettings(tx *gorm.DB, canonicalID, duplicateID uint) error {
	var settings []theme.ThemeSettingsRecordV2
	if err := tx.Where("user_id = ?", duplicateID).Find(&settings).Error; err != nil {
		return err
	}
	for _, setting := range settings {
		var existing theme.ThemeSettingsRecordV2
		err := tx.Where("user_id = ? AND package_id = ? AND revision_id = ?", canonicalID, setting.PackageID, setting.RevisionID).First(&existing).Error
		switch {
		case errors.Is(err, gorm.ErrRecordNotFound):
			if err := tx.Model(&theme.ThemeSettingsRecordV2{}).
				Where("user_id = ? AND package_id = ? AND revision_id = ?", duplicateID, setting.PackageID, setting.RevisionID).
				Update("user_id", canonicalID).Error; err != nil {
				return err
			}
		case err != nil:
			return err
		default:
			if err := tx.Where("user_id = ? AND package_id = ? AND revision_id = ?", duplicateID, setting.PackageID, setting.RevisionID).
				Delete(&theme.ThemeSettingsRecordV2{}).Error; err != nil {
				return err
			}
		}
	}
	return nil
}

// legacyThemePreferenceRow reads the retired preferences table.
type legacyThemePreferenceRow struct {
	UserID    uint
	Mode      string
	UpdatedAt time.Time
}

func mergeUserLegacyPreferences(tx *gorm.DB, canonicalID, duplicateID uint) error {
	if !tx.Migrator().HasTable("preferences") {
		return nil
	}
	var preference legacyThemePreferenceRow
	err := tx.Table("preferences").Where("user_id = ?", canonicalID).First(&preference).Error
	if errors.Is(err, gorm.ErrRecordNotFound) {
		return tx.Exec("UPDATE preferences SET user_id = ? WHERE user_id = ?", canonicalID, duplicateID).Error
	}
	if err != nil {
		return err
	}
	return tx.Exec("DELETE FROM preferences WHERE user_id = ?", duplicateID).Error
}

// finalizeLegacyThemePreferences copies the retired preferences table into the
// v2 preference table and then drops it.
func finalizeLegacyThemePreferences(db *gorm.DB) error {
	if !db.Migrator().HasTable("preferences") {
		return nil
	}
	var rows []legacyThemePreferenceRow
	if err := db.Table("preferences").Select("user_id, mode, updated_at").Find(&rows).Error; err != nil {
		return err
	}
	for _, row := range rows {
		mode := row.Mode
		if !validLegacyThemeMode(mode) {
			mode = "auto"
		}
		preference := theme.UserThemePreferenceV2{UserID: row.UserID, Mode: mode, UpdatedAt: row.UpdatedAt}
		if preference.UpdatedAt.IsZero() {
			preference.UpdatedAt = time.Now()
		}
		if err := db.Clauses(clause.OnConflict{DoNothing: true}).Create(&preference).Error; err != nil {
			return err
		}
	}
	return db.Migrator().DropTable("preferences")
}

func validLegacyThemeMode(mode string) bool {
	return mode == "light" || mode == "dark" || mode == "auto"
}

// dropLegacySchema removes retired columns, tables and SQLite foreign keys once
// their data has been migrated. Every step is guarded, so it is a no-op on
// fresh databases and on every start after the first one.
func dropLegacySchema(db *gorm.DB) error {
	// Foreign keys must go first: rebuilding the user table while another table
	// still references it fails under SQLite's foreign_keys pragma.
	constraints := []struct {
		model interface{}
		name  string
	}{
		{&repository.ItemIcon{}, "fk_item_icon_user"},
		{&repository.ItemIconGroup{}, "fk_item_icon_group_user"},
	}
	for _, constraint := range constraints {
		if db.Migrator().HasConstraint(constraint.model, constraint.name) {
			if err := db.Migrator().DropConstraint(constraint.model, constraint.name); err != nil {
				return fmt.Errorf("drop constraint %s: %w", constraint.name, err)
			}
		}
	}
	columns := []struct {
		model interface{}
		table string
		name  string
	}{
		{&repository.User{}, "user", "publiccode"},
		{&repository.User{}, "user", "oauth_provider"},
		{&repository.User{}, "user", "oauth_id"},
		{&repository.Space{}, "space", "team_id"},
	}
	for _, column := range columns {
		// Detect by table name (the retired field is no longer part of the
		// model, which HasColumn(model, name) would miss) and drop by model
		// (the SQLite driver needs a parsed schema to rebuild the table).
		if db.Migrator().HasColumn(column.table, column.name) {
			if err := db.Migrator().DropColumn(column.model, column.name); err != nil {
				return fmt.Errorf("drop column %s.%s: %w", column.table, column.name, err)
			}
		}
	}
	if db.Migrator().HasTable("team") {
		if err := db.Migrator().DropTable("team"); err != nil {
			return fmt.Errorf("drop table team: %w", err)
		}
	}
	return nil
}

// ensureUserMailIndex creates the unique mail index once the data is clean.
// It is created here instead of through a model tag because very old databases
// only receive the mail column during AutoMigrate, after which the legacy
// username backfill fills it. AutoMigrate cannot create the index before that.
func ensureUserMailIndex(db *gorm.DB) error {
	exists, err := schemaIndexExists(db, "user", userMailIndexName)
	if err != nil {
		return err
	}
	if exists {
		return nil
	}
	return db.Exec("CREATE UNIQUE INDEX " + userMailIndexName + " ON " + quoteIdent(db, "user") + " (mail)").Error
}

func schemaIndexExists(db *gorm.DB, table, index string) (bool, error) {
	var count int64
	switch db.Dialector.Name() {
	case SQLITE:
		err := db.Raw("SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = ?", index).Scan(&count).Error
		return count > 0, err
	case MYSQL:
		err := db.Raw("SELECT count(*) FROM information_schema.statistics WHERE table_schema = DATABASE() AND table_name = ? AND index_name = ?", table, index).Scan(&count).Error
		return count > 0, err
	case POSTGRES:
		err := db.Raw("SELECT count(*) FROM pg_indexes WHERE schemaname = current_schema() AND tablename = ? AND indexname = ?", table, index).Scan(&count).Error
		return count > 0, err
	default:
		return false, fmt.Errorf("unsupported database drive: %s", db.Dialector.Name())
	}
}
