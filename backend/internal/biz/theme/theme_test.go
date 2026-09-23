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
	"fmt"
	"strings"
	"testing"

	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
)

func TestBuiltinPackageValidates(t *testing.T) {
	if err := Validate(Builtin()); err != nil {
		t.Fatalf("built-in package rejected: %v", err)
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
	doc["color"].(map[string]any)["$type"] = "number"
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
	doc["invalid"] = map[string]any{"$type": "string", "reference": map[string]any{"$value": "{color.primary}"}}
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
	doc["color"].(map[string]any)[name].(map[string]any)["$value"] = value
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
		{name: "api version", mutate: func(pkg *Package) { pkg.Manifest.APIVersion = "2" }},
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
