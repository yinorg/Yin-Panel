package repository

import "time"

const (
	SpaceTypePersonal = "personal"
	SpaceTypeShared   = "shared"
	SpaceTypeTeam     = SpaceTypeShared // backward-compatible alias

	SpaceRoleAdmin  = "admin"
	SpaceRoleEditor = "editor"
	SpaceRoleViewer = "viewer"
)

// Space is the common container for personal and team panels.
type Space struct {
	BaseModel
	Type           string  `gorm:"type:varchar(20);not null;index" json:"type"`
	Name           string  `gorm:"type:varchar(100);not null" json:"name"`
	OwnerUserID    uint    `gorm:"not null;index" json:"ownerUserId"`
	TeamID         *uint   `gorm:"index" json:"teamId,omitempty"`
	PublicEnabled  bool    `gorm:"not null;default:false" json:"publicEnabled"`
	PublicID       *string `gorm:"type:varchar(30);uniqueIndex" json:"publicId,omitempty"`
	PublicMode     string  `gorm:"type:varchar(20);not null;default:'direct'" json:"publicMode"`
	PublicCodeHash string  `gorm:"type:varchar(128)" json:"-"`
}

type SpaceMember struct {
	BaseModel
	SpaceID  uint      `gorm:"not null;uniqueIndex:uk_space_member" json:"spaceId"`
	UserID   uint      `gorm:"not null;uniqueIndex:uk_space_member" json:"userId"`
	Role     string    `gorm:"type:varchar(20);not null" json:"role"`
	Source   string    `gorm:"type:varchar(20);not null;default:'manual'" json:"source"`
	JoinedAt time.Time `json:"joinedAt"`
}

type SpaceOIDCGroup struct {
	BaseModel
	SpaceID   uint   `gorm:"not null;uniqueIndex:uk_space_oidc_group" json:"spaceId"`
	Provider  string `gorm:"type:varchar(50);not null;uniqueIndex:uk_space_oidc_group" json:"provider"`
	GroupName string `gorm:"type:varchar(255);not null;uniqueIndex:uk_space_oidc_group" json:"groupName"`
	Role      string `gorm:"type:varchar(20);not null" json:"role"`
}

// Team holds team-level metadata. A team's panel is represented by a Space
// with Type=team and TeamID set to this record.
type Team struct {
	BaseModel
	Name        string `gorm:"type:varchar(100);not null" json:"name"`
	OwnerUserID uint   `gorm:"not null;index" json:"ownerUserId"`
}
