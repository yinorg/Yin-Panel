package panel

import (
	"bytes"
	"encoding/json"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"os"
	"strconv"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/util/i18n"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

// A bookmark file is a tree, and the import has to keep it one. These tests pin
// the two things the previous flat shape could not express: a folder's parent,
// and two folders that share a name in different places. They also pin the pair
// with the read side, so an export of what was imported comes back the same.

const (
	bookmarkTestUserID  = uint(11)
	bookmarkTestSpaceID = uint(7)
)

func bookmarkTestHarness(t *testing.T, name string) *gorm.DB {
	t.Helper()
	previousDB := repository.Db
	previousLang := i18n.Obj
	i18n.Obj = i18n.NewLang("../../../../lang/en-us.ini")
	db, err := gorm.Open(sqlite.Open("file:"+name+"?mode=memory&cache=shared"), &gorm.Config{
		NamingStrategy: schema.NamingStrategy{SingularTable: true},
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.AutoMigrate(&repository.Space{}, &repository.SpaceMember{}, &repository.ItemIconGroup{}, &repository.ItemIcon{}, &repository.File{}); err != nil {
		t.Fatal(err)
	}
	repository.Db = db
	t.Cleanup(func() {
		repository.Db = previousDB
		i18n.Obj = previousLang
	})
	if err := db.Create(&repository.Space{BaseModel: repository.BaseModel{ID: bookmarkTestSpaceID}, Name: "Bookmarks", OwnerUserID: bookmarkTestUserID, Side: "yin"}).Error; err != nil {
		t.Fatal(err)
	}
	return db
}

func bookmarkTestEngine() *gin.Engine {
	gin.SetMode(gin.TestMode)
	engine := gin.New()
	engine.Use(func(c *gin.Context) { c.Set("userInfo", base.UserInfo{ID: bookmarkTestUserID}) })
	router := NewSpaceRouter()
	engine.POST("/spaces/:spaceId/bookmarks/import-batch", router.ImportBookmarksBatch)
	engine.GET("/spaces/:spaceId/bookmarks/tree", router.BookmarkTree)
	return engine
}

// importTree posts a tree the way the picker does: one multipart field holding
// the JSON, with no icon files attached.
func importTree(t *testing.T, engine *gin.Engine, tree string) (int, string) {
	t.Helper()
	var body bytes.Buffer
	writer := multipart.NewWriter(&body)
	if err := writer.WriteField("bookmarks", tree); err != nil {
		t.Fatal(err)
	}
	writer.Close()

	request := httptest.NewRequest(http.MethodPost, "/spaces/"+strconv.FormatUint(uint64(bookmarkTestSpaceID), 10)+"/bookmarks/import-batch", &body)
	request.Header.Set("Content-Type", writer.FormDataContentType())
	response := httptest.NewRecorder()
	engine.ServeHTTP(response, request)

	var payload struct {
		Code int `json:"code"`
	}
	if err := json.Unmarshal(response.Body.Bytes(), &payload); err != nil {
		t.Fatalf("decode import response: %v (%s)", err, response.Body.String())
	}
	return payload.Code, response.Body.String()
}

type bookmarkTreeEntry struct {
	Title string `json:"title"`
	Items []struct {
		Title string `json:"title"`
		URL   string `json:"url"`
	} `json:"items"`
	Children []bookmarkTreeEntry `json:"children"`
}

func readTree(t *testing.T, engine *gin.Engine) []bookmarkTreeEntry {
	t.Helper()
	request := httptest.NewRequest(http.MethodGet, "/spaces/"+strconv.FormatUint(uint64(bookmarkTestSpaceID), 10)+"/bookmarks/tree", nil)
	response := httptest.NewRecorder()
	engine.ServeHTTP(response, request)
	var payload struct {
		Code int `json:"code"`
		Data struct {
			Groups []bookmarkTreeEntry `json:"groups"`
		} `json:"data"`
	}
	if err := json.Unmarshal(response.Body.Bytes(), &payload); err != nil {
		t.Fatalf("decode tree response: %v (%s)", err, response.Body.String())
	}
	if payload.Code != 0 {
		t.Fatalf("GET /bookmarks/tree code = %d, want 0: %s", payload.Code, response.Body.String())
	}
	return payload.Data.Groups
}

// The tree the tests import: two top-level folders, one of them with children,
// and one folder whose own entries sit beside its children.
const nestedTree = `{"groups":[
  {"title":"常用","items":[],"children":[
    {"title":"热门","items":[
      {"title":"bilibili","url":"https://www.bilibili.com/"},
      {"title":"微博","url":"https://weibo.com"}
    ],"children":[]},
    {"title":"购物","items":[{"title":"淘宝网","url":"https://www.taobao.com"}],"children":[]}
  ]},
  {"title":"生活","items":[{"title":"自己的站","url":"https://self.example/"}],"children":[
    {"title":"热门","items":[{"title":"另一个热门","url":"https://other.example/"}],"children":[]}
  ]}
]}`

func TestBookmarkImportKeepsParentAndChildFolders(t *testing.T) {
	db := bookmarkTestHarness(t, "bookmark-import-tree")
	engine := bookmarkTestEngine()

	if code, body := importTree(t, engine, nestedTree); code != 0 {
		t.Fatalf("import code = %d, want 0: %s", code, body)
	}

	var roots []repository.ItemIconGroup
	if err := db.Where("space_id = ? AND parent_id IS NULL", bookmarkTestSpaceID).Order("sort").Find(&roots).Error; err != nil {
		t.Fatal(err)
	}
	if len(roots) != 2 || roots[0].Title != "常用" || roots[1].Title != "生活" {
		t.Fatalf("top-level groups = %+v, want 常用 then 生活", roots)
	}

	var children []repository.ItemIconGroup
	if err := db.Where("space_id = ? AND parent_id = ?", bookmarkTestSpaceID, roots[0].ID).Order("sort").Find(&children).Error; err != nil {
		t.Fatal(err)
	}
	if len(children) != 2 || children[0].Title != "热门" || children[1].Title != "购物" {
		t.Fatalf("常用 children = %+v, want 热门 then 购物", children)
	}
	for _, child := range children {
		if child.ParentID == nil || *child.ParentID != roots[0].ID {
			t.Fatalf("child %q parent = %v, want %d", child.Title, child.ParentID, roots[0].ID)
		}
	}

	// 生活's 热门 is a different folder from 常用's, and the links land in the
	// right one: merging by title is exactly what the old shape did.
	var lifeRoot repository.ItemIconGroup
	if err := db.Where("space_id = ? AND title = ? AND parent_id IS NULL", bookmarkTestSpaceID, "生活").First(&lifeRoot).Error; err != nil {
		t.Fatal(err)
	}
	var lifeChildren []repository.ItemIconGroup
	if err := db.Where("space_id = ? AND parent_id = ?", bookmarkTestSpaceID, lifeRoot.ID).Find(&lifeChildren).Error; err != nil {
		t.Fatal(err)
	}
	if len(lifeChildren) != 1 || lifeChildren[0].Title != "热门" {
		t.Fatalf("生活 children = %+v, want one 热门", lifeChildren)
	}
	if lifeChildren[0].ID == children[0].ID {
		t.Fatal("the two 热门 folders were merged into one group")
	}

	var mine []repository.ItemIcon
	if err := db.Where("space_id = ? AND item_icon_group_id = ?", bookmarkTestSpaceID, lifeChildren[0].ID).Find(&mine).Error; err != nil {
		t.Fatal(err)
	}
	if len(mine) != 1 || mine[0].Url != "https://other.example/" {
		t.Fatalf("生活/热门 items = %+v, want only the link written under it", mine)
	}
}

func TestBookmarkTreeMatchesWhatWasImported(t *testing.T) {
	bookmarkTestHarness(t, "bookmark-tree-round-trip")
	engine := bookmarkTestEngine()

	if code, body := importTree(t, engine, nestedTree); code != 0 {
		t.Fatalf("import code = %d, want 0: %s", code, body)
	}

	var want struct {
		Groups []bookmarkTreeEntry `json:"groups"`
	}
	if err := json.Unmarshal([]byte(nestedTree), &want); err != nil {
		t.Fatal(err)
	}
	got := readTree(t, engine)
	if len(got) != len(want.Groups) {
		t.Fatalf("tree groups = %d, want %d", len(got), len(want.Groups))
	}
	for i := range want.Groups {
		assertEntry(t, got[i], want.Groups[i])
	}
}

func assertEntry(t *testing.T, got, want bookmarkTreeEntry) {
	t.Helper()
	if got.Title != want.Title {
		t.Fatalf("group = %q, want %q", got.Title, want.Title)
	}
	if len(got.Items) != len(want.Items) {
		t.Fatalf("%s items = %d, want %d", got.Title, len(got.Items), len(want.Items))
	}
	for i := range want.Items {
		if got.Items[i].Title != want.Items[i].Title || got.Items[i].URL != want.Items[i].URL {
			t.Fatalf("%s item %d = %+v, want %+v", got.Title, i, got.Items[i], want.Items[i])
		}
	}
	if len(got.Children) != len(want.Children) {
		t.Fatalf("%s children = %d, want %d", got.Title, len(got.Children), len(want.Children))
	}
	for i := range want.Children {
		assertEntry(t, got.Children[i], want.Children[i])
	}
}

func TestBookmarkImportIsIdempotent(t *testing.T) {
	db := bookmarkTestHarness(t, "bookmark-import-idempotent")
	engine := bookmarkTestEngine()

	for round := 1; round <= 2; round++ {
		if code, body := importTree(t, engine, nestedTree); code != 0 {
			t.Fatalf("import round %d code = %d, want 0: %s", round, code, body)
		}
	}

	var groupCount, itemCount int64
	if err := db.Model(&repository.ItemIconGroup{}).Where("space_id = ?", bookmarkTestSpaceID).Count(&groupCount).Error; err != nil {
		t.Fatal(err)
	}
	if err := db.Model(&repository.ItemIcon{}).Where("space_id = ?", bookmarkTestSpaceID).Count(&itemCount).Error; err != nil {
		t.Fatal(err)
	}
	// Five folders (常用, 生活, 常用/热门, 常用/购物, 生活/热门) and five links.
	if groupCount != 5 {
		t.Fatalf("groups = %d, want 5 after importing twice", groupCount)
	}
	if itemCount != 5 {
		t.Fatalf("items = %d, want 5 after importing twice", itemCount)
	}
}

func TestBookmarkTreePromotesAnOrphanToTheRoot(t *testing.T) {
	db := bookmarkTestHarness(t, "bookmark-tree-orphan")
	engine := bookmarkTestEngine()

	if code, body := importTree(t, engine, nestedTree); code != 0 {
		t.Fatalf("import code = %d, want 0: %s", code, body)
	}
	// A parent that no longer exists must not take the branch with it: the
	// Core's own group tree promotes such a group to the root, so this does too.
	missing := uint(99999)
	if err := db.Model(&repository.ItemIconGroup{}).Where("space_id = ? AND title = ? AND parent_id IS NOT NULL", bookmarkTestSpaceID, "购物").Update("parent_id", missing).Error; err != nil {
		t.Fatal(err)
	}

	roots := readTree(t, engine)
	titles := make([]string, 0, len(roots))
	for _, entry := range roots {
		titles = append(titles, entry.Title)
	}
	found := false
	for _, title := range titles {
		if title == "购物" {
			found = true
		}
	}
	if !found {
		t.Fatalf("top-level groups = %v, want the orphaned 购物 promoted to the root", titles)
	}
}

// The shape a real bookmark export has: seven top-level folders, sixty-eight
// nested ones and 322 links, taken from a Chrome-format file the picker wrote.
// Small hand-written trees miss the cases that only show up at this size, such
// as repeated folder names under different parents and folders holding many
// entries next to their own subfolders.
func TestBookmarkImportHandlesARealExport(t *testing.T) {
	db := bookmarkTestHarness(t, "bookmark-real-export")
	engine := bookmarkTestEngine()

	raw, err := os.ReadFile("testdata/51kim-tree.json")
	if err != nil {
		t.Fatal(err)
	}

	var want struct {
		Groups []bookmarkTreeEntry `json:"groups"`
	}
	if err := json.Unmarshal(raw, &want); err != nil {
		t.Fatal(err)
	}

	if code, body := importTree(t, engine, string(raw)); code != 0 {
		t.Fatalf("import code = %d, want 0: %s", code, body)
	}

	var roots, folders, items int64
	db.Model(&repository.ItemIconGroup{}).Where("space_id = ? AND parent_id IS NULL", bookmarkTestSpaceID).Count(&roots)
	db.Model(&repository.ItemIconGroup{}).Where("space_id = ?", bookmarkTestSpaceID).Count(&folders)
	db.Model(&repository.ItemIcon{}).Where("space_id = ?", bookmarkTestSpaceID).Count(&items)

	var wantFolders, wantItems int64
	var tally func(entries []bookmarkTreeEntry)
	tally = func(entries []bookmarkTreeEntry) {
		for _, entry := range entries {
			wantFolders++
			wantItems += int64(len(entry.Items))
			tally(entry.Children)
		}
	}
	tally(want.Groups)

	if roots != 7 {
		t.Fatalf("top-level folders = %d, want 7", roots)
	}
	if folders != wantFolders {
		t.Fatalf("folders = %d, want %d", folders, wantFolders)
	}
	if items != wantItems {
		t.Fatalf("items = %d, want %d", items, wantItems)
	}

	got := readTree(t, engine)
	if len(got) != len(want.Groups) {
		t.Fatalf("tree roots = %d, want %d", len(got), len(want.Groups))
	}
	for i := range want.Groups {
		assertEntry(t, got[i], want.Groups[i])
	}
}
