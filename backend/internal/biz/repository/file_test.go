package repository

import (
	"fmt"
	"path/filepath"
	"testing"

	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
)

func TestFileRepoGetPagedBoundsRowsAndReportsCount(t *testing.T) {
	db, err := gorm.Open(sqlite.Open(filepath.Join(t.TempDir(), "file-repository.db")), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.AutoMigrate(&File{}); err != nil {
		t.Fatal(err)
	}
	previousDB := Db
	Db = db
	t.Cleanup(func() { Db = previousDB })

	for i := 0; i < 5; i++ {
		if err := db.Create(&File{UserId: 1, FileName: fmt.Sprintf("file-%d.png", i)}).Error; err != nil {
			t.Fatal(err)
		}
	}

	repo := &FileRepo{}

	page1, count, err := repo.GetPaged(PagedParam{Limit: 2, Page: 1})
	if err != nil {
		t.Fatalf("page 1: %v", err)
	}
	if count != 5 {
		t.Fatalf("count = %d, want 5", count)
	}
	if len(page1) != 2 {
		t.Fatalf("page 1 rows = %d, want 2", len(page1))
	}

	page3, _, err := repo.GetPaged(PagedParam{Limit: 2, Page: 3})
	if err != nil {
		t.Fatalf("page 3: %v", err)
	}
	if len(page3) != 1 {
		t.Fatalf("page 3 rows = %d, want 1", len(page3))
	}
	if page3[0].ID == page1[0].ID {
		t.Fatalf("pages overlap: page 3 returned the same first row as page 1")
	}
}
