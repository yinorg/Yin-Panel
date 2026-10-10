package database

import (
	"path/filepath"
	"testing"

	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

type rpWidget struct {
	ID   uint   `gorm:"primaryKey"`
	Name string `gorm:"column:name"`
}

func (rpWidget) TableName() string { return "rp_widget" }

func openRW(t *testing.T, path string) *gorm.DB {
	t.Helper()
	db, err := gorm.Open(sqlite.Open(path), &gorm.Config{
		NamingStrategy: schema.NamingStrategy{SingularTable: true},
		DisableForeignKeyConstraintWhenMigrating: true,
	})
	if err != nil {
		t.Fatalf("open %s: %v", path, err)
	}
	return db
}

// Reads must be routed to the read pool, writes must stay on the writer, and
// transactional reads must run on the writer so they see uncommitted data.
func TestReadPoolRouterSplitsReadsAndWrites(t *testing.T) {
	dir := t.TempDir()
	writerPath := filepath.Join(dir, "writer.db")
	readerPath := filepath.Join(dir, "reader.db")

	writer := openRW(t, writerPath)
	if err := writer.AutoMigrate(&rpWidget{}); err != nil {
		t.Fatalf("migrate writer: %v", err)
	}
	if err := writer.Create(&rpWidget{Name: "on-writer"}).Error; err != nil {
		t.Fatalf("seed writer: %v", err)
	}
	// The reader is a distinct file so a routed read is observable.
	reader := openRW(t, readerPath)
	if err := reader.AutoMigrate(&rpWidget{}); err != nil {
		t.Fatalf("migrate reader: %v", err)
	}
	if err := reader.Create(&rpWidget{Name: "on-reader"}).Error; err != nil {
		t.Fatalf("seed reader: %v", err)
	}

	registerReadPoolRouter(writer, reader)

	var got []rpWidget
	if err := writer.Order("id").Find(&got).Error; err != nil {
		t.Fatalf("routed read: %v", err)
	}
	if len(got) != 1 || got[0].Name != "on-reader" {
		t.Fatalf("read not routed to the read pool, got %+v", got)
	}

	// A write must not land in the reader file.
	if err := writer.Create(&rpWidget{Name: "writer-write"}).Error; err != nil {
		t.Fatalf("writer create: %v", err)
	}
	var onReader rpWidget
	if err := reader.Where("name = ?", "writer-write").First(&onReader).Error; err == nil {
		t.Fatalf("write leaked into the read pool file")
	}
	direct := openRW(t, writerPath)
	var onWriter rpWidget
	if err := direct.Where("name = ?", "writer-write").First(&onWriter).Error; err != nil {
		t.Fatalf("write did not reach the writer file: %v", err)
	}

	// Inside a transaction the read must stay on the writer and see uncommitted rows.
	txErr := writer.Transaction(func(tx *gorm.DB) error {
		if err := tx.Create(&rpWidget{Name: "uncommitted"}).Error; err != nil {
			return err
		}
		var inTx []rpWidget
		if err := tx.Where("name = ?", "uncommitted").Find(&inTx).Error; err != nil {
			return err
		}
		if len(inTx) != 1 {
			t.Fatalf("transactional read was routed away from the writer: %+v", inTx)
		}
		return nil
	})
	if txErr != nil {
		t.Fatalf("transaction: %v", txErr)
	}
}

// The enabled read pool is genuinely read-only, and reads see committed writes.
func TestReadPoolIsReadOnlyAndSeesCommittedWrites(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "db.db")
	writer := openRW(t, path)
	if err := writer.Exec("PRAGMA journal_mode=WAL").Error; err != nil {
		t.Fatalf("wal: %v", err)
	}
	if err := writer.AutoMigrate(&rpWidget{}); err != nil {
		t.Fatalf("migrate: %v", err)
	}

	if err := EnableSQLiteReadPool(writer, path, 2); err != nil {
		t.Fatalf("enable read pool: %v", err)
	}
	reader := SQLiteReadPool()
	if reader == nil {
		t.Fatal("read pool is nil")
	}
	if err := reader.Exec("INSERT INTO rp_widget (name) VALUES (?)", "nope").Error; err == nil {
		t.Fatal("read pool accepted a write; it is not read-only")
	}

	// Read-after-write across the split must observe the committed row.
	if err := writer.Create(&rpWidget{Name: "committed"}).Error; err != nil {
		t.Fatalf("create: %v", err)
	}
	var got rpWidget
	if err := writer.Where("name = ?", "committed").First(&got).Error; err != nil {
		t.Fatalf("read after write: %v", err)
	}
}
