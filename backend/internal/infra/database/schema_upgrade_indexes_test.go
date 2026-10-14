package database

import (
	"reflect"
	"sort"
	"testing"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"

	"gorm.io/gorm"
)

// tableIndexNames lists the indexes GORM knows about for a model, sorted so the
// comparison is stable.
func tableIndexNames(t *testing.T, db *gorm.DB, model interface{}) []string {
	t.Helper()
	indexes, err := db.Migrator().GetIndexes(model)
	if err != nil {
		t.Fatalf("GetIndexes: %v", err)
	}
	names := make([]string, 0, len(indexes))
	for _, index := range indexes {
		names = append(names, index.Name())
	}
	sort.Strings(names)
	return names
}

// A retired column added by ALTER TABLE ADD COLUMN is stored without quotes, and
// GORM's SQLite DropColumn only recognises a quoted definition, so it used to
// leave the column in place forever. The column must actually go, and the table
// must keep its indexes.
func TestDropLegacySchemaRemovesUnquotedRetiredColumns(t *testing.T) {
	db := openSchemaUpgradeDB(t, "drop-legacy-schema-columns.db")
	if err := db.AutoMigrate(&repository.User{}, &repository.Space{}); err != nil {
		t.Fatal(err)
	}
	// No backticks: this is how SQLite itself writes an added column, and the
	// shape GORM's SQLite DropColumn fails to match.
	for _, statement := range []string{
		"ALTER TABLE user ADD COLUMN publiccode varchar(50)",
		"ALTER TABLE user ADD COLUMN oauth_provider varchar(50)",
		"ALTER TABLE user ADD COLUMN oauth_id varchar(255)",
		"ALTER TABLE space ADD COLUMN team_id integer",
	} {
		if err := db.Exec(statement).Error; err != nil {
			t.Fatal(err)
		}
	}

	before := tableIndexNames(t, db, &repository.Space{})
	if len(before) == 0 {
		t.Fatal("the fixture has no indexes to lose, so it cannot detect the regression")
	}

	if err := dropLegacySchema(db); err != nil {
		t.Fatal(err)
	}

	for _, column := range []string{"publiccode", "oauth_provider", "oauth_id"} {
		if db.Migrator().HasColumn("user", column) {
			t.Fatalf("user.%s was not dropped", column)
		}
	}
	if db.Migrator().HasColumn("space", "team_id") {
		t.Fatal("space.team_id was not dropped")
	}
	if after := tableIndexNames(t, db, &repository.Space{}); !reflect.DeepEqual(before, after) {
		t.Fatalf("indexes on space = %v, want %v", after, before)
	}
}

// Dropping a foreign key still recreates the table on SQLite, which drops every
// index on it. The indexes the model tags declare must come back.
func TestDropLegacySchemaRestoresIndexesOnRebuiltTables(t *testing.T) {
	db := openSchemaUpgradeDB(t, "drop-legacy-schema-constraints.db")
	for _, statement := range []string{
		"CREATE TABLE `item_icon` (`id` integer PRIMARY KEY AUTOINCREMENT, `user_id` integer, CONSTRAINT `fk_item_icon_user` FOREIGN KEY (`user_id`) REFERENCES `user`(`id`))",
		"CREATE INDEX `idx_item_icon_user_id` ON `item_icon` (`user_id`)",
	} {
		if err := db.Exec(statement).Error; err != nil {
			t.Fatal(err)
		}
	}
	if !db.Migrator().HasConstraint(&repository.ItemIcon{}, "fk_item_icon_user") {
		t.Fatal("the fixture has no foreign key to drop, so it cannot detect the regression")
	}

	if err := dropLegacySchema(db); err != nil {
		t.Fatal(err)
	}

	if db.Migrator().HasConstraint(&repository.ItemIcon{}, "fk_item_icon_user") {
		t.Fatal("item_icon foreign key was not dropped")
	}
	if !db.Migrator().HasIndex(&repository.ItemIcon{}, "idx_item_icon_user_id") {
		t.Fatal("idx_item_icon_user_id was not restored after the table rebuild")
	}
}
