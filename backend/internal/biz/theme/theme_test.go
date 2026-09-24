package theme

import (
	"archive/zip"
	"bytes"
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"testing"

	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
)

func TestBuiltinPackageValidates(t *testing.T) {
	for _, pkg := range Builtins() {
		if err := Validate(pkg); err != nil {
			t.Errorf("built-in theme %q rejected: %v", pkg.Manifest.ID, err)
		}
	}
}

func TestOfficialThemesHaveDistinctDTCGVisualDirections(t *testing.T) {
	themes := map[string]*Package{"yin": Builtin(), "glass": BuiltinGlass(), "minimal": BuiltinMinimal(), "cyber": BuiltinCyber()}
	cardModes := map[string]any{}
	for name, pkg := range themes {
		if pkg.Manifest.DTCGVersion != "2025.10" || pkg.Manifest.APIVersion != "3" || pkg.Manifest.Compatibility == nil {
			t.Fatalf("%s theme does not declare the DTCG v3 runtime contract", name)
		}
		var doc map[string]any
		if err := json.Unmarshal(pkg.Documents["light"], &doc); err != nil {
			t.Fatal(err)
		}
		card := doc["component"].(map[string]any)["card"].(map[string]any)
		cardModes[name] = card["surfaceMode"].(map[string]any)["$value"]
		for _, group := range []string{"primitive", "semantic", "component", "shape", "spacing", "density", "elevation", "motion", "background", "effect"} {
			if _, exists := doc[group]; !exists {
				t.Errorf("%s theme is missing DTCG token group %q", name, group)
			}
		}
	}
	if cardModes["yin"] == cardModes["glass"] || cardModes["yin"] == cardModes["cyber"] || cardModes["minimal"] == cardModes["glass"] {
		t.Fatalf("official themes do not expose distinct surface directions: %v", cardModes)
	}
	var cyber map[string]any
	if err := json.Unmarshal(themes["cyber"].Documents["light"], &cyber); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(fmt.Sprint(cyber["semantic"].(map[string]any)["typography"].(map[string]any)["display"].(map[string]any)["$value"]), "monospace") {
		t.Fatal("Cyber theme does not declare its monospace display type")
	}
}

func TestBuiltinMistRemovalPersistsUntilUpgrade(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:builtin-mist-removal?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	if defaultID, err := DefaultID(db); err != nil || defaultID != builtinDefaultID {
		t.Fatalf("initial default = %q, err=%v", defaultID, err)
	}
	if err := SetDefault(db, 1, builtinMistID); err != nil {
		t.Fatal(err)
	}
	if err := SetPreference(db, 9, builtinMistID, "dark"); err != nil {
		t.Fatal(err)
	}
	if err := Remove(db, 1, builtinMistID); err != nil {
		t.Fatalf("built-in alternate theme could not be removed: %v", err)
	}
	if defaultID, err := DefaultID(db); err != nil || defaultID != builtinDefaultID {
		t.Fatalf("removed theme default fallback = %q, err=%v", defaultID, err)
	}
	preference, err := PreferenceFor(db, 9)
	if err != nil || preference.PackageID != builtinDefaultID || preference.Mode != "dark" {
		t.Fatalf("removed theme preference fallback = %#v, err=%v", preference, err)
	}
	if _, err := Get(db, builtinMistID); !errors.Is(err, gorm.ErrRecordNotFound) {
		t.Fatalf("removed built-in theme remained readable: %v", err)
	}
	if err := SetPreference(db, 10, builtinMistID, "light"); !errors.Is(err, gorm.ErrRecordNotFound) {
		t.Fatalf("removed built-in theme remained selectable: %v", err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	if packages, err := List(db); err != nil || containsPackage(packages, builtinMistID) {
		t.Fatalf("removed theme was reseeded at the same version: packages=%v err=%v", packageIDs(packages), err)
	}

	if err := db.Model(&PackageRecord{}).Where("id = ?", builtinMistID).Update("version", "0.9.0").Error; err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	if packages, err := List(db); err != nil || !containsPackage(packages, builtinMistID) {
		t.Fatalf("new built-in version was not restored: packages=%v err=%v", packageIDs(packages), err)
	}
	if defaultID, err := DefaultID(db); err != nil || defaultID != builtinDefaultID {
		t.Fatalf("restoring theme changed default = %q, err=%v", defaultID, err)
	}
}

func TestBuiltinHorizonLayoutWallpaperAndRemoval(t *testing.T) {
	pkg := BuiltinHorizon()
	if err := Validate(pkg); err != nil {
		t.Fatalf("Horizon package rejected: %v", err)
	}
	var light map[string]any
	if err := json.Unmarshal(pkg.Documents["light"], &light); err != nil {
		t.Fatal(err)
	}
	if pkg.Manifest.APIVersion != "3" {
		t.Fatalf("Horizon package API = %q, want 3", pkg.Manifest.APIVersion)
	}
	if _, exists := light["design"]; exists {
		t.Fatal("Horizon declares visual layout configuration in its theme tokens")
	}
	if _, exists := light["component"].(map[string]any)["appIcon"]; !exists {
		t.Fatal("Horizon does not declare component tokens")
	}

	remote := Builtin()
	remote.Manifest.ID = "test.image-url"
	remote.Manifest.Name = "Image URL"
	opacity := 0.28
	remote.Manifest.Wallpapers = map[string]Wallpaper{
		"light": {Kind: "imageUrl", Source: "https://images.example.test/bridge.jpg", OverlayOpacity: &opacity},
		"dark":  {Kind: "imageUrl", Source: "https://images.example.test/bridge-dark.jpg", OverlayOpacity: &opacity},
	}
	if _, err := ParseArchive(archivePackage(t, remote, nil), true); err != nil {
		t.Fatalf("valid HTTPS image wallpaper rejected: %v", err)
	}
	opacity = 1.1
	if _, err := ParseArchive(archivePackage(t, remote, nil), true); err == nil {
		t.Fatal("out-of-range wallpaper overlay opacity accepted")
	}

	db, err := gorm.Open(sqlite.Open("file:builtin-horizon-removal?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	if err := SetDefault(db, 1, builtinHorizonID); err != nil {
		t.Fatal(err)
	}
	if err := Remove(db, 1, builtinHorizonID); err != nil {
		t.Fatal(err)
	}
	if got, err := DefaultID(db); err != nil || got != builtinDefaultID {
		t.Fatalf("Horizon removal default fallback = %q, err=%v", got, err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	if packages, err := List(db); err != nil || containsPackage(packages, builtinHorizonID) {
		t.Fatalf("removed Horizon was restored at the same version: packages=%v err=%v", packageIDs(packages), err)
	}
	if err := db.Model(&PackageRecord{}).Where("id = ?", builtinHorizonID).Update("version", "0.9.0").Error; err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	if packages, err := List(db); err != nil || !containsPackage(packages, builtinHorizonID) {
		t.Fatalf("Horizon was not restored after upgrade: packages=%v err=%v", packageIDs(packages), err)
	}
}

func TestBuiltinThemeIDsCannotBeInstalledAsArchives(t *testing.T) {
	pkg := Builtin()
	pkg.Manifest.ID = builtinMistID
	if err := validateManifest(pkg.Manifest); err == nil || !strings.Contains(err.Error(), "reserved package ID") {
		t.Fatalf("built-in theme ID was not reserved: %v", err)
	}
	db, err := gorm.Open(sqlite.Open("file:builtin-mist-reserved?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := Install(db, 1, pkg); err == nil {
		t.Fatal("built-in theme ID was accepted by direct package installation")
	}
}

func containsPackage(packages []PackageRecord, id string) bool {
	for _, pkg := range packages {
		if pkg.ID == id {
			return true
		}
	}
	return false
}

func packageIDs(packages []PackageRecord) []string {
	ids := make([]string, len(packages))
	for i, pkg := range packages {
		ids[i] = pkg.ID
	}
	return ids
}

func TestBuiltinHomeColumnsAndUpgrade(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:builtin-home-columns?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}

	old := Builtin()
	old.Manifest.PackageVersion = "2.0.1"
	manifest, err := json.Marshal(old.Manifest)
	if err != nil {
		t.Fatal(err)
	}
	documents, err := json.Marshal(old.Documents)
	if err != nil {
		t.Fatal(err)
	}
	record := PackageRecord{
		ID: old.Manifest.ID, Name: old.Manifest.Name, Version: old.Manifest.PackageVersion,
		AssetVersion: packageRevision(old), ManifestJSON: string(manifest), DocumentsJSON: string(documents), Verified: true,
	}
	if err := db.Create(&record).Error; err != nil {
		t.Fatal(err)
	}

	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	stored, err := Get(db, "org.yin.default")
	if err != nil {
		t.Fatal(err)
	}
	if stored.Manifest.PackageVersion != "2.1.0" || stored.Manifest.APIVersion != "3" {
		t.Fatalf("built-in package = %s API %s, want 2.1.0 API 3", stored.Manifest.PackageVersion, stored.Manifest.APIVersion)
	}
	for _, scheme := range stored.Manifest.Schemes {
		var document map[string]any
		if err := json.Unmarshal(stored.Documents[scheme], &document); err != nil {
			t.Fatal(err)
		}
		card := document["component"].(map[string]any)["card"].(map[string]any)
		if _, exists := card["padding"]; !exists {
			t.Errorf("%s component card padding token is missing", scheme)
		}
	}
}

func TestHomeColumnsRange(t *testing.T) {
	if err := validateDesignSlot("homeColumns", json.Number("12")); err != nil {
		t.Fatalf("12 columns rejected: %v", err)
	}
	if err := validateDesignSlot("homeColumns", json.Number("13")); err == nil {
		t.Fatal("13 columns accepted")
	}
}

func TestValidationRejectsContrastAndReferenceCycles(t *testing.T) {
	pkg := Builtin()
	setTokenValue(t, pkg, "light", "text", map[string]any{"colorSpace": "srgb", "components": []float64{.996, .996, .996}, "alpha": 1})
	if err := Validate(pkg); err == nil || !strings.Contains(err.Error(), "contrast") {
		t.Fatalf("expected contrast failure, got %v", err)
	}

	pkg = Builtin()
	light := strings.TrimSuffix(string(pkg.Documents["light"]), "}") + `,"loop":{"$type":"color","a":{"$value":"{loop.b}"},"b":{"$value":"{loop.a}"}}}`
	pkg.Documents["light"] = json.RawMessage(light)
	if err := Validate(pkg); err == nil || !strings.Contains(err.Error(), "cycle") {
		t.Fatalf("expected reference cycle failure, got %v", err)
	}
}

func TestValidationRejectsWrongBoundType(t *testing.T) {
	pkg := Builtin()
	var doc map[string]any
	if err := json.Unmarshal(pkg.Documents["light"], &doc); err != nil {
		t.Fatal(err)
	}
	doc["semantic"].(map[string]any)["color"].(map[string]any)["$type"] = "number"
	content, err := json.Marshal(doc)
	if err != nil {
		t.Fatal(err)
	}
	pkg.Documents["light"] = content
	if err := Validate(pkg); err == nil {
		t.Fatal("expected bound type failure")
	}
}

func TestValidationRejectsReferenceTypeMismatch(t *testing.T) {
	pkg := Builtin()
	var doc map[string]any
	if err := json.Unmarshal(pkg.Documents["light"], &doc); err != nil {
		t.Fatal(err)
	}
	doc["invalid"] = map[string]any{"$type": "string", "reference": map[string]any{"$value": "{primitive.color.primary}"}}
	content, err := json.Marshal(doc)
	if err != nil {
		t.Fatal(err)
	}
	pkg.Documents["light"] = content
	if err := Validate(pkg); err == nil || !strings.Contains(err.Error(), "changes token type") {
		t.Fatalf("expected reference type mismatch rejection, got %v", err)
	}
}

func TestValidationAcceptsStandardNonColorTokensAndRejectsUnknownTypes(t *testing.T) {
	pkg := Builtin()
	var doc map[string]any
	if err := json.Unmarshal(pkg.Documents["light"], &doc); err != nil {
		t.Fatal(err)
	}
	doc["space"] = map[string]any{"$type": "dimension", "small": map[string]any{"$value": map[string]any{"value": 8, "unit": "px"}}}
	doc["weight"] = map[string]any{"$type": "fontWeight", "regular": map[string]any{"$value": 400}}
	content, err := json.Marshal(doc)
	if err != nil {
		t.Fatal(err)
	}
	pkg.Documents["light"] = content
	if err := Validate(pkg); err != nil {
		t.Fatalf("valid DTCG companion tokens were rejected: %v", err)
	}

	doc["bad"] = map[string]any{"$type": "not-a-dtcg-type", "value": map[string]any{"$value": "anything"}}
	content, err = json.Marshal(doc)
	if err != nil {
		t.Fatal(err)
	}
	pkg.Documents["light"] = content
	if err := Validate(pkg); err == nil || !strings.Contains(err.Error(), "invalid not-a-dtcg-type") {
		t.Fatalf("unknown token type was accepted: %v", err)
	}
}

func setTokenValue(t *testing.T, pkg *Package, scheme, name string, value any) {
	t.Helper()
	var doc map[string]any
	if err := json.Unmarshal(pkg.Documents[scheme], &doc); err != nil {
		t.Fatal(err)
	}
	doc["semantic"].(map[string]any)["color"].(map[string]any)[name].(map[string]any)["$value"] = value
	content, err := json.Marshal(doc)
	if err != nil {
		t.Fatal(err)
	}
	pkg.Documents[scheme] = content
}

func TestArchiveConfirmationAndOfficialSignature(t *testing.T) {
	unsigned := archiveFor(t, nil)
	if _, err := ParseArchive(unsigned, false); err == nil || !strings.Contains(err.Error(), "confirmation") {
		t.Fatalf("expected confirmation error, got %v", err)
	}
	if _, err := ParseArchive(unsigned, true); err != nil {
		t.Fatalf("confirmed unsigned package rejected: %v", err)
	}

	pub, private, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	signed := archiveFor(t, private)
	got, err := parseArchive(signed, false, pub)
	if err != nil {
		t.Fatalf("signed package rejected: %v", err)
	}
	if !got.Verified {
		t.Fatal("valid signature was not marked verified")
	}
	if _, err := parseArchive(signed, false, OfficialPublicKey); err == nil || !strings.Contains(err.Error(), "invalid") {
		t.Fatalf("signature verified with the wrong key: %v", err)
	}
}

func TestArchiveRejectsTraversal(t *testing.T) {
	var buf bytes.Buffer
	w := zip.NewWriter(&buf)
	entry, err := w.Create("../manifest.json")
	if err != nil {
		t.Fatal(err)
	}
	_, _ = entry.Write([]byte(`{}`))
	_ = w.Close()
	if _, err := ParseArchive(buf.Bytes(), true); err == nil || !strings.Contains(err.Error(), "unsafe") {
		t.Fatalf("expected unsafe path rejection, got %v", err)
	}
}

func TestArchiveRejectsIncompatibleMetadata(t *testing.T) {
	pkg := Builtin()
	pkg.Manifest.DTCGVersion = "2025.09"
	if _, err := ParseArchive(archivePackage(t, pkg, nil), true); err == nil || !strings.Contains(err.Error(), "unsupported") {
		t.Fatalf("expected incompatible DTCG version rejection, got %v", err)
	}
}

func TestArchiveRejectsInvalidManifestBindingsAndSchemes(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*Package)
	}{
		{name: "api version", mutate: func(pkg *Package) { pkg.Manifest.APIVersion = "4" }},
		{name: "api v3 compatibility missing", mutate: func(pkg *Package) { pkg.Manifest.Compatibility = nil }},
		{name: "api v3 engine too new", mutate: func(pkg *Package) { pkg.Manifest.Compatibility.Minimum = "2.0.0" }},
		{name: "missing slot", mutate: func(pkg *Package) { delete(pkg.Manifest.Bindings, "focusRing") }},
		{name: "unknown pointer", mutate: func(pkg *Package) { pkg.Manifest.Bindings["focusRing"] = "/color/missing" }},
		{name: "unsupported scheme", mutate: func(pkg *Package) { pkg.Manifest.Schemes = []string{"contrast"} }},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			pkg := Builtin()
			pkg.Manifest.ID = "test.invalid"
			pkg.Manifest.Name = "Invalid Theme"
			tt.mutate(pkg)
			if _, err := ParseArchive(archivePackage(t, pkg, nil), true); err == nil {
				t.Fatal("invalid manifest was accepted")
			}
		})
	}
}

func TestArchiveRejectsFileCountAndExpandedSizeLimits(t *testing.T) {
	t.Run("file count", func(t *testing.T) {
		var buf bytes.Buffer
		w := zip.NewWriter(&buf)
		for i := 0; i < 65; i++ {
			entry, err := w.Create(fmt.Sprintf("extra/%d", i))
			if err != nil {
				t.Fatal(err)
			}
			if _, err := entry.Write([]byte("x")); err != nil {
				t.Fatal(err)
			}
		}
		if err := w.Close(); err != nil {
			t.Fatal(err)
		}
		if _, err := ParseArchive(buf.Bytes(), true); err == nil || !strings.Contains(err.Error(), "too many files") {
			t.Fatalf("expected file count rejection, got %v", err)
		}
	})

	t.Run("expanded size", func(t *testing.T) {
		var buf bytes.Buffer
		w := zip.NewWriter(&buf)
		entry, err := w.Create("large")
		if err != nil {
			t.Fatal(err)
		}
		if _, err := entry.Write(make([]byte, MaxExpanded+1)); err != nil {
			t.Fatal(err)
		}
		if err := w.Close(); err != nil {
			t.Fatal(err)
		}
		if _, err := ParseArchive(buf.Bytes(), true); err == nil || !strings.Contains(err.Error(), "expanded theme archive") {
			t.Fatalf("expected expanded size rejection, got %v", err)
		}
	})
}

func TestArchiveRejectsResourceDigestMismatch(t *testing.T) {
	pkg := Builtin()
	pkg.Manifest.ID = "test.digest"
	pkg.Manifest.Name = "Digest Test"
	pkg.Manifest.Resources = []Resource{{Path: "assets/logo.png", SHA256: strings.Repeat("0", 64), MediaType: "image/png"}}
	pkg.Resources = map[string]ResourceData{"assets/logo.png": {MediaType: "image/png", Content: []byte("not-an-image")}}
	if _, err := ParseArchive(archivePackage(t, pkg, nil), true); err == nil || !strings.Contains(err.Error(), "digest mismatch") {
		t.Fatalf("expected resource digest rejection, got %v", err)
	}
}

func TestThemeSchemaMigrationAndRemovalFallback(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:themes?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatalf("migration was not idempotent: %v", err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatalf("builtin seed was not idempotent: %v", err)
	}
	custom := Builtin()
	custom.Manifest.ID = "test.custom"
	custom.Manifest.Name = "Custom"
	if err := Install(db, 1, custom); err != nil {
		t.Fatal(err)
	}
	if err := Install(db, 1, custom); err == nil || !strings.Contains(err.Error(), "immutable") {
		t.Fatalf("same package version was accepted for replacement: %v", err)
	}
	custom.Manifest.PackageVersion = "1.0.1"
	if err := Install(db, 1, custom); err != nil {
		t.Fatalf("versioned upgrade failed: %v", err)
	}
	if err := SetDefault(db, 1, custom.Manifest.ID); err != nil {
		t.Fatal(err)
	}
	if err := SetPreference(db, 9, custom.Manifest.ID, "dark"); err != nil {
		t.Fatal(err)
	}
	if err := Remove(db, 1, custom.Manifest.ID); err != nil {
		t.Fatal(err)
	}
	defaultID, err := DefaultID(db)
	if err != nil || defaultID != "org.yin.default" {
		t.Fatalf("default fallback = %q, err=%v", defaultID, err)
	}
	preference, err := PreferenceFor(db, 9)
	if err != nil || preference.PackageID != "org.yin.default" || preference.Mode != "dark" {
		t.Fatalf("user fallback = %#v, err=%v", preference, err)
	}
}

func TestThemeMigrationPreservesExistingDatabaseRows(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:themes-existing?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := db.Exec("CREATE TABLE legacy_user_data (id INTEGER PRIMARY KEY, value TEXT NOT NULL)").Error; err != nil {
		t.Fatal(err)
	}
	if err := db.Exec("INSERT INTO legacy_user_data (id, value) VALUES (1, 'preserve-me')").Error; err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	var value string
	if err := db.Raw("SELECT value FROM legacy_user_data WHERE id = 1").Scan(&value).Error; err != nil {
		t.Fatal(err)
	}
	if value != "preserve-me" {
		t.Fatalf("existing row changed during theme migration: %q", value)
	}
}

func TestUpgradeRollsBackPackageAndAssetsWhenAuditFails(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:themes-atomic?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	oldAsset := []byte("old asset")
	newAsset := []byte("new asset")
	old := packageWithAsset("test.atomic", "1.0.0", oldAsset)
	if err := Install(db, 1, old); err != nil {
		t.Fatal(err)
	}
	var oldRecord PackageRecord
	if err := db.First(&oldRecord, "id = ?", old.Manifest.ID).Error; err != nil {
		t.Fatal(err)
	}
	if err := db.Exec("CREATE TRIGGER fail_theme_audit BEFORE INSERT ON audit_records BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END").Error; err != nil {
		t.Fatal(err)
	}
	next := packageWithAsset("test.atomic", "1.0.1", newAsset)
	setTokenValue(t, next, "light", "primary", "#006064")
	if err := Install(db, 1, next); err == nil {
		t.Fatal("upgrade succeeded despite an audit failure")
	}
	var stored PackageRecord
	if err := db.First(&stored, "id = ?", old.Manifest.ID).Error; err != nil {
		t.Fatal(err)
	}
	if stored.Version != "1.0.0" || stored.AssetVersion != oldRecord.AssetVersion {
		t.Fatalf("failed upgrade changed package record: before=%#v after=%#v", oldRecord, stored)
	}
	asset, err := Asset(db, old.Manifest.ID, oldRecord.AssetVersion, "assets/logo.png")
	if err != nil || string(asset.Content) != string(oldAsset) {
		t.Fatalf("failed upgrade changed package asset: content=%q err=%v", asset.Content, err)
	}
}

func packageWithAsset(id, version string, content []byte) *Package {
	pkg := Builtin()
	pkg.Manifest.ID = id
	pkg.Manifest.Name = "Atomic Theme"
	pkg.Manifest.PackageVersion = version
	sum := sha256.Sum256(content)
	pkg.Manifest.Resources = []Resource{{Path: "assets/logo.png", SHA256: hex.EncodeToString(sum[:]), MediaType: "image/png"}}
	pkg.Resources = map[string]ResourceData{"assets/logo.png": {MediaType: "image/png", Content: content}}
	return pkg
}

func archiveFor(t *testing.T, private ed25519.PrivateKey) []byte {
	t.Helper()
	pkg := Builtin()
	pkg.Manifest.ID = "test.package"
	pkg.Manifest.Name = "Test Package"
	return archivePackage(t, pkg, private)
}

func archivePackage(t *testing.T, pkg *Package, private ed25519.PrivateKey) []byte {
	t.Helper()
	manifest, err := json.Marshal(pkg.Manifest)
	if err != nil {
		t.Fatal(err)
	}
	files := map[string][]byte{"manifest.json": manifest}
	for scheme, name := range pkg.Manifest.Documents {
		files[name] = pkg.Documents[scheme]
	}
	for name, resource := range pkg.Resources {
		files[name] = resource.Content
	}
	if private != nil {
		payload, err := signaturePayload(files)
		if err != nil {
			t.Fatal(err)
		}
		files["signature.ed25519"] = []byte(base64.StdEncoding.EncodeToString(ed25519.Sign(private, payload)))
	}
	var buf bytes.Buffer
	w := zip.NewWriter(&buf)
	for name, content := range files {
		entry, err := w.Create(name)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := entry.Write(content); err != nil {
			t.Fatal(err)
		}
	}
	if err := w.Close(); err != nil {
		t.Fatal(err)
	}
	return buf.Bytes()
}

func TestV1PackageStillInstalls(t *testing.T) {
	pkg := Builtin()
	pkg.Manifest.ID = "test.legacy-theme"
	pkg.Manifest.APIVersion = "1"
	pkg.Manifest.PackageVersion = "1.0.0"
	for slot := range v2Slots {
		delete(pkg.Manifest.Bindings, slot)
	}
	installed, err := ParseArchive(archivePackage(t, pkg, nil), true)
	if err != nil || installed.Manifest.APIVersion != "1" {
		t.Fatalf("v1 package rejected: %v", err)
	}
}

func TestV2DesignBindingsRejectBadValues(t *testing.T) {
	pkg := Builtin()
	pkg.Manifest.APIVersion = "2"
	for slot := range v2Slots {
		pkg.Manifest.Bindings[slot] = "/design/" + slot
	}
	var doc map[string]any
	if err := json.Unmarshal(pkg.Documents["light"], &doc); err != nil {
		t.Fatal(err)
	}
	design := map[string]any{}
	for slot, value := range builtinDesignValues(false) {
		design[slot] = map[string]any{"$type": v2Slots[slot], "$value": value}
	}
	doc["design"] = design
	design["controlHeight"].(map[string]any)["$value"] = map[string]any{"value": 10, "unit": "px"}
	pkg.Documents["light"], _ = json.Marshal(doc)
	if err := Validate(pkg); err == nil || !strings.Contains(err.Error(), "controlHeight") {
		t.Fatalf("invalid component height accepted: %v", err)
	}
}

func TestExternalWallpaperNeedsPosterAndAdminConfirmation(t *testing.T) {
	pkg := Builtin()
	pkg.Manifest.ID = "test.remote-wallpaper"
	pkg.Manifest.Name = "Remote Wallpaper"
	poster := []byte("\x89PNG\r\n\x1a\nposter")
	sum := sha256.Sum256(poster)
	pkg.Manifest.Resources = []Resource{{Path: "wallpaper/poster.png", SHA256: hex.EncodeToString(sum[:]), MediaType: "image/png"}}
	pkg.Resources = map[string]ResourceData{"wallpaper/poster.png": {MediaType: "image/png", Content: poster}}
	pkg.Manifest.Wallpapers = map[string]Wallpaper{
		"light": {Kind: "externalUrl", Source: "https://example.test/wallpaper", Poster: "wallpaper/poster.png"},
		"dark":  {Kind: "externalUrl", Source: "https://example.test/wallpaper", Poster: "wallpaper/poster.png"},
	}
	if _, err := ParseArchive(archivePackage(t, pkg, nil), true); err != nil {
		t.Fatalf("valid external wallpaper rejected: %v", err)
	}
	db, err := gorm.Open(sqlite.Open("file:remote-wallpaper?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	if err := Install(db, 1, pkg); err != nil {
		t.Fatal(err)
	}
	if err := SetDefault(db, 1, pkg.Manifest.ID); err == nil || !strings.Contains(err.Error(), "confirmation") {
		t.Fatalf("unconfirmed remote default accepted: %v", err)
	}
	if err := SetDefaultConfirmed(db, 1, pkg.Manifest.ID, true); err != nil {
		t.Fatal(err)
	}
	id, err := DefaultID(db)
	if err != nil || id != pkg.Manifest.ID {
		t.Fatalf("remote default was not stored: %q %v", id, err)
	}
	pkg.Manifest.Wallpapers["dark"] = Wallpaper{Kind: "externalUrl", Source: "javascript:alert(1)", Poster: "wallpaper/poster.png"}
	if _, err := ParseArchive(archivePackage(t, pkg, nil), true); err == nil {
		t.Fatal("unsafe external URL accepted")
	}
}

func TestRemoteImageDefaultRequiresAdministratorConfirmation(t *testing.T) {
	pkg := Builtin()
	pkg.Manifest.ID = "test.remote-image-default"
	pkg.Manifest.Name = "Remote Image Default"
	pkg.Manifest.Wallpapers = map[string]Wallpaper{
		"light": {Kind: "imageUrl", Source: "https://images.example.test/light.jpg"},
		"dark":  {Kind: "imageUrl", Source: "https://images.example.test/dark.jpg"},
	}
	if _, err := ParseArchive(archivePackage(t, pkg, nil), true); err != nil {
		t.Fatalf("valid remote image theme rejected: %v", err)
	}
	db, err := gorm.Open(sqlite.Open("file:remote-image-default?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltin(db); err != nil {
		t.Fatal(err)
	}
	if err := Install(db, 1, pkg); err != nil {
		t.Fatal(err)
	}
	if err := SetDefault(db, 1, pkg.Manifest.ID); err == nil || !strings.Contains(err.Error(), "confirmation") {
		t.Fatalf("unconfirmed remote image default accepted: %v", err)
	}
	if err := SetDefaultConfirmed(db, 1, pkg.Manifest.ID, true); err != nil {
		t.Fatalf("confirmed remote image default rejected: %v", err)
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
	db, err := gorm.Open(sqlite.Open("file:web-wallpaper?mode=memory&cache=shared"), &gorm.Config{})
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
		t.Fatalf("stored web wallpaper cannot be served: %s %v", mediaType, err)
	}
}
