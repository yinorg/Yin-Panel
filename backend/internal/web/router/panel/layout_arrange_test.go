package panel

import (
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

func setupArrangeLayoutDB(t *testing.T) (spaceID, groupA, groupB, itemA, itemB uint) {
	t.Helper()
	previousDB := repository.Db
	db, err := gorm.Open(sqlite.Open("file:layout-arrange?mode=memory&cache=shared"), &gorm.Config{
		NamingStrategy:                    schema.NamingStrategy{SingularTable: true},
		DisableForeignKeyConstraintWhenMigrating: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.AutoMigrate(&repository.Space{}, &repository.SpaceMember{}, &repository.ItemIconGroup{}, &repository.ItemIcon{}); err != nil {
		t.Fatal(err)
	}
	repository.Db = db
	t.Cleanup(func() { repository.Db = previousDB })

	space := repository.Space{Type: repository.SpaceTypePersonal, Name: "P", OwnerUserID: 1, Side: "yin"}
	if err := db.Create(&space).Error; err != nil {
		t.Fatal(err)
	}
	a := repository.ItemIconGroup{Title: "A", SpaceID: space.ID, UserId: 1, Sort: 1}
	b := repository.ItemIconGroup{Title: "B", SpaceID: space.ID, UserId: 1, Sort: 2}
	for _, group := range []*repository.ItemIconGroup{&a, &b} {
		if err := db.Create(group).Error; err != nil {
			t.Fatal(err)
		}
	}
	item1 := repository.ItemIcon{Title: "1", ItemIconGroupId: a.ID, SpaceID: space.ID, UserId: 1, Sort: 1}
	item2 := repository.ItemIcon{Title: "2", ItemIconGroupId: b.ID, SpaceID: space.ID, UserId: 1, Sort: 1}
	for _, item := range []*repository.ItemIcon{&item1, &item2} {
		if err := db.Create(item).Error; err != nil {
			t.Fatal(err)
		}
	}
	return space.ID, a.ID, b.ID, item1.ID, item2.ID
}

func postArrangeLayout(t *testing.T, userID, spaceID uint, body string) *httptest.ResponseRecorder {
	t.Helper()
	ensureI18n(t)
	gin.SetMode(gin.TestMode)
	engine := gin.New()
	engine.Use(func(c *gin.Context) { c.Set("userInfo", base.UserInfo{ID: userID}) })
	engine.POST("/spaces/:spaceId/layout/arrange", NewSpaceRouter().ArrangeLayout)
	request := httptest.NewRequest(http.MethodPost, fmt.Sprintf("/spaces/%d/layout/arrange", spaceID), strings.NewReader(body))
	request.Header.Set("Content-Type", "application/json")
	recorder := httptest.NewRecorder()
	engine.ServeHTTP(recorder, request)
	return recorder
}

func TestArrangeLayoutMovesItemsAcrossGroupsAndSorts(t *testing.T) {
	spaceID, groupA, groupB, itemA, itemB := setupArrangeLayoutDB(t)
	// itemA -> groupB (sort 1), itemB -> groupB (sort 2); groupB before groupA.
	body := fmt.Sprintf(
		`{"items":[{"id":%d,"itemIconGroupId":%d,"sort":1},{"id":%d,"itemIconGroupId":%d,"sort":2}],"groups":[{"id":%d,"sort":1},{"id":%d,"sort":2}]}`,
		itemA, groupB, itemB, groupB, groupB, groupA,
	)
	recorder := postArrangeLayout(t, 1, spaceID, body)
	if recorder.Code != http.StatusOK {
		t.Fatalf("status = %d, want %d: %s", recorder.Code, http.StatusOK, recorder.Body.String())
	}
	if !strings.Contains(recorder.Body.String(), `"code":0`) {
		t.Fatalf("expected success, body = %s", recorder.Body.String())
	}
	var moved repository.ItemIcon
	if err := repository.Db.First(&moved, itemA).Error; err != nil {
		t.Fatal(err)
	}
	if moved.ItemIconGroupId != groupB || moved.Sort != 1 {
		t.Fatalf("item %d = group %d sort %d, want group %d sort 1", itemA, moved.ItemIconGroupId, moved.Sort, groupB)
	}
	var group repository.ItemIconGroup
	if err := repository.Db.First(&group, groupB).Error; err != nil {
		t.Fatal(err)
	}
	if group.Sort != 1 {
		t.Fatalf("group %d sort = %d, want 1", groupB, group.Sort)
	}
}

func TestArrangeLayoutRejectsUnknownItemAndRollsBack(t *testing.T) {
	spaceID, groupA, _, _, _ := setupArrangeLayoutDB(t)
	// The second entry references an item that is not in the space, so the whole
	// transaction must roll back: groupA's sort must stay 1.
	body := fmt.Sprintf(`{"items":[{"id":99999,"itemIconGroupId":%d,"sort":1}],"groups":[{"id":%d,"sort":5}]}`, groupA, groupA)
	recorder := postArrangeLayout(t, 1, spaceID, body)
	if strings.Contains(recorder.Body.String(), `"code":0`) {
		t.Fatalf("expected failure, body = %s", recorder.Body.String())
	}
	var group repository.ItemIconGroup
	if err := repository.Db.First(&group, groupA).Error; err != nil {
		t.Fatal(err)
	}
	if group.Sort != 1 {
		t.Fatalf("group %d sort = %d after rollback, want 1", groupA, group.Sort)
	}
}
