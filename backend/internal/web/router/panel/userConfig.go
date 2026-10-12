package panel

import (
	"errors"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/theme/wallpaperpack"
	"github.com/yinorg/Yin-Panel/backend/internal/constant"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/response"
	"net/url"
	"regexp"
	"strings"

	"github.com/gin-gonic/gin/binding"
	"gorm.io/gorm"

	"github.com/gin-gonic/gin"
)

// themeWallpaperEntryV2 matches a wallpaper entry a theme's own catalogue
// declared: `<packageId>:<entryId>`, both dot/dash/underscore-safe. It is a
// shape check only — the Core cannot know another package's entries — and the
// layer falls back to the theme wallpaper when one no longer resolves.
var themeWallpaperEntryV2 = regexp.MustCompile(`^[A-Za-z0-9][A-Za-z0-9._-]{0,63}:[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`)

// supportedLanguages mirrors the list the frontend offers
// (frontend/src/store/modules/app/helper.ts). A tag the client cannot resolve
// would silently drop the user onto the fallback locale, so an unknown one is
// rejected rather than stored. `auto` is the explicit choice to follow the
// browser, and an empty value means the user has never chosen.
var supportedLanguages = map[string]bool{
	"auto": true,
	"en-US": true, "zh-CN": true, "zh-TW": true, "ja-JP": true, "ko-KR": true,
	"de-DE": true, "fr-FR": true, "es-ES": true, "pt-BR": true, "ru-RU": true,
	"id-ID": true, "tr-TR": true, "pl-PL": true, "it-IT": true, "nl-NL": true,
	"hi-IN": true, "th-TH": true,
}

type UserConfigRouter struct {
}

func NewUserConfigRouter() *UserConfigRouter {
	return &UserConfigRouter{}
}

func (a *UserConfigRouter) InitRouter(router *gin.RouterGroup) {
	r := router.Group("")
	r.Use(interceptor.Auth)
	{
		r.POST("/panel/userConfig/setConfig", a.SetConfig)
		r.GET("/panel/userConfig/getConfig", a.GetConfig)
	}
}

func (a *UserConfigRouter) GetConfig(c *gin.Context) {
	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	cfg, err := global.UserConfigRepo.GetUserConfig(userInfo.ID)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			response.ErrorDataNotFound(c)
			return
		}

		response.ErrorDatabase(c, err.Error())
		return
	}
	if _, public := c.Get("publicSpaceID"); public {
		response.SuccessData(c, gin.H{"panel": publicPanelConfig(cfg.Panel)})
		return
	}

	response.SuccessData(c, cfg)
}

func (a *UserConfigRouter) SetConfig(c *gin.Context) {
	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	req := repository.UserConfig{}

	if err := c.ShouldBindBodyWith(&req, binding.JSON); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	if req.Language != "" && !supportedLanguages[req.Language] {
		response.ErrorParamFomat(c, "invalid language")
		return
	}
	if req.Panel != nil {
		wallpaper := req.Panel
		if wallpaper.WallpaperMode != "" && wallpaper.WallpaperMode != "theme" && wallpaper.WallpaperMode != "custom" && wallpaper.WallpaperMode != "library" && wallpaper.WallpaperMode != "none" {
			response.ErrorParamFomat(c, "invalid wallpaper mode")
			return
		}
		// A library selection only names one of the Core's packaged entries, so
		// the library itself is the validation: an unknown entry is rejected
		// rather than stored and silently ignored later. A theme's own catalogue
		// entry is namespaced with its package id and cannot be checked here, so
		// only its shape is enforced and the layer falls back if it is gone.
		if wallpaper.WallpaperMode == "library" {
			entry := wallpaper.WallpaperLibraryEntry
			if _, ok := wallpaperpack.Find(entry); !ok && !themeWallpaperEntryV2.MatchString(entry) {
				response.ErrorParamFomat(c, "invalid wallpaper library entry")
				return
			}
		}
		if wallpaper.WallpaperMode == "custom" {
			if wallpaper.WallpaperKind != "image" && wallpaper.WallpaperKind != "video" && wallpaper.WallpaperKind != "webBundle" && wallpaper.WallpaperKind != "externalUrl" {
				response.ErrorParamFomat(c, "invalid wallpaper kind")
				return
			}
			if wallpaper.WallpaperKind == "webBundle" {
				const prefix = "/api/theme/v2/wallpaper/web/"
				id := strings.TrimSuffix(strings.TrimPrefix(wallpaper.WallpaperSource, prefix), "/index.html")
				if !strings.HasPrefix(wallpaper.WallpaperSource, prefix) || wallpaper.WallpaperSource != prefix+id+"/index.html" || len(id) != 64 || strings.Trim(id, "0123456789abcdef") != "" {
					response.ErrorParamFomat(c, "invalid web wallpaper source")
					return
				}
			}
			if wallpaper.WallpaperKind == "externalUrl" {
				u, err := url.Parse(wallpaper.WallpaperSource)
				if err != nil || u.Scheme != "https" || u.Hostname() == "" || u.User != nil || u.Fragment != "" || len(wallpaper.WallpaperSource) > 2048 {
					response.ErrorParamFomat(c, "external wallpaper requires an HTTPS URL")
					return
				}
			} else if wallpaper.WallpaperSource != "" && !strings.HasPrefix(wallpaper.WallpaperSource, "/") {
				u, err := url.Parse(wallpaper.WallpaperSource)
				if err != nil || (u.Scheme != "http" && u.Scheme != "https") || u.Hostname() == "" || u.User != nil || wallpaper.WallpaperKind == "webBundle" {
					response.ErrorParamFomat(c, "invalid wallpaper source")
					return
				}
			}
		}
	}

	// Set user ID
	req.UserId = userInfo.ID

	// Save to database
	if err := global.UserConfigRepo.SaveUserConfig(&req); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}

	response.Success(c)
}
