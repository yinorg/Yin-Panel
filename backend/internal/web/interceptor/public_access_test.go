package interceptor

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strconv"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/util/i18n"
	"github.com/yinorg/Yin-Panel/backend/pkg/extension"
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
		{http.MethodGet, "/api/theme/v2/current", true},
		{http.MethodGet, "/api/theme/v2/effective", true},
		{http.MethodGet, "/api/theme/v2/packages", true},
		{http.MethodGet, "/api/theme/v2/package/:id", true},
		{http.MethodGet, "/api/theme/v2/revisions/:revision", true},
		{http.MethodGet, "/api/theme/v2/assets/:revision/*name", true},
		{http.MethodPost, "/api/theme/v2/admin/install", false},
		{http.MethodGet, "/api/theme/v2/preview/:token", true},
		{http.MethodGet, "/api/theme/v2/preview/:token/assets/*name", true},
		{http.MethodGet, "/api/theme/v2/grants/:revision", false},
		{http.MethodPost, "/api/spaces/:spaceId/items", false},
		{http.MethodPost, "/api/spaces/:spaceId/items/with-icon", false},
		{http.MethodPut, "/api/spaces/:spaceId/items/:itemId", false},
		{http.MethodPost, "/api/spaces/:spaceId/items/:itemId/update", false},
		{http.MethodPost, "/api/spaces/:spaceId/items/sort", false},
		{http.MethodDelete, "/api/spaces/:spaceId/items/:itemId", false},
		{http.MethodPost, "/api/spaces/:spaceId/items/:itemId/delete", false},
		{http.MethodPost, "/api/spaces/:spaceId/groups", false},
		{http.MethodPost, "/api/spaces/:spaceId/groups/sort", false},
		{http.MethodPost, "/api/spaces/:spaceId/layout/arrange", false},
		{http.MethodPut, "/api/spaces/:spaceId/groups/:groupId", false},
		{http.MethodPost, "/api/spaces/:spaceId/groups/:groupId/update", false},
		{http.MethodDelete, "/api/spaces/:spaceId/groups/:groupId", false},
		{http.MethodPost, "/api/spaces/:spaceId/groups/:groupId/delete", false},
		{http.MethodPost, "/api/spaces/:spaceId/bookmarks/import", false},
		{http.MethodPost, "/api/spaces/:spaceId/bookmarks/import-batch", false},
		{http.MethodGet, "/api/spaces/:spaceId/bookmarks/export", false},
		{http.MethodPost, "/api/spaces/:spaceId/clear", false},
		{http.MethodPost, "/api/spaces/:spaceId/public", false},
		{http.MethodPost, "/api/spaces/:spaceId/members", false},
		{http.MethodGet, "/api/panel/itemIconGroup/getGroups", false},
		{http.MethodPost, "/api/panel/itemIconGroup/edit", false},
		{http.MethodPost, "/api/panel/itemIconGroup/deletes", false},
		{http.MethodPost, "/api/panel/itemIconGroup/saveSort", false},
		{http.MethodGet, "/api/panel/itemIcon/getIcons", false},
		{http.MethodPost, "/api/panel/itemIcon/edit", false},
		{http.MethodPost, "/api/panel/itemIcon/delete", false},
		{http.MethodPost, "/api/panel/itemIcon/saveSort", false},
		{http.MethodPost, "/api/panel/userConfig/setConfig", false},
		{http.MethodGet, "/api/theme/v2/preference", false},
		{http.MethodGet, "/api/spaces/:spaceId/members", false},
		{http.MethodGet, "/api/system/monitor/getSnapshot", false},
		{http.MethodPost, "/api/system/monitor/getSnapshot", false},
		{http.MethodPost, "/api/system/monitor/getDiskMountpoints", false},
		{http.MethodPost, "/api/system/monitor/getDiskStateByPath", false},
		{http.MethodPost, "/api/system/monitor/getEnableStatus", false},
		{http.MethodGet, "/api/users", false},
	}
	for _, tc := range cases {
		t.Run(tc.method+" "+tc.route, func(t *testing.T) {
			got := PublicCodeRequestAllowed(tc.method, tc.route, "12", 12)
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

func TestAuthEnforcesPublicCodeReadScopeOnMatchedRoutes(t *testing.T) {
	previousDB := repository.Db
	previousUserRepo := global.UserRepo
	previousLang := i18n.Obj
	db, err := gorm.Open(sqlite.Open("file:public-code-auth-policy?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.AutoMigrate(&repository.User{}, &repository.Space{}); err != nil {
		t.Fatal(err)
	}
	repository.Db = db
	global.UserRepo = repository.NewUserRepo()
	i18n.Obj = i18n.NewLang("../../../lang/zh-cn.ini")
	t.Cleanup(func() {
		repository.Db = previousDB
		global.UserRepo = previousUserRepo
		i18n.Obj = previousLang
	})

	user := repository.User{Name: "Public owner", Mail: "public-owner@example.test", Status: 1}
	if err := db.Create(&user).Error; err != nil {
		t.Fatal(err)
	}
	publicID := "public-space-key"
	publicSpace := repository.Space{
		Type: repository.SpaceTypePersonal, Name: "Public", OwnerUserID: user.ID,
		Side: "yin", PublicEnabled: true, PublicID: &publicID, PublicMode: "direct",
	}
	if err := db.Create(&publicSpace).Error; err != nil {
		t.Fatal(err)
	}
	pairedSpace := repository.Space{
		Type: repository.SpaceTypePersonal, Name: "Paired", OwnerUserID: user.ID,
		PairID: publicSpace.ID, Side: "yang",
	}
	if err := db.Create(&pairedSpace).Error; err != nil {
		t.Fatal(err)
	}
	otherSpace := repository.Space{Type: repository.SpaceTypePersonal, Name: "Private", OwnerUserID: user.ID, Side: "yin"}
	if err := db.Create(&otherSpace).Error; err != nil {
		t.Fatal(err)
	}

	gin.SetMode(gin.TestMode)
	engine := gin.New()
	api := engine.Group("/api")
	api.Use(Auth)
	var handlerReached bool
	var actorMatches bool
	endpoint := func(c *gin.Context) {
		handlerReached = true
		value, ok := c.Get(extension.ActorContextKey)
		actor, isActor := value.(extension.Actor)
		actorMatches = ok && isActor && actor.ID == user.ID && actor.Username == user.Name
		c.Status(http.StatusNoContent)
	}
	api.GET("/spaces/:spaceId/items", endpoint)
	api.GET("/spaces/:spaceId/members", endpoint)
	api.POST("/spaces/:spaceId/items", endpoint)

	cases := []struct {
		name        string
		method      string
		path        string
		wantReached bool
	}{
		{name: "public space read", method: http.MethodGet, path: "/api/spaces/" + strconv.FormatUint(uint64(publicSpace.ID), 10) + "/items", wantReached: true},
		{name: "paired space read", method: http.MethodGet, path: "/api/spaces/" + strconv.FormatUint(uint64(pairedSpace.ID), 10) + "/items", wantReached: true},
		{name: "unrelated space read", method: http.MethodGet, path: "/api/spaces/" + strconv.FormatUint(uint64(otherSpace.ID), 10) + "/items"},
		{name: "unlisted public data read", method: http.MethodGet, path: "/api/spaces/" + strconv.FormatUint(uint64(publicSpace.ID), 10) + "/members"},
		{name: "write request", method: http.MethodPost, path: "/api/spaces/" + strconv.FormatUint(uint64(publicSpace.ID), 10) + "/items"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			handlerReached = false
			actorMatches = false
			request := httptest.NewRequest(tc.method, tc.path, nil)
			request.Header.Set("publiccode", publicID)
			response := httptest.NewRecorder()
			engine.ServeHTTP(response, request)
			if handlerReached != tc.wantReached {
				t.Fatalf("handler reached = %v, want %v (status %d, body %s)", handlerReached, tc.wantReached, response.Code, response.Body.String())
			}
			if tc.wantReached && response.Code != http.StatusNoContent {
				t.Fatalf("allowed route status = %d, want %d", response.Code, http.StatusNoContent)
			}
			if tc.wantReached && !actorMatches {
				t.Fatal("Core authentication did not provide the expected Actor context")
			}
			if !tc.wantReached {
				var body struct {
					Code int `json:"code"`
				}
				if err := json.Unmarshal(response.Body.Bytes(), &body); err != nil {
					t.Fatalf("decode denial response: %v", err)
				}
				if body.Code != 1005 {
					t.Fatalf("denial code = %d, want 1005", body.Code)
				}
			}
		})
	}
}
