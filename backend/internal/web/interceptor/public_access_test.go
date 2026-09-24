package interceptor

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
)

func TestPublicCodeRoutePolicyAllowsOnlyHomeReads(t *testing.T) {
	cases := []struct {
		method string
		route  string
		allow  bool
	}{
		{http.MethodGet, "/api/spaces", true},
		{http.MethodGet, "/api/spaces/:spaceId/groups", true},
		{http.MethodGet, "/api/spaces/:spaceId/items", true},
		{http.MethodGet, "/api/spaces/:spaceId/search-config", true},
		{http.MethodGet, "/api/panel/userConfig/getConfig", true},
		{http.MethodGet, "/api/theme/current", true},
		{http.MethodGet, "/api/theme/packages", true},
		{http.MethodGet, "/api/theme/packages/:id", true},
		{http.MethodGet, "/api/theme/v2/assets/:revision/*name", true},
		{http.MethodPost, "/api/theme/v2/admin/install", false},
		{http.MethodGet, "/api/theme/preview/:token", true},
		{http.MethodGet, "/api/theme/preview/:token/assets/*name", true},
		{http.MethodGet, "/api/theme/v2/grants/:revision", false},
		{http.MethodPost, "/api/spaces/:spaceId/items", false},
		{http.MethodGet, "/api/spaces/:spaceId/members", false},
		{http.MethodGet, "/api/system/monitor/getSnapshot", false},
		{http.MethodGet, "/api/users", false},
	}
	for _, tc := range cases {
		t.Run(tc.method+" "+tc.route, func(t *testing.T) {
			got := publicCodeRequestAllowed(tc.method, tc.route, "12", 12)
			if got != tc.allow {
				t.Fatalf("public route policy = %v, want %v", got, tc.allow)
			}
		})
	}
}

func TestPublicCodePolicyUsesMatchedGinRouteAndPathParameter(t *testing.T) {
	gin.SetMode(gin.TestMode)
	engine := gin.New()
	var allowed bool
	engine.GET("/api/spaces/:spaceId/groups", func(c *gin.Context) {
		allowed = publicCodeRouteAllowed(c, 12)
	})
	response := httptest.NewRecorder()
	engine.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/api/spaces/12/groups", nil))
	if response.Code != http.StatusOK || !allowed {
		t.Fatalf("matched public route result: status=%d allowed=%v", response.Code, allowed)
	}
}

func TestPublicCodeSpaceScopeIncludesOnlyItsPairedYangSpace(t *testing.T) {
	previousDB := repository.Db
	db, err := gorm.Open(sqlite.Open("file:public-code-space-scope?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.AutoMigrate(&repository.Space{}); err != nil {
		t.Fatal(err)
	}
	repository.Db = db
	t.Cleanup(func() { repository.Db = previousDB })

	yin := repository.Space{Type: repository.SpaceTypePersonal, Name: "Public", OwnerUserID: 1, Side: "yin"}
	if err := db.Create(&yin).Error; err != nil {
		t.Fatal(err)
	}
	yang := repository.Space{Type: repository.SpaceTypePersonal, Name: "Public-B", OwnerUserID: 1, PairID: yin.ID, Side: "yang"}
	if err := db.Create(&yang).Error; err != nil {
		t.Fatal(err)
	}
	other := repository.Space{Type: repository.SpaceTypePersonal, Name: "Private", OwnerUserID: 1, Side: "yin"}
	if err := db.Create(&other).Error; err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct {
		name string
		id   uint
		want bool
	}{{"public space", yin.ID, true}, {"paired space", yang.ID, true}, {"other space", other.ID, false}} {
		t.Run(tc.name, func(t *testing.T) {
			if got := publicSpaceIDAllowsTarget(yin.ID, tc.id); got != tc.want {
				t.Fatalf("public scope = %v, want %v", got, tc.want)
			}
		})
	}
}
