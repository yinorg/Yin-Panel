package panel

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"sort"
	"sync"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/util/i18n"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

var initI18nOnce sync.Once

// ensureI18n seeds the shared i18n object that response.ErrorNoAccess and
// response.ErrorParamFomat read. main.go normally does this at startup, so a
// minimal inline language file keeps these tests self-contained.
func ensureI18n(t *testing.T) {
	t.Helper()
	initI18nOnce.Do(func() {
		path := filepath.Join(t.TempDir(), "test.ini")
		content := "[common]\nno_access = no access\napi_error_param_format = invalid parameter\n"
		if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
			t.Fatal(err)
		}
		i18n.Obj = i18n.NewLang(path)
	})
}

type spaceAdminFixture struct {
	engine  *gin.Engine
	db      *gorm.DB
	yin     repository.Space
	yang    repository.Space
	ownerID uint
	adminID uint
	viewer  uint
}

// newSpaceAdminFixture builds a paired Yin/Yang space owned by ownerID with an
// extra admin and a viewer. It registers the full router so route collisions
// surface immediately, but requests are issued through call so the real auth
// middleware does not need a signed token.
func newSpaceAdminFixture(t *testing.T, name string) *spaceAdminFixture {
	t.Helper()
	ensureI18n(t)
	previousDB := repository.Db
	db, err := gorm.Open(sqlite.Open("file:"+name+"?mode=memory&cache=shared"), &gorm.Config{
		NamingStrategy: schema.NamingStrategy{SingularTable: true},
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.AutoMigrate(&repository.Space{}, &repository.SpaceMember{}, &repository.SpaceOIDCGroup{}, &repository.ItemIconGroup{}, &repository.ItemIcon{}, &repository.File{}); err != nil {
		t.Fatal(err)
	}
	repository.Db = db
	t.Cleanup(func() { repository.Db = previousDB })

	fixture := &spaceAdminFixture{db: db, ownerID: 1, adminID: 2, viewer: 3}
	yin := repository.Space{Type: repository.SpaceTypeShared, Name: "Shared", OwnerUserID: fixture.ownerID, Side: "yin"}
	if err := db.Create(&yin).Error; err != nil {
		t.Fatal(err)
	}
	// PairID on the Yin row always points at itself so the original ID stays stable.
	if err := db.Model(&repository.Space{}).Where("id = ?", yin.ID).Update("pair_id", yin.ID).Error; err != nil {
		t.Fatal(err)
	}
	if err := db.First(&yin, yin.ID).Error; err != nil {
		t.Fatal(err)
	}
	fixture.yin = yin
	fixture.yang = repository.Space{Type: repository.SpaceTypeShared, Name: "Shared-B", OwnerUserID: fixture.ownerID, PairID: yin.ID, Side: "yang"}
	if err := db.Create(&fixture.yang).Error; err != nil {
		t.Fatal(err)
	}

	for _, spaceID := range []uint{yin.ID, fixture.yang.ID} {
		for _, member := range []repository.SpaceMember{
			{SpaceID: spaceID, UserID: fixture.ownerID, Role: repository.SpaceRoleAdmin},
			{SpaceID: spaceID, UserID: fixture.adminID, Role: repository.SpaceRoleAdmin},
			{SpaceID: spaceID, UserID: fixture.viewer, Role: repository.SpaceRoleViewer},
		} {
			if err := db.Create(&member).Error; err != nil {
				t.Fatal(err)
			}
		}
		if err := db.Create(&repository.ItemIconGroup{Title: "APP", UserId: fixture.ownerID, SpaceID: spaceID}).Error; err != nil {
			t.Fatal(err)
		}
	}

	gin.SetMode(gin.TestMode)
	fixture.engine = gin.New()
	NewSpaceRouter().InitRouter(fixture.engine.Group("/api"))
	return fixture
}

// call invokes a handler directly with the given path params, bypassing the
// router and the auth middleware.
func (f *spaceAdminFixture) call(t *testing.T, handler gin.HandlerFunc, userID uint, params gin.Params, body any) (int, string) {
	t.Helper()
	var reader *bytes.Reader
	if body != nil {
		encoded, err := json.Marshal(body)
		if err != nil {
			t.Fatal(err)
		}
		reader = bytes.NewReader(encoded)
	} else {
		reader = bytes.NewReader(nil)
	}
	recorder := httptest.NewRecorder()
	c, _ := gin.CreateTestContext(recorder)
	c.Request = httptest.NewRequest(http.MethodPost, "/api/spaces", reader)
	c.Request.Header.Set("Content-Type", "application/json")
	c.Params = params
	c.Set("userInfo", base.UserInfo{ID: userID})
	handler(c)
	var payload struct {
		Code int    `json:"code"`
		Msg  string `json:"msg"`
	}
	if err := json.Unmarshal(recorder.Body.Bytes(), &payload); err != nil {
		t.Fatalf("handler returned a non-JSON body: %s", recorder.Body.String())
	}
	return payload.Code, payload.Msg
}

func spaceParams(spaceID uint) gin.Params {
	return gin.Params{{Key: "spaceId", Value: fmt.Sprint(spaceID)}}
}

func memberParams(spaceID, userID uint) gin.Params {
	return gin.Params{{Key: "spaceId", Value: fmt.Sprint(spaceID)}, {Key: "userId", Value: fmt.Sprint(userID)}}
}

func (f *spaceAdminFixture) memberRows(t *testing.T, userID uint) int64 {
	t.Helper()
	var count int64
	if err := f.db.Model(&repository.SpaceMember{}).Where("user_id = ?", userID).Count(&count).Error; err != nil {
		t.Fatal(err)
	}
	return count
}

func (f *spaceAdminFixture) countRows(t *testing.T, model any, query string, args ...any) int64 {
	t.Helper()
	var count int64
	if err := f.db.Model(model).Where(query, args...).Count(&count).Error; err != nil {
		t.Fatalf("count %T: %v", model, err)
	}
	return count
}

// TestSpaceRoutesExposePostAliases locks in the POST aliases the frontend
// request wrapper relies on for member mutations and space deletion.
func TestSpaceRoutesExposePostAliases(t *testing.T) {
	fixture := newSpaceAdminFixture(t, "space-route-aliases")
	registered := make([]string, 0, len(fixture.engine.Routes()))
	for _, route := range fixture.engine.Routes() {
		registered = append(registered, route.Method+" "+route.Path)
	}
	sort.Strings(registered)
	for _, want := range []string{
		"POST /api/spaces/:spaceId/members/:userId",
		"PUT /api/spaces/:spaceId/members/:userId",
		"DELETE /api/spaces/:spaceId/members/:userId",
		"POST /api/spaces/:spaceId/members/:userId/delete",
		"DELETE /api/spaces/:spaceId",
		"POST /api/spaces/:spaceId/delete",
	} {
		found := false
		for _, route := range registered {
			if route == want {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("route %q is not registered; got %v", want, registered)
		}
	}
}

func TestRemoveMemberRejectsTheSpaceOwner(t *testing.T) {
	fixture := newSpaceAdminFixture(t, "space-remove-owner")
	router := NewSpaceRouter()
	code, msg := fixture.call(t, router.RemoveMember, fixture.adminID, memberParams(fixture.yin.ID, fixture.ownerID), nil)
	if code == 0 {
		t.Fatalf("removing the owner must be rejected, got API code 0")
	}
	if msg == "" {
		t.Error("the rejection must explain that leaving the space is the owner action")
	}
	if got := fixture.memberRows(t, fixture.ownerID); got != 2 {
		t.Fatalf("owner membership rows = %d, want 2 (both paired spaces must be untouched)", got)
	}
}

func TestRemoveMemberDropsTheMemberFromBothPairedSpaces(t *testing.T) {
	fixture := newSpaceAdminFixture(t, "space-remove-member")
	router := NewSpaceRouter()
	code, msg := fixture.call(t, router.RemoveMember, fixture.adminID, memberParams(fixture.yin.ID, fixture.viewer), nil)
	if code != 0 {
		t.Fatalf("removing a member API code = %d (%s), want 0", code, msg)
	}
	if got := fixture.memberRows(t, fixture.viewer); got != 0 {
		t.Fatalf("viewer membership rows = %d, want 0", got)
	}
	if got := fixture.memberRows(t, fixture.adminID); got != 2 {
		t.Fatalf("admin membership rows = %d, want 2", got)
	}
}

func TestUpdateMemberAppliesToBothPairedSpaces(t *testing.T) {
	fixture := newSpaceAdminFixture(t, "space-update-member")
	router := NewSpaceRouter()
	code, msg := fixture.call(t, router.UpdateMember, fixture.ownerID, memberParams(fixture.yin.ID, fixture.viewer),
		memberRoleRequest{Role: repository.SpaceRoleEditor})
	if code != 0 {
		t.Fatalf("updating a member API code = %d (%s), want 0", code, msg)
	}
	for _, spaceID := range []uint{fixture.yin.ID, fixture.yang.ID} {
		var member repository.SpaceMember
		if err := fixture.db.Where("space_id = ? AND user_id = ?", spaceID, fixture.viewer).First(&member).Error; err != nil {
			t.Fatal(err)
		}
		if member.Role != repository.SpaceRoleEditor {
			t.Fatalf("space %d member role = %q, want %q", spaceID, member.Role, repository.SpaceRoleEditor)
		}
	}
}

func TestDeleteSpaceRequiresTheOwner(t *testing.T) {
	fixture := newSpaceAdminFixture(t, "space-delete-guard")
	router := NewSpaceRouter()
	code, _ := fixture.call(t, router.DeleteSpace, fixture.adminID, spaceParams(fixture.yin.ID), nil)
	if code == 0 {
		t.Fatal("a non-owner admin must not delete the space, got API code 0")
	}
	for _, spaceID := range []uint{fixture.yin.ID, fixture.yang.ID} {
		if got := fixture.countRows(t, &repository.Space{}, "id = ?", spaceID); got != 1 {
			t.Fatalf("space %d still has %d rows, want 1", spaceID, got)
		}
	}
}

func TestDeleteSpaceRemovesTheSpaceAndItsPairedYangSpace(t *testing.T) {
	fixture := newSpaceAdminFixture(t, "space-delete-owner")
	router := NewSpaceRouter()
	code, msg := fixture.call(t, router.DeleteSpace, fixture.ownerID, spaceParams(fixture.yin.ID), nil)
	if code != 0 {
		t.Fatalf("deleting the space API code = %d (%s), want 0", code, msg)
	}
	spaceIDs := []uint{fixture.yin.ID, fixture.yang.ID}
	for _, model := range []any{
		&repository.SpaceMember{},
		&repository.SpaceOIDCGroup{},
		&repository.ItemIconGroup{},
		&repository.ItemIcon{},
	} {
		if got := fixture.countRows(t, model, "space_id IN ?", spaceIDs); got != 0 {
			t.Fatalf("%T still has %d rows for the deleted spaces, want 0", model, got)
		}
	}
	if got := fixture.countRows(t, &repository.Space{}, "id IN ?", spaceIDs); got != 0 {
		t.Fatalf("deleted spaces still have %d rows, want 0", got)
	}
}
