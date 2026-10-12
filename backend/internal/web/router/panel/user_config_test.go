package panel

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/util/i18n"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

// userConfigTestHarness gives a test its own database and points both the shared
// repository handle and the package-level repo the router calls at it.
func userConfigTestHarness(t *testing.T, name string) *gorm.DB {
	t.Helper()
	previousDB := repository.Db
	previousRepo := global.UserConfigRepo
	// The rejection paths answer through the message table, which only the
	// running server loads.
	previousLang := i18n.Obj
	i18n.Obj = i18n.NewLang("../../../../lang/en-us.ini")
	db, err := gorm.Open(sqlite.Open("file:"+name+"?mode=memory&cache=shared"), &gorm.Config{
		NamingStrategy: schema.NamingStrategy{SingularTable: true},
	})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.AutoMigrate(&repository.UserConfig{}); err != nil {
		t.Fatal(err)
	}
	repository.Db = db
	global.UserConfigRepo = repository.NewUserConfigRepo()
	t.Cleanup(func() {
		repository.Db = previousDB
		global.UserConfigRepo = previousRepo
		i18n.Obj = previousLang
	})
	return db
}

// userConfigTestEngine installs the acting user, the way the auth interceptor
// does, so the handlers can resolve it.
func userConfigTestEngine(userID uint, publicSpaceID uint) *gin.Engine {
	gin.SetMode(gin.TestMode)
	engine := gin.New()
	engine.Use(func(c *gin.Context) {
		c.Set("userInfo", base.UserInfo{ID: userID})
		if publicSpaceID != 0 {
			c.Set("publicSpaceID", publicSpaceID)
		}
	})
	router := NewUserConfigRouter()
	engine.POST("/config", router.SetConfig)
	engine.GET("/config", router.GetConfig)
	return engine
}

func postUserConfig(t *testing.T, engine *gin.Engine, body string) map[string]any {
	t.Helper()
	request := httptest.NewRequest(http.MethodPost, "/config", strings.NewReader(body))
	request.Header.Set("Content-Type", "application/json")
	response := httptest.NewRecorder()
	engine.ServeHTTP(response, request)
	payload := map[string]any{}
	if err := json.Unmarshal(response.Body.Bytes(), &payload); err != nil {
		t.Fatalf("decode POST /config response: %v (%s)", err, response.Body.String())
	}
	return payload
}

// Saving the language must not disturb the panel the same user already stored.
// A request that carries no panel serializes nothing, and the stored JSON has to
// survive it; writing the absent panel as "null" would silently wipe the
// wallpaper and every display preference the user had set.
func TestUserConfigLanguageSavePreservesPanel(t *testing.T) {
	db := userConfigTestHarness(t, "user-config-language-preserves-panel")
	engine := userConfigTestEngine(7, 0)

	const storedPanel = `{"wallpaperMode":"library","wallpaperLibraryEntry":"dynamic-001","backgroundBlur":12}`
	if err := db.Create(&repository.UserConfig{UserId: 7, PanelJson: storedPanel}).Error; err != nil {
		t.Fatal(err)
	}

	payload := postUserConfig(t, engine, `{"language":"en-US"}`)
	if code, _ := payload["code"].(float64); code != 0 {
		t.Fatalf("POST language-only config code = %v, want 0: %v", payload["code"], payload)
	}

	saved, err := global.UserConfigRepo.GetUserConfig(7)
	if err != nil {
		t.Fatal(err)
	}
	if saved.Language != "en-US" {
		t.Fatalf("stored language = %q, want en-US", saved.Language)
	}
	if saved.PanelJson != storedPanel {
		t.Fatalf("panel JSON = %q, want it untouched as %q", saved.PanelJson, storedPanel)
	}
	if saved.Panel == nil || saved.Panel.WallpaperLibraryEntry != "dynamic-001" || saved.Panel.BackgroundBlur == nil || *saved.Panel.BackgroundBlur != 12 {
		t.Fatalf("panel preferences were not preserved: %+v", saved.Panel)
	}

	// The saved language also comes back on the read path the client boots from.
	response := httptest.NewRecorder()
	engine.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/config", nil))
	var read struct {
		Code int `json:"code"`
		Data struct {
			Language string `json:"language"`
			Panel    struct {
				WallpaperLibraryEntry string `json:"wallpaperLibraryEntry"`
			} `json:"panel"`
		} `json:"data"`
	}
	if err := json.Unmarshal(response.Body.Bytes(), &read); err != nil {
		t.Fatal(err)
	}
	if read.Code != 0 || read.Data.Language != "en-US" {
		t.Fatalf("GET /config = %+v, want code 0 and language en-US", read)
	}
	if read.Data.Panel.WallpaperLibraryEntry != "dynamic-001" {
		t.Fatalf("GET /config panel entry = %q, want dynamic-001", read.Data.Panel.WallpaperLibraryEntry)
	}
}

// A later panel save still writes the panel, so the preservation rule does not
// turn the endpoint into an append-only one.
func TestUserConfigPanelSaveStillWritesPanel(t *testing.T) {
	db := userConfigTestHarness(t, "user-config-panel-save-writes")
	engine := userConfigTestEngine(7, 0)

	if err := db.Create(&repository.UserConfig{UserId: 7, Language: "ja-JP", PanelJson: `{"logoText":"before"}`}).Error; err != nil {
		t.Fatal(err)
	}

	payload := postUserConfig(t, engine, `{"panel":{"logoText":"after","wallpaperMode":"none"}}`)
	if code, _ := payload["code"].(float64); code != 0 {
		t.Fatalf("POST panel config code = %v, want 0: %v", payload["code"], payload)
	}

	saved, err := global.UserConfigRepo.GetUserConfig(7)
	if err != nil {
		t.Fatal(err)
	}
	if saved.Panel == nil || saved.Panel.LogoText != "after" {
		t.Fatalf("panel was not updated: %+v", saved.Panel)
	}
	if saved.Language != "ja-JP" {
		t.Fatalf("language = %q, want the stored ja-JP to survive a panel save", saved.Language)
	}
}

// A tag the client cannot resolve would drop the user onto the fallback locale,
// so it is refused instead of stored.
func TestUserConfigRejectsUnknownLanguage(t *testing.T) {
	db := userConfigTestHarness(t, "user-config-rejects-unknown-language")
	engine := userConfigTestEngine(7, 0)

	payload := postUserConfig(t, engine, `{"language":"xx-XX"}`)
	if code, _ := payload["code"].(float64); code == 0 {
		t.Fatalf("POST unknown language code = 0, want a rejection: %v", payload)
	}

	if _, err := global.UserConfigRepo.GetUserConfig(7); err == nil {
		t.Fatal("a rejected language still created a config record")
	}
	if err := db.First(&repository.UserConfig{}, "user_id = ?", 7).Error; err == nil {
		t.Fatal("a rejected language still reached the database")
	}
}

// Following the browser is a real choice a user can make and keep.
func TestUserConfigAcceptsAutoLanguage(t *testing.T) {
	userConfigTestHarness(t, "user-config-accepts-auto")
	engine := userConfigTestEngine(7, 0)

	payload := postUserConfig(t, engine, `{"language":"auto"}`)
	if code, _ := payload["code"].(float64); code != 0 {
		t.Fatalf("POST auto language code = %v, want 0: %v", payload["code"], payload)
	}
	saved, err := global.UserConfigRepo.GetUserConfig(7)
	if err != nil {
		t.Fatal(err)
	}
	if saved.Language != "auto" {
		t.Fatalf("stored language = %q, want auto", saved.Language)
	}
}

// A public link is served to visitors who are not the account owner, so the
// projection they receive carries display preferences only.
func TestUserConfigPublicProjectionHidesLanguage(t *testing.T) {
	db := userConfigTestHarness(t, "user-config-public-hides-language")
	if err := db.Create(&repository.UserConfig{UserId: 7, Language: "ru-RU", PanelJson: `{"logoText":"Shared"}`}).Error; err != nil {
		t.Fatal(err)
	}

	engine := userConfigTestEngine(7, 42)
	response := httptest.NewRecorder()
	engine.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/config", nil))

	var read struct {
		Code int            `json:"code"`
		Data map[string]any `json:"data"`
	}
	if err := json.Unmarshal(response.Body.Bytes(), &read); err != nil {
		t.Fatal(err)
	}
	if read.Code != 0 {
		t.Fatalf("public GET /config code = %d, want 0: %s", read.Code, response.Body.String())
	}
	if _, exposed := read.Data["language"]; exposed {
		t.Fatalf("public projection exposed the owner's language: %v", read.Data)
	}
	if _, exposed := read.Data["userId"]; exposed {
		t.Fatalf("public projection exposed the owner id: %v", read.Data)
	}
	if read.Data["panel"] == nil {
		t.Fatalf("public projection dropped the display preferences: %v", read.Data)
	}
}
