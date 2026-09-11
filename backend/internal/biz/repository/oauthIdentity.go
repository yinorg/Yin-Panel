package repository

// OAuthIdentity stores a stable identity-provider subject separately from the
// local user account. A user may have identities from multiple providers.
type OAuthIdentity struct {
	BaseModel
	UserID   uint   `gorm:"index;not null"`
	Provider string `gorm:"type:varchar(50);not null;uniqueIndex:uk_oauth_identity"`
	Subject  string `gorm:"type:varchar(255);not null;uniqueIndex:uk_oauth_identity"`
	Email    string `gorm:"type:varchar(255)"`
}
