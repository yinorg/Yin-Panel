package theme

import (
	"time"

	"gorm.io/gorm"
)

type AuditRecord struct {
	ID        uint      `gorm:"primaryKey" json:"id"`
	ActorID   uint      `json:"actorId"`
	Action    string    `gorm:"size:32" json:"action"`
	PackageID string    `gorm:"size:128" json:"packageId"`
	CreatedAt time.Time `gorm:"index:idx_audit_record_created_at" json:"createdAt"`
}

func Migrate(db *gorm.DB) error {
	if err := db.AutoMigrate(&AuditRecord{}, &WebWallpaperRecord{}); err != nil {
		return err
	}
	return migrateRevisionV2(db)
}
