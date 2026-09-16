package global

import (
	"github.com/yinorg/Yin-Panel/backend/internal/biz/cache"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/service"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/kvcache"
	"github.com/yinorg/Yin-Panel/backend/pkg/storage"
	"time"
)

// 构建时，通过 --ldflags 注入
var (
	RUNCODE = "debug" // 运行模式：debug | release
	VERSION = "v0.3.15"
)

var (
	Config  *config.Config
	Storage storage.Storage
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
