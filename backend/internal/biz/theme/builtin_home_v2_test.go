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
		"shared":  "builtin_themes/shared/home.css",
		"glass":   "builtin_themes/glass/styles/home.css",
		"cyber":   "builtin_themes/cyber/styles/home.css",
		"minimal": "builtin_themes/minimal/styles/home.css",
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
// must ship under a new version or startup aborts. Guard every built-in theme,
// including the three that share the shared home view, against silently reusing
// a version whose resources have moved on.
func TestBuiltinHomeVersionsAdvanceWhenSharedHomeResourcesChange(t *testing.T) {
	versions := map[string]string{}
	for _, pkg := range builtinPackagesV2() {
		versions[pkg.Manifest.ID] = pkg.Manifest.Version
	}
	for _, id := range []string{"org.yin.default", "org.yin.glass", "org.yin.cyber", "org.yin.minimal"} {
		version, ok := versions[id]
		if !ok {
			t.Fatalf("built-in package %s must expose a version", id)
		}
		if version == "2.3.1" {
			t.Errorf("built-in package %s still uses version 2.3.1 after its home resources changed; bump it or startup aborts on the immutability guard", id)
		}
	}
	if versions["org.yin.glass"] != versions["org.yin.cyber"] || versions["org.yin.glass"] != versions["org.yin.minimal"] {
		t.Error("the three palettes that share one home view must move together, otherwise the shared change ships half-applied")
	}
}
