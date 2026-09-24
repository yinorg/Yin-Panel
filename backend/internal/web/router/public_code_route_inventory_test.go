package router

import (
	"strings"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
)

func TestRegisteredAuthenticatedRoutesDefaultDenyPublicCode(t *testing.T) {
	gin.SetMode(gin.TestMode)
	previousConfig := config.AppConfig
	previousGlobalConfig := global.Config
	config.AppConfig = &config.Config{}
	global.Config = &config.Config{}
	t.Cleanup(func() {
		config.AppConfig = previousConfig
		global.Config = previousGlobalConfig
	})
	engine := gin.New()
	api := engine.Group("/api")
	for _, route := range RouterArray() {
		route.InitRouter(api)
	}

	registeredRoutes := 0
	for _, route := range engine.Routes() {
		registeredRoutes++
		spaceParam := ""
		if strings.Contains(route.Path, ":spaceId") {
			spaceParam = "12"
		}
		got := interceptor.PublicCodeRequestAllowed(route.Method, route.Path, spaceParam, 12)
		want := publicCodeRouteExpectedForCoreAPI(route.Method, route.Path)
		if got != want {
			t.Errorf("public-code policy for registered %s %s = %v, want %v", route.Method, route.Path, got, want)
		}
	}
	if registeredRoutes < 60 {
		t.Fatalf("route inventory found only %d Core API routes; expected at least 60", registeredRoutes)
	}
}

func publicCodeRouteExpectedForCoreAPI(method, path string) bool {
	if method != "GET" {
		return false
	}
	switch path {
	case "/api/spaces",
		"/api/panel/userConfig/getConfig",
		"/api/theme/v2/current",
		"/api/theme/v2/packages",
		"/api/theme/v2/package/:id",
		"/api/theme/v2/revisions/:revision",
		"/api/theme/v2/preview/:token",
		"/api/theme/v2/preview/:token/assets/*name",
		"/api/theme/v2/assets/:revision/*name":
		return true
	case "/api/spaces/:spaceId/groups",
		"/api/spaces/:spaceId/items",
		"/api/spaces/:spaceId/search-config":
		return true
	default:
		return false
	}
}
