package router

import (
	"strconv"
	"strings"

	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
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
		c.Header("CDN-Cache-Control", "no-store")
		c.Header("Surrogate-Control", "no-store")
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
		system.NewThemeRouter(),
		panel.NewItemIconRouter(),
		panel.NewUserConfigRouter(),
		panel.NewUsersRouter(),
		panel.NewPublicVisitRouter(),
		panel.NewSpaceRouter(),
	}
}

func registerExtensionRoutes(routerGroup *gin.RouterGroup, modules []extension.Module) {
	for _, module := range modules {
		if module.RegisterRoutes != nil {
			authenticated := routerGroup.Group("")
			middleware := make([]gin.HandlerFunc, 0, 1+len(module.Middleware))
			middleware = append(middleware, interceptor.Auth)
			middleware = append(middleware, module.Middleware...)
			authenticated.Use(middleware...)
			module.RegisterRoutes(authenticated)
		}
	}
}

// securityHeaders sets stable document hardening headers. The Content-Security-
// Policy itself lives in `index.html` as a meta tag: the PWA service worker
// precaches the entry document with its response headers, so a header-based CSP
// would go stale until the next frontend build. A meta policy travels with the
// document content and can never be older than the shell that carries it.
func securityHeaders() gin.HandlerFunc {
	return func(c *gin.Context) {
		c.Header("X-Content-Type-Options", "nosniff")
		c.Header("Referrer-Policy", "strict-origin-when-cross-origin")
		c.Header("X-Frame-Options", "SAMEORIGIN")
		c.Next()
	}
}

func InitRouters(addr string) error {
	router := gin.Default()
	rootRouter := router.Group("/")
	rootRouter.Use(securityHeaders())

	// 注册标准 API 路由
	routerGroup := rootRouter.Group("api")
	for _, router := range RouterArray() {
		router.InitRouter(routerGroup)
	}
	registerExtensionRoutes(routerGroup, extension.Modules())
	system.NewFileRouter().InitPublicRouter(rootRouter)

	registerStaticRoutes(rootRouter)

	zaplog.Logger.Info("Yin-Panel is Started.  Listening and serving HTTP on ", addr)
	return router.Run(addr)
}

// registerStaticRoutes wires the built web files onto the root router. It is
// separate from InitRouters so a test can inspect the routing decisions without
// starting a listener, which would collide with a running instance.
func registerStaticRoutes(rootRouter *gin.RouterGroup) {
	router := rootRouter
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
		// `/clear.html` is a standalone document, not an SPA route. It must be
		// served as a file: its whole purpose is to work when the application does
		// not, so it carries its own script and loads no bundle. Without this route
		// it would fall through to `/:publicId` below and be answered with the SPA
		// entry document, silently losing the page exactly when it is needed.
		noCacheGroup.StaticFile("/clear.html", webPath+"/clear.html")

		// 处理根目录下的特定文件
		noCacheGroup.StaticFile("/", webPath+"/index.html")
		// Vue history mode routes (for example the OAuth /login redirect)
		// must fall back to the SPA entry document.
		noCacheGroup.StaticFile("/login", webPath+"/index.html")
		noCacheGroup.StaticFile("/oauth/callback", webPath+"/index.html")
		// Theme-contributed surfaces (`/theme/:view`) are SPA routes rendered by the
		// theme on top of the Core shell. They must deep-link and survive a reload,
		// so they fall back to the SPA entry document like the routes above.
		noCacheGroup.GET("/theme/:view", func(c *gin.Context) { c.File(webPath + "/index.html") })
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
}
