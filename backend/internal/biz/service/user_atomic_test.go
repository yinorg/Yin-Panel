package service

import (
	"errors"
	"path/filepath"
	"testing"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"go.uber.org/zap"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	gormlogger "gorm.io/gorm/logger"
	"gorm.io/gorm/schema"
)

// A failure part-way through account creation must leave nothing behind.
func TestCreateUserRollsBackOnPartialFailure(t *testing.T) {
	db, err := gorm.Open(sqlite.Open(filepath.Join(t.TempDir(), "create-user.db")), &gorm.Config{
		Logger:         gormlogger.Default.LogMode(gormlogger.Silent),
		NamingStrategy: schema.NamingStrategy{SingularTable: true},
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.AutoMigrate(&repository.User{}, &repository.Space{}, &repository.SpaceMember{}, &repository.ItemIconGroup{}); err != nil {
		t.Fatal(err)
	}
	previousDB := repository.Db
	repository.Db = db
	t.Cleanup(func() { repository.Db = previousDB })

	previousConfig := config.AppConfig
	config.AppConfig = &config.Config{}
	t.Cleanup(func() { config.AppConfig = previousConfig })

	previousLogger := zaplog.Logger
	zaplog.Logger = zap.NewNop().Sugar()
	t.Cleanup(func() { zaplog.Logger = previousLogger })

	// Fail the last default-group insert so the whole operation must roll back.
	db.Callback().Create().Before("gorm:create").Register("test:fail_item_group", func(tx *gorm.DB) {
		if tx.Statement != nil && tx.Statement.Table == "item_icon_group" {
			tx.AddError(errors.New("injected group failure"))
		}
	})

	svc := NewUserService(&repository.UserRepo{}, &repository.ItemIconGroupRepo{})
	user := &repository.User{Mail: "atomic@example.com", Name: "Atomic", Status: 1, Role: 2}
	if err := svc.CreateUser(user); err == nil {
		t.Fatal("expected CreateUser to fail when a later step fails")
	}

	var users, spaces, members, groups int64
	db.Model(&repository.User{}).Count(&users)
	db.Model(&repository.Space{}).Count(&spaces)
	db.Model(&repository.SpaceMember{}).Count(&members)
	db.Model(&repository.ItemIconGroup{}).Count(&groups)
	if users != 0 || spaces != 0 || members != 0 || groups != 0 {
		t.Fatalf("partial write survived: users=%d spaces=%d members=%d groups=%d", users, spaces, members, groups)
	}
}
