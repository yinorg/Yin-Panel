package database

import (
	"path/filepath"
	"testing"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/theme"

	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/schema"
)

// openSchemaUpgradeDB mirrors the production SQLite configuration: singular
// table names and no foreign-key constraints.
func openSchemaUpgradeDB(t *testing.T, name string) *gorm.DB {
	t.Helper()
	db, err := gorm.Open(sqlite.Open(filepath.Join(t.TempDir(), name)), &gorm.Config{
		NamingStrategy:                           schema.NamingStrategy{SingularTable: true},
		DisableForeignKeyConstraintWhenMigrating: true,
	})
	if err != nil {
		t.Fatal(err)
	}
	return db
}

func migrateSchemaUpgradeModels(t *testing.T, db *gorm.DB) {
	t.Helper()
	models := []interface{}{
		&repository.User{},
		&repository.OAuthIdentity{},
		&repository.Space{},
		&repository.SpaceMember{},
		&repository.Team{},
		&repository.ItemIcon{},
		&repository.ItemIconGroup{},
		&repository.File{},
		&repository.UserConfig{},
		&repository.ModuleConfig{},
		&repository.SystemSetting{},
		&theme.LegacyPreferenceRecord{},
		&theme.AuditRecord{},
		&theme.WebWallpaperRecord{},
		&theme.GrantRecordV2{},
		&theme.ThemeSettingsRecordV2{},
		&theme.UserThemePreferenceV2{},
		&theme.UserSpaceThemePreferenceV2{},
	}
	if err := db.AutoMigrate(models...); err != nil {
		t.Fatal(err)
	}
}

func TestPrepareSchemaUpgradeDeduplicatesLegacyRows(t *testing.T) {
	db := openSchemaUpgradeDB(t, "prepare-schema-upgrade.db")
	if err := db.Exec(`CREATE TABLE user_config (user_id integer, panel_json text)`).Error; err != nil {
		t.Fatal(err)
	}
	if err := db.Exec(`CREATE TABLE module_config (id integer primary key autoincrement, created_at datetime, updated_at datetime, user_id integer, name varchar(255), value_json text)`).Error; err != nil {
		t.Fatal(err)
	}
	if err := db.Exec(`CREATE TABLE system_setting (id integer primary key autoincrement, config_name varchar(50), config_value text)`).Error; err != nil {
		t.Fatal(err)
	}
	statements := []string{
		`INSERT INTO user_config (user_id, panel_json) VALUES (1, '{"a":1}')`,
		`INSERT INTO user_config (user_id, panel_json) VALUES (1, '{"a":2}')`,
		`INSERT INTO user_config (user_id, panel_json) VALUES (2, '{"b":1}')`,
		`INSERT INTO module_config (user_id, name, value_json) VALUES (1, 'alpha', 'v1')`,
		`INSERT INTO module_config (user_id, name, value_json) VALUES (1, 'alpha', 'v2')`,
		`INSERT INTO module_config (user_id, name, value_json) VALUES (1, 'beta', 'v3')`,
		`INSERT INTO system_setting (config_name, config_value) VALUES ('foo', '1')`,
		`INSERT INTO system_setting (config_name, config_value) VALUES ('foo', '2')`,
		`INSERT INTO system_setting (config_name, config_value) VALUES ('bar', '3')`,
	}
	for _, statement := range statements {
		if err := db.Exec(statement).Error; err != nil {
			t.Fatal(err)
		}
	}

	if err := prepareSchemaUpgrade(db); err != nil {
		t.Fatal(err)
	}

	var userConfigs []repository.UserConfig
	if err := db.Order("user_id").Find(&userConfigs).Error; err != nil {
		t.Fatal(err)
	}
	if len(userConfigs) != 2 {
		t.Fatalf("user_config rows = %d, want 2", len(userConfigs))
	}
	if userConfigs[0].UserId != 1 || userConfigs[0].PanelJson != `{"a":2}` {
		t.Fatalf("user_config survivor = %#v, want the last row", userConfigs[0])
	}

	var alpha repository.ModuleConfig
	if err := db.Where("user_id = ? AND name = ?", 1, "alpha").First(&alpha).Error; err != nil {
		t.Fatal(err)
	}
	if alpha.ValueJson != "v2" {
		t.Fatalf("module_config alpha = %q, want v2", alpha.ValueJson)
	}
	var moduleCount int64
	if err := db.Model(&repository.ModuleConfig{}).Count(&moduleCount).Error; err != nil {
		t.Fatal(err)
	}
	if moduleCount != 2 {
		t.Fatalf("module_config rows = %d, want 2", moduleCount)
	}

	var foo repository.SystemSetting
	if err := db.Where("config_name = ?", "foo").First(&foo).Error; err != nil {
		t.Fatal(err)
	}
	if foo.ConfigValue != "2" {
		t.Fatalf("system_setting foo = %q, want 2", foo.ConfigValue)
	}
	var settingCount int64
	if err := db.Model(&repository.SystemSetting{}).Count(&settingCount).Error; err != nil {
		t.Fatal(err)
	}
	if settingCount != 2 {
		t.Fatalf("system_setting rows = %d, want 2", settingCount)
	}

	// The step must be safe to run again.
	if err := prepareSchemaUpgrade(db); err != nil {
		t.Fatal(err)
	}
}

func TestPrepareSchemaUpgradeWithoutLegacyTables(t *testing.T) {
	db := openSchemaUpgradeDB(t, "prepare-schema-upgrade-empty.db")
	if err := prepareSchemaUpgrade(db); err != nil {
		t.Fatal(err)
	}
}

func TestFinalizeSchemaUpgradeMergesDuplicateAccounts(t *testing.T) {
	db := openSchemaUpgradeDB(t, "finalize-schema-upgrade.db")
	migrateSchemaUpgradeModels(t, db)

	canonical := repository.User{Mail: "shared@example.com", Name: "Canonical", Status: 1, Role: 1, Publiccode: "code-canonical"}
	duplicate := repository.User{Mail: "Shared@Example.COM ", Name: "Duplicate", Status: 1, Role: 2, Publiccode: "code-duplicate", OauthProvider: "github", OauthID: "gh-123"}
	for _, user := range []*repository.User{&canonical, &duplicate} {
		if err := db.Create(user).Error; err != nil {
			t.Fatal(err)
		}
	}

	shared := repository.Space{Type: repository.SpaceTypeShared, Name: "Shared", OwnerUserID: canonical.ID, Side: "yin"}
	personal := repository.Space{Type: repository.SpaceTypePersonal, Name: "Duplicate", OwnerUserID: duplicate.ID, Side: "yin"}
	for _, space := range []*repository.Space{&shared, &personal} {
		if err := db.Create(space).Error; err != nil {
			t.Fatal(err)
		}
	}
	members := []repository.SpaceMember{
		{SpaceID: shared.ID, UserID: canonical.ID, Role: repository.SpaceRoleViewer},
		{SpaceID: shared.ID, UserID: duplicate.ID, Role: repository.SpaceRoleAdmin},
		{SpaceID: personal.ID, UserID: duplicate.ID, Role: repository.SpaceRoleAdmin},
	}
	for i := range members {
		if err := db.Create(&members[i]).Error; err != nil {
			t.Fatal(err)
		}
	}
	group := repository.ItemIconGroup{Title: "APP", UserId: duplicate.ID, SpaceID: personal.ID, Sort: 1}
	if err := db.Create(&group).Error; err != nil {
		t.Fatal(err)
	}
	item := repository.ItemIcon{Title: "Item", UserId: duplicate.ID, SpaceID: personal.ID, ItemIconGroupId: group.ID, Sort: 1}
	if err := db.Create(&item).Error; err != nil {
		t.Fatal(err)
	}
	file := repository.File{UserId: duplicate.ID, FileName: "icon.png"}
	if err := db.Create(&file).Error; err != nil {
		t.Fatal(err)
	}
	if err := db.Create(&repository.UserConfig{UserId: duplicate.ID, PanelJson: `{"dup":true}`}).Error; err != nil {
		t.Fatal(err)
	}
	moduleConfigs := []repository.ModuleConfig{
		{UserId: canonical.ID, Name: "alpha", ValueJson: `"canonical"`},
		{UserId: duplicate.ID, Name: "alpha", ValueJson: `"duplicate"`},
		{UserId: duplicate.ID, Name: "beta", ValueJson: `"duplicate"`},
	}
	for i := range moduleConfigs {
		if err := db.Create(&moduleConfigs[i]).Error; err != nil {
			t.Fatal(err)
		}
	}
	grants := []theme.GrantRecordV2{
		{UserID: canonical.ID, RevisionID: "rev-1", ExecutionMode: "trusted", PermissionsJSON: `["a"]`},
		{UserID: duplicate.ID, RevisionID: "rev-1", ExecutionMode: "trusted", PermissionsJSON: `["b"]`},
		{UserID: duplicate.ID, RevisionID: "rev-2", ExecutionMode: "trusted", PermissionsJSON: `["c"]`},
	}
	for i := range grants {
		if err := db.Create(&grants[i]).Error; err != nil {
			t.Fatal(err)
		}
	}
	settings := theme.ThemeSettingsRecordV2{UserID: duplicate.ID, PackageID: "org.demo", RevisionID: "rev-1", SchemaVersion: 1, DataJSON: `{"mode":"dark"}`}
	if err := db.Create(&settings).Error; err != nil {
		t.Fatal(err)
	}
	if err := db.Create(&theme.UserThemePreferenceV2{UserID: duplicate.ID, Mode: "dark", ThemeMode: "custom"}).Error; err != nil {
		t.Fatal(err)
	}
	if err := db.Create(&theme.UserSpaceThemePreferenceV2{UserID: duplicate.ID, SpaceID: shared.ID, PackageID: "org.demo", RevisionID: "rev-1"}).Error; err != nil {
		t.Fatal(err)
	}
	if err := db.Create(&theme.LegacyPreferenceRecord{UserID: duplicate.ID, PackageID: "org.demo", Mode: "dark"}).Error; err != nil {
		t.Fatal(err)
	}
	if err := db.Create(&theme.AuditRecord{ActorID: duplicate.ID, Action: "install-v2", PackageID: "org.demo"}).Error; err != nil {
		t.Fatal(err)
	}
	if err := db.Create(&theme.WebWallpaperRecord{ID: "wallpaper-1", OwnerID: duplicate.ID, HTML: []byte("<canvas>"), Poster: []byte("p"), PosterType: "image/png"}).Error; err != nil {
		t.Fatal(err)
	}

	if err := finalizeSchemaUpgrade(db); err != nil {
		t.Fatal(err)
	}

	var users []repository.User
	if err := db.Find(&users).Error; err != nil {
		t.Fatal(err)
	}
	if len(users) != 1 || users[0].ID != canonical.ID {
		t.Fatalf("users = %#v, want only canonical %d", users, canonical.ID)
	}
	if users[0].Mail != "shared@example.com" {
		t.Fatalf("canonical mail = %q, want normalized address", users[0].Mail)
	}

	var spaceCount int64
	if err := db.Model(&repository.Space{}).Where("owner_user_id = ?", canonical.ID).Count(&spaceCount).Error; err != nil {
		t.Fatal(err)
	}
	if spaceCount != 2 {
		t.Fatalf("spaces owned by canonical = %d, want 2", spaceCount)
	}

	var sharedMember repository.SpaceMember
	if err := db.Where("space_id = ?", shared.ID).First(&sharedMember).Error; err != nil {
		t.Fatal(err)
	}
	if sharedMember.UserID != canonical.ID || sharedMember.Role != repository.SpaceRoleAdmin {
		t.Fatalf("shared membership = %#v, want canonical admin", sharedMember)
	}
	var memberCount int64
	if err := db.Model(&repository.SpaceMember{}).Count(&memberCount).Error; err != nil {
		t.Fatal(err)
	}
	if memberCount != 2 {
		t.Fatalf("space_member rows = %d, want 2", memberCount)
	}

	var movedItem repository.ItemIcon
	if err := db.First(&movedItem, item.ID).Error; err != nil {
		t.Fatal(err)
	}
	if movedItem.UserId != canonical.ID {
		t.Fatalf("item user = %d, want %d", movedItem.UserId, canonical.ID)
	}
	var movedGroup repository.ItemIconGroup
	if err := db.First(&movedGroup, group.ID).Error; err != nil {
		t.Fatal(err)
	}
	if movedGroup.UserId != canonical.ID {
		t.Fatalf("group user = %d, want %d", movedGroup.UserId, canonical.ID)
	}
	var movedFile repository.File
	if err := db.First(&movedFile, file.ID).Error; err != nil {
		t.Fatal(err)
	}
	if movedFile.UserId != canonical.ID {
		t.Fatalf("file user = %d, want %d", movedFile.UserId, canonical.ID)
	}

	var identity repository.OAuthIdentity
	if err := db.Where("provider = ? AND subject = ?", "github", "gh-123").First(&identity).Error; err != nil {
		t.Fatal(err)
	}
	if identity.UserID != canonical.ID {
		t.Fatalf("oauth identity user = %d, want %d", identity.UserID, canonical.ID)
	}

	var config repository.UserConfig
	if err := db.Where("user_id = ?", canonical.ID).First(&config).Error; err != nil {
		t.Fatal(err)
	}
	if config.PanelJson != `{"dup":true}` {
		t.Fatalf("user config = %q, want the duplicate's row", config.PanelJson)
	}

	var alpha repository.ModuleConfig
	if err := db.Where("user_id = ? AND name = ?", canonical.ID, "alpha").First(&alpha).Error; err != nil {
		t.Fatal(err)
	}
	if alpha.ValueJson != `"canonical"` {
		t.Fatalf("module config alpha = %q, want canonical value", alpha.ValueJson)
	}
	var beta repository.ModuleConfig
	if err := db.Where("user_id = ? AND name = ?", canonical.ID, "beta").First(&beta).Error; err != nil {
		t.Fatal(err)
	}
	if beta.ValueJson != `"duplicate"` {
		t.Fatalf("module config beta = %q, want moved duplicate value", beta.ValueJson)
	}

	var rev1 theme.GrantRecordV2
	if err := db.Where("user_id = ? AND revision_id = ? AND execution_mode = ?", canonical.ID, "rev-1", "trusted").First(&rev1).Error; err != nil {
		t.Fatal(err)
	}
	if rev1.PermissionsJSON != `["a"]` {
		t.Fatalf("grant rev-1 = %q, want canonical permissions", rev1.PermissionsJSON)
	}
	var rev2Count int64
	if err := db.Model(&theme.GrantRecordV2{}).Where("user_id = ? AND revision_id = ?", canonical.ID, "rev-2").Count(&rev2Count).Error; err != nil {
		t.Fatal(err)
	}
	if rev2Count != 1 {
		t.Fatalf("grant rev-2 rows = %d, want 1", rev2Count)
	}

	var movedSettings theme.ThemeSettingsRecordV2
	if err := db.Where("user_id = ? AND package_id = ?", canonical.ID, "org.demo").First(&movedSettings).Error; err != nil {
		t.Fatal(err)
	}
	var movedPreference theme.UserThemePreferenceV2
	if err := db.First(&movedPreference, "user_id = ?", canonical.ID).Error; err != nil {
		t.Fatal(err)
	}
	var movedSpacePreference theme.UserSpaceThemePreferenceV2
	if err := db.Where("user_id = ? AND space_id = ?", canonical.ID, shared.ID).First(&movedSpacePreference).Error; err != nil {
		t.Fatal(err)
	}
	var movedLegacy theme.LegacyPreferenceRecord
	if err := db.First(&movedLegacy, "user_id = ?", canonical.ID).Error; err != nil {
		t.Fatal(err)
	}
	var movedAudit theme.AuditRecord
	if err := db.First(&movedAudit, "actor_id = ?", canonical.ID).Error; err != nil {
		t.Fatal(err)
	}
	var movedWallpaper theme.WebWallpaperRecord
	if err := db.First(&movedWallpaper, "id = ?", "wallpaper-1").Error; err != nil {
		t.Fatal(err)
	}
	if movedWallpaper.OwnerID != canonical.ID {
		t.Fatalf("wallpaper owner = %d, want %d", movedWallpaper.OwnerID, canonical.ID)
	}

	// Running the upgrade again must not change anything.
	if err := finalizeSchemaUpgrade(db); err != nil {
		t.Fatal(err)
	}
	var userCount int64
	if err := db.Model(&repository.User{}).Count(&userCount).Error; err != nil {
		t.Fatal(err)
	}
	if userCount != 1 {
		t.Fatalf("users after second run = %d, want 1", userCount)
	}
}

func TestFinalizeSchemaUpgradePlaceholdersAndUniqueMailIndex(t *testing.T) {
	db := openSchemaUpgradeDB(t, "finalize-schema-upgrade-mail.db")
	if err := db.AutoMigrate(&repository.User{}, &repository.OAuthIdentity{}); err != nil {
		t.Fatal(err)
	}
	first := repository.User{Mail: "", Name: "First", Status: 1, Publiccode: "code-1"}
	second := repository.User{Mail: "  ", Name: "Second", Status: 1, Publiccode: "code-2"}
	for _, user := range []*repository.User{&first, &second} {
		if err := db.Create(user).Error; err != nil {
			t.Fatal(err)
		}
	}

	if err := finalizeSchemaUpgrade(db); err != nil {
		t.Fatal(err)
	}

	var users []repository.User
	if err := db.Order("id").Find(&users).Error; err != nil {
		t.Fatal(err)
	}
	if len(users) != 2 {
		t.Fatalf("users = %d, want 2", len(users))
	}
	if users[0].Mail != "orphan-1@local.invalid" || users[1].Mail != "orphan-2@local.invalid" {
		t.Fatalf("placeholder mails = %q, %q", users[0].Mail, users[1].Mail)
	}
	exists, err := schemaIndexExists(db, "user", userMailIndexName)
	if err != nil {
		t.Fatal(err)
	}
	if !exists {
		t.Fatal("unique mail index was not created")
	}
	if err := db.Omit("Publiccode").Create(&repository.User{Mail: "orphan-1@local.invalid", Name: "Copy", Status: 1}).Error; err == nil {
		t.Fatal("duplicate mail insert unexpectedly succeeded")
	}

	// A second run with a clean database must succeed.
	if err := finalizeSchemaUpgrade(db); err != nil {
		t.Fatal(err)
	}
}
