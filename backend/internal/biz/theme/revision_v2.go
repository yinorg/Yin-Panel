package theme

import (
	"crypto/sha256"
	"encoding/json"
	"errors"
	"fmt"
	"net/url"
	"sort"
	"strconv"
	"strings"
	"time"

	"gorm.io/gorm"
)

const InstanceThemeScopeV2 = "instance"
const ThemeTrialDurationV2 = 30 * time.Second

var ErrTrustedRuntimeNotEnabledV2 = errors.New("trusted runtime is not enabled by the instance administrator")

type PackageRecordV2 struct {
	ID             string `gorm:"primaryKey;size:128"`
	Name           string `gorm:"size:100;not null"`
	LatestRevision string `gorm:"size:64;not null"`
	Removed        bool   `gorm:"not null;default:false"`
	CreatedAt      time.Time
	UpdatedAt      time.Time
}

type RevisionRecordV2 struct {
	ID           string `gorm:"primaryKey;size:64"`
	PackageID    string `gorm:"uniqueIndex:uk_theme_package_version_v2,priority:1;size:128;not null"`
	Version      string `gorm:"uniqueIndex:uk_theme_package_version_v2,priority:2;size:80;not null"`
	ManifestJSON string `gorm:"type:longtext;not null"`
	TokensJSON   string `gorm:"type:longtext;not null"`
	Verified     bool   `gorm:"not null;default:false"`
	CreatedAt    time.Time
}

type AssetRecordV2 struct {
	RevisionID string `gorm:"primaryKey;size:64"`
	Path       string `gorm:"primaryKey;size:240"`
	MediaType  string `gorm:"size:80;not null"`
	Content    []byte `gorm:"type:longblob;not null"`
}

type ActivationRecordV2 struct {
	Scope              string `gorm:"primaryKey;size:128"`
	PackageID          string `gorm:"size:128;not null"`
	ActiveRevisionID   string `gorm:"size:64;not null"`
	LastGoodRevisionID string `gorm:"size:64"`
	PendingRevisionID  string `gorm:"size:64"`
	TrialStartedAt     *time.Time
	UpdatedAt          time.Time
}

type GrantRecordV2 struct {
	UserID          uint   `gorm:"primaryKey;autoIncrement:false"`
	RevisionID      string `gorm:"primaryKey;size:64"`
	ExecutionMode   string `gorm:"primaryKey;size:16"`
	PermissionsJSON string `gorm:"type:text;not null"`
	UpdatedAt       time.Time
}

type TrustedRuntimePolicyV2 struct {
	RevisionID string `gorm:"primaryKey;size:64"`
	Enabled    bool   `gorm:"not null;default:false"`
	UpdatedBy  uint   `gorm:"not null;default:0"`
	UpdatedAt  time.Time
}

type ThemeSettingsRecordV2 struct {
	UserID        uint   `gorm:"primaryKey;autoIncrement:false"`
	PackageID     string `gorm:"primaryKey;size:128"`
	RevisionID    string `gorm:"primaryKey;size:64"`
	SchemaVersion int    `gorm:"not null"`
	DataJSON      string `gorm:"type:longtext;not null"`
	UpdatedAt     time.Time
}

type UserThemePreferenceV2 struct {
	UserID uint `gorm:"primaryKey;autoIncrement:false"`
	// Mode is the colour scheme (light/dark/auto).
	Mode string `gorm:"size:8;not null"`
	// ThemeMode is the theme-choice behaviour: "custom" (use the user's own
	// theme) or "follow-space" (inherit the space's theme). Existing rows default
	// to "custom"; a user with no row follows the space.
	ThemeMode string `gorm:"size:16;not null;default:'custom'"`
	UpdatedAt time.Time
}

// SpaceThemePreferenceV2 is the theme a space shows. Absence means "inherit the
// system default", so changing the default reaches every space that has not
// overridden it.
type SpaceThemePreferenceV2 struct {
	SpaceID    uint   `gorm:"primaryKey;autoIncrement:false"`
	PackageID  string `gorm:"size:128;not null"`
	RevisionID string `gorm:"size:64;not null"`
	UpdatedBy  uint   `gorm:"not null;default:0"`
	UpdatedAt  time.Time
}

// UserSpaceThemePreferenceV2 is a user's per-space theme override. Reserved:
// resolution already honours it, but no write path is exposed yet.
type UserSpaceThemePreferenceV2 struct {
	UserID     uint   `gorm:"primaryKey;autoIncrement:false"`
	SpaceID    uint   `gorm:"primaryKey;autoIncrement:false"`
	PackageID  string `gorm:"size:128;not null"`
	RevisionID string `gorm:"size:64;not null"`
	UpdatedAt  time.Time
}

type builtinPaletteV2 struct {
	id, name    string
	light, dark map[string]string
	radius      string
	spacing     string
	shadow      string
	motion      string
	fontBody    []string
	density     float64
	surface     float64
	blur        float64
	glow        float64
	grid        float64
	dots        float64
	cardPadding string
	iconSize    string
}

func migrateRevisionV2(db *gorm.DB) error {
	return db.AutoMigrate(&PackageRecordV2{}, &RevisionRecordV2{}, &AssetRecordV2{}, &ActivationRecordV2{}, &GrantRecordV2{}, &TrustedRuntimePolicyV2{}, &ThemeSettingsRecordV2{}, &UserThemePreferenceV2{}, &SpaceThemePreferenceV2{}, &UserSpaceThemePreferenceV2{})
}

func InstallPackageV2(db *gorm.DB, actorID uint, pkg *PackageV2) error {
	if pkg == nil || pkg.Manifest.ID == "" || len(pkg.Revision) != 64 {
		return errors.New("invalid v2 theme package")
	}
	manifestJSON, err := json.Marshal(pkg.Manifest)
	if err != nil {
		return err
	}
	tokensJSON, err := json.Marshal(pkg.Tokens)
	if err != nil {
		return err
	}
	return db.Transaction(func(tx *gorm.DB) error {
		var existingRevision RevisionRecordV2
		err := tx.First(&existingRevision, "package_id = ? AND version = ?", pkg.Manifest.ID, pkg.Manifest.Version).Error
		if err == nil {
			if existingRevision.ID != pkg.Revision {
				return errors.New("same theme package version has different content")
			}
			var packageRecord PackageRecordV2
			if err := tx.First(&packageRecord, "id = ?", pkg.Manifest.ID).Error; err != nil {
				return err
			}
			return tx.Model(&packageRecord).Updates(map[string]any{"name": pkg.Manifest.Name, "latest_revision": pkg.Revision, "removed": false}).Error
		}
		if !errors.Is(err, gorm.ErrRecordNotFound) {
			return err
		}
		var packageRecord PackageRecordV2
		err = tx.First(&packageRecord, "id = ?", pkg.Manifest.ID).Error
		if err != nil && !errors.Is(err, gorm.ErrRecordNotFound) {
			return err
		}
		if errors.Is(err, gorm.ErrRecordNotFound) {
			packageRecord = PackageRecordV2{ID: pkg.Manifest.ID, Name: pkg.Manifest.Name}
			if err := tx.Create(&packageRecord).Error; err != nil {
				return err
			}
		}
		if packageRecord.Removed {
			return errors.New("removed theme packages must be explicitly restored before installation")
		}
		revision := RevisionRecordV2{ID: pkg.Revision, PackageID: pkg.Manifest.ID, Version: pkg.Manifest.Version, ManifestJSON: string(manifestJSON), TokensJSON: string(tokensJSON), Verified: pkg.Verified}
		if err := tx.Create(&revision).Error; err != nil {
			return err
		}
		for name, asset := range pkg.Files {
			row := AssetRecordV2{RevisionID: pkg.Revision, Path: name, MediaType: asset.MediaType, Content: asset.Content}
			if err := tx.Create(&row).Error; err != nil {
				return err
			}
		}
		if err := tx.Model(&packageRecord).Updates(map[string]any{"name": pkg.Manifest.Name, "latest_revision": pkg.Revision, "removed": false}).Error; err != nil {
			return err
		}
		return tx.Create(&AuditRecord{ActorID: actorID, Action: "install-v2", PackageID: pkg.Manifest.ID}).Error
	})
}

func GetPackageRevisionV2(db *gorm.DB, revisionID string) (RevisionRecordV2, error) {
	var revision RevisionRecordV2
	err := db.First(&revision, "id = ?", revisionID).Error
	return revision, err
}

type PublicPackageV2 struct {
	Manifest PackageManifestV2          `json:"manifest"`
	Tokens   map[string]json.RawMessage `json:"tokens"`
	Verified bool                       `json:"verified"`
	Revision string                     `json:"revision"`
}

func PackageRevisionPublicV2(db *gorm.DB, revisionID string) (PublicPackageV2, error) {
	revisionID = strings.TrimSpace(revisionID)
	var revision RevisionRecordV2
	if err := db.First(&revision, "id = ?", revisionID).Error; err != nil {
		return PublicPackageV2{}, err
	}
	result := PublicPackageV2{Verified: revision.Verified, Revision: revision.ID}
	if err := json.Unmarshal([]byte(revision.ManifestJSON), &result.Manifest); err != nil {
		return PublicPackageV2{}, err
	}
	if err := json.Unmarshal([]byte(revision.TokensJSON), &result.Tokens); err != nil {
		return PublicPackageV2{}, err
	}
	for name := range result.Manifest.Tokens.Docs {
		if _, ok := result.Tokens[name]; !ok {
			return PublicPackageV2{}, fmt.Errorf("revision is missing %s DTCG document", name)
		}
	}
	for i := range result.Manifest.Resources {
		resource := &result.Manifest.Resources[i]
		segments := strings.Split(resource.Path, "/")
		for j := range segments {
			segments[j] = url.PathEscape(segments[j])
		}
		resource.URL = "/api/theme/v2/assets/" + url.PathEscape(revision.ID) + "/" + strings.Join(segments, "/")
	}
	return result, nil
}

func ListPackageRevisionsV2(db *gorm.DB) ([]RevisionRecordV2, error) {
	var records []RevisionRecordV2
	err := db.Model(&RevisionRecordV2{}).Select("id", "package_id", "version", "verified", "created_at").Order("package_id, created_at desc").Find(&records).Error
	return records, err
}

type PackageSummaryV2 struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	Version   string    `json:"version"`
	Revision  string    `json:"revision"`
	Verified  bool      `json:"verified"`
	CreatedAt time.Time `json:"createdAt"`
}

func ListPackagesV2(db *gorm.DB) ([]PackageSummaryV2, error) {
	var packages []PackageRecordV2
	if err := db.Where("removed = ?", false).Order("id").Find(&packages).Error; err != nil {
		return nil, err
	}
	result := make([]PackageSummaryV2, 0, len(packages))
	for _, pkg := range packages {
		revision, err := GetPackageRevisionV2(db, pkg.LatestRevision)
		if err != nil {
			return nil, err
		}
		result = append(result, PackageSummaryV2{ID: pkg.ID, Name: pkg.Name, Version: revision.Version, Revision: revision.ID, Verified: revision.Verified, CreatedAt: revision.CreatedAt})
	}
	return result, nil
}

func LatestPackageRevisionV2(db *gorm.DB, packageID string) (RevisionRecordV2, error) {
	var pkg PackageRecordV2
	if err := db.First(&pkg, "id = ? AND removed = ?", packageID, false).Error; err != nil {
		return RevisionRecordV2{}, err
	}
	return GetPackageRevisionV2(db, pkg.LatestRevision)
}

func ActivePackageRevisionV2(db *gorm.DB, scope, fallbackRevisionID string) (RevisionRecordV2, error) {
	var activation ActivationRecordV2
	err := db.First(&activation, "scope = ?", scope).Error
	if errors.Is(err, gorm.ErrRecordNotFound) {
		if fallbackRevisionID == "" {
			return RevisionRecordV2{}, err
		}
		return GetPackageRevisionV2(db, fallbackRevisionID)
	}
	if err != nil {
		return RevisionRecordV2{}, err
	}
	if activation.PendingRevisionID != "" && activation.TrialStartedAt != nil && time.Since(*activation.TrialStartedAt) > ThemeTrialDurationV2 {
		if err := RollbackActivationV2(db, scope, fallbackRevisionID); err != nil {
			return RevisionRecordV2{}, err
		}
		return ActivePackageRevisionV2(db, scope, fallbackRevisionID)
	}
	// A pending trial revision takes priority over the active one, as before.
	// Resolving either may fail with ErrRecordNotFound when a database seeded by
	// an earlier release still points at a package this build no longer ships.
	// That is not a database error: fall through to the caller's default, which
	// is always the seeded Yin package, rather than failing every theme read.
	//
	// The lookup order is preserved exactly; only a missing revision changes the
	// outcome, from ErrRecordNotFound to the fallback.
	pending := activation.PendingRevisionID
	active := activation.ActiveRevisionID
	for _, revisionID := range []string{pending, active} {
		if revisionID == "" {
			continue
		}
		revision, err := GetPackageRevisionV2(db, revisionID)
		if err == nil {
			return revision, nil
		}
		if !errors.Is(err, gorm.ErrRecordNotFound) {
			return RevisionRecordV2{}, err
		}
	}
	if fallbackRevisionID == "" {
		return RevisionRecordV2{}, gorm.ErrRecordNotFound
	}
	return GetPackageRevisionV2(db, fallbackRevisionID)
}

func InstanceDefaultRevisionV2(db *gorm.DB) (RevisionRecordV2, error) {
	var packageRecord PackageRecordV2
	if err := db.First(&packageRecord, "id = ? AND removed = ?", "org.yin.default", false).Error; err != nil {
		return RevisionRecordV2{}, err
	}
	return GetPackageRevisionV2(db, packageRecord.LatestRevision)
}

func ActivatePackageV2(db *gorm.DB, scope, packageID, revisionID string) error {
	if scope == "" || packageID == "" || revisionID == "" {
		return errors.New("activation scope, package and revision are required")
	}
	return db.Transaction(func(tx *gorm.DB) error {
		var packageRecord PackageRecordV2
		if err := tx.First(&packageRecord, "id = ? AND removed = ?", packageID, false).Error; err != nil {
			return err
		}
		var revision RevisionRecordV2
		if err := tx.First(&revision, "id = ? AND package_id = ?", revisionID, packageID).Error; err != nil {
			return err
		}
		var activation ActivationRecordV2
		err := tx.First(&activation, "scope = ?", scope).Error
		if err != nil && !errors.Is(err, gorm.ErrRecordNotFound) {
			return err
		}
		if errors.Is(err, gorm.ErrRecordNotFound) {
			activation = ActivationRecordV2{Scope: scope, PackageID: packageID, ActiveRevisionID: revisionID, LastGoodRevisionID: revisionID}
		} else {
			activation.PackageID = packageID
			activation.ActiveRevisionID = revisionID
			activation.LastGoodRevisionID = revisionID
		}
		activation.PendingRevisionID = ""
		activation.TrialStartedAt = nil
		activation.UpdatedAt = time.Now()
		return tx.Save(&activation).Error
	})
}

func SetUserThemeSelectionV2(db *gorm.DB, userID uint, packageID, revisionID, mode string) error {
	if userID == 0 || packageID == "" || revisionID == "" {
		return errors.New("theme selection requires a user, package and revision")
	}
	if !validThemeModeV2(mode) {
		return errors.New("mode must be light, dark, or auto")
	}
	return db.Transaction(func(tx *gorm.DB) error {
		var packageRecord PackageRecordV2
		if err := tx.First(&packageRecord, "id = ? AND removed = ?", packageID, false).Error; err != nil {
			return err
		}
		var revision RevisionRecordV2
		if err := tx.First(&revision, "id = ? AND package_id = ?", revisionID, packageID).Error; err != nil {
			return err
		}
		scope := ActivationScopeForUserV2(userID)
		var activation ActivationRecordV2
		err := tx.First(&activation, "scope = ?", scope).Error
		if err != nil && !errors.Is(err, gorm.ErrRecordNotFound) {
			return err
		}
		if errors.Is(err, gorm.ErrRecordNotFound) {
			activation = ActivationRecordV2{Scope: scope, PackageID: packageID}
		}
		activation.PackageID = packageID
		activation.ActiveRevisionID = revisionID
		activation.LastGoodRevisionID = revisionID
		activation.PendingRevisionID = ""
		activation.TrialStartedAt = nil
		activation.UpdatedAt = time.Now()
		if err := tx.Save(&activation).Error; err != nil {
			return err
		}
		preference := UserThemePreferenceV2{UserID: userID, Mode: mode, ThemeMode: "custom", UpdatedAt: time.Now()}
		if err := tx.Save(&preference).Error; err != nil {
			return err
		}
		return tx.Create(&AuditRecord{ActorID: userID, Action: "select", PackageID: packageID}).Error
	})
}

func UserThemeModeV2(db *gorm.DB, userID uint) (string, error) {
	var preference UserThemePreferenceV2
	err := db.First(&preference, "user_id = ?", userID).Error
	if errors.Is(err, gorm.ErrRecordNotFound) {
		return "auto", nil
	}
	if err != nil {
		return "", err
	}
	if !validThemeModeV2(preference.Mode) {
		return "auto", nil
	}
	return preference.Mode, nil
}

// ThemeResolutionV2 is the theme a viewer sees, plus which step of the chain
// chose it.
type ThemeResolutionV2 struct {
	Revision RevisionRecordV2
	Source   string
}

// ResolveThemeV2 picks the theme a viewer sees for a space. Precedence, highest
// first: a user's per-space override (reserved, not yet writable) -> the user's
// own choice -> the space's theme -> the system default -> the built-in default.
// A preference pointing at a revision that no longer exists is skipped, so a
// removed theme falls back down the chain instead of failing the read.
func ResolveThemeV2(db *gorm.DB, userID, spaceID uint) (ThemeResolutionV2, error) {
	yin, err := InstanceDefaultRevisionV2(db)
	if err != nil {
		return ThemeResolutionV2{}, err
	}
	systemRevision, err := ActivePackageRevisionV2(db, InstanceThemeScopeV2, yin.ID)
	if err != nil {
		return ThemeResolutionV2{}, err
	}
	if userID != 0 && spaceID != 0 {
		revision, ok, err := userSpaceThemeRevisionV2(db, userID, spaceID)
		if err != nil {
			return ThemeResolutionV2{}, err
		}
		if ok {
			return ThemeResolutionV2{Revision: revision, Source: "user-space"}, nil
		}
	}
	if userID != 0 {
		choice, err := UserThemeChoiceModeV2(db, userID)
		if err != nil {
			return ThemeResolutionV2{}, err
		}
		if choice == "custom" {
			revision, err := ActivePackageRevisionV2(db, ActivationScopeForUserV2(userID), "")
			if err == nil {
				return ThemeResolutionV2{Revision: revision, Source: "user"}, nil
			}
			if !errors.Is(err, gorm.ErrRecordNotFound) {
				return ThemeResolutionV2{}, err
			}
		}
	}
	if spaceID != 0 {
		revision, ok, err := spaceThemeRevisionV2(db, spaceID)
		if err != nil {
			return ThemeResolutionV2{}, err
		}
		if ok {
			return ThemeResolutionV2{Revision: revision, Source: "space"}, nil
		}
	}
	return ThemeResolutionV2{Revision: systemRevision, Source: "system"}, nil
}

func userSpaceThemeRevisionV2(db *gorm.DB, userID, spaceID uint) (RevisionRecordV2, bool, error) {
	var preference UserSpaceThemePreferenceV2
	err := db.First(&preference, "user_id = ? AND space_id = ?", userID, spaceID).Error
	if errors.Is(err, gorm.ErrRecordNotFound) {
		return RevisionRecordV2{}, false, nil
	}
	if err != nil {
		return RevisionRecordV2{}, false, err
	}
	return presentRevisionV2(db, preference.RevisionID)
}

func spaceThemeRevisionV2(db *gorm.DB, spaceID uint) (RevisionRecordV2, bool, error) {
	var preference SpaceThemePreferenceV2
	err := db.First(&preference, "space_id = ?", spaceID).Error
	if errors.Is(err, gorm.ErrRecordNotFound) {
		return RevisionRecordV2{}, false, nil
	}
	if err != nil {
		return RevisionRecordV2{}, false, err
	}
	return presentRevisionV2(db, preference.RevisionID)
}

func presentRevisionV2(db *gorm.DB, revisionID string) (RevisionRecordV2, bool, error) {
	revision, err := GetPackageRevisionV2(db, revisionID)
	if errors.Is(err, gorm.ErrRecordNotFound) {
		return RevisionRecordV2{}, false, nil
	}
	if err != nil {
		return RevisionRecordV2{}, false, err
	}
	// A removed package must not keep serving: the revision row survives removal,
	// so the space/override preference has to fall through instead of pinning it.
	var packageRecord PackageRecordV2
	if err := db.First(&packageRecord, "id = ? AND removed = ?", revision.PackageID, false).Error; err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return RevisionRecordV2{}, false, nil
		}
		return RevisionRecordV2{}, false, err
	}
	return revision, true, nil
}

// UserThemeChoiceModeV2 reports whether the user follows their own theme
// ("custom") or the space's ("follow-space"). A user who never selected a theme
// has no preference row and follows the space, which matches their previous
// behaviour of falling through to the instance default.
func UserThemeChoiceModeV2(db *gorm.DB, userID uint) (string, error) {
	var preference UserThemePreferenceV2
	err := db.First(&preference, "user_id = ?", userID).Error
	if errors.Is(err, gorm.ErrRecordNotFound) {
		return "follow-space", nil
	}
	if err != nil {
		return "", err
	}
	if preference.ThemeMode == "custom" || preference.ThemeMode == "follow-space" {
		return preference.ThemeMode, nil
	}
	return "custom", nil
}

// SetUserThemeChoiceV2 changes only the theme-choice behaviour, leaving the
// colour scheme and the selected theme untouched.
func SetUserThemeChoiceV2(db *gorm.DB, userID uint, themeMode string) error {
	if userID == 0 || (themeMode != "custom" && themeMode != "follow-space") {
		return errors.New("themeMode must be custom or follow-space")
	}
	var preference UserThemePreferenceV2
	err := db.First(&preference, "user_id = ?", userID).Error
	if errors.Is(err, gorm.ErrRecordNotFound) {
		preference = UserThemePreferenceV2{UserID: userID, Mode: "auto"}
	} else if err != nil {
		return err
	}
	preference.ThemeMode = themeMode
	preference.UpdatedAt = time.Now()
	return db.Save(&preference).Error
}

func validateThemeRevisionV2(db *gorm.DB, packageID, revisionID string) error {
	if packageID == "" || revisionID == "" {
		return errors.New("a theme package and revision are required")
	}
	var packageRecord PackageRecordV2
	if err := db.First(&packageRecord, "id = ? AND removed = ?", packageID, false).Error; err != nil {
		return err
	}
	var revision RevisionRecordV2
	return db.First(&revision, "id = ? AND package_id = ?", revisionID, packageID).Error
}

// SetSpaceThemeV2 sets the theme a space shows. Absence (ClearSpaceThemeV2) means
// the space inherits the system default.
func SetSpaceThemeV2(db *gorm.DB, spaceID uint, packageID, revisionID string, actorID uint) error {
	if spaceID == 0 {
		return errors.New("a space is required")
	}
	if err := validateThemeRevisionV2(db, packageID, revisionID); err != nil {
		return err
	}
	preference := SpaceThemePreferenceV2{SpaceID: spaceID, PackageID: packageID, RevisionID: revisionID, UpdatedBy: actorID, UpdatedAt: time.Now()}
	return db.Save(&preference).Error
}

func ClearSpaceThemeV2(db *gorm.DB, spaceID uint) error {
	return db.Delete(&SpaceThemePreferenceV2{}, "space_id = ?", spaceID).Error
}

func SpaceThemePreferenceForV2(db *gorm.DB, spaceID uint) (SpaceThemePreferenceV2, error) {
	var preference SpaceThemePreferenceV2
	err := db.First(&preference, "space_id = ?", spaceID).Error
	return preference, err
}

// SetUserSpaceThemeV2 stores a user's per-space override.
func SetUserSpaceThemeV2(db *gorm.DB, userID, spaceID uint, packageID, revisionID string) error {
	if userID == 0 || spaceID == 0 {
		return errors.New("a user and a space are required")
	}
	if err := validateThemeRevisionV2(db, packageID, revisionID); err != nil {
		return err
	}
	preference := UserSpaceThemePreferenceV2{UserID: userID, SpaceID: spaceID, PackageID: packageID, RevisionID: revisionID, UpdatedAt: time.Now()}
	return db.Save(&preference).Error
}

func ClearUserSpaceThemeV2(db *gorm.DB, userID, spaceID uint) error {
	return db.Delete(&UserSpaceThemePreferenceV2{}, "user_id = ? AND space_id = ?", userID, spaceID).Error
}

func UserSpaceThemePreferenceForV2(db *gorm.DB, userID, spaceID uint) (UserSpaceThemePreferenceV2, error) {
	var preference UserSpaceThemePreferenceV2
	err := db.First(&preference, "user_id = ? AND space_id = ?", userID, spaceID).Error
	return preference, err
}

func validThemeModeV2(mode string) bool {
	return mode == "light" || mode == "dark" || mode == "auto"
}

func ActiveRevisionIDV2(db *gorm.DB, scope string) (string, error) {
	var activation ActivationRecordV2
	if err := db.First(&activation, "scope = ?", scope).Error; err != nil {
		return "", err
	}
	return activation.ActiveRevisionID, nil
}

func GetActivationV2(db *gorm.DB, scope string) (ActivationRecordV2, error) {
	var activation ActivationRecordV2
	err := db.First(&activation, "scope = ?", scope).Error
	return activation, err
}

func GrantForRevisionV2(db *gorm.DB, userID uint, revisionID string) (GrantRecordV2, error) {
	var grant GrantRecordV2
	err := db.First(&grant, "user_id = ? AND revision_id = ?", userID, revisionID).Error
	return grant, err
}

func GetPackageAssetV2(db *gorm.DB, revisionID, name string) (AssetRecordV2, error) {
	var asset AssetRecordV2
	err := db.First(&asset, "revision_id = ? AND path = ?", revisionID, name).Error
	return asset, err
}

func ImplicitBuiltinPermissionsV2(db *gorm.DB, revisionID string) ([]string, bool, error) {
	revision, err := GetPackageRevisionV2(db, revisionID)
	if err != nil {
		return nil, false, err
	}
	if !revision.Verified {
		return nil, false, nil
	}
	var manifest PackageManifestV2
	if err := json.Unmarshal([]byte(revision.ManifestJSON), &manifest); err != nil {
		return nil, false, err
	}
	if !isBuiltinThemeID(manifest.ID) {
		return nil, false, nil
	}
	permissions := make([]string, 0, len(manifest.Permissions.Required))
	for _, permission := range manifest.Permissions.Required {
		permissions = append(permissions, permission.Name)
	}
	return permissions, true, nil
}

func InitializeActivationV2(db *gorm.DB, scope, packageID, revisionID string) error {
	if scope == "" || packageID == "" || revisionID == "" {
		return errors.New("activation scope, package and revision are required")
	}
	var revision RevisionRecordV2
	if err := db.First(&revision, "id = ? AND package_id = ?", revisionID, packageID).Error; err != nil {
		return err
	}
	activation := ActivationRecordV2{Scope: scope, PackageID: packageID, ActiveRevisionID: revisionID, LastGoodRevisionID: revisionID, UpdatedAt: time.Now()}
	return db.Save(&activation).Error
}

func SetPendingActivationV2(db *gorm.DB, scope, packageID, revisionID string, startedAt time.Time) error {
	if scope == "" || packageID == "" || revisionID == "" {
		return errors.New("activation scope, package and revision are required")
	}
	return db.Transaction(func(tx *gorm.DB) error {
		var packageRecord PackageRecordV2
		if err := tx.First(&packageRecord, "id = ? AND removed = ?", packageID, false).Error; err != nil {
			return err
		}
		var revision RevisionRecordV2
		if err := tx.First(&revision, "id = ? AND package_id = ?", revisionID, packageID).Error; err != nil {
			return err
		}
		var activation ActivationRecordV2
		err := tx.First(&activation, "scope = ?", scope).Error
		if err != nil && !errors.Is(err, gorm.ErrRecordNotFound) {
			return err
		}
		if errors.Is(err, gorm.ErrRecordNotFound) {
			activation = ActivationRecordV2{Scope: scope, PackageID: packageID}
		}
		activation.PendingRevisionID = revisionID
		activation.TrialStartedAt = &startedAt
		activation.UpdatedAt = time.Now()
		return tx.Save(&activation).Error
	})
}

func ConfirmActivationV2(db *gorm.DB, scope, revisionID string) error {
	return db.Transaction(func(tx *gorm.DB) error {
		var activation ActivationRecordV2
		if err := tx.First(&activation, "scope = ?", scope).Error; err != nil {
			return err
		}
		if activation.PendingRevisionID == "" || activation.PendingRevisionID != revisionID {
			return errors.New("no matching theme activation trial")
		}
		var revision RevisionRecordV2
		if err := tx.First(&revision, "id = ?", revisionID).Error; err != nil {
			return err
		}
		activation.PackageID = revision.PackageID
		activation.ActiveRevisionID = revisionID
		activation.PendingRevisionID = ""
		activation.TrialStartedAt = nil
		activation.UpdatedAt = time.Now()
		return tx.Save(&activation).Error
	})
}

func MarkActivationHealthyV2(db *gorm.DB, scope, revisionID string) error {
	return db.Model(&ActivationRecordV2{}).Where("scope = ? AND active_revision_id = ? AND pending_revision_id = ''", scope, revisionID).Updates(map[string]any{"last_good_revision_id": revisionID, "updated_at": time.Now()}).Error
}

func RollbackActivationV2(db *gorm.DB, scope, fallbackRevisionID string) error {
	return db.Transaction(func(tx *gorm.DB) error {
		var activation ActivationRecordV2
		if err := tx.First(&activation, "scope = ?", scope).Error; err != nil {
			return err
		}
		revisionID := activation.LastGoodRevisionID
		if revisionID == "" {
			revisionID = fallbackRevisionID
		}
		var revision RevisionRecordV2
		if err := tx.First(&revision, "id = ?", revisionID).Error; err != nil {
			return fmt.Errorf("load rollback revision: %w", err)
		}
		activation.PackageID = revision.PackageID
		activation.ActiveRevisionID = revision.ID
		activation.PendingRevisionID = ""
		activation.TrialStartedAt = nil
		activation.UpdatedAt = time.Now()
		return tx.Save(&activation).Error
	})
}

func ActivationScopeForUserV2(userID uint) string {
	return "user:" + strconv.FormatUint(uint64(userID), 10)
}

func SaveGrantV2(db *gorm.DB, grant GrantRecordV2) error {
	if grant.UserID == 0 || grant.RevisionID == "" || grant.ExecutionMode != "sandbox" && grant.ExecutionMode != "trusted" {
		return errors.New("invalid theme runtime grant")
	}
	grant.UpdatedAt = time.Now()
	return db.Save(&grant).Error
}

func SaveUserGrantV2(db *gorm.DB, grant GrantRecordV2) error {
	if grant.UserID == 0 || grant.RevisionID == "" || (grant.ExecutionMode != "sandbox" && grant.ExecutionMode != "trusted") {
		return errors.New("invalid theme runtime grant")
	}
	var permissions []string
	if err := json.Unmarshal([]byte(grant.PermissionsJSON), &permissions); err != nil {
		return errors.New("invalid theme runtime grant permissions")
	}
	return db.Transaction(func(tx *gorm.DB) error {
		var revision RevisionRecordV2
		if err := tx.First(&revision, "id = ?", grant.RevisionID).Error; err != nil {
			return err
		}
		var manifest PackageManifestV2
		if err := json.Unmarshal([]byte(revision.ManifestJSON), &manifest); err != nil {
			return fmt.Errorf("stored theme manifest is invalid: %w", err)
		}
		if err := ValidateThemeGrantV2(manifest, grant.ExecutionMode, permissions); err != nil {
			return err
		}
		if grant.ExecutionMode == "trusted" {
			var policy TrustedRuntimePolicyV2
			if err := tx.First(&policy, "revision_id = ? AND enabled = ?", grant.RevisionID, true).Error; err != nil {
				if errors.Is(err, gorm.ErrRecordNotFound) {
					return ErrTrustedRuntimeNotEnabledV2
				}
				return err
			}
		}
		grant.UpdatedAt = time.Now()
		return tx.Save(&grant).Error
	})
}

func GetGrantV2(db *gorm.DB, userID uint, revisionID, executionMode string) (GrantRecordV2, error) {
	var grant GrantRecordV2
	err := db.First(&grant, "user_id = ? AND revision_id = ? AND execution_mode = ?", userID, revisionID, executionMode).Error
	return grant, err
}

func RevokeGrantV2(db *gorm.DB, userID uint, revisionID, executionMode string) error {
	return db.Delete(&GrantRecordV2{}, "user_id = ? AND revision_id = ? AND execution_mode = ?", userID, revisionID, executionMode).Error
}

func GetTrustedRuntimePolicyV2(db *gorm.DB, revisionID string) (TrustedRuntimePolicyV2, error) {
	var policy TrustedRuntimePolicyV2
	err := db.First(&policy, "revision_id = ?", revisionID).Error
	return policy, err
}

func SetTrustedRuntimePolicyV2(db *gorm.DB, revisionID string, enabled bool, actorID uint) error {
	if revisionID == "" || actorID == 0 {
		return errors.New("invalid trusted runtime policy")
	}
	return db.Transaction(func(tx *gorm.DB) error {
		var revision RevisionRecordV2
		if err := tx.First(&revision, "id = ?", revisionID).Error; err != nil {
			return err
		}
		if enabled {
			var manifest PackageManifestV2
			if err := json.Unmarshal([]byte(revision.ManifestJSON), &manifest); err != nil {
				return fmt.Errorf("stored theme manifest is invalid: %w", err)
			}
			if !containsString(manifest.Runtime.SupportedModes, "trusted") {
				return errors.New("theme does not declare support for the trusted runtime")
			}
		}
		policy := TrustedRuntimePolicyV2{RevisionID: revisionID, Enabled: enabled, UpdatedBy: actorID, UpdatedAt: time.Now()}
		if err := tx.Save(&policy).Error; err != nil {
			return err
		}
		if !enabled {
			return tx.Delete(&GrantRecordV2{}, "revision_id = ? AND execution_mode = ?", revisionID, "trusted").Error
		}
		return nil
	})
}

func TrustedRuntimeEnabledV2(db *gorm.DB, revisionID string) (bool, error) {
	policy, err := GetTrustedRuntimePolicyV2(db, revisionID)
	if errors.Is(err, gorm.ErrRecordNotFound) {
		return false, nil
	}
	return policy.Enabled, err
}

func SaveThemeSettingsV2(db *gorm.DB, settings ThemeSettingsRecordV2) error {
	if settings.UserID == 0 || settings.PackageID == "" || settings.RevisionID == "" || settings.SchemaVersion < 1 || !json.Valid([]byte(settings.DataJSON)) {
		return errors.New("invalid theme settings data")
	}
	settings.UpdatedAt = time.Now()
	return db.Save(&settings).Error
}

func RemovePackageV2(db *gorm.DB, actorID uint, packageID string) error {
	if isBuiltinThemeID(packageID) {
		return errors.New("built-in themes cannot be removed")
	}
	return db.Transaction(func(tx *gorm.DB) error {
		var record PackageRecordV2
		if err := tx.First(&record, "id = ? AND removed = ?", packageID, false).Error; err != nil {
			return err
		}
		if err := tx.Model(&record).Update("removed", true).Error; err != nil {
			return err
		}
		var yin PackageRecordV2
		if err := tx.First(&yin, "id = ? AND removed = ?", "org.yin.default", false).Error; err != nil {
			return err
		}
		if err := tx.Model(&ActivationRecordV2{}).Where("package_id = ?", packageID).Updates(map[string]any{
			"package_id": yin.ID, "active_revision_id": yin.LatestRevision,
			"last_good_revision_id": yin.LatestRevision, "pending_revision_id": "",
			"trial_started_at": nil, "updated_at": time.Now(),
		}).Error; err != nil {
			return err
		}
		return tx.Create(&AuditRecord{ActorID: actorID, Action: "remove-v2", PackageID: packageID}).Error
	})
}

func EnsureBuiltinV2(db *gorm.DB) error {
	for _, builtin := range builtinPackagesV2() {
		previous, previousErr := LatestPackageRevisionV2(db, builtin.Manifest.ID)
		if previousErr != nil && !errors.Is(previousErr, gorm.ErrRecordNotFound) {
			return fmt.Errorf("load built-in theme %s: %w", builtin.Manifest.ID, previousErr)
		}
		if err := InstallPackageV2(db, 0, builtin); err != nil {
			return fmt.Errorf("install built-in theme %s: %w", builtin.Manifest.ID, err)
		}
		if previousErr == nil && previous.ID != builtin.Revision {
			if err := db.Model(&ActivationRecordV2{}).Where("package_id = ? AND active_revision_id = ? AND pending_revision_id = ''", builtin.Manifest.ID, previous.ID).Updates(map[string]any{
				"active_revision_id":    builtin.Revision,
				"last_good_revision_id": builtin.Revision,
				"updated_at":            time.Now(),
			}).Error; err != nil {
				return fmt.Errorf("upgrade active built-in theme %s: %w", builtin.Manifest.ID, err)
			}
		}
	}
	if err := db.Transaction(func(tx *gorm.DB) error {
		var activation ActivationRecordV2
		err := tx.First(&activation, "scope = ?", InstanceThemeScopeV2).Error
		if err == nil {
			return nil
		}
		if !errors.Is(err, gorm.ErrRecordNotFound) {
			return err
		}
		var yin PackageRecordV2
		if err := tx.First(&yin, "id = ?", "org.yin.default").Error; err != nil {
			return err
		}
		now := time.Now()
		activation = ActivationRecordV2{Scope: InstanceThemeScopeV2, PackageID: yin.ID, ActiveRevisionID: yin.LatestRevision, LastGoodRevisionID: yin.LatestRevision, UpdatedAt: now}
		return tx.Create(&activation).Error
	}); err != nil {
		return err
	}
	return nil
}

func builtinPackagesV2() []*PackageV2 {
	palettes := []builtinPaletteV2{
		{"org.yin.default", "Yin", map[string]string{"canvas": "#f4f7f8", "surface": "#ffffff", "text": "#20282c", "muted": "#64737a", "border": "#d8e0e3", "primary": "#176b80"}, map[string]string{"canvas": "#171d20", "surface": "#22292c", "text": "#f2f5f6", "muted": "#b7c1c4", "border": "#536066", "primary": "#78c5d4"}, "8px", "16px", "0 8px 24px #18242a22", "180ms", []string{"Inter", "system-ui", "sans-serif"}, 1, 1, 0, 0, 0, 0, "16px", "70px"},
		{"org.yin.glass", "Glass", map[string]string{"canvas": "#e7edf4", "surface": "#edf2f8cc", "text": "#182535", "muted": "#53677c", "border": "#a9bed3aa", "primary": "#2666a6"}, map[string]string{"canvas": "#101827", "surface": "#19263acc", "text": "#eef5ff", "muted": "#a6b8cd", "border": "#526c89aa", "primary": "#73b9ff"}, "18px", "22px", "0 18px 50px #18365b55", "260ms", []string{"Inter", "system-ui", "sans-serif"}, 1.08, .82, 18, 0, 0, .08, "22px", "76px"},
	}
	result := make([]*PackageV2, 0, len(palettes))
	for _, palette := range palettes {
		docs := map[string]json.RawMessage{"light": builtinDTCGDocumentV2(palette.light, palette), "dark": builtinDTCGDocumentV2(palette.dark, palette)}
		files := builtinHomeResourcesV2(palette.id)
		version := "2.3.2"
		requiredPermissions := []PermissionV2{{Name: "spaces.read"}, {Name: "groups.read"}, {Name: "items.read"}}
		contributedViews := []string{"home"}
		if palette.id == "org.yin.default" {
			// Bump whenever a home resource changes; a published version is
			// immutable, so reusing it would abort startup.
			version = "2.3.60"
			requiredPermissions = append(requiredPermissions,
				PermissionV2{Name: "items.write"},
				PermissionV2{Name: "groups.write"},
				// The theme exposes the settings surface and the LAN/WAN switch,
				// which the Core gates on these preferences permissions.
				PermissionV2{Name: "preferences.read"},
				PermissionV2{Name: "preferences.write"},
				// The theme reports its measured layout so the Core can place the
				// monitor layer from real geometry.
				PermissionV2{Name: "diagnostics.report"},
			)
			// The settings surface is rendered by the theme (theme-settings),
			// falling back to the Core page when absent; theme-page is a whole
			// page the theme owns.
			contributedViews = append(contributedViews, "theme-settings", "theme-page")
		}
		resources := make([]ResourceV2, 0, len(files))
		for name, asset := range files {
			sum := sha256.Sum256(asset.Content)
			resources = append(resources, ResourceV2{Path: name, SHA256: fmt.Sprintf("%x", sum[:]), MediaType: asset.MediaType})
		}
		sort.Slice(resources, func(i, j int) bool { return resources[i].Path < resources[j].Path })
		manifest := PackageManifestV2{
			Format: "yin-theme", FormatVersion: PackageFormatVersionV2, ID: palette.id, Name: palette.name, Version: version, ThemeAPI: "^1.0.0", Core: ">=0.4.0", Author: "Yin", License: "AGPL-3.0",
			Tokens: TokenSetV2{Format: "DTCG", Version: DTCGVersion, Docs: map[string]string{"light": "tokens/light.json", "dark": "tokens/dark.json"}}, DefaultScheme: "light",
			Entrypoints: EntrypointsV2{Script: "views/home.mjs", Styles: []string{"styles/home.css"}}, Runtime: RuntimeV2{SupportedModes: []string{"sandbox"}},
			Contributes: ContributionsV2{Views: contributedViews}, Permissions: PermissionsV2{Required: requiredPermissions}, Resources: resources,
		}
		manifestJSON, _ := json.Marshal(manifest)
		revisionFiles := resourceBytesV2(files)
		revisionFiles["manifest.json"] = manifestJSON
		for scheme, document := range docs {
			revisionFiles[manifest.Tokens.Docs[scheme]] = document
		}
		result = append(result, &PackageV2{Manifest: manifest, Tokens: docs, Files: files, Verified: true, Revision: packageRevisionV2(revisionFiles)})
	}
	return result
}

func resourceBytesV2(files map[string]ResourceData) map[string][]byte {
	result := make(map[string][]byte, len(files))
	for name, file := range files {
		if name != "manifest.json" {
			result[name] = file.Content
		}
	}
	return result
}

func builtinDTCGDocumentV2(colors map[string]string, palette builtinPaletteV2) json.RawMessage {
	semantic := map[string]any{}
	completeColors := map[string]string{"canvas": "#f4f7f8", "surface": "#ffffff", "text": "#20282c", "muted": "#64737a", "border": "#d8e0e3", "primary": "#176b80", "onPrimary": "#ffffff", "secondary": "#526b5d", "success": "#28734d", "warning": "#805500", "danger": "#a63338", "focusRing": "#176b80"}
	for name, value := range colors {
		completeColors[name] = value
	}
	for _, key := range []string{"focusRing"} {
		if _, exists := colors[key]; !exists {
			completeColors[key] = completeColors["primary"]
		}
	}
	for name, value := range completeColors {
		semantic[name] = map[string]any{"$type": "color", "$value": dtcgColorFromHexV2(value)}
	}
	parsePx := func(raw string, fallback float64) float64 {
		n, err := strconv.ParseFloat(strings.TrimSuffix(raw, "px"), 64)
		if err != nil {
			return fallback
		}
		return n
	}
	shadowParts := strings.Fields(palette.shadow)
	offsetX, offsetY, blur := 0.0, 0.0, 0.0
	shadowColor := "#18242a"
	if len(shadowParts) >= 4 {
		offsetX, _ = strconv.ParseFloat(strings.TrimSuffix(shadowParts[0], "px"), 64)
		offsetY, _ = strconv.ParseFloat(strings.TrimSuffix(shadowParts[1], "px"), 64)
		blur, _ = strconv.ParseFloat(strings.TrimSuffix(shadowParts[2], "px"), 64)
		if strings.HasPrefix(shadowParts[3], "#") {
			shadowColor = shadowParts[3]
		}
	}
	shadow := map[string]any{"color": dtcgColorFromHexV2(shadowColor), "offsetX": map[string]any{"value": offsetX, "unit": "px"}, "offsetY": map[string]any{"value": offsetY, "unit": "px"}, "blur": map[string]any{"value": blur, "unit": "px"}, "spread": map[string]any{"value": 0, "unit": "px"}}
	duration, _ := strconv.ParseFloat(strings.TrimSuffix(palette.motion, "ms"), 64)
	semantic["shape"] = map[string]any{"cardRadius": map[string]any{"$type": "dimension", "$value": map[string]any{"value": parsePx(palette.radius, 8), "unit": "px"}}, "controlRadius": map[string]any{"$type": "dimension", "$value": map[string]any{"value": parsePx(palette.radius, 8), "unit": "px"}}}
	semantic["spacing"] = map[string]any{"component": map[string]any{"$type": "dimension", "$value": map[string]any{"value": parsePx(palette.spacing, 16), "unit": "px"}}}
	semantic["elevation"] = map[string]any{"card": map[string]any{"$type": "shadow", "$value": shadow}}
	semantic["motion"] = map[string]any{"duration": map[string]any{"$type": "duration", "$value": map[string]any{"value": duration, "unit": "ms"}}}
	semantic["typography"] = map[string]any{"body": map[string]any{"$type": "fontFamily", "$value": palette.fontBody}, "bodySize": map[string]any{"$type": "dimension", "$value": map[string]any{"value": 14, "unit": "px"}}}
	component := map[string]any{"card": map[string]any{"padding": map[string]any{"$type": "dimension", "$value": map[string]any{"value": parsePx(palette.cardPadding, 16), "unit": "px"}}, "surfaceOpacity": map[string]any{"$type": "number", "$value": palette.surface}, "surfaceBlur": map[string]any{"$type": "dimension", "$value": map[string]any{"value": palette.blur, "unit": "px"}}, "surfaceGlow": map[string]any{"$type": "dimension", "$value": map[string]any{"value": palette.glow, "unit": "px"}}}, "iconography": map[string]any{"size": map[string]any{"$type": "dimension", "$value": map[string]any{"value": parsePx(palette.iconSize, 70), "unit": "px"}}}}
	density := map[string]any{"scale": map[string]any{"$type": "number", "$value": palette.density}}
	background := map[string]any{"gridOpacity": map[string]any{"$type": "number", "$value": palette.grid}, "dotsOpacity": map[string]any{"$type": "number", "$value": palette.dots}}
	shape := map[string]any{"$type": "dimension", "card": map[string]any{"$value": map[string]any{"value": parsePx(palette.radius, 8), "unit": "px"}}, "control": map[string]any{"$value": map[string]any{"value": parsePx(palette.radius, 8), "unit": "px"}}}
	document := map[string]any{"$schema": DTCGSchema202510, "semantic": semantic, "component": component, "density": density, "background": background, "shape": shape}
	encoded, _ := json.Marshal(document)
	return encoded
}

func dtcgColorFromHexV2(value string) map[string]any {
	value = strings.TrimPrefix(value, "#")
	if len(value) == 8 {
		value = value[:6]
	}
	r, _ := strconv.ParseUint(value[0:2], 16, 8)
	g, _ := strconv.ParseUint(value[2:4], 16, 8)
	b, _ := strconv.ParseUint(value[4:6], 16, 8)
	return map[string]any{"colorSpace": "srgb", "components": []float64{float64(r) / 255, float64(g) / 255, float64(b) / 255}, "alpha": 1}
}
