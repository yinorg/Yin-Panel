// Package app provides the public Core launcher used by both CE and EE
// entrypoints. Enterprise code registers extensions before calling Run.
package app

import (
	"context"
	"fmt"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/service"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/database"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"github.com/yinorg/Yin-Panel/backend/internal/util/i18n"
	"github.com/yinorg/Yin-Panel/backend/internal/util/jwt"
	"github.com/yinorg/Yin-Panel/backend/internal/web/router"
	"github.com/yinorg/Yin-Panel/backend/pkg/extension"
	"github.com/yinorg/Yin-Panel/backend/pkg/storage"
)

// Init initializes and serves the Core application. Extensions must be
// registered before this function is called.
func Init(configPath string) error {
	gin.SetMode(global.RUNCODE)
	if err := zaplog.InitLog(global.RUNCODE, "running.zaplog"); err != nil {
		return fmt.Errorf("zaplog initialization error: %w", err)
	}

	var err error
	global.Config, err = config.Init(configPath)
	if err != nil {
		return err
	}
	i18n.LangInit("zh-cn")
	global.UserService = service.NewUserService(global.UserRepo, global.ItemIconGroupRepo)

	if err = databaseConnect(); err != nil {
		return err
	}

	modules := extension.Modules()
	for _, module := range modules {
		if module.Init != nil {
			if err := module.Init(context.Background(), extension.Runtime{DB: repository.Db}); err != nil {
				return fmt.Errorf("initialize extension %s: %w", module.Name, err)
			}
		}
	}

	global.Storage, err = initStorage(context.Background(), modules)
	if err != nil {
		return fmt.Errorf("storage initialization error: %w", err)
	}
	if err := jwt.InitJWT(); err != nil {
		return fmt.Errorf("JWT initialization error: %w", err)
	}
	return router.InitRouters(":" + config.AppConfig.Base.HTTPPort)
}

func databaseConnect() error {
	var client database.DbClient
	switch config.AppConfig.Base.DatabaseDrive {
	case database.MYSQL:
		client = &database.MySQLConfig{Username: config.AppConfig.MySQL.Username, Password: config.AppConfig.MySQL.Password, Host: config.AppConfig.MySQL.Host, Port: config.AppConfig.MySQL.Port, Database: config.AppConfig.MySQL.DBName, WaitTimeout: config.AppConfig.MySQL.WaitTimeout}
	case database.SQLITE:
		client = &database.SQLiteConfig{Filename: config.AppConfig.SQLite.FilePath}
	default:
		return fmt.Errorf("unsupported database drive: %s", config.AppConfig.Base.DatabaseDrive)
	}
	db, err := database.DbInit(client)
	if err != nil {
		return fmt.Errorf("database initialization: %w", err)
	}
	repository.Db = db
	if err = database.CreateDefaultUser(); err != nil {
		return fmt.Errorf("create default user: %w", err)
	}
	if err = database.EnsurePersonalSpaces(db); err != nil {
		return fmt.Errorf("ensure personal spaces: %w", err)
	}
	if global.Config.Base.EnableMonitor {
		global.CacheMonitor.Start()
	}
	return nil
}

func initStorage(ctx context.Context, modules []extension.Module) (storage.Storage, error) {
	for _, module := range modules {
		if module.StorageFactory == nil {
			continue
		}
		storageCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
		backend, err := module.StorageFactory(storageCtx)
		cancel()
		if err != nil {
			return nil, fmt.Errorf("extension %s storage: %w", module.Name, err)
		}
		if backend == nil {
			return nil, fmt.Errorf("extension %s returned nil storage", module.Name)
		}
		return backend, nil
	}
	if config.AppConfig.Rclone.Type != "" && config.AppConfig.Rclone.Type != "local" {
		return nil, fmt.Errorf("Core only supports local storage; use the Enterprise image for %q", config.AppConfig.Rclone.Type)
	}
	return storage.NewLocal(config.AppConfig.Rclone.Bucket)
}
