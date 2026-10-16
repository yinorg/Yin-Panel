package router

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gin-gonic/gin"
)

// The API describes the signed-in user's own data, so a cache must never replay it.
// Handlers that serve immutable, content-addressed bytes still overwrite the header
// with their own lifetime — the theme and wallpaper asset routes rely on that.
func TestAPIRoutesSayNoStoreUnlessTheyOverrideIt(t *testing.T) {
	gin.SetMode(gin.TestMode)
	engine := gin.New()
	group := engine.Group("/api")
	group.Use(noStoreAPI())
	group.GET("/things", func(c *gin.Context) { c.JSON(http.StatusOK, gin.H{"ok": true}) })
	group.GET("/asset", func(c *gin.Context) {
		c.Header("Cache-Control", "public, max-age=31536000, immutable")
		c.Status(http.StatusOK)
	})

	plain := httptest.NewRecorder()
	engine.ServeHTTP(plain, httptest.NewRequest(http.MethodGet, "/api/things", nil))
	if got := plain.Header().Get("Cache-Control"); got != "no-store" {
		t.Fatalf("plain API Cache-Control = %q, want no-store", got)
	}

	asset := httptest.NewRecorder()
	engine.ServeHTTP(asset, httptest.NewRequest(http.MethodGet, "/api/asset", nil))
	if got := asset.Header().Get("Cache-Control"); got != "public, max-age=31536000, immutable" {
		t.Fatalf("asset route could not override the API policy: %q", got)
	}
}
