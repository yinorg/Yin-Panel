package repository

type SystemSetting struct {
	ID          uint   `gorm:"primaryKey"`
	ConfigName  string `gorm:"type:varchar(50);uniqueIndex:uk_system_setting_config_name"`
	ConfigValue string `gorm:"type:text"`
}
