package global

import (
	"yin-panel/internal/biz/cache"
	"yin-panel/internal/biz/repository"
	"yin-panel/internal/biz/service"
	"yin-panel/internal/infra/config"
	"yin-panel/internal/infra/kvcache"
	"yin-panel/internal/infra/storage"
	"time"
)

// 构建时，通过 --ldflags 注入
var (
	RUNCODE = "debug" // 运行模式：debug | release
	VERSION = "v1.0.0"
)

var (
	Config  *config.Config
	Storage *storage.RcloneStorage
)

// repositories
var (
	ItemIconRepo      = repository.NewItemIconRepo()
	ItemIconGroupRepo = repository.NewItemIconGroupRepo()
	UserRepo          = repository.NewUserRepo()
	FileRepo          = repository.NewFileRepo()
	ModuleConfigRepo  = repository.NewModuleConfigRepo()
	UserConfigRepo    = repository.NewUserConfigRepo()
	SystemSettingRepo = repository.NewSystemSettingRepo()
)

// services
var (
	UserService *service.UserService
)

// caches
var (
	CacheSystemSetting = &cache.SystemSetting{
		Cache:             kvcache.NewLocalCache[any](5*time.Hour, -1),
		SystemSettingRepo: SystemSettingRepo,
	}
	CacheMonitor = &cache.Monitor{
		Cache: kvcache.NewLocalCache[any](5*time.Hour, -1),
	}
)
