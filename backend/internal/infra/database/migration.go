package database

import (
	"crypto/md5"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"

	"gorm.io/gorm"
)

const migrationVersion = "0.3.11-legacy-space-md5"

type migrationReport struct {
	Users, Spaces, Groups, Items int64
	Renamed, Deduplicated        int
	Orphans                      []string
}

// PrepareMigrationBackup must run before AutoMigrate or any legacy backfill.
func PrepareMigrationBackup(db *gorm.DB) error {
	if db.Dialector.Name() != SQLITE {
		return nil
	}
	done, err := migrationCompleted(db)
	if err != nil || done {
		return err
	}
	_, err = createMigrationBackup(db)
	return err
}

// RunMigration creates a recoverable snapshot before changing an existing
// database, then performs the legacy space and upload-file conversion once.
func RunMigration(db *gorm.DB) error {
	if db.Dialector.Name() != SQLITE {
		return nil
	}
	if config.AppConfig != nil && config.AppConfig.Migration.Enabled != nil && !*config.AppConfig.Migration.Enabled {
		return nil
	}
	if migrated, err := migrationCompleted(db); err != nil || migrated {
		return err
	}

	backup, err := createMigrationBackup(db)
	if err != nil {
		return fmt.Errorf("create migration backup: %w", err)
	}
	report := migrationReport{}
	if err := db.Transaction(func(tx *gorm.DB) error {
		if err := tx.Model(&repository.User{}).Count(&report.Users).Error; err != nil {
			return err
		}
		if err := tx.Model(&repository.Space{}).Count(&report.Spaces).Error; err != nil {
			return err
		}
		if err := tx.Model(&repository.ItemIconGroup{}).Count(&report.Groups).Error; err != nil {
			return err
		}
		if err := tx.Model(&repository.ItemIcon{}).Count(&report.Items).Error; err != nil {
			return err
		}
		return nil
	}); err != nil {
		return err
	}

	if config.AppConfig != nil && config.AppConfig.Migration.DryRun {
		fmt.Printf("Migration dry-run: backup=%s users=%d spaces=%d groups=%d items=%d\n", backup, report.Users, report.Spaces, report.Groups, report.Items)
		return nil
	}
	if err := migrateUploads(db, &report, backup); err != nil {
		return fmt.Errorf("migrate uploads: %w (backup: %s)", err, backup)
	}
	if err := validateMigration(db); err != nil {
		return fmt.Errorf("migration validation failed: %w (backup: %s)", err, backup)
	}
	if err := db.Exec("CREATE TABLE IF NOT EXISTS schema_migration (version VARCHAR(100) PRIMARY KEY, completed_at DATETIME NOT NULL)").Error; err != nil {
		return err
	}
	return db.Exec("INSERT INTO schema_migration(version, completed_at) VALUES (?, ?)", migrationVersion, time.Now()).Error
}

func migrationCompleted(db *gorm.DB) (bool, error) {
	var count int64
	if err := db.Raw("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='schema_migration'").Scan(&count).Error; err != nil {
		return false, err
	}
	if count == 0 {
		return false, nil
	}
	var version string
	err := db.Raw("SELECT version FROM schema_migration WHERE version = ? LIMIT 1", migrationVersion).Scan(&version).Error
	return version != "", err
}

func createMigrationBackup(db *gorm.DB) (string, error) {
	base := "./backup"
	if config.AppConfig != nil && config.AppConfig.Migration.BackupPath != "" {
		base = config.AppConfig.Migration.BackupPath
	}
	dir := filepath.Join(base, time.Now().Format("20060102-150405"))
	if err := os.MkdirAll(dir, 0700); err != nil {
		return "", err
	}
	if db.Dialector.Name() == SQLITE {
		if err := db.Exec("PRAGMA wal_checkpoint(TRUNCATE)").Error; err != nil {
			return "", err
		}
		file := config.AppConfig.SQLite.FilePath
		if err := copyFile(file, filepath.Join(dir, "database.db")); err != nil {
			return "", err
		}
	}
	if config.AppConfig != nil && strings.EqualFold(config.AppConfig.Rclone.Type, "local") {
		if err := copyDir(config.AppConfig.Rclone.Bucket, filepath.Join(dir, "uploads")); err != nil && !os.IsNotExist(err) {
			return "", err
		}
	}
	return dir, nil
}

func migrateUploads(db *gorm.DB, report *migrationReport, backup string) error {
	if config.AppConfig == nil || !strings.EqualFold(config.AppConfig.Rclone.Type, "local") {
		return nil
	}
	root := config.AppConfig.Rclone.Bucket
	entries, err := os.ReadDir(root)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}
	var obsolete []string
	obsoleteDir := filepath.Join(backup, "obsolete")
	if err := os.MkdirAll(obsoleteDir, 0700); err != nil {
		return err
	}
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		oldPath := filepath.Join(root, entry.Name())
		newName, err := md5FileName(oldPath)
		if err != nil {
			return err
		}
		newPath := filepath.Join(root, newName)
		if entry.Name() == newName {
			continue
		}
		if _, err := os.Stat(newPath); err == nil {
			same, err := filesEqual(oldPath, newPath)
			if err != nil {
				return err
			}
			if !same {
				return fmt.Errorf("MD5 filename collision with different content: %s", newName)
			}
			report.Deduplicated++
			if err := replaceIconReferences(db, entry.Name(), newName); err != nil {
				return err
			}
			obsolete = append(obsolete, oldPath)
			continue
		} else if !os.IsNotExist(err) {
			return err
		}
		// Keep the legacy file until every database reference has been updated;
		// this makes a failed migration recoverable without restoring files.
		if err := copyFile(oldPath, newPath); err != nil {
			return err
		}
		if err := replaceIconReferences(db, entry.Name(), newName); err != nil {
			return err
		}
		obsolete = append(obsolete, oldPath)
		report.Renamed++
	}
	for _, oldPath := range obsolete {
		if err := os.Rename(oldPath, filepath.Join(obsoleteDir, filepath.Base(oldPath))); err != nil {
			return err
		}
	}
	return nil
}

func filesEqual(left, right string) (bool, error) {
	leftName, err := md5FileName(left)
	if err != nil {
		return false, err
	}
	rightName, err := md5FileName(right)
	if err != nil {
		return false, err
	}
	return strings.TrimSuffix(leftName, filepath.Ext(leftName)) == strings.TrimSuffix(rightName, filepath.Ext(rightName)), nil
}

func md5FileName(path string) (string, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer f.Close()
	h := md5.New()
	if _, err := io.Copy(h, f); err != nil {
		return "", err
	}
	return hex.EncodeToString(h.Sum(nil)) + strings.ToLower(filepath.Ext(path)), nil
}

func replaceIconReferences(db *gorm.DB, oldName, newName string) error {
	var items []repository.ItemIcon
	if err := db.Find(&items).Error; err != nil {
		return err
	}
	for _, item := range items {
		var icon map[string]any
		if json.Unmarshal([]byte(item.IconJson), &icon) != nil {
			continue
		}
		changed := false
		for _, field := range []string{"fileName", "src"} {
			if value, ok := icon[field].(string); ok && (value == oldName || strings.HasSuffix(value, "/"+oldName)) {
				if field == "src" && strings.HasSuffix(value, "/"+oldName) {
					icon[field] = strings.TrimSuffix(value, oldName) + newName
				} else {
					icon[field] = newName
				}
				changed = true
			}
		}
		if changed {
			data, err := json.Marshal(icon)
			if err != nil {
				return err
			}
			if err := db.Model(&repository.ItemIcon{}).Where("id = ?", item.ID).Update("icon_json", string(data)).Error; err != nil {
				return err
			}
		}
	}
	return db.Model(&repository.File{}).Where("file_name = ?", oldName).Update("file_name", newName).Error
}

func validateMigration(db *gorm.DB) error {
	var broken int64
	if err := db.Model(&repository.ItemIcon{}).Where("space_id IS NULL OR space_id = 0").Count(&broken).Error; err != nil {
		return err
	}
	if broken > 0 {
		return fmt.Errorf("%d items have no space", broken)
	}
	return nil
}

func copyFile(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.OpenFile(dst, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0600)
	if err != nil {
		return err
	}
	defer out.Close()
	_, err = io.Copy(out, in)
	return err
}

func copyDir(src, dst string) error {
	entries, err := os.ReadDir(src)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(dst, 0700); err != nil {
		return err
	}
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		if err := copyFile(filepath.Join(src, entry.Name()), filepath.Join(dst, entry.Name())); err != nil {
			return err
		}
	}
	return nil
}
