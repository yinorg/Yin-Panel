package theme

import (
	"errors"
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
		for scheme, document := range pkg.Tokens {
			if err := validateDTCGDocument202510(document); err != nil {
				t.Errorf("%s %s DTCG: %v", summary.ID, scheme, err)
			}
		}
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
