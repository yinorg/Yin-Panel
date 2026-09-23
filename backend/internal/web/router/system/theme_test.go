package system

import (
	"net/http/httptest"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/util/i18n"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
)

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
