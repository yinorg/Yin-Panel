package theme

import (
	"strings"
	"testing"
)

// The default Yin theme and the shared theme view are two implementations of
// the same contract, so the two guarantees the Yin home established must hold
// for every built-in theme: the theme does not render its own space navigation,
// and the brand mark is the side switch rather than a second standalone button.
func TestSharedHomeViewDropsTheSpaceNavAndMovesTheSideSwitchToTheBrandMark(t *testing.T) {
	view, err := builtinThemeFiles.ReadFile("builtin_themes/shared/home.mjs")
	if err != nil {
		t.Fatalf("shared home view must be embedded: %v", err)
	}
	source := string(view)

	for _, forbidden := range []string{"home-spaces", "space.select"} {
		if strings.Contains(source, forbidden) {
			t.Errorf("shared home view still references %q; space selection belongs to the Core space selector", forbidden)
		}
	}
	if strings.Contains(source, "'side-toggle'") || strings.Contains(source, `"side-toggle"`) {
		t.Error("shared home view still renders a standalone side toggle button")
	}
	for _, required := range []string{"theme-side-toggle", "space.toggleSide", "'role', 'button'", "aria-label", "tabIndex", "keydown"} {
		if !strings.Contains(source, required) {
			t.Errorf("shared home view is missing %q; the brand mark must remain a labelled, focusable switch", required)
		}
	}
}

// Every non-default theme composes shared/home.css with its own palette sheet.
// Removing the space navigation and the toggle button must not leave either
// selector orphaned in the shared base or in a variant.
func TestBuiltinPaletteSheetsLeaveNoDeadSpaceOrToggleSelectors(t *testing.T) {
	sheets := map[string]string{
		"shared": "builtin_themes/shared/home.css",
		"glass":  "builtin_themes/glass/styles/home.css",
	}
	for name, path := range sheets {
		data, err := builtinThemeFiles.ReadFile(path)
		if err != nil {
			t.Fatalf("%s stylesheet %q must be embedded: %v", name, path, err)
		}
		source := string(data)
		for _, dead := range []string{".home-spaces", ".space-button", ".side-toggle"} {
			if strings.Contains(source, dead) {
				t.Errorf("%s stylesheet still styles the removed %q selector", name, dead)
			}
		}
	}
	shared, err := builtinThemeFiles.ReadFile(sheets["shared"])
	if err != nil {
		t.Fatalf("shared stylesheet must be embedded: %v", err)
	}
	if !strings.Contains(string(shared), `.home-title[role="button"]`) {
		t.Error("shared stylesheet must style the brand mark so the switch is discoverable and focusable")
	}
}

// A published revision is immutable, so a theme whose home resources changed
// must ship under a new version or startup aborts. Guard every built-in theme
// against silently reusing a version whose resources have moved on.
func TestBuiltinHomeVersionsAdvanceWhenSharedHomeResourcesChange(t *testing.T) {
	versions := map[string]string{}
	for _, pkg := range builtinPackagesV2() {
		versions[pkg.Manifest.ID] = pkg.Manifest.Version
	}
	for _, id := range []string{"org.yin.default", "org.yin.glass"} {
		version, ok := versions[id]
		if !ok {
			t.Fatalf("built-in package %s must expose a version", id)
		}
		if version == "2.3.1" {
			t.Errorf("built-in package %s still uses version 2.3.1 after its home resources changed; bump it or startup aborts on the immutability guard", id)
		}
	}
}

// Yin and Glass are the only built-in themes. A retired palette must not come
// back through the seed table, the built-in ID set, or the embedded filesystem,
// because a database seeded by an earlier release can still carry its rows and
// the resolution fallback then treats them as a missing package.
func TestOnlyYinAndGlassAreBuiltIn(t *testing.T) {
	seeded := map[string]bool{}
	for _, pkg := range builtinPackagesV2() {
		seeded[pkg.Manifest.ID] = true
	}
	for _, id := range []string{"org.yin.default", "org.yin.glass"} {
		if !seeded[id] {
			t.Errorf("built-in package %s must be seeded on every startup", id)
		}
		if !isBuiltinThemeID(id) {
			t.Errorf("%s must be recognised as built-in, otherwise it can be removed or overwritten like a community package", id)
		}
	}
	if len(seeded) != 2 {
		t.Errorf("expected exactly 2 built-in packages, got %d", len(seeded))
	}
	for _, retired := range []string{"org.yin.minimal", "org.yin.cyber"} {
		if seeded[retired] {
			t.Errorf("retired built-in %s is still seeded", retired)
		}
		if isBuiltinThemeID(retired) {
			t.Errorf("retired built-in %s is still treated as built-in", retired)
		}
		// Its stylesheet must no longer be embedded, or the package could be
		// rebuilt from source under a version this build no longer knows.
		if _, err := builtinThemeFiles.ReadFile("builtin_themes/" + strings.TrimPrefix(retired, "org.yin.") + "/styles/home.css"); err == nil {
			t.Errorf("retired built-in %s still has an embedded stylesheet", retired)
		}
	}
}
