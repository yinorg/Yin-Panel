package database

import (
	"fmt"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/theme"
	"gorm.io/gorm"
)

// queryIndexes extends the leading columns of the hot read indexes with the
// ORDER BY tie-breaker so list queries stop using a temporary B-tree, and so a
// group-filtered item list reads only that group instead of the whole space.
var queryIndexes = []struct {
	name    string
	table   string
	columns string
	model   any
}{
	{"idx_item_space_group_sort", "item_icon", "space_id, item_icon_group_id, sort, created_at", &repository.ItemIcon{}},
	{"idx_item_space_sort_created", "item_icon", "space_id, sort, created_at", &repository.ItemIcon{}},
	{"idx_group_space_sort_created", "item_icon_group", "space_id, sort, created_at", &repository.ItemIconGroup{}},
	{"idx_file_created_at", "file", "created_at", &repository.File{}},
	{"idx_revision_package_created", "revision_record_v2", "package_id, created_at DESC", &theme.RevisionRecordV2{}},
}

// legacyQueryIndexes are strict prefixes of the indexes above; they are dropped
// once the wider index exists.
var legacyQueryIndexes = []struct {
	name  string
	model any
}{
	{"idx_item_space_group", &repository.ItemIcon{}},
	{"idx_item_space_sort", &repository.ItemIcon{}},
	{"idx_group_space_sort", &repository.ItemIconGroup{}},
}

// ensureQueryIndexes is idempotent and dialect-safe: it creates the wider
// indexes when missing and drops the superseded narrow ones. It runs on every
// start, after AutoMigrate.
func ensureQueryIndexes(db *gorm.DB) error {
	for _, idx := range legacyQueryIndexes {
		if db.Migrator().HasIndex(idx.model, idx.name) {
			if err := db.Migrator().DropIndex(idx.model, idx.name); err != nil {
				return fmt.Errorf("drop legacy index %s: %w", idx.name, err)
			}
		}
	}
	for _, idx := range queryIndexes {
		if db.Migrator().HasIndex(idx.model, idx.name) {
			continue
		}
		ddl := fmt.Sprintf("CREATE INDEX %s ON %s (%s)", quoteIdent(db, idx.name), quoteIdent(db, idx.table), idx.columns)
		if db.Dialector.Name() == SQLITE {
			ddl = fmt.Sprintf("CREATE INDEX IF NOT EXISTS %s ON %s (%s)", quoteIdent(db, idx.name), quoteIdent(db, idx.table), idx.columns)
		}
		if err := db.Exec(ddl).Error; err != nil {
			return fmt.Errorf("create index %s: %w", idx.name, err)
		}
	}
	return nil
}
