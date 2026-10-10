package database

import (
	"path/filepath"
	"testing"
	"time"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

func TestSanitizeTimesReplacesZeroAndAncientValues(t *testing.T) {
	valid := time.Date(2026, 1, 2, 3, 4, 5, 0, time.UTC)
	rows := []repository.SpaceMember{
		{JoinedAt: time.Time{}},
		{JoinedAt: time.Date(1, 1, 1, 0, 0, 0, 0, time.UTC)},
		{JoinedAt: valid},
	}
	sanitizeTimes(&rows)
	if rows[0].JoinedAt.IsZero() || rows[0].JoinedAt.Year() < 1000 {
		t.Fatalf("zero time not sanitized: %v", rows[0].JoinedAt)
	}
	if rows[1].JoinedAt.Year() < 1000 {
		t.Fatalf("ancient time not sanitized: %v", rows[1].JoinedAt)
	}
	if !rows[2].JoinedAt.Equal(valid) {
		t.Fatalf("valid time changed: %v", rows[2].JoinedAt)
	}
}

func openCopyDB(t *testing.T, path string) *gorm.DB {
	t.Helper()
	db, err := gorm.Open(sqlite.Open(path), &gorm.Config{
		NamingStrategy:                           schema.NamingStrategy{SingularTable: true},
		DisableForeignKeyConstraintWhenMigrating: true,
	})
	if err != nil {
		t.Fatalf("open %s: %v", path, err)
	}
	return db
}

// CopyDatabase must move every row with its primary key preserved.
func TestCopyDatabasePreservesRowsAndIDs(t *testing.T) {
	source := openCopyDB(t, filepath.Join(t.TempDir(), "source.db"))
	target := openCopyDB(t, filepath.Join(t.TempDir(), "target.db"))

	if err := source.AutoMigrate(SchemaModels()...); err != nil {
		t.Fatalf("migrate source: %v", err)
	}
	user := repository.User{Mail: "copy@example.com", Name: "Copy", Status: 1, Role: 1}
	if err := source.Create(&user).Error; err != nil {
		t.Fatal(err)
	}
	space := repository.Space{Type: repository.SpaceTypePersonal, Name: "Copy", OwnerUserID: user.ID, Side: "yin"}
	if err := source.Create(&space).Error; err != nil {
		t.Fatal(err)
	}
	group := repository.ItemIconGroup{Title: "APP", UserId: user.ID, SpaceID: space.ID}
	if err := source.Create(&group).Error; err != nil {
		t.Fatal(err)
	}
	item := repository.ItemIcon{Title: "bookmark", ItemIconGroupId: group.ID, UserId: user.ID, SpaceID: space.ID}
	if err := source.Create(&item).Error; err != nil {
		t.Fatal(err)
	}

	if err := CopyDatabase(source, target); err != nil {
		t.Fatalf("CopyDatabase: %v", err)
	}

	var gotUser repository.User
	if err := target.First(&gotUser, user.ID).Error; err != nil {
		t.Fatalf("user not copied with id %d: %v", user.ID, err)
	}
	if gotUser.Mail != "copy@example.com" {
		t.Fatalf("user mail = %q", gotUser.Mail)
	}
	var gotItem repository.ItemIcon
	if err := target.First(&gotItem, item.ID).Error; err != nil {
		t.Fatalf("item not copied with id %d: %v", item.ID, err)
	}
	if gotItem.ItemIconGroupId != group.ID || gotItem.SpaceID != space.ID {
		t.Fatalf("item references not preserved: %+v", gotItem)
	}

	// Re-running must replace, not duplicate.
	if err := CopyDatabase(source, target); err != nil {
		t.Fatalf("second CopyDatabase: %v", err)
	}
	var users, items int64
	target.Model(&repository.User{}).Count(&users)
	target.Model(&repository.ItemIcon{}).Count(&items)
	if users != 1 || items != 1 {
		t.Fatalf("copy not idempotent: users=%d items=%d", users, items)
	}
}
