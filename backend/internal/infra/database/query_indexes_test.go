package database

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/theme"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

func TestEnsureQueryIndexesCreatesWideAndDropsNarrow(t *testing.T) {
	db, err := gorm.Open(sqlite.Open(filepath.Join(t.TempDir(), "query-indexes.db")), &gorm.Config{
		NamingStrategy: schema.NamingStrategy{SingularTable: true},
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.AutoMigrate(&repository.ItemIcon{}, &repository.ItemIconGroup{}, &repository.File{}, &theme.RevisionRecordV2{}); err != nil {
		t.Fatal(err)
	}
	// Recreate the superseded narrow indexes as an existing database would have.
	for _, ddl := range []string{
		"CREATE INDEX idx_item_space_group ON item_icon (space_id, item_icon_group_id)",
		"CREATE INDEX idx_item_space_sort ON item_icon (space_id, sort)",
		"CREATE INDEX idx_group_space_sort ON item_icon_group (space_id, sort)",
	} {
		if err := db.Exec(ddl).Error; err != nil {
			t.Fatal(err)
		}
	}

	if err := ensureQueryIndexes(db); err != nil {
		t.Fatalf("ensureQueryIndexes: %v", err)
	}
	if err := ensureQueryIndexes(db); err != nil {
		t.Fatalf("ensureQueryIndexes not idempotent: %v", err)
	}

	wide := []struct {
		name  string
		model any
		cols  string
	}{
		{"idx_item_space_group_sort", &repository.ItemIcon{}, "item_icon_group_id, sort, created_at"},
		{"idx_item_space_sort_created", &repository.ItemIcon{}, "sort, created_at"},
		{"idx_group_space_sort_created", &repository.ItemIconGroup{}, "sort, created_at"},
		{"idx_file_created_at", &repository.File{}, "created_at"},
		{"idx_revision_package_created", &theme.RevisionRecordV2{}, "created_at"},
	}
	for _, idx := range wide {
		if !db.Migrator().HasIndex(idx.model, idx.name) {
			t.Fatalf("expected index %s to exist", idx.name)
		}
		var sql string
		if err := db.Raw("SELECT sql FROM sqlite_master WHERE name = ?", idx.name).Scan(&sql).Error; err != nil {
			t.Fatal(err)
		}
		if !strings.Contains(sql, idx.cols) {
			t.Fatalf("index %s columns = %q, want to contain %q", idx.name, sql, idx.cols)
		}
	}
	for _, idx := range []struct {
		name  string
		model any
	}{
		{"idx_item_space_group", &repository.ItemIcon{}},
		{"idx_item_space_sort", &repository.ItemIcon{}},
		{"idx_group_space_sort", &repository.ItemIconGroup{}},
	} {
		if db.Migrator().HasIndex(idx.model, idx.name) {
			t.Fatalf("legacy index %s should have been dropped", idx.name)
		}
	}
}
