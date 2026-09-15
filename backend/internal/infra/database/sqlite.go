package database

import (
	"github.com/yinorg/Yin-Panel/backend/internal/util"
	_ "gorm.io/driver/mysql"
	"gorm.io/driver/sqlite"
	_ "gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
	"os"
	"path"
)

type SQLiteConfig struct {
	Filename string
}

func (d *SQLiteConfig) Connect() (db *gorm.DB, err error) {
	filePath := d.Filename
	exists := false
	if exists, err = util.PathExists(path.Dir(filePath)); err != nil {
		return
	} else {
		if !exists {
			if err = os.MkdirAll(path.Dir(filePath), 0700); err != nil {
				return
			}
		}

		db, err = gorm.Open(sqlite.Open(filePath), &gorm.Config{
			Logger: GetLogger(),
			NamingStrategy: schema.NamingStrategy{
				SingularTable: true,
			},
		})
		if err != nil {
			return
		}
		// WAL keeps readers independent from the single SQLite writer and makes
		// recovery after an interrupted commit more reliable.
		for _, pragma := range []string{
			"PRAGMA journal_mode=WAL",
			"PRAGMA synchronous=NORMAL",
			"PRAGMA busy_timeout=5000",
			"PRAGMA foreign_keys=ON",
		} {
			if err = db.Exec(pragma).Error; err != nil {
				return
			}
		}
	}

	return
}
