package panel

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

func TestSpaceListCanEditReflectsCurrentUserSpaceRole(t *testing.T) {
	previousDB := repository.Db
	db, err := gorm.Open(sqlite.Open("file:space-list-can-edit?mode=memory&cache=shared"), &gorm.Config{
		NamingStrategy: schema.NamingStrategy{SingularTable: true},
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.AutoMigrate(&repository.Space{}, &repository.SpaceMember{}); err != nil {
		t.Fatal(err)
	}
	repository.Db = db
	t.Cleanup(func() { repository.Db = previousDB })

	shared := repository.Space{Type: repository.SpaceTypeShared, Name: "Shared", OwnerUserID: 99, Side: "yin"}
	if err := db.Create(&shared).Error; err != nil {
		t.Fatal(err)
	}
	personal := repository.Space{Type: repository.SpaceTypePersonal, Name: "Personal", OwnerUserID: 1, Side: "yin"}
	if err := db.Create(&personal).Error; err != nil {
		t.Fatal(err)
	}
	for _, member := range []repository.SpaceMember{
		{SpaceID: shared.ID, UserID: 2, Role: repository.SpaceRoleAdmin},
		{SpaceID: shared.ID, UserID: 3, Role: repository.SpaceRoleEditor},
		{SpaceID: shared.ID, UserID: 4, Role: repository.SpaceRoleViewer},
	} {
		if err := db.Create(&member).Error; err != nil {
			t.Fatal(err)
		}
	}

	gin.SetMode(gin.TestMode)
	for _, tc := range []struct {
		name    string
		userID  uint
		spaceID uint
		want    bool
	}{
		{name: "shared admin", userID: 2, spaceID: shared.ID, want: true},
		{name: "shared editor", userID: 3, spaceID: shared.ID, want: true},
		{name: "shared viewer", userID: 4, spaceID: shared.ID, want: false},
		{name: "personal owner", userID: 1, spaceID: personal.ID, want: true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			engine := gin.New()
			engine.Use(func(c *gin.Context) {
				c.Set("userInfo", base.UserInfo{ID: tc.userID})
			})
			engine.GET("/spaces", NewSpaceRouter().List)
			response := httptest.NewRecorder()
			engine.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/spaces", nil))
			if response.Code != http.StatusOK {
				t.Fatalf("GET /spaces status = %d, want %d: %s", response.Code, http.StatusOK, response.Body.String())
			}
			var payload struct {
				Code int `json:"code"`
				Data []struct {
					ID      uint `json:"id"`
					CanEdit bool `json:"canEdit"`
				} `json:"data"`
			}
			if err := json.Unmarshal(response.Body.Bytes(), &payload); err != nil {
				t.Fatal(err)
			}
			if payload.Code != 0 {
				t.Fatalf("GET /spaces API code = %d, want 0: %s", payload.Code, response.Body.String())
			}
			for _, space := range payload.Data {
				if space.ID == tc.spaceID {
					if space.CanEdit != tc.want {
						t.Fatalf("space %d canEdit = %v, want %v", space.ID, space.CanEdit, tc.want)
					}
					return
				}
			}
			t.Fatalf("space %d missing from /spaces response: %s", tc.spaceID, response.Body.String())
		})
	}
}
