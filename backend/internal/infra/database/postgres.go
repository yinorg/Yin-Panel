package database

import (
	"fmt"
	"time"

	_ "gorm.io/driver/mysql"
	"gorm.io/driver/postgres"
	_ "gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

type PostgresConfig struct {
	Host         string
	Port         string
	Username     string
	Password     string
	Database     string
	SSLMode      string
	TimeZone     string
	MaxOpenConns int
	MaxIdleConns int
	WaitTimeout  int
}

func (d *PostgresConfig) Connect() (db *gorm.DB, err error) {
	sslmode := d.SSLMode
	if sslmode == "" {
		sslmode = "disable"
	}
	timezone := d.TimeZone
	if timezone == "" {
		timezone = "UTC"
	}
	dsn := fmt.Sprintf("host=%s port=%s user=%s password=%s dbname=%s sslmode=%s TimeZone=%s",
		d.Host, d.Port, d.Username, d.Password, d.Database, sslmode, timezone)
	db, err = gorm.Open(postgres.Open(dsn), &gorm.Config{
		Logger: GetLogger(),
		NamingStrategy: schema.NamingStrategy{
			SingularTable: true,
		},
		DisableForeignKeyConstraintWhenMigrating: true,
	})
	if err != nil {
		return
	}
	sqlDB, dbErr := db.DB()
	if dbErr != nil {
		err = dbErr
		return
	}
	maxOpen := d.MaxOpenConns
	if maxOpen <= 0 {
		maxOpen = 100
	}
	maxIdle := d.MaxIdleConns
	if maxIdle <= 0 {
		maxIdle = 10
	}
	sqlDB.SetMaxOpenConns(maxOpen)
	sqlDB.SetMaxIdleConns(maxIdle)
	if d.WaitTimeout > 0 {
		sqlDB.SetConnMaxLifetime(time.Duration(d.WaitTimeout) * time.Second)
	}
	return
}
