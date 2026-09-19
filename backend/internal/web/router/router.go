package router

import (
	"strconv"
	"strings"

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

func noCacheStatic() gin.HandlerFunc {
	return func(c *gin.Context) {
		c.Header("Cache-Control", "no-store, no-cache, must-revalidate")
		c.Header("Pragma", "no-cache")
		c.Header("Expires", "0")
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
		router.Group("/assets").Use(cacheStatic(2592000, true)).StaticFS("", gin.Dir(webPath+"/assets", false))
		router.Group("/custom").Use(cacheStatic(86400, false)).StaticFS("", gin.Dir(webPath+"/custom", false))

		// Entry documents and PWA control files must always be revalidated. The
		// hashed assets they reference are safe to cache independently.
		noCacheGroup := router.Group("/").Use(noCacheStatic())
		noCacheGroup.StaticFile("/index.html", webPath+"/index.html")
		noCacheGroup.StaticFile("/registerSW.js", webPath+"/registerSW.js")
		noCacheGroup.StaticFile("/sw.js", webPath+"/sw.js")
		noCacheGroup.StaticFile("/manifest.webmanifest", webPath+"/manifest.webmanifest")

		// 处理根目录下的特定文件
		noCacheGroup.StaticFile("/", webPath+"/index.html")
		// Vue history mode routes (for example the OAuth /login redirect)
		// must fall back to the SPA entry document.
		noCacheGroup.StaticFile("/login", webPath+"/index.html")
		// Public space links are handled by the SPA router.
		noCacheGroup.GET("/:publicId", func(c *gin.Context) { c.File(webPath + "/index.html") })

		// The Workbox runtime has a content hash in its filename and can be
		// cached like the other immutable build assets. Keep the route dynamic so
		// upgrading vite-plugin-pwa does not require a backend route change.
		workboxGroup := router.Group("/").Use(cacheStatic(2592000, true))
		workboxGroup.GET("/workbox-:filename", func(c *gin.Context) {
			filename := c.Param("filename")
			if !strings.HasSuffix(filename, ".js") || strings.Contains(filename, "/") {
				c.Status(404)
				return
			}
			c.File(webPath + "/workbox-" + filename)
		})

		zaplog.Logger.Info("Static file server is enabled")
	} else {
		zaplog.Logger.Info("Static file server is disabled")
	}

	zaplog.Logger.Info("Yin-Panel is Started.  Listening and serving HTTP on ", addr)
	return router.Run(addr)
}
