package theme

import (
	"crypto/sha256"
	"errors"
	"fmt"
	"strings"
	"testing"
	"time"

	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
)

func TestResolveThemeV2FollowsPrecedenceChain(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-resolve-v2?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	install := func(id string) RevisionRecordV2 {
		pkg := resolverFixtureV2(id, "1.0.0")
		if err := InstallPackageV2(db, 1, pkg); err != nil {
			t.Fatalf("install %s: %v", id, err)
		}
		return RevisionRecordV2{ID: pkg.Revision, PackageID: id}
	}
	system := install("org.yin.default")
	userTheme := install("user.example")
	spaceTheme := install("space.example")

	if err := InitializeActivationV2(db, InstanceThemeScopeV2, "org.yin.default", system.ID); err != nil {
		t.Fatal(err)
	}

	resolve := func(userID, spaceID uint) ThemeResolutionV2 {
		t.Helper()
		result, resolveErr := ResolveThemeV2(db, userID, spaceID)
		if resolveErr != nil {
			t.Fatalf("resolve(%d,%d): %v", userID, spaceID, resolveErr)
		}
		return result
	}

	if got := resolve(0, 0); got.Source != "system" || got.Revision.ID != system.ID {
		t.Fatalf("anonymous resolution = %s/%s, want system/%s", got.Source, got.Revision.ID, system.ID)
	}

	if err := db.Create(&SpaceThemePreferenceV2{SpaceID: 5, PackageID: "space.example", RevisionID: spaceTheme.ID}).Error; err != nil {
		t.Fatal(err)
	}
	if got := resolve(0, 5); got.Source != "space" || got.Revision.ID != spaceTheme.ID {
		t.Fatalf("space resolution = %s/%s, want space/%s", got.Source, got.Revision.ID, spaceTheme.ID)
	}

	if err := SetUserThemeSelectionV2(db, 42, "user.example", userTheme.ID, "auto"); err != nil {
		t.Fatal(err)
	}
	if got := resolve(42, 5); got.Source != "user" || got.Revision.ID != userTheme.ID {
		t.Fatalf("user resolution = %s/%s, want user/%s", got.Source, got.Revision.ID, userTheme.ID)
	}

	// follow-space skips the user's own theme and uses the space theme.
	if err := SetUserThemeChoiceV2(db, 42, "follow-space"); err != nil {
		t.Fatal(err)
	}
	if got := resolve(42, 5); got.Source != "space" || got.Revision.ID != spaceTheme.ID {
		t.Fatalf("follow-space resolution = %s/%s, want space/%s", got.Source, got.Revision.ID, spaceTheme.ID)
	}
	if err := SetUserThemeChoiceV2(db, 42, "custom"); err != nil {
		t.Fatal(err)
	}
	if got := resolve(42, 5); got.Source != "user" {
		t.Fatalf("custom mode did not restore the user theme: got %s", got.Source)
	}

	if err := db.Create(&UserSpaceThemePreferenceV2{UserID: 42, SpaceID: 5, PackageID: "org.yin.default", RevisionID: system.ID}).Error; err != nil {
		t.Fatal(err)
	}
	if got := resolve(42, 5); got.Source != "user-space" || got.Revision.ID != system.ID {
		t.Fatalf("per-space override resolution = %s/%s, want user-space/%s", got.Source, got.Revision.ID, system.ID)
	}

	// A preference pointing at a revision that no longer exists falls through.
	if err := db.Model(&UserSpaceThemePreferenceV2{}).Where("user_id = ? AND space_id = ?", 42, 5).Update("revision_id", strings.Repeat("0", 64)).Error; err != nil {
		t.Fatal(err)
	}
	if got := resolve(42, 5); got.Source != "user" {
		t.Fatalf("stale override did not fall back: got %s", got.Source)
	}

	// A guest never sees a user's choice.
	if got := resolve(0, 5); got.Source != "space" {
		t.Fatalf("guest resolution = %s, want space", got.Source)
	}

	// Removing a package makes the space preference fall back to the system
	// default instead of pinning a theme that is no longer offered.
	if err := RemovePackageV2(db, 1, "space.example"); err != nil {
		t.Fatal(err)
	}
	if got := resolve(0, 5); got.Source != "system" {
		t.Fatalf("removed space theme did not fall back: got %s", got.Source)
	}
}

func resolverFixtureV2(id, version string) *PackageV2 {
	revisionBytes := []byte("revision:" + id + ":" + version)
	return &PackageV2{
		Manifest: PackageManifestV2{ID: id, Name: id, Version: version},
		Files:    map[string]ResourceData{"views/home.mjs": {MediaType: "text/javascript", Content: []byte("export default {}")}},
		Revision: packageRevisionV2(map[string][]byte{"manifest.json": revisionBytes}),
	}
}

func TestRevisionV2InstallIsImmutableAndActivationCanRollback(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-revisions-v2?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	first := packageFixtureV2("1.0.0", "a")
	if err := InstallPackageV2(db, 7, first); err != nil {
		t.Fatal(err)
	}
	if err := InstallPackageV2(db, 7, first); err != nil {
		t.Fatalf("identical package install was not idempotent: %v", err)
	}
	asset, err := GetPackageAssetV2(db, first.Revision, "assets/mark.png")
	if err != nil || string(asset.Content) != "image-a" {
		t.Fatalf("installed revision asset = %q, error = %v", asset.Content, err)
	}

	conflicting := packageFixtureV2("1.0.0", "b")
	if err := InstallPackageV2(db, 7, conflicting); err == nil {
		t.Fatal("same version with different content was accepted")
	}
	second := packageFixtureV2("1.1.0", "b")
	if err := InstallPackageV2(db, 7, second); err != nil {
		t.Fatal(err)
	}
	oldAsset, err := GetPackageAssetV2(db, first.Revision, "assets/mark.png")
	if err != nil || string(oldAsset.Content) != "image-a" {
		t.Fatalf("updating package changed the prior immutable asset: %q, %v", oldAsset.Content, err)
	}

	scope := InstanceThemeScopeV2
	if err := InitializeActivationV2(db, scope, first.Manifest.ID, first.Revision); err != nil {
		t.Fatal(err)
	}
	if err := SetPendingActivationV2(db, scope, second.Manifest.ID, second.Revision, time.Now()); err != nil {
		t.Fatal(err)
	}
	var activation ActivationRecordV2
	if err := db.First(&activation, "scope = ?", scope).Error; err != nil {
		t.Fatal(err)
	}
	if activation.ActiveRevisionID != first.Revision || activation.PendingRevisionID != second.Revision {
		t.Fatalf("trial changed the active revision early: %+v", activation)
	}
	if err := ConfirmActivationV2(db, scope, second.Revision); err != nil {
		t.Fatal(err)
	}
	if err := RollbackActivationV2(db, scope, first.Revision); err != nil {
		t.Fatal(err)
	}
	if err := db.First(&activation, "scope = ?", scope).Error; err != nil {
		t.Fatal(err)
	}
	if activation.ActiveRevisionID != first.Revision || activation.PendingRevisionID != "" {
		t.Fatalf("rollback did not restore prior revision: %+v", activation)
	}
}

func TestTrustedRuntimePolicyRequiresManifestSupportAndRevokesUserGrants(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-trusted-policy-v2?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	unsupported := packageFixtureV2("1.0.0", "unsupported")
	if err := InstallPackageV2(db, 1, unsupported); err != nil {
		t.Fatal(err)
	}
	if err := SetTrustedRuntimePolicyV2(db, unsupported.Revision, true, 1); err == nil {
		t.Fatal("trusted mode enabled for a package that does not declare it")
	}
	if enabled, err := TrustedRuntimeEnabledV2(db, unsupported.Revision); err != nil || enabled {
		t.Fatalf("unsupported theme trusted policy = %v, err=%v", enabled, err)
	}

	trusted := packageFixtureV2("2.0.0", "trusted")
	trusted.Manifest.Runtime.SupportedModes = []string{"sandbox", "trusted"}
	trusted.Manifest.Permissions.Required = []PermissionV2{{Name: "items.read"}}
	if err := InstallPackageV2(db, 1, trusted); err != nil {
		t.Fatal(err)
	}
	userGrant := GrantRecordV2{UserID: 43, RevisionID: trusted.Revision, ExecutionMode: "trusted", PermissionsJSON: "[\"items.read\"]"}
	if err := SaveUserGrantV2(db, userGrant); err == nil {
		t.Fatal("trusted user grant accepted before administrator enablement")
	}
	if err := SaveGrantV2(db, GrantRecordV2{UserID: 42, RevisionID: trusted.Revision, ExecutionMode: "trusted", PermissionsJSON: "[\"items.read\"]"}); err != nil {
		t.Fatal(err)
	}
	if err := SetTrustedRuntimePolicyV2(db, trusted.Revision, true, 1); err != nil {
		t.Fatalf("enable trusted policy: %v", err)
	}
	if enabled, err := TrustedRuntimeEnabledV2(db, trusted.Revision); err != nil || !enabled {
		t.Fatalf("enabled trusted policy = %v, err=%v", enabled, err)
	}
	if _, err := GetGrantV2(db, 42, trusted.Revision, "trusted"); err != nil {
		t.Fatalf("enable policy removed existing user grant: %v", err)
	}
	if err := SaveUserGrantV2(db, userGrant); err != nil {
		t.Fatalf("trusted user grant rejected after administrator enablement: %v", err)
	}
	if err := SetTrustedRuntimePolicyV2(db, trusted.Revision, false, 1); err != nil {
		t.Fatalf("disable trusted policy: %v", err)
	}
	if enabled, err := TrustedRuntimeEnabledV2(db, trusted.Revision); err != nil || enabled {
		t.Fatalf("disabled trusted policy = %v, err=%v", enabled, err)
	}
	if _, err := GetGrantV2(db, 42, trusted.Revision, "trusted"); !errors.Is(err, gorm.ErrRecordNotFound) {
		t.Fatalf("trusted grant survived policy disable: %v", err)
	}
	if _, err := GetGrantV2(db, 43, trusted.Revision, "trusted"); !errors.Is(err, gorm.ErrRecordNotFound) {
		t.Fatalf("user grant survived policy disable: %v", err)
	}
}

func TestRevisionV2RollbackRequiresInstalledFallback(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-revisions-v2-fallback?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	activation := ActivationRecordV2{Scope: "user:10", PackageID: "missing", ActiveRevisionID: "missing"}
	if err := db.Create(&activation).Error; err != nil {
		t.Fatal(err)
	}
	if err := RollbackActivationV2(db, activation.Scope, "not-installed"); err == nil || errors.Is(err, gorm.ErrRecordNotFound) == false {
		t.Fatalf("missing fallback error = %v", err)
	}
}

func TestLegacyThemePreferenceMigrationPreservesModeButNotPackageSelection(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-preference-v2-migration?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	legacy := LegacyPreferenceRecord{UserID: 41, PackageID: "community.legacy-theme", Mode: "dark"}
	if err := db.Create(&legacy).Error; err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltinV2(db); err != nil {
		t.Fatal(err)
	}
	mode, err := UserThemeModeV2(db, legacy.UserID)
	if err != nil || mode != "dark" {
		t.Fatalf("migrated theme mode = %q, err=%v", mode, err)
	}
	if _, err := GetActivationV2(db, ActivationScopeForUserV2(legacy.UserID)); !errors.Is(err, gorm.ErrRecordNotFound) {
		t.Fatalf("legacy package selection was unexpectedly activated: %v", err)
	}
	if err := EnsureBuiltinV2(db); err != nil {
		t.Fatal(err)
	}
	mode, err = UserThemeModeV2(db, legacy.UserID)
	if err != nil || mode != "dark" {
		t.Fatalf("repeated migration changed theme mode = %q, err=%v", mode, err)
	}
}

func TestSetUserThemeSelectionV2IsAtomicWithAudit(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-preference-v2-atomic?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltinV2(db); err != nil {
		t.Fatal(err)
	}
	pkg := packageFixtureV2("1.0.0", "selection")
	if err := InstallPackageV2(db, 41, pkg); err != nil {
		t.Fatal(err)
	}
	if err := db.Exec("CREATE TRIGGER fail_theme_selection_audit BEFORE INSERT ON audit_records BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END").Error; err != nil {
		t.Fatal(err)
	}
	if err := SetUserThemeSelectionV2(db, 41, pkg.Manifest.ID, pkg.Revision, "dark"); err == nil {
		t.Fatal("theme selection succeeded despite an audit failure")
	}
	if _, err := GetActivationV2(db, ActivationScopeForUserV2(41)); !errors.Is(err, gorm.ErrRecordNotFound) {
		t.Fatalf("failed selection left an activation behind: %v", err)
	}
	mode, err := UserThemeModeV2(db, 41)
	if err != nil || mode != "auto" {
		t.Fatalf("failed selection changed the theme mode to %q, err=%v", mode, err)
	}
}

func TestActivePackageRevisionV2ExpiresThemeTrialAtConfiguredDuration(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-revisions-v2-trial-expiry?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	first := packageFixtureV2("1.0.0", "trial-stable")
	second := packageFixtureV2("1.1.0", "trial-candidate")
	if err := InstallPackageV2(db, 7, first); err != nil {
		t.Fatal(err)
	}
	if err := InstallPackageV2(db, 7, second); err != nil {
		t.Fatal(err)
	}
	if err := InitializeActivationV2(db, "instance", first.Manifest.ID, first.Revision); err != nil {
		t.Fatal(err)
	}
	if err := SetPendingActivationV2(db, "instance", second.Manifest.ID, second.Revision, time.Now().Add(-ThemeTrialDurationV2-time.Second)); err != nil {
		t.Fatal(err)
	}
	active, err := ActivePackageRevisionV2(db, "instance", first.Revision)
	if err != nil || active.ID != first.Revision {
		t.Fatalf("expired trial active revision = %s, err=%v", active.ID, err)
	}
	activation, err := GetActivationV2(db, "instance")
	if err != nil || activation.PendingRevisionID != "" || activation.ActiveRevisionID != first.Revision {
		t.Fatalf("expired trial activation = %+v, err=%v", activation, err)
	}
}

func TestBuiltinPackagesV2AreCompleteDTCGAndRepeatable(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-builtins-v2?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltinV2(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltinV2(db); err != nil {
		t.Fatalf("second builtin migration: %v", err)
	}
	packages, err := ListPackagesV2(db)
	if err != nil {
		t.Fatal(err)
	}
	if len(packages) != 2 {
		t.Fatalf("expected Yin/Glass, got %d", len(packages))
	}
	expectedPermissions := map[string][]string{
		"org.yin.default": {"spaces.read", "groups.read", "items.read", "items.write", "groups.write", "preferences.read", "preferences.write", "diagnostics.report"},
		"org.yin.glass":   {"spaces.read", "groups.read", "items.read"},
	}
	for _, summary := range packages {
		pkg, err := PackageRevisionPublicV2(db, summary.Revision)
		if err != nil {
			t.Fatal(err)
		}
		if pkg.Manifest.FormatVersion != PackageFormatVersionV2 || pkg.Manifest.Tokens.Version != DTCGVersion || !pkg.Verified {
			t.Errorf("incomplete builtin package %q: %+v", summary.ID, pkg.Manifest)
		}
		if pkg.Manifest.Entrypoints.Script != "views/home.mjs" || len(pkg.Manifest.Entrypoints.Styles) != 1 || len(pkg.Manifest.Contributes.Views) != 1 || pkg.Manifest.Contributes.Views[0] != "home" {
			t.Errorf("builtin %q does not contribute a packaged home view: %+v", summary.ID, pkg.Manifest)
		}
		for _, resource := range pkg.Manifest.Resources {
			asset, err := GetPackageAssetV2(db, summary.Revision, resource.Path)
			if err != nil {
				t.Errorf("%s resource %s missing: %v", summary.ID, resource.Path, err)
				continue
			}
			digest := sha256.Sum256(asset.Content)
			if fmt.Sprintf("%x", digest) != resource.SHA256 {
				t.Errorf("%s resource %s digest mismatch", summary.ID, resource.Path)
			}
		}
		permissions, builtin, err := ImplicitBuiltinPermissionsV2(db, summary.Revision)
		wantPermissions, knownBuiltin := expectedPermissions[summary.ID]
		if err != nil || !builtin || !knownBuiltin || strings.Join(permissions, ",") != strings.Join(wantPermissions, ",") {
			t.Errorf("%s implicit permissions = %v, want %v, builtin=%v, err=%v", summary.ID, permissions, wantPermissions, builtin, err)
		}
		wantVersion := "2.3.2"
		if summary.ID == "org.yin.default" {
			wantVersion = "2.3.57"
		}
		if pkg.Manifest.Version != wantVersion {
			t.Errorf("%s builtin version = %q, want %q", summary.ID, pkg.Manifest.Version, wantVersion)
		}
		switch summary.ID {
		case "org.yin.default":
			if !strings.Contains(string(mustPackageAsset(t, db, summary.Revision, "styles/home.css")), ".yin-masthead") {
				t.Error("Yin home styling is missing its Yin-only layout rules")
			}
		case "org.yin.glass":
			if !strings.Contains(string(mustPackageAsset(t, db, summary.Revision, "styles/home.css")), "backdrop-filter") {
				t.Error("Glass home styling is missing translucent surfaces")
			}
		}
		for scheme, document := range pkg.Tokens {
			if err := validateDTCGDocument202510(document); err != nil {
				t.Errorf("%s %s DTCG: %v", summary.ID, scheme, err)
			}
		}
	}
}

func TestBuiltinHomeAssetsLoadFromPackagedThemeSources(t *testing.T) {
	ids := []string{"org.yin.default", "org.yin.glass"}
	styles := map[string]string{}
	for _, id := range ids {
		resources := builtinHomeResourcesV2(id)
		if !strings.Contains(string(resources["views/home.mjs"].Content), "apiVersion: '1.0.0'") {
			t.Errorf("%s does not load a Theme API v1 source view", id)
		}
		if id == "org.yin.default" && !strings.Contains(string(resources["views/home.mjs"].Content), "yin-theme-root") {
			t.Error("default Yin does not load its independent home view")
		}
		if id != "org.yin.default" && strings.Contains(string(resources["views/home.mjs"].Content), "yin-theme-root") {
			t.Errorf("%s unexpectedly loads the independent Yin home view", id)
		}
		styles[id] = string(resources["styles/home.css"].Content)
	}
	if styles[ids[0]] == styles[ids[1]] {
		t.Fatal("Glass source styles must differ from Yin")
	}
	for id, marker := range map[string]string{
		"org.yin.glass": "backdrop-filter: blur(18px)",
	} {
		if !strings.Contains(styles[id], marker) {
			t.Errorf("%s source is missing its visual signature %q", id, marker)
		}
	}
}

func TestBuiltinHomeResourceHashesOnlyChangeForDefaultYin(t *testing.T) {
	expected := map[string]map[string]string{
		"org.yin.default": {
			// 手机宽度下第一个书签分组的上间距与桌面统一，去掉视口比例额外间距。
			// 改动的是 Yin 主题的分组定位，所以只有它的哈希变。
			"views/home.mjs": "86092315567d95c9af30e7a5bdc4d9cd878acb2aaaba156378e11f27485bd6fd",
			"styles/home.css": "3c01b2aed105b337042ab9257ad3b29220220aae2dca2d732730c99317b966c4",
		},
		"org.yin.glass": {
			"views/home.mjs":  "6f380f2079ba8f523ad4ce149b9f9faa7b5f7d11a9053e8a336b0624e0b1a091",
			"styles/home.css": "a46ef90408799b5fafad0f26a59faea0d8c54f29d1f5518f6335b81bf87559cc",
		},
	}
	for id, want := range expected {
		resources := builtinHomeResourcesV2(id)
		if len(resources) != len(want) {
			t.Errorf("%s resource count = %d, want %d", id, len(resources), len(want))
		}
		for path, wantHash := range want {
			resource, ok := resources[path]
			if !ok {
				t.Errorf("%s is missing resource %s", id, path)
				continue
			}
			got := sha256.Sum256(resource.Content)
			if digest := fmt.Sprintf("%x", got); digest != wantHash {
				t.Errorf("%s %s hash = %s, want %s", id, path, digest, wantHash)
			}
		}
	}
}

func mustPackageAsset(t *testing.T, db *gorm.DB, revisionID, path string) []byte {
	t.Helper()
	asset, err := GetPackageAssetV2(db, revisionID, path)
	if err != nil {
		t.Fatalf("get package asset %s: %v", path, err)
	}
	return asset.Content
}

func TestEnsureBuiltinV2UpgradesOnlyActivationForSelectedBuiltin(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-builtins-v2-upgrade?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	old := packageFixtureV2("2.0.0", "old")
	old.Manifest.ID = "org.yin.default"
	old.Manifest.Name = "Yin"
	if err := InstallPackageV2(db, 0, old); err != nil {
		t.Fatal(err)
	}
	if err := InitializeActivationV2(db, "user:12", old.Manifest.ID, old.Revision); err != nil {
		t.Fatal(err)
	}
	community := packageFixtureV2("1.0.0", "community")
	if err := InstallPackageV2(db, 12, community); err != nil {
		t.Fatal(err)
	}
	if err := InitializeActivationV2(db, "user:13", community.Manifest.ID, community.Revision); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltinV2(db); err != nil {
		t.Fatal(err)
	}
	oldRevision, err := GetPackageRevisionV2(db, old.Revision)
	if err != nil || oldRevision.ID != old.Revision {
		t.Fatalf("old builtin revision must remain rollback-capable: %+v err=%v", oldRevision, err)
	}
	activation, err := GetActivationV2(db, "user:12")
	if err != nil {
		t.Fatal(err)
	}
	latest, err := LatestPackageRevisionV2(db, "org.yin.default")
	if err != nil || latest.Version != "2.3.57" || activation.ActiveRevisionID != latest.ID {
		t.Fatalf("selected builtin activation = %+v, latest=%s, err=%v", activation, latest.ID, err)
	}
	if latest.ID == old.Revision {
		t.Fatal("Yin permission update did not create a new immutable revision")
	}
	activation, err = GetActivationV2(db, "user:13")
	if err != nil || activation.ActiveRevisionID != community.Revision {
		t.Fatalf("community activation changed during builtin update: %+v, err=%v", activation, err)
	}
}

// A database seeded by an earlier release keeps the rows of themes a later build
// no longer ships, and an activation record can still point at one. Reading the
// active theme must not fail on that; it must fall back to Yin, the one package
// guaranteed to be seeded on every startup.
func TestActivePackageRevisionV2FallsBackWhenTheActivatedPackageIsGone(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-builtins-v2-retired?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	if err := EnsureBuiltinV2(db); err != nil {
		t.Fatal(err)
	}
	yin, err := InstanceDefaultRevisionV2(db)
	if err != nil {
		t.Fatal(err)
	}
	// Reproduce the retired-package state: an activation pointing at a revision
	// that no longer resolves, with no migration to repair it.
	retired := packageFixtureV2("2.3.2", "retired")
	retired.Manifest.ID = "org.yin.minimal"
	retired.Manifest.Name = "Minimal"
	if err := InstallPackageV2(db, 0, retired); err != nil {
		t.Fatal(err)
	}
	if err := InitializeActivationV2(db, ActivationScopeForUserV2(7), retired.Manifest.ID, retired.Revision); err != nil {
		t.Fatal(err)
	}
	if err := db.Where("id = ?", retired.Revision).Delete(&RevisionRecordV2{}).Error; err != nil {
		t.Fatal(err)
	}

	// Reading the retired package's revision directly still reports it missing,
	// so the fallback cannot be passing because the lookup succeeded.
	if _, err := GetPackageRevisionV2(db, retired.Revision); !errors.Is(err, gorm.ErrRecordNotFound) {
		t.Fatalf("retired revision should be gone, got err=%v", err)
	}

	got, err := ActivePackageRevisionV2(db, ActivationScopeForUserV2(7), yin.ID)
	if err != nil {
		t.Fatalf("resolving a retired activation must fall back, got err=%v", err)
	}
	if got.ID != yin.ID {
		t.Fatalf("retired activation resolved to %s, want Yin %s", got.ID, yin.ID)
	}

	// A resolvable activation must still win over the fallback, or this change
	// would silently reset every user onto Yin.
	working := packageFixtureV2("1.0.0", "working")
	working.Manifest.ID = "com.example.working"
	if err := InstallPackageV2(db, 7, working); err != nil {
		t.Fatal(err)
	}
	if err := InitializeActivationV2(db, ActivationScopeForUserV2(8), working.Manifest.ID, working.Revision); err != nil {
		t.Fatal(err)
	}
	got, err = ActivePackageRevisionV2(db, ActivationScopeForUserV2(8), yin.ID)
	if err != nil || got.ID != working.Revision {
		t.Fatalf("live activation = %s, want %s, err=%v", got.ID, working.Revision, err)
	}
}

func TestRevisionV2RejectsSameVersionWithDifferentContent(t *testing.T) {
	db, err := gorm.Open(sqlite.Open("file:theme-revision-immutable?mode=memory&cache=shared"), &gorm.Config{})
	if err != nil {
		t.Fatal(err)
	}
	if err := Migrate(db); err != nil {
		t.Fatal(err)
	}
	first := packageFixtureV2("1.0.0", "first")
	if err := InstallPackageV2(db, 0, first); err != nil {
		t.Fatal(err)
	}
	second := packageFixtureV2("1.0.0", "second")
	if err := InstallPackageV2(db, 0, second); err == nil {
		t.Fatal("same version with changed content must be rejected")
	}
}

func packageFixtureV2(version, content string) *PackageV2 {
	revisionBytes := []byte("revision:" + version + ":" + content)
	return &PackageV2{
		Manifest: PackageManifestV2{ID: "community.example.revisions", Name: "Revision fixture", Version: version},
		Files:    map[string]ResourceData{"assets/mark.png": {MediaType: "image/png", Content: []byte("image-" + content)}},
		Revision: packageRevisionV2(map[string][]byte{"manifest.json": revisionBytes}),
	}
}
