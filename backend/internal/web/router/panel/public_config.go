package panel

import "github.com/yinorg/Yin-Panel/backend/internal/biz/repository"

func publicPanelConfig(source *repository.PanelConfig) *repository.PanelConfig {
	if source == nil {
		return nil
	}
	monitorDisabled := false
	return &repository.PanelConfig{
		HomeLayout:                  source.HomeLayout,
		BackgroundImageSrc:          source.BackgroundImageSrc,
		WallpaperMode:               source.WallpaperMode,
		WallpaperKind:               source.WallpaperKind,
		WallpaperSource:             source.WallpaperSource,
		WallpaperPoster:             source.WallpaperPoster,
		BackgroundBlur:              source.BackgroundBlur,
		BackgroundMaskNumber:        source.BackgroundMaskNumber,
		IconStyle:                   source.IconStyle,
		IconTextColor:               source.IconTextColor,
		IconTextInfoHideDescription: source.IconTextInfoHideDescription,
		IconTextIconHideTitle:       source.IconTextIconHideTitle,
		LogoText:                    source.LogoText,
		LogoImageSrc:                source.LogoImageSrc,
		ClockShowSecond:             source.ClockShowSecond,
		ClockColor:                  source.ClockColor,
		SearchBoxShow:               source.SearchBoxShow,
		SearchBoxSearchIcon:         source.SearchBoxSearchIcon,
		MarginTop:                   source.MarginTop,
		MarginBottom:                source.MarginBottom,
		MaxWidth:                    source.MaxWidth,
		MaxWidthUnit:                source.MaxWidthUnit,
		MarginX:                     source.MarginX,
		SystemMonitorShow:           &monitorDisabled,
		SystemMonitorShowTitle:      &monitorDisabled,
		UseThemeDefaults:            source.UseThemeDefaults,
	}
}
