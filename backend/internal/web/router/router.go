package router

import (
	"strconv"

	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"github.com/yinorg/Yin-Panel/backend/internal/web/router/panel"
	"github.com/yinorg/Yin-Panel/backend/internal/web/router/system"
	"github.com/yinorg/Yin-Panel/backend/pkg/extension"

	"github.com/gin-gonic/gin"
)

func cacheStatic(maxAge int, immutable bool) gin.HandlerFunc {
	return func(c *gin.Context) {
		value := "public, max-age=" + strconv.Itoa(maxAge)
		if immutable {
			value += ", immutable"
		}
		c.Header("Cache-Control", value)
		c.Next()
	}
}

type IRouter interface {
	InitRouter(Router *gin.RouterGroup)
}

func RouterArray() []IRouter {
	return []IRouter{
		system.NewAboutRouter(),
		system.NewCapabilitiesRouter(),
		system.NewLoginRouter(),
		system.NewFileRouter(),
		system.NewUserRouter(),
		system.NewModuleConfigRouter(),
		system.NewMonitorRouter(),
		system.NewOAuthRouter(),
		panel.NewItemIconRouter(),
		panel.NewUserConfigRouter(),
		panel.NewUsersRouter(),
		panel.NewPublicVisitRouter(),
		panel.NewSpaceRouter(),
	}
}

func InitRouters(addr string) error {
	router := gin.Default()
	rootRouter := router.Group("/")

	// 注册标准 API 路由
	routerGroup := rootRouter.Group("api")
	for _, router := range RouterArray() {
		router.InitRouter(routerGroup)
	}
	for _, module := range extension.Modules() {
		if module.RegisterRoutes != nil {
			module.RegisterRoutes(routerGroup)
		}
	}
	system.NewFileRouter().InitPublicRouter(rootRouter)

	// WEB文件服务
	if config.AppConfig.Base.EnableStaticServer {
		webPath := "./web"

		// 使用StaticFS处理所有静态资源
		router.Group("/assets").Use(cacheStatic(31536000, true)).StaticFS("", gin.Dir(webPath+"/assets", false))
		router.Group("/custom").Use(cacheStatic(86400, false)).StaticFS("", gin.Dir(webPath+"/custom", false))
		// PWA files are emitted at the web root and must be served before SPA fallback routes.
		router.StaticFile("/registerSW.js", webPath+"/registerSW.js")
		router.StaticFile("/sw.js", webPath+"/sw.js")
		router.StaticFile("/workbox-3625d7b0.js", webPath+"/workbox-3625d7b0.js")
		router.StaticFile("/manifest.webmanifest", webPath+"/manifest.webmanifest")

		// 处理根目录下的特定文件
		router.StaticFile("/", webPath+"/index.html")
		// Vue history mode routes (for example the OAuth /login redirect)
		// must fall back to the SPA entry document.
		router.StaticFile("/login", webPath+"/index.html")
		// Public space links are handled by the SPA router.
		router.GET("/:publicId", func(c *gin.Context) { c.File(webPath + "/index.html") })

		zaplog.Logger.Info("Static file server is enabled")
	} else {
		zaplog.Logger.Info("Static file server is disabled")
	}

	zaplog.Logger.Info("Yin-Panel is Started.  Listening and serving HTTP on ", addr)
	return router.Run(addr)
}
