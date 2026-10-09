package theme

import (
	"time"

	"gorm.io/gorm"
)

// LegacyPreferenceRecord is only a read source for migrating the color mode.
// The package selection remains stored but is never activated by the v2 runtime.
type LegacyPreferenceRecord struct {
	UserID    uint   `gorm:"primaryKey"`
	PackageID string `gorm:"size:128"`
	Mode      string `gorm:"size:8"`
	UpdatedAt time.Time
}

func (LegacyPreferenceRecord) TableName() string { return "preferences" }

type AuditRecord struct {
	ID        uint      `gorm:"primaryKey" json:"id"`
	ActorID   uint      `json:"actorId"`
	Action    string    `gorm:"size:32" json:"action"`
	PackageID string    `gorm:"size:128" json:"packageId"`
	CreatedAt time.Time `gorm:"index:idx_audit_record_created_at" json:"createdAt"`
}

func Migrate(db *gorm.DB) error {
	if err := db.AutoMigrate(&LegacyPreferenceRecord{}, &AuditRecord{}, &WebWallpaperRecord{}); err != nil {
		return err
	}
	return migrateRevisionV2(db)
}
