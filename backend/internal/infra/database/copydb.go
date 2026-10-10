package database

import (
	"fmt"
	"reflect"
	"time"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/theme"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"gorm.io/gorm"
)

// SchemaModels is every model the application migrates. It is shared by the
// schema bootstrap and the SQLite -> server database copy tool.
func SchemaModels() []any {
	return []any{
		&repository.User{}, &repository.OAuthIdentity{}, &repository.Space{},
		&repository.SpaceMember{}, &repository.SpaceOIDCGroup{}, &repository.SystemSetting{},
		&repository.ItemIcon{}, &repository.UserConfig{}, &repository.File{},
		&repository.ItemIconGroup{}, &repository.ModuleConfig{},
		&theme.AuditRecord{}, &theme.WebWallpaperRecord{},
		&theme.PackageRecordV2{}, &theme.RevisionRecordV2{}, &theme.AssetRecordV2{},
		&theme.ActivationRecordV2{}, &theme.GrantRecordV2{}, &theme.TrustedRuntimePolicyV2{},
		&theme.ThemeSettingsRecordV2{}, &theme.UserThemePreferenceV2{},
		&theme.SpaceThemePreferenceV2{}, &theme.UserSpaceThemePreferenceV2{},
	}
}

// MigrateDB copies an existing SQLite deployment into a MySQL/MariaDB or
// PostgreSQL database. The target must be empty (or disposable): the tool
// creates the schema and replaces each table's contents. Identifiers and
// sequences are preserved so existing sessions and links keep working.
func MigrateDB(sourceConfigPath, targetConfigPath string) error {
	sourceCfg, err := config.Init(sourceConfigPath)
	if err != nil {
		return fmt.Errorf("read source config: %w", err)
	}
	targetCfg, err := config.Init(targetConfigPath)
	if err != nil {
		return fmt.Errorf("read target config: %w", err)
	}
	if sourceCfg.Base.DatabaseDrive != SQLITE {
		return fmt.Errorf("source must be sqlite, got %q", sourceCfg.Base.DatabaseDrive)
	}

	source, err := (&SQLiteConfig{Filename: sourceCfg.SQLite.FilePath}).Connect()
	if err != nil {
		return fmt.Errorf("open source sqlite: %w", err)
	}
	target, err := connectTarget(targetCfg)
	if err != nil {
		return err
	}
	return CopyDatabase(source, target)
}

func connectTarget(cfg *config.Config) (*gorm.DB, error) {
	switch cfg.Base.DatabaseDrive {
	case MYSQL:
		return (&MySQLConfig{
			Username: cfg.MySQL.Username, Password: cfg.MySQL.Password,
			Host: cfg.MySQL.Host, Port: cfg.MySQL.Port, Database: cfg.MySQL.DBName,
			WaitTimeout: cfg.MySQL.WaitTimeout,
		}).Connect()
	case POSTGRES:
		return (&PostgresConfig{
			Host: cfg.Postgres.Host, Port: cfg.Postgres.Port,
			Username: cfg.Postgres.Username, Password: cfg.Postgres.Password,
			Database: cfg.Postgres.DBName, SSLMode: cfg.Postgres.SSLMode,
			TimeZone: cfg.Postgres.TimeZone, WaitTimeout: cfg.Postgres.WaitTimeout,
		}).Connect()
	default:
		return nil, fmt.Errorf("target must be mysql or postgres, got %q", cfg.Base.DatabaseDrive)
	}
}

// CopyDatabase creates the target schema and copies every model table. Typed
// models (not raw rows) are used so booleans, times and byte blobs convert
// correctly across dialects.
func CopyDatabase(source, target *gorm.DB) error {
	if err := target.AutoMigrate(SchemaModels()...); err != nil {
		return fmt.Errorf("create target schema: %w", err)
	}
	for _, model := range SchemaModels() {
		table := tableName(target, model)
		if table == "" {
			continue
		}
		if !source.Migrator().HasTable(model) {
			continue
		}
		if err := copyModel(source, target, model, table); err != nil {
			return fmt.Errorf("copy %s: %w", table, err)
		}
	}
	if err := ensureQueryIndexes(target); err != nil {
		return fmt.Errorf("target indexes: %w", err)
	}
	return resetSequences(target)
}

func copyModel(source, target *gorm.DB, model any, table string) error {
	slicePtr := reflect.New(reflect.SliceOf(reflect.TypeOf(model).Elem())).Interface()
	if err := source.Find(slicePtr).Error; err != nil {
		return err
	}
	if reflect.ValueOf(slicePtr).Elem().Len() == 0 {
		return nil
	}
	sanitizeTimes(slicePtr)
	if err := target.Exec("DELETE FROM " + quoteIdent(target, table)).Error; err != nil {
		return err
	}
	return target.CreateInBatches(slicePtr, 500).Error
}

// sanitizeTimes replaces zero or too-old timestamps with a valid value. Legacy
// SQLite rows can hold Go's zero time ("0001-01-01"), which MySQL's DATETIME
// rejects and PostgreSQL normalizes oddly.
func sanitizeTimes(slicePtr any) {
	slice := reflect.ValueOf(slicePtr).Elem()
	now := reflect.ValueOf(time.Now())
	for i := 0; i < slice.Len(); i++ {
		sanitizeValue(slice.Index(i), now)
	}
}

func sanitizeValue(v reflect.Value, now reflect.Value) {
	if !v.IsValid() {
		return
	}
	switch v.Kind() {
	case reflect.Struct:
		if v.Type() == reflect.TypeOf(time.Time{}) {
			t := v.Interface().(time.Time)
			if (t.IsZero() || t.Year() < 1000) && v.CanSet() {
				v.Set(now)
			}
			return
		}
		for j := 0; j < v.NumField(); j++ {
			sanitizeValue(v.Field(j), now)
		}
	case reflect.Ptr:
		if v.IsNil() {
			return
		}
		if v.Type() == reflect.TypeOf(&time.Time{}) {
			t := v.Interface().(*time.Time)
			if (t.IsZero() || t.Year() < 1000) && v.CanSet() {
				v.Set(reflect.Zero(v.Type()))
			}
			return
		}
		sanitizeValue(v.Elem(), now)
	}
}

func tableName(db *gorm.DB, model any) string {
	stmt := &gorm.Statement{DB: db}
	if err := stmt.Parse(model); err != nil || stmt.Schema == nil {
		return ""
	}
	return stmt.Schema.Table
}

// resetSequences advances PostgreSQL identity sequences past the copied ids so
// new inserts do not collide. MySQL advances AUTO_INCREMENT on explicit ids.
func resetSequences(target *gorm.DB) error {
	if target.Dialector.Name() != POSTGRES {
		return nil
	}
	for _, model := range SchemaModels() {
		table := tableName(target, model)
		if table == "" {
			continue
		}
		var idColumns int64
		if err := target.Raw("SELECT count(*) FROM information_schema.columns WHERE table_name = ? AND column_name = 'id' AND data_type IN ('bigint','integer')", table).Scan(&idColumns).Error; err != nil {
			return err
		}
		if idColumns == 0 {
			continue
		}
		quoted := quoteIdent(target, table)
		q := fmt.Sprintf("SELECT setval(pg_get_serial_sequence('%s', 'id'), COALESCE((SELECT MAX(id) FROM %s), 1), true)", quoted, quoted)
		if err := target.Exec(q).Error; err != nil {
			return fmt.Errorf("reset sequence for %s: %w", table, err)
		}
	}
	return nil
}
