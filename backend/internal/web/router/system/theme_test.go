package system

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/util/i18n"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
)

func TestThemePackageRoutesExposeOnlyV2PackageContract(t *testing.T) {
	gin.SetMode(gin.TestMode)
	engine := gin.New()
	NewThemeRouter().InitRouter(engine.Group("/api"))
	routes := make(map[string]bool)
	for _, route := range engine.Routes() {
		routes[route.Method+" "+route.Path] = true
	}
	for _, route := range []string{
		http.MethodGet + " /api/theme/v2/current",
		http.MethodGet + " /api/theme/v2/mine",
		http.MethodGet + " /api/theme/v2/preference",
		http.MethodPost + " /api/theme/v2/preference",
		http.MethodGet + " /api/theme/v2/packages",
		http.MethodGet + " /api/theme/v2/package/:id",
		http.MethodGet + " /api/theme/v2/revisions/:revision",
		http.MethodGet + " /api/theme/v2/preview/:token",
		http.MethodPost + " /api/theme/v2/admin/install",
		http.MethodPost + " /api/theme/v2/admin/preview",
		http.MethodPost + " /api/theme/v2/admin/trial",
		http.MethodPost + " /api/theme/v2/admin/confirm",
		http.MethodPost + " /api/theme/v2/admin/rollback",
		http.MethodGet + " /api/theme/v2/admin/trusted/:revision",
		http.MethodPut + " /api/theme/v2/admin/trusted/:revision",
		http.MethodGet + " /api/theme/v2/grants/:revision",
		http.MethodPost + " /api/theme/v2/grants/:revision",
		http.MethodDelete + " /api/theme/v2/grants/:revision",
	} {
		if !routes[route] {
			t.Errorf("missing V2 Theme API route %q", route)
		}
	}
	for _, route := range []string{
		http.MethodGet + " /api/theme/current",
		http.MethodGet + " /api/theme/packages",
		http.MethodGet + " /api/theme/packages/:id",
		http.MethodGet + " /api/theme/preview/:token",
		http.MethodPost + " /api/theme/admin/install",
		http.MethodPost + " /api/theme/admin/preview",
		http.MethodGet + " /api/theme/admin/packages",
	} {
		if routes[route] {
			t.Errorf("legacy Theme package route %q is still registered", route)
		}
	}
}

func TestThemeSVGAssetsAreSandboxedDocuments(t *testing.T) {
	gin.SetMode(gin.TestMode)
	recorder := httptest.NewRecorder()
	ctx, _ := gin.CreateTestContext(recorder)
	ctx.Request = httptest.NewRequest(http.MethodGet, "/api/theme/v2/assets/revision/assets/icon.svg", nil)
	serveThemeAsset(ctx, "image/svg+xml", []byte(`<svg xmlns="http://www.w3.org/2000/svg"><script>parent.document.body.dataset.pwned='1'</script></svg>`), "assets/icon.svg", true)
	if got := recorder.Header().Get("X-Content-Type-Options"); got != "nosniff" {
		t.Fatalf("SVG nosniff header = %q", got)
	}
	policy := recorder.Header().Get("Content-Security-Policy")
	for _, directive := range []string{"sandbox;", "default-src 'none'", "script-src 'none'", "object-src 'none'", "base-uri 'none'"} {
		if !strings.Contains(policy, directive) {
			t.Errorf("SVG CSP %q is missing %q", policy, directive)
		}
	}
	if recorder.Code != http.StatusOK || recorder.Body.Len() == 0 {
		t.Fatalf("SVG response status/body = %d/%d", recorder.Code, recorder.Body.Len())
	}
}

func TestThemeRoutesRequireJWTAndAdminRole(t *testing.T) {
	gin.SetMode(gin.TestMode)
	i18n.Obj = i18n.NewLang("../../../../lang/en-us.ini")
	t.Run("public access cannot manage a theme", func(t *testing.T) {
		ctx, _ := gin.CreateTestContext(httptest.NewRecorder())
		ctx.Set("authMethod", "publiccode")
		themeJWTOnly(ctx)
		if !ctx.IsAborted() {
			t.Fatal("public access was not aborted")
		}
	})
	t.Run("authenticated non-admin cannot manage themes", func(t *testing.T) {
		ctx, _ := gin.CreateTestContext(httptest.NewRecorder())
		ctx.Set("authMethod", "jwt")
		ctx.Set("userInfo", base.UserInfo{ID: 4, Role: 0})
		interceptor.AdminInterceptor(ctx)
		if !ctx.IsAborted() {
			t.Fatal("non-admin access was not aborted")
		}
	})
	t.Run("admin JWT is allowed", func(t *testing.T) {
		ctx, _ := gin.CreateTestContext(httptest.NewRecorder())
		ctx.Set("authMethod", "jwt")
		ctx.Set("userInfo", base.UserInfo{ID: 1, Role: 1})
		themeJWTOnly(ctx)
		interceptor.AdminInterceptor(ctx)
		if ctx.IsAborted() {
			t.Fatal("admin JWT was aborted")
		}
	})
}

// HTML assets carry a policy in their headers, and those headers can change without
// the body (or the revision-addressed URL) moving. They therefore revalidate instead
// of being cached as immutable, and a 304 has to repeat the policy so a browser that
// already stored the document picks the new one up.
func TestThemeHTMLAssetsRevalidateAndRepeatTheirPolicy(t *testing.T) {
	gin.SetMode(gin.TestMode)
	body := []byte("<!doctype html><html><body>scene</body></html>")
	// Routed through an engine: gin flushes the status only when the handler chain
	// finishes, and a 304 carries no body to flush it implicitly.
	engine := gin.New()
	engine.GET("/asset", func(c *gin.Context) { serveThemeAsset(c, "text/html", body, "page.html", true) })

	first := httptest.NewRecorder()
	engine.ServeHTTP(first, httptest.NewRequest(http.MethodGet, "/asset", nil))

	cacheControl := first.Header().Get("Cache-Control")
	for _, want := range []string{"max-age=0", "must-revalidate", "no-transform"} {
		if !strings.Contains(cacheControl, want) {
			t.Errorf("HTML asset Cache-Control %q is missing %q", cacheControl, want)
		}
	}
	if strings.Contains(cacheControl, "immutable") {
		t.Errorf("HTML asset must not be immutable: %q", cacheControl)
	}
	etag := first.Header().Get("ETag")
	if etag == "" {
		t.Fatal("HTML asset has no ETag, so a revalidation would refetch the body")
	}
	policy := first.Header().Get("Content-Security-Policy")
	if !strings.Contains(policy, "sandbox allow-scripts") || !strings.Contains(policy, "connect-src https://cloudflareinsights.com") {
		t.Errorf("HTML asset sandbox policy = %q", policy)
	}

	second := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodGet, "/asset", nil)
	request.Header.Set("If-None-Match", etag)
	engine.ServeHTTP(second, request)
	if second.Code != http.StatusNotModified {
		t.Fatalf("revalidation status = %d, want %d", second.Code, http.StatusNotModified)
	}
	if got := second.Header().Get("Content-Security-Policy"); got != policy {
		t.Errorf("304 did not repeat the policy:\n got %q\nwant %q", got, policy)
	}
}
