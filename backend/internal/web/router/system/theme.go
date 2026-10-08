package system

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"path"
	"strconv"
	"strings"
	"time"

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
	router.GET("/theme/v2/current", a.CurrentV2)
	router.GET("/theme/v2/effective", interceptor.OptionalAuth, a.EffectiveV2)
	router.GET("/theme/v2/packages", a.PublicListV2)
	router.GET("/theme/v2/package/:id", a.PackageByIDV2)
	router.GET("/theme/v2/revisions/:revision", a.PackageV2)
	router.GET("/theme/v2/preview/:token", a.PreviewPackageV2)
	router.GET("/theme/v2/preview/:token/assets/*name", a.PreviewAssetV2)
	router.GET("/theme/v2/assets/:revision/*name", a.AssetV2)
	router.GET("/theme/v2/wallpaper/web/:id/:name", a.WebWallpaperAsset)
	user := router.Group("")
	user.Use(interceptor.Auth, themeJWTOnly)
	user.GET("/theme/v2/mine", a.Mine)
	user.GET("/theme/v2/preference", a.Preference)
	user.POST("/theme/v2/preference", a.SetPreference)
	user.POST("/theme/v2/wallpaper/web", a.UploadWebWallpaper)
	user.GET("/theme/v2/grants/:revision", a.ThemeGrantV2)
	user.POST("/theme/v2/grants/:revision", a.SetThemeGrantV2)
	user.DELETE("/theme/v2/grants/:revision", a.RevokeThemeGrantV2)
	user.GET("/theme/v2/space/:spaceId", a.SpaceThemeV2)
	user.PUT("/theme/v2/space/:spaceId", a.SetSpaceThemeV2)
	user.DELETE("/theme/v2/space/:spaceId", a.ClearSpaceThemeV2)
	user.PUT("/theme/v2/space/:spaceId/mine", a.SetUserSpaceThemeV2)
	user.DELETE("/theme/v2/space/:spaceId/mine", a.ClearUserSpaceThemeV2)
	admin := router.Group("")
	admin.Use(interceptor.Auth, themeJWTOnly, interceptor.AdminInterceptor)
	admin.GET("/theme/v2/admin/packages", a.RevisionsV2)
	admin.POST("/theme/v2/admin/install", a.InstallV2)
	admin.POST("/theme/v2/admin/preview", a.PreviewV2)
	admin.GET("/theme/v2/admin/revisions", a.RevisionsV2)
	admin.POST("/theme/v2/admin/default", a.SetDefaultV2)
	admin.POST("/theme/v2/admin/trial", a.BeginTrialV2)
	admin.POST("/theme/v2/admin/confirm", a.ConfirmTrialV2)
	admin.POST("/theme/v2/admin/rollback", a.RollbackV2)
	admin.GET("/theme/v2/admin/trusted/:revision", a.TrustedRuntimePolicyV2)
	admin.PUT("/theme/v2/admin/trusted/:revision", a.SetTrustedRuntimePolicyV2)
	admin.DELETE("/theme/v2/admin/packages/:id", a.RemoveV2)
	admin.GET("/theme/v2/admin/audit", a.Audit)
}

func themeJWTOnly(c *gin.Context) {
	if c.GetString("authMethod") != "jwt" {
		response.ErrorNoAccess(c)
		c.Abort()
	}
}

func (a *ThemeRouter) CurrentV2(c *gin.Context) {
	c.Header("Cache-Control", "no-store")
	yin, err := theme.InstanceDefaultRevisionV2(repository.Db)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	instanceRevision, err := theme.ActivePackageRevisionV2(repository.Db, theme.InstanceThemeScopeV2, yin.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	revision := instanceRevision
	if c.GetString("authMethod") == "jwt" {
		user, ok := base.GetCurrentUserInfo(c)
		if !ok || user.ID == 0 {
			response.ErrorByCode(c, constant.CodeNotLogin)
			return
		}
		if userRevision, userErr := theme.ActivePackageRevisionV2(repository.Db, theme.ActivationScopeForUserV2(user.ID), instanceRevision.ID); userErr == nil {
			revision = userRevision
		} else if !errors.Is(userErr, gorm.ErrRecordNotFound) {
			response.ErrorDatabase(c, userErr.Error())
			return
		}
	}
	pkg, err := theme.PackageRevisionPublicV2(repository.Db, revision.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, pkg)
}

// EffectiveV2 resolves which theme a viewer sees for a space, applying the full
// precedence chain. Anonymous visitors (public links) and signed-in users share
// this endpoint; only the chain differs.
func (a *ThemeRouter) EffectiveV2(c *gin.Context) {
	c.Header("Cache-Control", "no-store")
	var spaceID uint
	if raw := strings.TrimSpace(c.Query("spaceId")); raw != "" {
		parsed, err := strconv.ParseUint(raw, 10, 64)
		if err != nil || parsed == 0 {
			response.ErrorParamFomat(c, "spaceId must be a positive integer")
			return
		}
		spaceID = uint(parsed)
	}
	var userID uint
	if c.GetString("authMethod") == "jwt" {
		if user, ok := base.GetCurrentUserInfo(c); ok {
			userID = user.ID
		}
	}
	resolution, err := theme.ResolveThemeV2(repository.Db, userID, spaceID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	pkg, err := theme.PackageRevisionPublicV2(repository.Db, resolution.Revision.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	mode := "auto"
	if userID != 0 {
		if userMode, modeErr := theme.UserThemeModeV2(repository.Db, userID); modeErr == nil {
			mode = userMode
		}
	}
	response.SuccessData(c, gin.H{"package": pkg, "revision": resolution.Revision.ID, "source": resolution.Source, "mode": mode})
}

func parseSpaceIDParam(c *gin.Context) (uint, bool) {
	parsed, err := strconv.ParseUint(c.Param("spaceId"), 10, 32)
	if err != nil || parsed == 0 {
		response.ErrorParamFomat(c, "spaceId must be a positive integer")
		return 0, false
	}
	return uint(parsed), true
}

// spaceAdminUserID returns the acting user when they may manage the space's
// theme (owner or space admin); otherwise it writes the refusal and returns false.
func spaceAdminUserID(c *gin.Context, spaceID uint) (uint, bool) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return 0, false
	}
	var space repository.Space
	if err := repository.Db.Select("id, owner_user_id").Where("id = ?", spaceID).First(&space).Error; err != nil {
		response.ErrorDataNotFound(c)
		return 0, false
	}
	if space.OwnerUserID == user.ID {
		return user.ID, true
	}
	var member repository.SpaceMember
	if err := repository.Db.Where("space_id = ? AND user_id = ? AND role = ?", spaceID, user.ID, repository.SpaceRoleAdmin).First(&member).Error; err == nil {
		return user.ID, true
	}
	response.ErrorNoAccess(c)
	return 0, false
}

// bindThemeSelection reads a package/revision pair from the body, resolving
// whichever half is missing. It writes the refusal and returns ok=false on error.
func bindThemeSelection(c *gin.Context) (packageID, revisionID string, ok bool) {
	var req struct {
		Revision  string `json:"revision"`
		PackageID string `json:"packageId"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return "", "", false
	}
	packageID, revisionID = req.PackageID, req.Revision
	if revisionID == "" && packageID != "" {
		latest, err := theme.LatestPackageRevisionV2(repository.Db, packageID)
		if err != nil {
			response.ErrorDataNotFound(c)
			return "", "", false
		}
		revisionID = latest.ID
	}
	if packageID == "" && revisionID != "" {
		revision, err := theme.GetPackageRevisionV2(repository.Db, revisionID)
		if err != nil {
			response.ErrorDataNotFound(c)
			return "", "", false
		}
		packageID = revision.PackageID
	}
	if packageID == "" || revisionID == "" {
		response.ErrorParamFomat(c, "packageId or revision is required")
		return "", "", false
	}
	return packageID, revisionID, true
}

// SpaceThemeV2 reports a space's theme and the caller's per-space override.
func (a *ThemeRouter) SpaceThemeV2(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}
	spaceID, ok := parseSpaceIDParam(c)
	if !ok {
		return
	}
	result := gin.H{"spaceId": spaceID, "theme": nil, "override": nil}
	if preference, err := theme.SpaceThemePreferenceForV2(repository.Db, spaceID); err == nil {
		result["theme"] = gin.H{"packageId": preference.PackageID, "revision": preference.RevisionID}
	} else if !errors.Is(err, gorm.ErrRecordNotFound) {
		response.ErrorDatabase(c, err.Error())
		return
	}
	if preference, err := theme.UserSpaceThemePreferenceForV2(repository.Db, user.ID, spaceID); err == nil {
		result["override"] = gin.H{"packageId": preference.PackageID, "revision": preference.RevisionID}
	} else if !errors.Is(err, gorm.ErrRecordNotFound) {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, result)
}

func (a *ThemeRouter) SetSpaceThemeV2(c *gin.Context) {
	spaceID, ok := parseSpaceIDParam(c)
	if !ok {
		return
	}
	actorID, ok := spaceAdminUserID(c, spaceID)
	if !ok {
		return
	}
	packageID, revisionID, ok := bindThemeSelection(c)
	if !ok {
		return
	}
	if err := theme.SetSpaceThemeV2(repository.Db, spaceID, packageID, revisionID, actorID); err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			response.ErrorDataNotFound(c)
			return
		}
		response.ErrorParamFomat(c, err.Error())
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) ClearSpaceThemeV2(c *gin.Context) {
	spaceID, ok := parseSpaceIDParam(c)
	if !ok {
		return
	}
	if _, ok := spaceAdminUserID(c, spaceID); !ok {
		return
	}
	if err := theme.ClearSpaceThemeV2(repository.Db, spaceID); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) SetUserSpaceThemeV2(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}
	spaceID, ok := parseSpaceIDParam(c)
	if !ok {
		return
	}
	packageID, revisionID, ok := bindThemeSelection(c)
	if !ok {
		return
	}
	if err := theme.SetUserSpaceThemeV2(repository.Db, user.ID, spaceID, packageID, revisionID); err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			response.ErrorDataNotFound(c)
			return
		}
		response.ErrorParamFomat(c, err.Error())
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) ClearUserSpaceThemeV2(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}
	spaceID, ok := parseSpaceIDParam(c)
	if !ok {
		return
	}
	if err := theme.ClearUserSpaceThemeV2(repository.Db, user.ID, spaceID); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) PublicListV2(c *gin.Context) {
	packages, err := theme.ListPackagesV2(repository.Db)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, packages)
}

func (a *ThemeRouter) Mine(c *gin.Context) {
	c.Header("Cache-Control", "no-store")
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}
	yin, err := theme.InstanceDefaultRevisionV2(repository.Db)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	instanceRevision, err := theme.ActivePackageRevisionV2(repository.Db, theme.InstanceThemeScopeV2, yin.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	userScope := theme.ActivationScopeForUserV2(user.ID)
	activation, activationErr := theme.GetActivationV2(repository.Db, userScope)
	selectedPackageID := ""
	if activationErr == nil {
		selectedPackageID = activation.PackageID
	} else if !errors.Is(activationErr, gorm.ErrRecordNotFound) {
		response.ErrorDatabase(c, activationErr.Error())
		return
	}
	revision, err := theme.ActivePackageRevisionV2(repository.Db, userScope, instanceRevision.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	pkg, err := theme.PackageRevisionPublicV2(repository.Db, revision.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	mode, err := theme.UserThemeModeV2(repository.Db, user.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	themeMode, err := theme.UserThemeChoiceModeV2(repository.Db, user.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, gin.H{"package": pkg, "preference": gin.H{"revision": revision.ID, "packageId": selectedPackageID, "mode": mode, "themeMode": themeMode}})
}

func (a *ThemeRouter) SetPreference(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}
	var req struct {
		Revision  string `json:"revision"`
		PackageID string `json:"packageId"`
		Mode      string `json:"mode"`
		ThemeMode string `json:"themeMode"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	// A request that carries no selection (only a themeMode change) must not
	// reset the user's chosen theme to the instance default.
	if req.Revision != "" || req.PackageID != "" || req.Mode != "" {
		if req.Mode == "" {
			req.Mode = "auto"
		}
		if req.Mode != "light" && req.Mode != "dark" && req.Mode != "auto" {
			response.ErrorParamFomat(c, "mode must be light, dark, or auto")
			return
		}
		revisionID := req.Revision
		if revisionID == "" && req.PackageID != "" {
			latest, latestErr := theme.LatestPackageRevisionV2(repository.Db, req.PackageID)
			if latestErr != nil {
				response.ErrorDataNotFound(c)
				return
			}
			revisionID = latest.ID
		}
		if revisionID == "" {
			yin, yinErr := theme.InstanceDefaultRevisionV2(repository.Db)
			if yinErr != nil {
				response.ErrorDatabase(c, yinErr.Error())
				return
			}
			instanceRevision, instanceErr := theme.ActivePackageRevisionV2(repository.Db, theme.InstanceThemeScopeV2, yin.ID)
			if instanceErr != nil {
				response.ErrorDatabase(c, instanceErr.Error())
				return
			}
			revisionID = instanceRevision.ID
		}
		revision, err := theme.GetPackageRevisionV2(repository.Db, revisionID)
		if err != nil {
			response.ErrorDataNotFound(c)
			return
		}
		if err := theme.SetUserThemeSelectionV2(repository.Db, user.ID, revision.PackageID, revision.ID, req.Mode); err != nil {
			response.ErrorParamFomat(c, err.Error())
			return
		}
	}
	// Applied last so an explicit themeMode wins over the "custom" implied by a
	// theme selection in the same request.
	if req.ThemeMode != "" {
		if err := theme.SetUserThemeChoiceV2(repository.Db, user.ID, req.ThemeMode); err != nil {
			response.ErrorParamFomat(c, err.Error())
			return
		}
	}
	response.Success(c)
}

func (a *ThemeRouter) Preference(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}
	mode, err := theme.UserThemeModeV2(repository.Db, user.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	themeMode, err := theme.UserThemeChoiceModeV2(repository.Db, user.ID)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, gin.H{"mode": mode, "themeMode": themeMode})
}

func (a *ThemeRouter) ThemeGrantV2(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}
	executionMode, ok := requestedThemeExecutionMode(c)
	if !ok {
		response.ErrorParamFomat(c, "executionMode must be sandbox or trusted")
		return
	}
	revision, err := theme.GetPackageRevisionV2(repository.Db, c.Param("revision"))
	if errors.Is(err, gorm.ErrRecordNotFound) {
		response.ErrorDataNotFound(c)
		return
	}
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	var manifest theme.PackageManifestV2
	if err := json.Unmarshal([]byte(revision.ManifestJSON), &manifest); err != nil {
		response.ErrorDatabase(c, "stored theme manifest is invalid")
		return
	}
	available := false
	for _, mode := range manifest.Runtime.SupportedModes {
		if mode == executionMode {
			available = true
			break
		}
	}
	if executionMode == "trusted" {
		enabled, err := theme.TrustedRuntimeEnabledV2(repository.Db, revision.ID)
		if err != nil {
			response.ErrorDatabase(c, err.Error())
			return
		}
		available = available && enabled
	}
	grant, err := theme.GetGrantV2(repository.Db, user.ID, revision.ID, executionMode)
	if errors.Is(err, gorm.ErrRecordNotFound) {
		permissions, builtin, builtinErr := theme.ImplicitBuiltinPermissionsV2(repository.Db, revision.ID)
		if builtinErr != nil && !errors.Is(builtinErr, gorm.ErrRecordNotFound) {
			response.ErrorDatabase(c, builtinErr.Error())
			return
		}
		if builtinErr == nil && builtin {
			if executionMode != "sandbox" {
				permissions = []string{}
			}
			response.SuccessData(c, gin.H{"revision": revision.ID, "executionMode": executionMode, "available": available, "granted": executionMode == "sandbox" && available, "permissions": permissions})
			return
		}
		response.SuccessData(c, gin.H{"revision": revision.ID, "executionMode": executionMode, "available": available, "granted": false, "permissions": []string{}})
		return
	}
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	var permissions []string
	if err := json.Unmarshal([]byte(grant.PermissionsJSON), &permissions); err != nil {
		response.ErrorDatabase(c, "stored theme grant is invalid")
		return
	}
	response.SuccessData(c, gin.H{"revision": grant.RevisionID, "executionMode": grant.ExecutionMode, "available": available, "granted": available, "permissions": permissions})
}

func (a *ThemeRouter) SetThemeGrantV2(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}
	var request struct {
		ExecutionMode string   `json:"executionMode"`
		Permissions   []string `json:"permissions"`
	}
	if err := c.ShouldBindJSON(&request); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	executionMode := request.ExecutionMode
	if executionMode == "" {
		executionMode = "sandbox"
	}
	if executionMode != "sandbox" && executionMode != "trusted" {
		response.ErrorParamFomat(c, "executionMode must be sandbox or trusted")
		return
	}
	revision, err := theme.GetPackageRevisionV2(repository.Db, c.Param("revision"))
	if err != nil {
		response.ErrorDataNotFound(c)
		return
	}
	var manifest theme.PackageManifestV2
	if err := json.Unmarshal([]byte(revision.ManifestJSON), &manifest); err != nil {
		response.ErrorDatabase(c, "stored theme manifest is invalid")
		return
	}
	if err := theme.ValidateThemeGrantV2(manifest, executionMode, request.Permissions); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	permissions, err := json.Marshal(request.Permissions)
	if err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	grant := theme.GrantRecordV2{UserID: user.ID, RevisionID: revision.ID, ExecutionMode: executionMode, PermissionsJSON: string(permissions)}
	if err := theme.SaveUserGrantV2(repository.Db, grant); err != nil {
		if errors.Is(err, theme.ErrTrustedRuntimeNotEnabledV2) {
			response.ErrorNoAccess(c)
			return
		}
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) RevokeThemeGrantV2(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}
	executionMode, ok := requestedThemeExecutionMode(c)
	if !ok {
		response.ErrorParamFomat(c, "executionMode must be sandbox or trusted")
		return
	}
	_, builtin, builtinErr := theme.ImplicitBuiltinPermissionsV2(repository.Db, c.Param("revision"))
	if executionMode == "sandbox" && builtinErr == nil && builtin {
		if err := theme.SaveGrantV2(repository.Db, theme.GrantRecordV2{UserID: user.ID, RevisionID: c.Param("revision"), ExecutionMode: "sandbox", PermissionsJSON: "[]"}); err != nil {
			response.ErrorDatabase(c, err.Error())
			return
		}
		response.Success(c)
		return
	}
	if err := theme.RevokeGrantV2(repository.Db, user.ID, c.Param("revision"), executionMode); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func requestedThemeExecutionMode(c *gin.Context) (string, bool) {
	mode := c.Query("executionMode")
	if mode == "" {
		mode = "sandbox"
	}
	return mode, mode == "sandbox" || mode == "trusted"
}

func (a *ThemeRouter) TrustedRuntimePolicyV2(c *gin.Context) {
	if _, err := theme.GetPackageRevisionV2(repository.Db, c.Param("revision")); err != nil {
		response.ErrorDataNotFound(c)
		return
	}
	enabled, err := theme.TrustedRuntimeEnabledV2(repository.Db, c.Param("revision"))
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, gin.H{"revision": c.Param("revision"), "enabled": enabled})
}

func (a *ThemeRouter) SetTrustedRuntimePolicyV2(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok || user.ID == 0 || user.Role != 1 {
		response.ErrorNoAccess(c)
		return
	}
	var request struct {
		Enabled bool `json:"enabled"`
	}
	if err := c.ShouldBindJSON(&request); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	if err := theme.SetTrustedRuntimePolicyV2(repository.Db, c.Param("revision"), request.Enabled, user.ID); err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			response.ErrorDataNotFound(c)
			return
		}
		response.ErrorParamFomat(c, err.Error())
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) InstallV2(c *gin.Context) {
	a.installPackageV2(c, false)
}

func (a *ThemeRouter) installPackageV2(c *gin.Context, preview bool) {
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
		response.ErrorParamFomat(c, "theme package exceeds 32 MiB")
		return
	}
	confirm := strings.EqualFold(c.PostForm("confirmUnverified"), "true")
	pkg, err := theme.ParsePackageArchiveV2(data, confirm)
	if err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	user, _ := base.GetCurrentUserInfo(c)
	if err := theme.InstallPackageV2(repository.Db, user.ID, pkg); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, gin.H{"id": pkg.Manifest.ID, "revision": pkg.Revision, "verified": pkg.Verified, "preview": preview})
}

func (a *ThemeRouter) PreviewV2(c *gin.Context) {
	a.previewPackageV2(c)
}

func (a *ThemeRouter) previewPackageV2(c *gin.Context) {
	c.Request.Body = http.MaxBytesReader(c.Writer, c.Request.Body, theme.MaxArchive+(1<<20))
	file, _, err := c.Request.FormFile("package")
	if err != nil {
		response.ErrorParamFomat(c, "theme package file is required")
		return
	}
	defer file.Close()
	data, err := io.ReadAll(io.LimitReader(file, theme.MaxArchive+1))
	if err != nil || len(data) > theme.MaxArchive {
		response.ErrorParamFomat(c, "theme package exceeds 32 MiB")
		return
	}
	pkg, err := theme.ParsePackageArchiveV2(data, true)
	if err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	c.Header("Cache-Control", "no-store")
	token, err := theme.SavePackagePreviewV2(pkg)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, gin.H{"token": token, "package": theme.PackagePreviewPublicV2(pkg, token)})
}

func (a *ThemeRouter) PreviewPackageV2(c *gin.Context) {
	pkg, err := theme.GetPackagePreviewV2(c.Param("token"))
	if err != nil {
		c.Status(http.StatusNotFound)
		return
	}
	c.Header("Cache-Control", "no-store")
	response.SuccessData(c, theme.PackagePreviewPublicV2(pkg, c.Param("token")))
}

func (a *ThemeRouter) PreviewAssetV2(c *gin.Context) {
	pkg, err := theme.GetPackagePreviewV2(c.Param("token"))
	if err != nil {
		c.Status(http.StatusNotFound)
		return
	}
	name := strings.TrimPrefix(c.Param("name"), "/")
	if name == "" || strings.Contains(name, "\\") || path.Clean(name) != name || name == "." || strings.HasPrefix(name, "../") {
		c.Status(http.StatusNotFound)
		return
	}
	asset, exists := pkg.Files[name]
	if !exists {
		c.Status(http.StatusNotFound)
		return
	}
	c.Header("Cache-Control", "no-store")
	c.Header("X-Content-Type-Options", "nosniff")
	c.Data(http.StatusOK, asset.MediaType, asset.Content)
}

func (a *ThemeRouter) SetDefaultV2(c *gin.Context) {
	var req struct {
		Revision  string `json:"revision"`
		PackageID string `json:"packageId"`
	}
	if err := c.ShouldBindJSON(&req); err != nil || (req.Revision == "" && req.PackageID == "") {
		response.ErrorParamFomat(c, "revision or packageId is required")
		return
	}
	revisionID := req.Revision
	if revisionID == "" {
		latest, latestErr := theme.LatestPackageRevisionV2(repository.Db, req.PackageID)
		if latestErr != nil {
			response.ErrorDataNotFound(c)
			return
		}
		revisionID = latest.ID
	}
	revision, err := theme.GetPackageRevisionV2(repository.Db, revisionID)
	if err != nil {
		response.ErrorDataNotFound(c)
		return
	}
	if err := theme.ActivatePackageV2(repository.Db, theme.InstanceThemeScopeV2, revision.PackageID, revision.ID); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) RemoveV2(c *gin.Context) {
	user, _ := base.GetCurrentUserInfo(c)
	if err := theme.RemovePackageV2(repository.Db, user.ID, c.Param("id")); err != nil {
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

func (a *ThemeRouter) AssetV2(c *gin.Context) {
	name := strings.TrimPrefix(c.Param("name"), "/")
	if name == "" || path.Clean(name) != name || strings.HasPrefix(name, "../") || strings.Contains(name, "\\") {
		c.Status(http.StatusNotFound)
		return
	}
	asset, err := theme.GetPackageAssetV2(repository.Db, c.Param("revision"), name)
	if err != nil {
		c.Status(http.StatusNotFound)
		return
	}
	serveThemeAsset(c, asset.MediaType, asset.Content, name, true)
}

func (a *ThemeRouter) PackageV2(c *gin.Context) {
	c.Header("Cache-Control", "no-store")
	pkg, err := theme.PackageRevisionPublicV2(repository.Db, c.Param("revision"))
	if err != nil {
		c.Status(http.StatusNotFound)
		return
	}
	response.SuccessData(c, pkg)
}

func (a *ThemeRouter) RevisionsV2(c *gin.Context) {
	packages, err := theme.ListPackagesV2(repository.Db)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	defaultRevision, defaultErr := theme.ActivePackageRevisionV2(repository.Db, theme.InstanceThemeScopeV2, "")
	if defaultErr != nil {
		defaultRevision, defaultErr = theme.InstanceDefaultRevisionV2(repository.Db)
	}
	if defaultErr != nil {
		response.ErrorDatabase(c, defaultErr.Error())
		return
	}
	revisions, err := theme.ListPackageRevisionsV2(repository.Db)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	activation, err := theme.GetActivationV2(repository.Db, theme.InstanceThemeScopeV2)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	revisionSummaries := make([]gin.H, 0, len(revisions))
	for _, revision := range revisions {
		revisionSummaries = append(revisionSummaries, gin.H{"id": revision.ID, "packageId": revision.PackageID, "version": revision.Version, "verified": revision.Verified, "createdAt": revision.CreatedAt})
	}
	response.SuccessData(c, gin.H{
		"packages": packages, "revisions": revisionSummaries, "defaultPackage": defaultRevision.PackageID,
		"activeRevision": activation.ActiveRevisionID, "lastGoodRevision": activation.LastGoodRevisionID,
		"pendingRevision": activation.PendingRevisionID, "trialStartedAt": activation.TrialStartedAt,
	})
}

func (a *ThemeRouter) PackageByIDV2(c *gin.Context) {
	revision, err := theme.LatestPackageRevisionV2(repository.Db, c.Param("id"))
	if err != nil {
		c.Status(http.StatusNotFound)
		return
	}
	pkg, err := theme.PackageRevisionPublicV2(repository.Db, revision.ID)
	if err != nil {
		c.Status(http.StatusNotFound)
		return
	}
	response.SuccessData(c, pkg)
}

func (a *ThemeRouter) BeginTrialV2(c *gin.Context) {
	var req struct {
		Revision string `json:"revision"`
	}
	if err := c.ShouldBindJSON(&req); err != nil || req.Revision == "" {
		response.ErrorParamFomat(c, "revision is required")
		return
	}
	revision, err := theme.GetPackageRevisionV2(repository.Db, req.Revision)
	if err != nil {
		response.ErrorDataNotFound(c)
		return
	}
	if err := theme.SetPendingActivationV2(repository.Db, theme.InstanceThemeScopeV2, revision.PackageID, revision.ID, time.Now()); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	response.SuccessData(c, gin.H{"revision": revision.ID, "expiresInSeconds": int(theme.ThemeTrialDurationV2.Seconds())})
}

func (a *ThemeRouter) ConfirmTrialV2(c *gin.Context) {
	var req struct {
		Revision string `json:"revision"`
	}
	if err := c.ShouldBindJSON(&req); err != nil || req.Revision == "" {
		response.ErrorParamFomat(c, "revision is required")
		return
	}
	if err := theme.ConfirmActivationV2(repository.Db, theme.InstanceThemeScopeV2, req.Revision); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	if err := theme.MarkActivationHealthyV2(repository.Db, theme.InstanceThemeScopeV2, req.Revision); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) RollbackV2(c *gin.Context) {
	fallback, err := theme.InstanceDefaultRevisionV2(repository.Db)
	if err == nil {
		err = theme.RollbackActivationV2(repository.Db, theme.InstanceThemeScopeV2, fallback.ID)
	}
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func (a *ThemeRouter) UploadWebWallpaper(c *gin.Context) {
	c.Request.Body = http.MaxBytesReader(c.Writer, c.Request.Body, theme.MaxWebWallpaper+(1<<20))
	file, _, err := c.Request.FormFile("package")
	if err != nil {
		response.ErrorParamFomat(c, "web wallpaper package is required")
		return
	}
	defer file.Close()
	data, err := io.ReadAll(io.LimitReader(file, theme.MaxWebWallpaper+1))
	if err != nil || len(data) > theme.MaxWebWallpaper {
		response.ErrorParamFomat(c, "web wallpaper package exceeds 6 MiB")
		return
	}
	user, _ := base.GetCurrentUserInfo(c)
	record, err := theme.SaveWebWallpaper(repository.Db, user.ID, data)
	if err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	poster := "poster.png"
	if record.PosterType == "image/jpeg" {
		poster = "poster.jpg"
	} else if record.PosterType == "image/webp" {
		poster = "poster.webp"
	}
	root := "/api/theme/v2/wallpaper/web/" + record.ID + "/"
	response.SuccessData(c, gin.H{"source": root + "index.html", "poster": root + poster})
}

func (a *ThemeRouter) WebWallpaperAsset(c *gin.Context) {
	mediaType, content, err := theme.WebWallpaperAsset(repository.Db, c.Param("id"), c.Param("name"))
	if err != nil {
		c.Status(http.StatusNotFound)
		return
	}
	serveThemeAsset(c, mediaType, content, c.Param("name"), true)
}

func serveThemeAsset(c *gin.Context, mediaType string, content []byte, name string, immutable bool) {
	c.Header("X-Content-Type-Options", "nosniff")
	if immutable {
		c.Header("Cache-Control", "public, max-age=31536000, immutable")
	} else {
		c.Header("Cache-Control", "no-store")
	}
	if mediaType == "text/html" {
		c.Header("Content-Security-Policy", "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; connect-src 'none'; font-src 'none'; media-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'")
		c.Header("Referrer-Policy", "no-referrer")
	} else if mediaType == "image/svg+xml" {
		c.Header("Content-Security-Policy", "sandbox; default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; object-src 'none'; form-action 'none'; base-uri 'none'")
		c.Header("Referrer-Policy", "no-referrer")
	}
	c.Header("Content-Type", mediaType)
	http.ServeContent(c.Writer, c.Request, path.Base(name), time.Time{}, bytes.NewReader(content))
}
