package repository

import (
	"encoding/json"
	"errors"
	"github.com/yinorg/Yin-Panel/backend/internal/util"

	"gorm.io/gorm"
)

type PanelConfig struct {
	BackgroundImageSrc          string   `json:"backgroundImageSrc,omitempty"`
	WallpaperMode               string   `json:"wallpaperMode,omitempty"`
	WallpaperKind               string   `json:"wallpaperKind,omitempty"`
	WallpaperSource             string   `json:"wallpaperSource,omitempty"`
	WallpaperPoster             string   `json:"wallpaperPoster,omitempty"`
	WallpaperLibraryEntry       string   `json:"wallpaperLibraryEntry,omitempty"`
	BackgroundBlur              *int     `json:"backgroundBlur,omitempty"`
	BackgroundMaskNumber        *float64 `json:"backgroundMaskNumber,omitempty"`
	IconStyle                   *int     `json:"iconStyle,omitempty"`
	IconTextColor               string   `json:"iconTextColor,omitempty"`
	IconTextInfoHideDescription *bool    `json:"iconTextInfoHideDescription,omitempty"`
	IconTextIconHideTitle       *bool    `json:"iconTextIconHideTitle,omitempty"`
	LogoText                    string   `json:"logoText,omitempty"`
	LogoImageSrc                string   `json:"logoImageSrc,omitempty"`
	ClockShowSecond             *bool    `json:"clockShowSecond,omitempty"`
	ClockColor                  string   `json:"clockColor,omitempty"`
	SearchBoxShow               *bool    `json:"searchBoxShow,omitempty"`
	SearchBoxSearchIcon         *bool    `json:"searchBoxSearchIcon,omitempty"`
	MarginTop                   *int     `json:"marginTop,omitempty"`
	MarginBottom                *int     `json:"marginBottom,omitempty"`
	MaxWidth                    *int     `json:"maxWidth,omitempty"`
	MaxWidthUnit                string   `json:"maxWidthUnit"`
	MarginX                     *int     `json:"marginX,omitempty"`
	FooterHtml                  string   `json:"footerHtml"`
	SystemMonitorShow           *bool    `json:"systemMonitorShow,omitempty"`
	SystemMonitorShowTitle      *bool    `json:"systemMonitorShowTitle,omitempty"`
	NetModeChangeButtonShow     *bool    `json:"netModeChangeButtonShow,omitempty"`
	UseThemeDefaults            bool     `json:"useThemeDefaults,omitempty"`
}

type UserConfig struct {
	// One panel configuration per user. The unique index also closes the
	// First-then-Create race in SaveUserConfig.
	UserId uint `gorm:"uniqueIndex:uk_user_config_user_id" json:"userId"`

	// 面板样式数据
	PanelJson string       `json:"-"`
	Panel     *PanelConfig `gorm:"-" json:"panel"`

	// The language the user chose, so the choice belongs to the account rather
	// than to one browser's storage. Empty means "never chosen": the client then
	// keeps whatever it already resolved, and `auto` is the explicit choice to
	// follow the browser.
	Language string `json:"language,omitempty"`
}

type UserConfigRepo struct {
}

type IUserConfigRepo interface {
	GetUserConfig(userId uint) (UserConfig, error)
	SaveUserConfig(config *UserConfig) error
}

func NewUserConfigRepo() IUserConfigRepo {
	return &UserConfigRepo{}
}

// retrieves user configuration from database by user ID
func (r *UserConfigRepo) GetUserConfig(userId uint) (UserConfig, error) {
	cfg := UserConfig{}
	if err := Db.First(&cfg, "user_id=?", userId).Error; err != nil {
		return cfg, err
	}

	// Process JSON fields
	if err := json.Unmarshal([]byte(cfg.PanelJson), &cfg.Panel); err != nil {
		cfg.Panel = nil
	}

	return cfg, nil
}

// saves user configuration to database
// It will create a new record if not exists, or update existing one
func (r *UserConfigRepo) SaveUserConfig(config *UserConfig) error {
	// Process JSON fields. Only a request that carries a panel may rewrite it: a
	// save of some other field alone would otherwise serialize the absent panel
	// as "null" and erase the wallpaper and display settings already stored. An
	// empty PanelJson is a zero value, so GORM leaves the existing column alone.
	if config.Panel != nil {
		config.PanelJson = util.ToJSONString(config.Panel)
	}

	// Check if record exists
	if err := Db.First(&UserConfig{}, "user_id=?", config.UserId).Error; err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			// Create new record
			if err := Db.Create(config).Error; err != nil {
				return err
			}
		} else {
			// Database error
			return err
		}
	} else {
		// Update existing record
		if err := Db.Where("user_id=?", config.UserId).Updates(config).Error; err != nil {
			return err
		}
	}

	return nil
}
