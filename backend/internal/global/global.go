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
	RUNCODE    = "debug"   // 运行模式：debug | release
	VERSION    = "v0.4.0"  // 语义化版本
	COMMIT     = "unknown" // 构建注入：Git commit
	BUILD_TIME = "unknown" // 构建注入：构建时间（RFC3339, UTC）
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
)

// services
var (
	UserService *service.UserService
)

// caches
var (
	CacheMonitor = &cache.Monitor{
		Cache: kvcache.NewLocalCache[any](5*time.Hour, -1),
	}
)
