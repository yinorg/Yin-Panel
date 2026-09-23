package theme

import (
	"archive/zip"
	"bytes"
	"crypto/ed25519"
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
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
