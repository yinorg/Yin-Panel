package theme

import (
	"archive/zip"
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestDTCG202510ResolvesStandardTypesAliasesRootAndExtends(t *testing.T) {
	document := map[string]any{
		"$schema": DTCGSchema202510,
		"primitive": map[string]any{
			"color": map[string]any{
				"$type": "color",
				"ink":   map[string]any{"$value": map[string]any{"colorSpace": "srgb", "components": []any{json.Number("0.1"), json.Number("0.2"), json.Number("0.3")}, "alpha": json.Number("1")}},
			},
			"space": map[string]any{
				"$type": "dimension",
				"$root": map[string]any{"$value": map[string]any{"value": json.Number("4"), "unit": "px"}},
				"small": map[string]any{"$value": map[string]any{"value": json.Number("8"), "unit": "px"}},
			},
		},
		"semantic": map[string]any{
			"ink": map[string]any{"$type": "color", "$value": "{primitive.color.ink}"},
			"spacing": map[string]any{
				"$extends": "{primitive.space}",
				"medium":   map[string]any{"$value": map[string]any{"value": json.Number("16"), "unit": "px"}},
			},
		},
	}
	encoded, err := json.Marshal(document)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateDTCGDocument202510(encoded); err != nil {
		t.Fatalf("valid DTCG document was rejected: %v", err)
	}
}

func TestDTCG202510AcceptsSharedCSSColor4ConformanceFixture(t *testing.T) {
	_, sourceFile, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("could not locate theme package source")
	}
	path := filepath.Join(filepath.Dir(sourceFile), "../../../../shared/theme/dtcg-conformance/color-spaces.json")
	document, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateDTCGDocument202510(document); err != nil {
		t.Fatalf("shared DTCG 2025.10 color fixture was rejected: %v", err)
	}
}

func TestDTCG202510RejectsUnknownTypesReferencesAndCycles(t *testing.T) {
	tests := []struct {
		name string
		doc  string
	}{
		{"missing schema", `{"primitive":{"$type":"number","one":{"$value":1}}}`},
		{"nonstandard type", `{"$schema":"` + DTCGSchema202510 + `","custom":{"$type":"asset","image":{"$value":"image.png"}}}`},
		{"color alpha out of range", `{"$schema":"` + DTCGSchema202510 + `","color":{"$type":"color","ink":{"$value":{"colorSpace":"srgb","components":[0.1,0.2,0.3],"alpha":1.1}}}}`},
		{"unknown reference", `{"$schema":"` + DTCGSchema202510 + `","color":{"$type":"color","ink":{"$value":"{missing.ink}"}}}`},
		{"reference cycle", `{"$schema":"` + DTCGSchema202510 + `","number":{"$type":"number","a":{"$value":"{number.b}"},"b":{"$value":"{number.a}"}}}`},
		{"extension cycle", `{"$schema":"` + DTCGSchema202510 + `","a":{"$extends":"{b}"},"b":{"$extends":"{a}"}}`},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if err := validateDTCGDocument202510([]byte(tc.doc)); err == nil {
				t.Fatal("invalid DTCG document was accepted")
			}
		})
	}
}

func TestPackageManifestV2RejectsOldAPIAndChecksCoreRange(t *testing.T) {
	manifest := PackageManifestV2{
		Format: "yin-theme", FormatVersion: PackageFormatVersionV2,
		ID: "community.example.editorial", Name: "Editorial", Version: "1.0.0",
		ThemeAPI: "^1.0.0", Core: ">=0.4.0 <1.0.0", Author: "Community", License: "MIT",
		Tokens:        TokenSetV2{Format: "DTCG", Version: DTCGVersion, Docs: map[string]string{"light": "tokens/light.json", "dark": "tokens/dark.json"}},
		DefaultScheme: "light",
	}
	if err := ValidatePackageManifestV2(manifest, "0.4.0"); err != nil {
		t.Fatalf("valid v2 manifest was rejected: %v", err)
	}
	if err := ValidatePackageManifestV2(manifest, "0.3.16"); err == nil {
		t.Fatal("incompatible Core version was accepted")
	}
	manifest.FormatVersion = 1
	if err := ValidatePackageManifestV2(manifest, "0.4.0"); err == nil || !strings.Contains(err.Error(), "unsupported theme package format") {
		t.Fatalf("legacy package format error = %v", err)
	}
}

func TestPackageManifestV2ValidatesThemeContributions(t *testing.T) {
	manifest := PackageManifestV2{
		Format: "yin-theme", FormatVersion: PackageFormatVersionV2,
		ID: "community.example.view", Name: "View", Version: "1.0.0",
		ThemeAPI: "^1.0.0", Core: ">=0.4.0", Author: "Community", License: "MIT",
		Tokens:        TokenSetV2{Format: "DTCG", Version: DTCGVersion, Docs: map[string]string{"light": "tokens/light.json", "dark": "tokens/dark.json"}},
		DefaultScheme: "light", Contributes: ContributionsV2{Views: []string{"home"}},
	}
	if err := ValidatePackageManifestV2(manifest, "0.4.0"); err == nil || !strings.Contains(err.Error(), "require a script entrypoint") {
		t.Fatalf("view without script error = %v", err)
	}
	manifest.Entrypoints.Script = "scripts/theme.mjs"
	manifest.Resources = []ResourceV2{{Path: "scripts/theme.mjs", SHA256: strings.Repeat("a", 64), MediaType: "text/javascript"}}
	manifest.Runtime.SupportedModes = []string{"sandbox"}
	if err := ValidatePackageManifestV2(manifest, "0.4.0"); err != nil {
		t.Fatalf("valid home view contribution was rejected: %v", err)
	}
	manifest.Contributes.Views = []string{"home", "private-core-dom"}
	if err := ValidatePackageManifestV2(manifest, "0.4.0"); err == nil || !strings.Contains(err.Error(), "unknown or duplicate theme view") {
		t.Fatalf("unknown view contribution error = %v", err)
	}
}

func TestPackagePreviewPublicV2ResolvesAssetsThroughOpaqueToken(t *testing.T) {
	pkg := &PackageV2{Manifest: PackageManifestV2{Resources: []ResourceV2{{Path: "styles/theme.css"}, {Path: "assets/ui font.woff2"}}}}
	public := PackagePreviewPublicV2(pkg, "preview-token")
	if got := public.Manifest.Resources[0].URL; got != "/api/theme/preview/preview-token/assets/styles/theme.css" {
		t.Fatalf("stylesheet preview URL = %q", got)
	}
	if got := public.Manifest.Resources[1].URL; got != "/api/theme/preview/preview-token/assets/assets/ui%20font.woff2" {
		t.Fatalf("font preview URL = %q", got)
	}
	if pkg.Manifest.Resources[0].URL != "" {
		t.Fatal("public preview mapping mutated the stored package manifest")
	}
}

func TestThemeRuntimeGrantMustMatchManifestAndIncludeRequiredPermissions(t *testing.T) {
	manifest := PackageManifestV2{
		Runtime:     RuntimeV2{SupportedModes: []string{"sandbox"}},
		Permissions: PermissionsV2{Required: []PermissionV2{{Name: "items.read"}}, Optional: []PermissionV2{{Name: "theme.storage"}, {Name: "preferences.write"}}},
	}
	if err := ValidateThemeGrantV2(manifest, "sandbox", []string{"items.read", "theme.storage", "preferences.write"}); err != nil {
		t.Fatalf("valid sandbox grant rejected: %v", err)
	}
	for _, permissions := range [][]string{{}, {"items.read", "network.fetch"}, {"items.read", "items.read"}} {
		if err := ValidateThemeGrantV2(manifest, "sandbox", permissions); err == nil {
			t.Errorf("invalid permission grant accepted: %#v", permissions)
		}
	}
	if err := ValidateThemeGrantV2(manifest, "trusted", []string{"items.read"}); err == nil {
		t.Fatal("trusted execution grant was accepted by the sandbox grant API")
	}
}

func TestThemeCoreRangeRejectsMalformedComparators(t *testing.T) {
	for _, expression := range []string{"", "latest", "^0.4.0", ">=", ">=0.4.0 || <1.0.0"} {
		if themeCoreRangeSupports(expression, "0.4.0") {
			t.Errorf("accepted unsupported Core range %q", expression)
		}
	}
}

func TestPackageArchiveV2ValidatesResourcesAndRevision(t *testing.T) {
	css := []byte(":host { color: var(--yin-text); }")
	script := []byte("export function setup() { return {}; }")
	manifest := PackageManifestV2{
		Format: "yin-theme", FormatVersion: PackageFormatVersionV2,
		ID: "community.example.package", Name: "Example", Version: "1.0.0",
		ThemeAPI: "^1.0.0", Core: ">=0.0.0 <1.0.0", Author: "Community", License: "MIT",
		Tokens:        TokenSetV2{Format: "DTCG", Version: DTCGVersion, Docs: map[string]string{"light": "tokens/light.json", "dark": "tokens/dark.json"}},
		DefaultScheme: "light",
		Entrypoints:   EntrypointsV2{Script: "scripts/theme.mjs", Styles: []string{"styles/theme.css"}},
		Runtime:       RuntimeV2{SupportedModes: []string{"sandbox"}},
		Resources: []ResourceV2{
			{Path: "scripts/theme.mjs", SHA256: digestV2(script), MediaType: "text/javascript"},
			{Path: "styles/theme.css", SHA256: digestV2(css), MediaType: "text/css"},
		},
	}
	files := map[string][]byte{
		"tokens/light.json": []byte(validDTCGDocumentV2()),
		"tokens/dark.json":  []byte(validDTCGDocumentV2()),
		"scripts/theme.mjs": script,
		"styles/theme.css":  css,
	}
	archive := makePackageArchiveV2(t, manifest, files)
	pkg, err := ParsePackageArchiveV2(archive, true)
	if err != nil {
		t.Fatalf("valid v2 archive was rejected: %v", err)
	}
	if pkg.Verified || len(pkg.Files) != 2 || len(pkg.Revision) != 64 {
		t.Fatalf("unexpected parsed archive state: verified=%v files=%d revision=%q", pkg.Verified, len(pkg.Files), pkg.Revision)
	}
	if _, err := ParsePackageArchiveV2(archive, false); err == nil || !strings.Contains(err.Error(), "unsigned") {
		t.Fatalf("unsigned package error = %v", err)
	}

	files["tokens/light.json"] = []byte(strings.Replace(validDTCGDocumentV2(), "0.1", "0.4", 1))
	updatedArchive := makePackageArchiveV2(t, manifest, files)
	updated, err := ParsePackageArchiveV2(updatedArchive, true)
	if err != nil {
		t.Fatal(err)
	}
	if updated.Revision == pkg.Revision {
		t.Fatal("token document change did not change the immutable revision")
	}
}

func TestPackageArchiveV2RejectsResourceDigestMismatch(t *testing.T) {
	css := []byte(".theme { color: red; }")
	manifest := PackageManifestV2{
		Format: "yin-theme", FormatVersion: PackageFormatVersionV2,
		ID: "community.example.bad-digest", Name: "Bad digest", Version: "1.0.0",
		ThemeAPI: "^1.0.0", Core: ">=0.0.0", Author: "Community", License: "MIT",
		Tokens:        TokenSetV2{Format: "DTCG", Version: DTCGVersion, Docs: map[string]string{"light": "tokens/light.json", "dark": "tokens/dark.json"}},
		DefaultScheme: "light",
		Resources:     []ResourceV2{{Path: "styles/theme.css", SHA256: strings.Repeat("0", 64), MediaType: "text/css"}},
	}
	files := map[string][]byte{"tokens/light.json": []byte(validDTCGDocumentV2()), "tokens/dark.json": []byte(validDTCGDocumentV2()), "styles/theme.css": css}
	if _, err := ParsePackageArchiveV2(makePackageArchiveV2(t, manifest, files), true); err == nil || !strings.Contains(err.Error(), "digest mismatch") {
		t.Fatalf("resource digest error = %v", err)
	}
}

func validDTCGDocumentV2() string {
	return `{"$schema":"` + DTCGSchema202510 + `","color":{"$type":"color","ink":{"$value":{"colorSpace":"srgb","components":[0.1,0.2,0.3],"alpha":1}}}}`
}

func digestV2(content []byte) string {
	sum := sha256.Sum256(content)
	return hex.EncodeToString(sum[:])
}

func makePackageArchiveV2(t *testing.T, manifest PackageManifestV2, files map[string][]byte) []byte {
	t.Helper()
	manifestBytes, err := json.Marshal(manifest)
	if err != nil {
		t.Fatal(err)
	}
	files["manifest.json"] = manifestBytes
	var output bytes.Buffer
	writer := zip.NewWriter(&output)
	for name, content := range files {
		entry, err := writer.Create(name)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := fmt.Fprint(entry, string(content)); err != nil {
			t.Fatal(err)
		}
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}
	return output.Bytes()
}
