package theme

import (
	"archive/zip"
	"bytes"
	"testing"

	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
)

func TestMigrationPreservesLegacyRowsAndOnlyMigratesColorMode(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-v2-only-migration?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	for _, statement := range []string{
		`CREATE TABLE package_records (id TEXT PRIMARY KEY, version TEXT, manifest_json TEXT)`,
		`INSERT INTO package_records (id, version, manifest_json) VALUES ('community.old', '1.0.0', '{"formatVersion":1}')`,
		`CREATE TABLE asset_records (package_id TEXT, path TEXT, content BLOB)`,
		`INSERT INTO asset_records (package_id, path, content) VALUES ('community.old', 'assets/old.png', X'010203')`,
		`CREATE TABLE instance_settings (id INTEGER PRIMARY KEY, default_package TEXT)`,
		`INSERT INTO instance_settings (id, default_package) VALUES (1, 'community.old')`,
		`CREATE TABLE preferences (user_id INTEGER PRIMARY KEY, package_id TEXT, mode TEXT, updated_at DATETIME)`,
		`INSERT INTO preferences (user_id, package_id, mode) VALUES (42, 'community.old', 'dark')`,
	} {
		if err := db.Exec(statement).Error; err != nil {
			t.Fatal(err)
		}
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltinV2(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltinV2(db); err != nil {
		t.Fatalf("second migration was not idempotent: %v", err)
	}

	var legacyPackage struct {
		Version      string
		ManifestJSON string `gorm:"column:manifest_json"`
	}
	if err := db.Raw(`SELECT version, manifest_json FROM package_records WHERE id = ?`, "community.old").Scan(&legacyPackage).Error; err != nil {
		t.Fatal(err)
	}
	if legacyPackage.Version != "1.0.0" || legacyPackage.ManifestJSON != `{"formatVersion":1}` {
		t.Fatalf("legacy package row was changed: %+v", legacyPackage)
	}
	var legacyAsset struct{ Content []byte }
	if err := db.Raw(`SELECT content FROM asset_records WHERE package_id = ?`, "community.old").Scan(&legacyAsset).Error; err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(legacyAsset.Content, []byte{1, 2, 3}) {
		t.Fatalf("legacy asset was changed: %v", legacyAsset.Content)
	}
	var legacyDefault string
	if err := db.Raw(`SELECT default_package FROM instance_settings WHERE id = 1`).Scan(&legacyDefault).Error; err != nil {
		t.Fatal(err)
	}
	if legacyDefault != "community.old" {
		t.Fatalf("legacy default row was changed: %q", legacyDefault)
	}
	mode, err := UserThemeModeV2(db, 42)
	if err != nil || mode != "dark" {
		t.Fatalf("migrated mode = %q err=%v", mode, err)
	}
	if _, err := GetActivationV2(db, ActivationScopeForUserV2(42)); err == nil {
		t.Fatal("legacy package selection was activated")
	}
	packages, err := ListPackagesV2(db)
	if err != nil || len(packages) != 4 {
		t.Fatalf("v2 package list = %v err=%v", packages, err)
	}
}

func TestWebWallpaperPackage(t *testing.T) {
	var buffer bytes.Buffer
	writer := zip.NewWriter(&buffer)
	for name, content := range map[string][]byte{
		"index.html": []byte("<!doctype html><canvas></canvas><script>document.querySelector('canvas').width=2</script>"),
		"poster.png": []byte("\x89PNG\r\n\x1a\nposter"),
	} {
		entry, err := writer.Create(name)
		if err != nil {
			t.Fatal(err)
		}
		_, _ = entry.Write(content)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}
	db, err := gorm.Open(sqlite.Open("file:web-wallpaper-v2?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	record, err := SaveWebWallpaper(db, 7, buffer.Bytes())
	if err != nil {
		t.Fatal(err)
	}
	mediaType, content, err := WebWallpaperAsset(db, record.ID, "index.html")
	if err != nil || mediaType != "text/html" || !bytes.Contains(content, []byte("<canvas>")) {
		t.Fatalf("web wallpaper asset = %q err=%v", mediaType, err)
	}
}
