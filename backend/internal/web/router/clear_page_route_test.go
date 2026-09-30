package router

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"go.uber.org/zap"
)

// `/clear.html` is the way out of a browser whose worker or cache is broken, so
// it has to be its own document rather than an SPA route. The router also maps
// `/:publicId` to the SPA entry document for public space links, and a
// single-segment path like `/clear.html` matches that pattern — so without an
// explicit route the clear page silently becomes a second copy of the app, and it
// fails at the one moment it exists for.
//
// This asserts the routing decision, not the file's contents: served as a file,
// and reachable as a plain GET with no authentication.
func TestClearPageIsServedAsAStandaloneDocument(t *testing.T) {
	gin.SetMode(gin.TestMode)
	previousConfig := config.AppConfig
	previousGlobal := global.Config
	config.AppConfig = &config.Config{}
	global.Config = &config.Config{}
	config.AppConfig.Base.EnableStaticServer = true
	if zaplog.Logger == nil {
		zaplog.Logger = zap.NewNop().Sugar()
	}
	t.Cleanup(func() {
		config.AppConfig = previousConfig
		global.Config = previousGlobal
	})

	// `registerStaticRoutes` resolves the files through `./web`, relative to the
	// service's working directory — the backend directory. Tests run from the
	// package directory, so the assertion runs from the backend directory instead.
	previous, err := os.Getwd()
	if err != nil {
		t.Fatalf("getwd: %v", err)
	}
	backendDir, err := filepath.Abs(filepath.Join("..", "..", ".."))
	if err != nil {
		t.Fatalf("abs: %v", err)
	}
	if _, err := os.Stat(filepath.Join(backendDir, "web", "clear.html")); err != nil {
		t.Skipf("built web assets are not present: %v", err)
	}
	if err := os.Chdir(backendDir); err != nil {
		t.Fatalf("chdir: %v", err)
	}
	t.Cleanup(func() {
		if err := os.Chdir(previous); err != nil {
			t.Errorf("restore working directory: %v", err)
		}
	})

	engine := gin.New()
	root := engine.Group("/")
	registerStaticRoutes(root)

	recorder := httptest.NewRecorder()
	engine.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, "/clear.html", nil))

	if recorder.Code != http.StatusOK {
		t.Fatalf("GET /clear.html = %d, want 200", recorder.Code)
	}
	body := recorder.Body.String()

	if !strings.Contains(body, "static-clear") {
		t.Fatal("GET /clear.html did not return the clear page; the SPA fallback answered instead, which loses the page when the application cannot load")
	}
	// The entry document is what the fallback would have returned. Its presence
	// would mean the route was swallowed.
	if strings.Contains(body, `<div id="app">`) {
		t.Fatal("GET /clear.html returned the SPA entry document instead of the clear page")
	}
	// It must not depend on the bundle this page exists to escape.
	if strings.Contains(body, "/src/main.ts") || strings.Contains(body, `<script type="module" src="/assets/`) {
		t.Fatal("the clear page must not load the application bundle")
	}
}
