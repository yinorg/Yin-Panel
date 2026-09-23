package system

import (
	"io"
	"net/http"
	"path"
	"strings"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/theme"
	"github.com/yinorg/Yin-Panel/backend/internal/constant"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/response"
	"gorm.io/gorm"
)

type ThemeRouter struct{}

func NewThemeRouter() *ThemeRouter { return &ThemeRouter{} }

func (a *ThemeRouter) InitRouter(router *gin.RouterGroup) {
	router.GET("/theme/current", a.Current)
	router.GET("/theme/packages", a.PublicList)
	router.GET("/theme/assets/:id/:version/*name", a.Asset)
	user := router.Group("")
	user.Use(interceptor.Auth, themeJWTOnly)
	user.GET("/theme/mine", a.Mine)
	user.POST("/theme/preference", a.SetPreference)
	admin := router.Group("")
	admin.Use(interceptor.Auth, themeJWTOnly, interceptor.AdminInterceptor)
	admin.GET("/theme/admin/packages", a.List)
	admin.POST("/theme/admin/install", a.Install)
	admin.POST("/theme/admin/default", a.SetDefault)
	admin.DELETE("/theme/admin/packages/:id", a.Remove)
	admin.GET("/theme/admin/audit", a.Audit)
}

func themeJWTOnly(c *gin.Context) {
	if c.GetString("authMethod") != "jwt" {
		response.ErrorNoAccess(c)
		c.Abort()
	}
}

func (a *ThemeRouter) Current(c *gin.Context) {
	c.Header("Cache-Control", "no-store")
	pkg, err := theme.Current(repository.Db)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, pkg)
}

func (a *ThemeRouter) PublicList(c *gin.Context) {
	list, err := theme.List(repository.Db)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, list)
}

func (a *ThemeRouter) Mine(c *gin.Context) {
	c.Header("Cache-Control", "no-store")
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}
	pkg, preference, err := theme.UserPackage(repository.Db, user.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, gin.H{"package": pkg, "preference": preference})
}

func (a *ThemeRouter) SetPreference(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}
	var req struct {
		PackageID string `json:"packageId"`
		Mode      string `json:"mode"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	if err := theme.SetPreference(repository.Db, user.ID, req.PackageID, req.Mode); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) List(c *gin.Context) {
	list, err := theme.List(repository.Db)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	defaultID, err := theme.DefaultID(repository.Db)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, gin.H{"packages": list, "defaultPackage": defaultID})
}

func (a *ThemeRouter) Install(c *gin.Context) {
	c.Request.Body = http.MaxBytesReader(c.Writer, c.Request.Body, theme.MaxArchive+(1<<20))
	if err := c.Request.ParseMultipartForm(theme.MaxArchive + (1 << 20)); err != nil {
		response.ErrorParamFomat(c, "invalid multipart theme upload")
		return
	}
	file, _, err := c.Request.FormFile("package")
	if err != nil {
		response.ErrorParamFomat(c, "theme package file is required")
		return
	}
	defer file.Close()
	data, err := io.ReadAll(io.LimitReader(file, theme.MaxArchive+1))
	if err != nil || len(data) > theme.MaxArchive {
		response.ErrorParamFomat(c, "theme package exceeds 10 MiB")
		return
	}
	confirm := strings.EqualFold(c.PostForm("confirmUnverified"), "true")
	pkg, err := theme.ParseArchive(data, confirm)
	if err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	user, _ := base.GetCurrentUserInfo(c)
	if err := theme.Install(repository.Db, user.ID, pkg); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, gin.H{"id": pkg.Manifest.ID, "verified": pkg.Verified})
}

func (a *ThemeRouter) SetDefault(c *gin.Context) {
	var req struct {
		PackageID string `json:"packageId"`
	}
	if err := c.ShouldBindJSON(&req); err != nil || req.PackageID == "" {
		response.ErrorParamFomat(c, "packageId is required")
		return
	}
	user, _ := base.GetCurrentUserInfo(c)
	if err := theme.SetDefault(repository.Db, user.ID, req.PackageID); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) Remove(c *gin.Context) {
	user, _ := base.GetCurrentUserInfo(c)
	if err := theme.Remove(repository.Db, user.ID, c.Param("id")); err != nil {
		if err == gorm.ErrRecordNotFound {
			response.ErrorDataNotFound(c)
		} else {
			response.ErrorParamFomat(c, err.Error())
		}
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) Audit(c *gin.Context) {
	var rows []theme.AuditRecord
	if err := repository.Db.Order("created_at desc").Limit(200).Find(&rows).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, rows)
}

func (a *ThemeRouter) Asset(c *gin.Context) {
	name := strings.TrimPrefix(c.Param("name"), "/")
	if name == "" || path.Clean(name) != name || strings.HasPrefix(name, "../") || strings.Contains(name, "\\") {
		c.Status(http.StatusNotFound)
		return
	}
	asset, err := theme.Asset(repository.Db, c.Param("id"), c.Param("version"), name)
	if err != nil {
		c.Status(http.StatusNotFound)
		return
	}
	c.Header("Cache-Control", "public, max-age=31536000, immutable")
	c.Data(http.StatusOK, asset.MediaType, asset.Content)
}
