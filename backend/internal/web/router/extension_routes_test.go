package router

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/pkg/extension"
)

func TestExtensionRoutesAlwaysRequireCoreAuthentication(t *testing.T) {
	gin.SetMode(gin.TestMode)
	engine := gin.New()
	api := engine.Group("/api")
	privateHandlerCalled := false
	moduleMiddlewareCalled := false
	registerExtensionRoutes(api, []extension.Module{{
		Name: "test-extension",
		Middleware: []gin.HandlerFunc{func(c *gin.Context) {
			moduleMiddlewareCalled = true
			c.Next()
		}},
		RegisterRoutes: func(routes *gin.RouterGroup) {
			routes.GET("/extension/private", func(c *gin.Context) {
				privateHandlerCalled = true
				c.Status(http.StatusNoContent)
			})
		},
	}})

	privateResponse := httptest.NewRecorder()
	engine.ServeHTTP(privateResponse, httptest.NewRequest(http.MethodGet, "/api/extension/private", nil))
	if privateHandlerCalled || moduleMiddlewareCalled {
		t.Fatal("unauthenticated request reached extension middleware or handler")
	}
	if privateResponse.Code == http.StatusNoContent {
		t.Fatal("unauthenticated extension request unexpectedly succeeded")
	}

}
