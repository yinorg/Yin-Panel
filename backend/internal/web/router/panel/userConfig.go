package panel

import (
	"errors"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/constant"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/response"
	"net/url"
	"strings"

	"github.com/gin-gonic/gin/binding"
	"gorm.io/gorm"

	"github.com/gin-gonic/gin"
)

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
	if req.Panel != nil {
		wallpaper := req.Panel
		if wallpaper.WallpaperMode != "" && wallpaper.WallpaperMode != "theme" && wallpaper.WallpaperMode != "custom" && wallpaper.WallpaperMode != "none" {
			response.ErrorParamFomat(c, "invalid wallpaper mode")
			return
		}
		if wallpaper.WallpaperMode == "custom" {
			if wallpaper.WallpaperKind != "image" && wallpaper.WallpaperKind != "video" && wallpaper.WallpaperKind != "webBundle" && wallpaper.WallpaperKind != "externalUrl" {
				response.ErrorParamFomat(c, "invalid wallpaper kind")
				return
			}
			if wallpaper.WallpaperKind == "webBundle" {
				const prefix = "/api/theme/wallpaper/web/"
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
