package database

import (
	"os"
	"strings"
	"testing"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/theme"
	"gorm.io/driver/mysql"
	"gorm.io/driver/postgres"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

// These tests run only when the matching DSN is provided, so the normal unit
// suite stays database-server free:
//
//	YIN_PANEL_TEST_POSTGRES_DSN="host=... user=... password=... dbname=... sslmode=disable" \
//	YIN_PANEL_TEST_MYSQL_DSN="user:pass@tcp(host:port)/db?charset=utf8mb4&parseTime=True&loc=Local" \
//	  go test ./internal/infra/database/ -run Dialect -v
func dialectCases(t *testing.T) map[string]*gorm.DB {
	t.Helper()
	cases := map[string]*gorm.DB{}
	if dsn := os.Getenv("YIN_PANEL_TEST_POSTGRES_DSN"); dsn != "" {
		db, err := gorm.Open(postgres.Open(dsn), &gorm.Config{
			NamingStrategy:                           schema.NamingStrategy{SingularTable: true},
			DisableForeignKeyConstraintWhenMigrating: true,
		})
		if err != nil {
			t.Fatalf("open postgres: %v", err)
		}
		cases["postgres"] = db
	}
	if dsn := os.Getenv("YIN_PANEL_TEST_MYSQL_DSN"); dsn != "" {
		db, err := gorm.Open(mysql.Open(dsn), &gorm.Config{
			NamingStrategy:                           schema.NamingStrategy{SingularTable: true},
			DisableForeignKeyConstraintWhenMigrating: true,
		})
		if err != nil {
			t.Fatalf("open mysql: %v", err)
		}
		cases["mysql"] = db
	}
	if len(cases) == 0 {
		t.Skip("set YIN_PANEL_TEST_POSTGRES_DSN and/or YIN_PANEL_TEST_MYSQL_DSN")
	}
	return cases
}

// The whole schema must migrate, the optimized indexes must exist, and the hot
// list query must run on every supported server dialect.
func TestDialectSchemaAndHotQuery(t *testing.T) {
	for name, db := range dialectCases(t) {
		t.Run(name, func(t *testing.T) {
			if err := db.Migrator().DropTable(SchemaModels()...); err != nil {
				t.Fatalf("drop tables: %v", err)
			}
			if err := db.AutoMigrate(SchemaModels()...); err != nil {
				t.Fatalf("automigrate: %v", err)
			}
			if err := ensureQueryIndexes(db); err != nil {
				t.Fatalf("ensureQueryIndexes: %v", err)
			}
			for _, index := range []string{"idx_item_space_group_sort", "idx_item_space_sort_created", "idx_group_space_sort_created", "idx_file_created_at", "idx_revision_package_created"} {
				ok, err := schemaIndexExists(db, tableForIndex(index), index)
				if err != nil {
					t.Fatalf("schemaIndexExists(%s): %v", index, err)
				}
				if !ok {
					t.Fatalf("index %s missing on %s", index, name)
				}
			}

			// CRUD across the reserved-word `user` table and a space/group/item.
			user := repository.User{Mail: "dialect@example.com", Name: "Dialect", Status: 1, Role: 1}
			if err := db.Create(&user).Error; err != nil {
				t.Fatalf("create user: %v", err)
			}
			space := repository.Space{Type: repository.SpaceTypePersonal, Name: "Dialect", OwnerUserID: user.ID, Side: "yin"}
			if err := db.Create(&space).Error; err != nil {
				t.Fatalf("create space: %v", err)
			}
			group := repository.ItemIconGroup{Title: "APP", UserId: user.ID, SpaceID: space.ID}
			if err := db.Create(&group).Error; err != nil {
				t.Fatalf("create group: %v", err)
			}
			for i := 0; i < 5; i++ {
				if err := db.Create(&repository.ItemIcon{Title: "x", ItemIconGroupId: group.ID, UserId: user.ID, SpaceID: space.ID, Sort: i}).Error; err != nil {
					t.Fatalf("create item: %v", err)
				}
			}
			var items []repository.ItemIcon
			if err := db.Where("space_id = ? AND item_icon_group_id = ?", space.ID, group.ID).Order("sort, created_at").Find(&items).Error; err != nil {
				t.Fatalf("hot query: %v", err)
			}
			if len(items) != 5 {
				t.Fatalf("hot query returned %d items, want 5", len(items))
			}
			// bool + large text round-trip (dialect type mapping). The manifest
			// exceeds MySQL's 64 KiB TEXT limit, so it must map to longtext there.
			rev := theme.RevisionRecordV2{
				ID:           "rev-dialect",
				PackageID:    "org.test",
				Version:      "1.0.0",
				ManifestJSON: strings.Repeat("x", 70000),
				TokensJSON:   "{}",
				Verified:     true,
			}
			if err := db.Create(&rev).Error; err != nil {
				t.Fatalf("create revision: %v", err)
			}
			var gotRev theme.RevisionRecordV2
			if err := db.First(&gotRev, "id = ?", "rev-dialect").Error; err != nil {
				t.Fatalf("read revision: %v", err)
			}
			if len(gotRev.ManifestJSON) != 70000 {
				t.Fatalf("manifest round-trip truncated: %d bytes", len(gotRev.ManifestJSON))
			}
		})
	}
}

func tableForIndex(index string) string {
	switch index {
	case "idx_item_space_group_sort", "idx_item_space_sort_created":
		return "item_icon"
	case "idx_group_space_sort_created":
		return "item_icon_group"
	case "idx_file_created_at":
		return "file"
	case "idx_revision_package_created":
		return "revision_record_v2"
	}
	return index
}
