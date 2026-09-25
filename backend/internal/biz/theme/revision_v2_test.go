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
	if len(packages) != 4 {
		t.Fatalf("expected Yin/Glass/Minimal/Cyber, got %d", len(packages))
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
		if err != nil || !builtin || len(permissions) != 3 {
			t.Errorf("%s built-in read permissions = %v, builtin=%v, err=%v", summary.ID, permissions, builtin, err)
		}
		switch summary.ID {
		case "org.yin.default":
			if !strings.Contains(string(mustPackageAsset(t, db, summary.Revision, "styles/home.css")), ".home-header") {
				t.Error("Yin home styling is missing its layout rules")
			}
		case "org.yin.glass":
			if !strings.Contains(string(mustPackageAsset(t, db, summary.Revision, "styles/home.css")), "backdrop-filter") {
				t.Error("Glass home styling is missing translucent surfaces")
			}
		case "org.yin.minimal":
			if !strings.Contains(string(mustPackageAsset(t, db, summary.Revision, "styles/home.css")), "flex-direction: column; gap: 0") {
				t.Error("Minimal home styling is missing its editorial list treatment")
			}
		case "org.yin.cyber":
			if !strings.Contains(string(mustPackageAsset(t, db, summary.Revision, "styles/home.css")), "background-size: 28px 28px") {
				t.Error("Cyber home styling is missing its technical grid treatment")
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
	ids := []string{"org.yin.default", "org.yin.glass", "org.yin.minimal", "org.yin.cyber"}
	styles := map[string]string{}
	for _, id := range ids {
		resources := builtinHomeResourcesV2(id)
		if !strings.Contains(string(resources["views/home.mjs"].Content), "apiVersion: '1.0.0'") {
			t.Errorf("%s does not load a Theme API v1 source view", id)
		}
		styles[id] = string(resources["styles/home.css"].Content)
	}
	if styles[ids[0]] == styles[ids[1]] || styles[ids[0]] == styles[ids[2]] || styles[ids[0]] == styles[ids[3]] {
		t.Fatal("official theme source styles must differ from Yin")
	}
	for id, marker := range map[string]string{
		"org.yin.glass":   "backdrop-filter: blur(18px)",
		"org.yin.minimal": "flex-direction: column; gap: 0",
		"org.yin.cyber":   "background-size: 28px 28px",
	} {
		if !strings.Contains(styles[id], marker) {
			t.Errorf("%s source is missing its visual signature %q", id, marker)
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
	if err != nil || latest.Version != "2.3.0" || activation.ActiveRevisionID != latest.ID {
		t.Fatalf("selected builtin activation = %+v, latest=%s, err=%v", activation, latest.ID, err)
	}
	activation, err = GetActivationV2(db, "user:13")
	if err != nil || activation.ActiveRevisionID != community.Revision {
		t.Fatalf("community activation changed during builtin update: %+v, err=%v", activation, err)
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
