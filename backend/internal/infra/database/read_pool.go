package database

import (
	"database/sql"
	"fmt"

	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

// DefaultSQLiteReadPoolSize is used when the configuration does not set one.
const DefaultSQLiteReadPoolSize = 4

var sqliteReadPool *gorm.DB

// SQLiteReadPool returns the read-only pool when it is enabled, else nil.
// Application code keeps using repository.Db; the router splits reads for it.
func SQLiteReadPool() *gorm.DB { return sqliteReadPool }

// EnableSQLiteReadPool opens a read-only pool over the same SQLite file and
// installs a router on writer that sends non-transactional reads to it. Writes,
// DDL, migrations, PRAGMA and lock operations remain on writer. Callers should
// treat an error as non-fatal and keep the single-pool writer.
func EnableSQLiteReadPool(writer *gorm.DB, filePath string, size int) error {
	if writer == nil {
		return fmt.Errorf("read pool: nil writer")
	}
	if filePath == "" {
		return fmt.Errorf("read pool: empty file path")
	}
	if size <= 0 {
		size = DefaultSQLiteReadPoolSize
	}
	// mode=ro makes the connection genuinely read-only; WAL is a persistent
	// database setting so the reader inherits it without a write pragma.
	dsn := "file:" + filePath + "?mode=ro&_busy_timeout=5000"
	reader, err := gorm.Open(sqlite.Open(dsn), &gorm.Config{
		Logger: GetLogger(),
		NamingStrategy: schema.NamingStrategy{
			SingularTable: true,
		},
		DisableForeignKeyConstraintWhenMigrating: true,
	})
	if err != nil {
		return fmt.Errorf("open sqlite read pool: %w", err)
	}
	sqlReader, err := reader.DB()
	if err != nil {
		return fmt.Errorf("sqlite read pool handle: %w", err)
	}
	sqlReader.SetMaxOpenConns(size)
	sqlReader.SetMaxIdleConns(size)
	if err := sqlReader.Ping(); err != nil {
		return fmt.Errorf("sqlite read pool ping: %w", err)
	}

	registerReadPoolRouter(writer, reader)
	sqliteReadPool = reader
	return nil
}

// registerReadPoolRouter hooks GORM's real execution callbacks. Reads flow
// through the Query (Find/First/Scan/Count) and Row (Row/Rows) callbacks;
// writes flow through Create/Update/Delete and raw Exec, which are never
// touched. Inside a transaction GORM replaces ConnPool with *sql.Tx, so
// transactional reads stay on the writer and observe uncommitted data.
func registerReadPoolRouter(writer, reader *gorm.DB) {
	route := func(tx *gorm.DB) {
		if tx == nil || tx.Statement == nil || tx.Statement.ConnPool == nil {
			return
		}
		if _, isTx := tx.Statement.ConnPool.(*sql.Tx); isTx {
			return
		}
		tx.Statement.ConnPool = reader.ConnPool
	}
	writer.Callback().Query().Before("gorm:query").Register("yin:sqlite_read_pool:query", route)
	writer.Callback().Row().Before("gorm:row").Register("yin:sqlite_read_pool:row", route)
}
