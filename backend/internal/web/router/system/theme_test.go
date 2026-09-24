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
