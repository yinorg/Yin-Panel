package theme

import (
	"crypto/sha256"
	"encoding/json"
	"errors"
	"fmt"
	"net/url"
	"sort"
	"strings"
	"time"

	"gorm.io/gorm"
)

type PackageRecord struct {
	ID            string    `gorm:"primaryKey;size:128" json:"id"`
	Name          string    `gorm:"size:100" json:"name"`
	Version       string    `gorm:"size:80" json:"version"`
	AssetVersion  string    `gorm:"size:64" json:"-"`
	ManifestJSON  string    `gorm:"type:text" json:"-"`
	DocumentsJSON string    `gorm:"type:text" json:"-"`
	Verified      bool      `json:"verified"`
	CreatedAt     time.Time `json:"createdAt"`
	UpdatedAt     time.Time `json:"updatedAt"`
}

type AssetRecord struct {
	PackageID string `gorm:"primaryKey;size:128" json:"-"`
	Path      string `gorm:"primaryKey;size:240" json:"-"`
	MediaType string `gorm:"size:80" json:"mediaType"`
	Content   []byte `gorm:"type:longblob" json:"-"`
}

type Preference struct {
	UserID    uint      `gorm:"primaryKey" json:"userId"`
	PackageID string    `gorm:"size:128" json:"packageId"`
	Mode      string    `gorm:"size:8" json:"mode"`
	UpdatedAt time.Time `json:"updatedAt"`
}

type InstanceSettings struct {
	ID             uint   `gorm:"primaryKey"`
	DefaultPackage string `gorm:"size:128"`
}

type AuditRecord struct {
	ID        uint      `gorm:"primaryKey" json:"id"`
	ActorID   uint      `json:"actorId"`
	Action    string    `gorm:"size:32" json:"action"`
	PackageID string    `gorm:"size:128" json:"packageId"`
	CreatedAt time.Time `json:"createdAt"`
}

type PublicPackage struct {
	Manifest  Manifest                   `json:"manifest"`
	Documents map[string]json.RawMessage `json:"documents"`
	Verified  bool                       `json:"verified"`
}

func Migrate(db *gorm.DB) error {
	return db.AutoMigrate(&PackageRecord{}, &AssetRecord{}, &Preference{}, &InstanceSettings{}, &AuditRecord{}, &WebWallpaperRecord{})
}

func EnsureBuiltin(db *gorm.DB) error {
	builtin := Builtin()
	manifest, _ := json.Marshal(builtin.Manifest)
	documents, _ := json.Marshal(builtin.Documents)
	return db.Transaction(func(tx *gorm.DB) error {
		var existing PackageRecord
		if err := tx.First(&existing, "id = ?", builtin.Manifest.ID).Error; errors.Is(err, gorm.ErrRecordNotFound) {
			record := PackageRecord{ID: builtin.Manifest.ID, Name: builtin.Manifest.Name, Version: builtin.Manifest.PackageVersion, AssetVersion: packageRevision(builtin), ManifestJSON: string(manifest), DocumentsJSON: string(documents), Verified: true}
			if err := tx.Create(&record).Error; err != nil {
				return err
			}
		} else if err != nil {
			return err
		} else if existing.Version != builtin.Manifest.PackageVersion || existing.ManifestJSON != string(manifest) || existing.DocumentsJSON != string(documents) {
			if err := tx.Model(&existing).Updates(map[string]any{
				"name": builtin.Manifest.Name, "version": builtin.Manifest.PackageVersion,
				"asset_version": packageRevision(builtin), "manifest_json": string(manifest),
				"documents_json": string(documents), "verified": true,
			}).Error; err != nil {
				return err
			}
		}
		settings := InstanceSettings{ID: 1, DefaultPackage: builtin.Manifest.ID}
		if err := tx.FirstOrCreate(&settings, InstanceSettings{ID: 1}).Error; err != nil {
			return err
		}
		return nil
	})
}

func List(db *gorm.DB) ([]PackageRecord, error) {
	var records []PackageRecord
	err := db.Select("id", "name", "version", "verified", "created_at", "updated_at").Order("id").Find(&records).Error
	return records, err
}

func Get(db *gorm.DB, id string) (PublicPackage, error) {
	var record PackageRecord
	if err := db.First(&record, "id = ?", id).Error; err != nil {
		return PublicPackage{}, err
	}
	var result PublicPackage
	if err := json.Unmarshal([]byte(record.ManifestJSON), &result.Manifest); err != nil {
		return result, err
	}
	for i := range result.Manifest.Resources {
		resource := &result.Manifest.Resources[i]
		parts := strings.Split(resource.Path, "/")
		for j := range parts {
			parts[j] = url.PathEscape(parts[j])
		}
		resource.URL = "/api/theme/assets/" + url.PathEscape(record.ID) + "/" + record.AssetVersion + "/" + strings.Join(parts, "/")
	}
	if err := json.Unmarshal([]byte(record.DocumentsJSON), &result.Documents); err != nil {
		return result, err
	}
	result.Verified = record.Verified
	return result, nil
}

func DefaultID(db *gorm.DB) (string, error) {
	var settings InstanceSettings
	err := db.First(&settings, 1).Error
	return settings.DefaultPackage, err
}

func Current(db *gorm.DB) (PublicPackage, error) {
	id, err := DefaultID(db)
	if err != nil {
		return PublicPackage{}, err
	}
	return Get(db, id)
}

func PreferenceFor(db *gorm.DB, userID uint) (Preference, error) {
	var preference Preference
	err := db.First(&preference, "user_id = ?", userID).Error
	if errors.Is(err, gorm.ErrRecordNotFound) {
		return Preference{UserID: userID, Mode: "auto"}, nil
	}
	return preference, err
}

func UserPackage(db *gorm.DB, userID uint) (PublicPackage, Preference, error) {
	preference, err := PreferenceFor(db, userID)
	if err != nil {
		return PublicPackage{}, preference, err
	}
	id := preference.PackageID
	if id == "" {
		id, err = DefaultID(db)
		if err != nil {
			return PublicPackage{}, preference, err
		}
	}
	pkg, err := Get(db, id)
	return pkg, preference, err
}

func SetPreference(db *gorm.DB, userID uint, packageID, mode string) error {
	if mode != "light" && mode != "dark" && mode != "auto" {
		return errors.New("mode must be light, dark, or auto")
	}
	if packageID != "" {
		var count int64
		if err := db.Model(&PackageRecord{}).Where("id = ?", packageID).Count(&count).Error; err != nil {
			return err
		}
		if count == 0 {
			return gorm.ErrRecordNotFound
		}
	}
	return db.Transaction(func(tx *gorm.DB) error {
		preference := Preference{UserID: userID, PackageID: packageID, Mode: mode, UpdatedAt: time.Now()}
		if err := tx.Save(&preference).Error; err != nil {
			return err
		}
		return tx.Create(&AuditRecord{ActorID: userID, Action: "select", PackageID: packageID}).Error
	})
}

func Install(db *gorm.DB, actorID uint, pkg *Package) error {
	manifest, err := json.Marshal(pkg.Manifest)
	if err != nil {
		return err
	}
	documents, err := json.Marshal(pkg.Documents)
	if err != nil {
		return err
	}
	return db.Transaction(func(tx *gorm.DB) error {
		var existing PackageRecord
		err := tx.First(&existing, "id = ?", pkg.Manifest.ID).Error
		if err == nil && existing.Version == pkg.Manifest.PackageVersion {
			return fmt.Errorf("package version %s is immutable; increment packageVersion for an upgrade", existing.Version)
		}
		if err != nil && !errors.Is(err, gorm.ErrRecordNotFound) {
			return err
		}
		record := PackageRecord{ID: pkg.Manifest.ID, Name: pkg.Manifest.Name, Version: pkg.Manifest.PackageVersion, AssetVersion: packageRevision(pkg), ManifestJSON: string(manifest), DocumentsJSON: string(documents), Verified: pkg.Verified}
		if err := tx.Save(&record).Error; err != nil {
			return err
		}
		if err := tx.Where("package_id = ?", pkg.Manifest.ID).Delete(&AssetRecord{}).Error; err != nil {
			return err
		}
		for name, asset := range pkg.Resources {
			row := AssetRecord{PackageID: pkg.Manifest.ID, Path: name, MediaType: asset.MediaType, Content: asset.Content}
			if err := tx.Create(&row).Error; err != nil {
				return err
			}
		}
		action := "install"
		if err == nil {
			action = "upgrade"
		}
		return tx.Create(&AuditRecord{ActorID: actorID, Action: action, PackageID: pkg.Manifest.ID}).Error
	})
}

func SetDefault(db *gorm.DB, actorID uint, packageID string) error {
	return SetDefaultConfirmed(db, actorID, packageID, false)
}

func SetDefaultConfirmed(db *gorm.DB, actorID uint, packageID string, confirmExternal bool) error {
	return db.Transaction(func(tx *gorm.DB) error {
		var record PackageRecord
		if err := tx.First(&record, "id = ?", packageID).Error; err != nil {
			return err
		}
		var manifest Manifest
		if err := json.Unmarshal([]byte(record.ManifestJSON), &manifest); err != nil {
			return err
		}
		for _, wallpaper := range manifest.Wallpapers {
			if wallpaper.Kind == "externalUrl" && !confirmExternal {
				return errors.New("external wallpaper requires administrator confirmation")
			}
		}
		if err := tx.Model(&InstanceSettings{}).Where("id = ?", 1).Update("default_package", packageID).Error; err != nil {
			return err
		}
		action := "default"
		if confirmExternal {
			action = "default-external"
		}
		return tx.Create(&AuditRecord{ActorID: actorID, Action: action, PackageID: packageID}).Error
	})
}

func Remove(db *gorm.DB, actorID uint, packageID string) error {
	if packageID == "org.yin.default" {
		return errors.New("built-in theme cannot be removed")
	}
	return db.Transaction(func(tx *gorm.DB) error {
		var record PackageRecord
		if err := tx.First(&record, "id = ?", packageID).Error; err != nil {
			return err
		}
		if _, err := Get(tx, "org.yin.default"); err != nil {
			return err
		}
		if err := tx.Model(&InstanceSettings{}).Where("id = ? AND default_package = ?", 1, packageID).Update("default_package", "org.yin.default").Error; err != nil {
			return err
		}
		if err := tx.Model(&Preference{}).Where("package_id = ?", packageID).Update("package_id", "org.yin.default").Error; err != nil {
			return err
		}
		if err := tx.Where("package_id = ?", packageID).Delete(&AssetRecord{}).Error; err != nil {
			return err
		}
		if err := tx.Delete(&record).Error; err != nil {
			return err
		}
		return tx.Create(&AuditRecord{ActorID: actorID, Action: "remove", PackageID: packageID}).Error
	})
}

func Asset(db *gorm.DB, packageID, version, name string) (AssetRecord, error) {
	var record PackageRecord
	if err := db.First(&record, "id = ? AND asset_version = ?", packageID, version).Error; err != nil {
		return AssetRecord{}, err
	}
	var asset AssetRecord
	if err := db.First(&asset, "package_id = ? AND path = ?", packageID, name).Error; err != nil {
		return AssetRecord{}, err
	}
	return asset, nil
}

func packageRevision(pkg *Package) string {
	h := sha256.New()
	manifest, _ := json.Marshal(pkg.Manifest)
	_, _ = h.Write(manifest)
	for _, scheme := range pkg.Manifest.Schemes {
		_, _ = h.Write([]byte(scheme))
		_, _ = h.Write(pkg.Documents[scheme])
	}
	names := make([]string, 0, len(pkg.Resources))
	for name := range pkg.Resources {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		_, _ = h.Write([]byte(name))
		_, _ = h.Write(pkg.Resources[name].Content)
	}
	return fmt.Sprintf("%x", h.Sum(nil))
}

func Audit(db *gorm.DB, actorID uint, action, packageID string) error {
	if action == "" || packageID == "" {
		return fmt.Errorf("audit action and package are required")
	}
	return db.Create(&AuditRecord{ActorID: actorID, Action: action, PackageID: packageID}).Error
}
