package router

import (
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"github.com/yinorg/Yin-Panel/backend/internal/web/router/panel"
	"github.com/yinorg/Yin-Panel/backend/internal/web/router/system"
	"github.com/yinorg/Yin-Panel/backend/pkg/extension"

	"github.com/gin-gonic/gin"
)

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
		router.StaticFS("/assets", gin.Dir(webPath+"/assets", false))
		router.StaticFS("/custom", gin.Dir(webPath+"/custom", false))

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
