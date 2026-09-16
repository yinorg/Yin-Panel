package panel

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/response"
	"html"
	"regexp"
	"strconv"
	"strings"
	"unicode"

	"github.com/gin-gonic/gin"
	"gorm.io/gorm"
)

type SpaceRouter struct{}
type createSpaceRequest struct {
	Name string `json:"name" binding:"required,max=100"`
}

func NewSpaceRouter() *SpaceRouter { return &SpaceRouter{} }
func (r *SpaceRouter) InitRouter(router *gin.RouterGroup) {
	g := router.Group("/spaces")
	g.Use(interceptor.Auth)
	g.GET("", r.List)
	g.GET("/:spaceId/groups", r.Groups)
	g.GET("/:spaceId/items", r.Items)
	g.POST("/:spaceId/items", r.CreateItem)
	g.PUT("/:spaceId/items/:itemId", r.UpdateItem)
	g.POST("/:spaceId/items/:itemId/update", r.UpdateItem)
	g.POST("/:spaceId/items/sort", r.SortItems)
	g.DELETE("/:spaceId/items/:itemId", r.DeleteItem)
	g.POST("/:spaceId/items/:itemId/delete", r.DeleteItem)
	g.GET("/:spaceId/bookmarks/export", r.ExportBookmarks)
	g.POST("/:spaceId/bookmarks/import", r.ImportBookmarks)
	g.POST("/:spaceId/clear", r.ClearSpace)
	g.POST("/:spaceId/groups", r.CreateGroup)
	g.PUT("/:spaceId/groups/:groupId", r.UpdateGroup)
	g.POST("/:spaceId/groups/:groupId/update", r.UpdateGroup)
	g.DELETE("/:spaceId/groups/:groupId", r.DeleteGroup)
	g.POST("/:spaceId/groups/:groupId/delete", r.DeleteGroup)
	g.GET("/:spaceId/members", r.Members)
	g.POST("/teams", r.CreateTeam)
	g.POST("/shared", r.CreateTeam)
	g.PUT("/:spaceId", r.Rename)
	// The frontend request wrapper uses POST for JSON mutations.
	g.POST("/:spaceId", r.Rename)
	g.POST("/:spaceId/transfer", r.Transfer)
	g.POST("/:spaceId/copy", r.Copy)
	g.GET("/:spaceId/oidc-groups", r.OIDCGroups)
	g.GET("/:spaceId/public", r.GetPublicConfig)
	g.POST("/:spaceId/public", r.SetPublicConfig)
	g.POST("/:spaceId/oidc-groups", r.AddOIDCGroup)
	g.DELETE("/:spaceId/oidc-groups/:ruleId", r.DeleteOIDCGroup)
	g.POST("/:spaceId/members", r.AddMember)
	g.PUT("/:spaceId/members/:userId", r.UpdateMember)
	g.DELETE("/:spaceId/members/:userId", r.RemoveMember)
}

type memberRequest struct {
	Email string `json:"email" binding:"required,email"`
	Role  string `json:"role" binding:"required"`
}

type renameRequest struct {
	Name string `json:"name" binding:"required,max=100"`
}
type transferRequest struct {
	Email string `json:"email" binding:"required,email"`
}
type oidcGroupRequest struct {
	Provider  string `json:"provider" binding:"required"`
	GroupName string `json:"groupName" binding:"required"`
	Role      string `json:"role" binding:"required"`
}

type publicConfigRequest struct {
	Enabled    bool   `json:"enabled"`
	PublicID   string `json:"publicId"`
	Mode       string `json:"mode"`
	AccessCode string `json:"accessCode"`
}

type bookmarkImportRequest struct {
	Groups []struct {
		Title    string `json:"title"`
		Children []struct {
			Title string `json:"title"`
			Items []struct {
				Title string `json:"title"`
				URL   string `json:"url"`
			} `json:"items"`
		} `json:"children"`
		Items []struct {
			Title string `json:"title"`
			URL   string `json:"url"`
		} `json:"items"`
	} `json:"groups"`
}

func (r *SpaceRouter) ExportBookmarks(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canAccessSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var groups []repository.ItemIconGroup
	if err = repository.Db.Where("space_id = ?", id).Order("sort, id").Find(&groups).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	var items []repository.ItemIcon
	if err = repository.Db.Where("space_id = ?", id).Order("sort, id").Find(&items).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	c.Header("Content-Type", "text/html; charset=utf-8")
	c.Header("Content-Disposition", fmt.Sprintf("attachment; filename=space-%d-bookmarks.html", id))
	c.String(200, "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Bookmarks</TITLE><H1>Bookmarks</H1><DL><p>"+
		bookmarkHTML(groups, items)+"</DL><p>")
}

func (r *SpaceRouter) ClearSpace(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canManageSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	err = repository.Db.Transaction(func(tx *gorm.DB) error {
		if err := tx.Where("space_id = ?", id).Delete(&repository.ItemIcon{}).Error; err != nil {
			return err
		}
		return tx.Where("space_id = ?", id).Delete(&repository.ItemIconGroup{}).Error
	})
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func bookmarkHTML(groups []repository.ItemIconGroup, items []repository.ItemIcon) string {
	return bookmarkHTMLForParent(groups, items, nil)
}

func bookmarkHTMLForParent(groups []repository.ItemIconGroup, items []repository.ItemIcon, parent *uint) string {
	result := ""
	for _, group := range groups {
		if (group.ParentID == nil) != (parent == nil) || (parent != nil && *group.ParentID != *parent) {
			continue
		}
		result += "<DT><H3>" + html.EscapeString(group.Title) + "</H3><DL><p>"
		for _, item := range items {
			if item.ItemIconGroupId == int(group.ID) {
				result += "<DT><A HREF=\"" + html.EscapeString(item.Url) + "\">" + html.EscapeString(item.Title) + "</A>"
			}
		}
		result += bookmarkHTMLForParent(groups, items, &group.ID)
		result += "</DL><p>"
	}
	return result
}

func (r *SpaceRouter) ImportBookmarks(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canEditSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var req bookmarkImportRequest
	if err = c.ShouldBindJSON(&req); err != nil {
		response.ErrorParamFomat(c, "invalid bookmarks")
		return
	}
	err = repository.Db.Transaction(func(tx *gorm.DB) error {
		for _, group := range req.Groups {
			if strings.TrimSpace(group.Title) == "" {
				continue
			}
			var target repository.ItemIconGroup
			if err := tx.Where("space_id = ? AND title = ?", id, group.Title).First(&target).Error; err != nil {
				target = repository.ItemIconGroup{Title: group.Title, UserId: user.ID, SpaceID: id, Sort: 9999}
				if err := tx.Create(&target).Error; err != nil {
					return err
				}
			}
			for _, imported := range group.Items {
				if strings.TrimSpace(imported.URL) == "" {
					continue
				}
				icon := repository.ItemIconIconInfo{ItemType: 4, BackgroundColor: "#2a2a2a6b"}
				data, _ := json.Marshal(icon)
				item := repository.ItemIcon{IconJson: string(data), Title: limitTitle(imported.Title), Url: imported.URL, OpenMethod: 2, ItemIconGroupId: int(target.ID), UserId: user.ID, SpaceID: id}
				item.Icon = icon
				ensureItemIcon(&item)
				iconData, _ := json.Marshal(item.Icon)
				item.IconJson = string(iconData)
				if err := tx.Create(&item).Error; err != nil {
					return err
				}
			}
		}
		return nil
	})
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

var publicIDPattern = regexp.MustCompile(`^[a-z][a-z0-9-]{4,28}[a-z0-9]$`)

func (r *SpaceRouter) GetPublicConfig(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canManageSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var space repository.Space
	if err := repository.Db.First(&space, id).Error; err != nil {
		response.ErrorDataNotFound(c)
		return
	}
	response.SuccessData(c, gin.H{"enabled": space.PublicEnabled, "publicId": space.PublicID, "mode": space.PublicMode})
}

func (r *SpaceRouter) SetPublicConfig(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canManageSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var req publicConfigRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	if req.Enabled && !publicIDPattern.MatchString(req.PublicID) {
		response.ErrorParamFomat(c, "invalid publicId")
		return
	}
	if req.Mode != "direct" && req.Mode != "code" {
		response.ErrorParamFomat(c, "invalid public mode")
		return
	}
	if req.Mode == "code" && req.Enabled && (len(req.AccessCode) < 4 || len(req.AccessCode) > 12) {
		response.ErrorParamFomat(c, "invalid access code")
		return
	}
	var publicID *string
	if req.Enabled {
		publicID = &req.PublicID
	}
	updates := map[string]any{"public_enabled": req.Enabled, "public_id": publicID, "public_mode": req.Mode}
	if req.Mode == "code" && req.AccessCode != "" {
		sum := sha256.Sum256([]byte(req.AccessCode))
		updates["public_code_hash"] = hex.EncodeToString(sum[:])
	}
	if !req.Enabled {
		updates["public_code_hash"] = ""
	}
	if err := repository.Db.Model(&repository.Space{}).Where("id = ?", id).Updates(updates).Error; err != nil {
		response.Error(c, fmt.Sprintf("save public config: %v", err))
		return
	}
	response.Success(c)
}

func (r *SpaceRouter) OIDCGroups(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canManageSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var rules []repository.SpaceOIDCGroup
	if err := repository.Db.Where("space_id = ?", id).Find(&rules).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, rules)
}
func (r *SpaceRouter) AddOIDCGroup(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canManageSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var req oidcGroupRequest
	if c.ShouldBindJSON(&req) != nil || !validSpaceRole(req.Role) || req.Role == repository.SpaceRoleAdmin {
		response.ErrorParamFomat(c, "invalid OIDC group rule")
		return
	}
	rule := repository.SpaceOIDCGroup{SpaceID: id, Provider: req.Provider, GroupName: req.GroupName, Role: req.Role}
	if err := repository.Db.Create(&rule).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, rule)
}
func (r *SpaceRouter) DeleteOIDCGroup(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canManageSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	rid, err := strconv.ParseUint(c.Param("ruleId"), 10, 32)
	if err != nil {
		response.ErrorParamFomat(c, "invalid ruleId")
		return
	}
	if err := repository.Db.Where("id = ? AND space_id = ?", rid, id).Delete(&repository.SpaceOIDCGroup{}).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func (r *SpaceRouter) Rename(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canManageSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var req renameRequest
	if c.ShouldBindJSON(&req) != nil {
		response.ErrorParamFomat(c, "invalid name")
		return
	}
	if err := repository.Db.Model(&repository.Space{}).Where("id = ?", id).Update("name", req.Name).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}
func (r *SpaceRouter) Transfer(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canManageSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var req transferRequest
	if c.ShouldBindJSON(&req) != nil {
		response.ErrorParamFomat(c, "invalid userId")
		return
	}
	var targetUser repository.User
	if repository.Db.Where("lower(mail) = ?", strings.ToLower(strings.TrimSpace(req.Email))).First(&targetUser).Error != nil {
		response.ErrorDataNotFound(c)
		return
	}
	err = repository.Db.Transaction(func(tx *gorm.DB) error {
		if err := tx.Model(&repository.Space{}).Where("id = ?", id).Update("owner_user_id", targetUser.ID).Error; err != nil {
			return err
		}
		tx.Model(&repository.SpaceMember{}).Where("space_id = ? AND user_id = ?", id, user.ID).Update("role", repository.SpaceRoleEditor)
		return tx.Model(&repository.SpaceMember{}).Where("space_id = ? AND user_id = ?", id, targetUser.ID).Assign(map[string]any{"role": repository.SpaceRoleAdmin}).FirstOrCreate(&repository.SpaceMember{SpaceID: id, UserID: targetUser.ID, Role: repository.SpaceRoleAdmin}).Error
	})
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}
func (r *SpaceRouter) Copy(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canAccessSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var source repository.Space
	if repository.Db.First(&source, id).Error != nil {
		response.ErrorDataNotFound(c)
		return
	}
	var req renameRequest
	_ = c.ShouldBindJSON(&req)
	name := req.Name
	if name == "" {
		name = source.Name + " Copy"
	}
	err = repository.Db.Transaction(func(tx *gorm.DB) error {
		dst := repository.Space{Type: repository.SpaceTypeShared, Name: name, OwnerUserID: user.ID}
		if err := tx.Create(&dst).Error; err != nil {
			return err
		}
		if err := tx.Create(&repository.SpaceMember{SpaceID: dst.ID, UserID: user.ID, Role: repository.SpaceRoleAdmin}).Error; err != nil {
			return err
		}
		var groups []repository.ItemIconGroup
		if err := tx.Where("space_id = ?", id).Find(&groups).Error; err != nil {
			return err
		}
		for _, g := range groups {
			old := g.ID
			g.ID = 0
			g.SpaceID = dst.ID
			g.UserId = user.ID
			if err := tx.Create(&g).Error; err != nil {
				return err
			}
			var items []repository.ItemIcon
			if err := tx.Where("space_id = ? AND item_icon_group_id = ?", id, old).Find(&items).Error; err != nil {
				return err
			}
			for _, item := range items {
				item.ID = 0
				item.SpaceID = dst.ID
				item.UserId = user.ID
				item.ItemIconGroupId = int(g.ID)
				if err := tx.Create(&item).Error; err != nil {
					return err
				}
			}
		}
		return nil
	})
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func spaceID(c *gin.Context) (uint, error) {
	v, err := strconv.ParseUint(c.Param("spaceId"), 10, 32)
	return uint(v), err
}
func canManageSpace(userID, spaceID uint) bool {
	var space repository.Space
	if repository.Db.Where("id = ?", spaceID).First(&space).Error != nil {
		return false
	}
	if space.OwnerUserID == userID {
		return true
	}
	var member repository.SpaceMember
	return repository.Db.Where("space_id = ? AND user_id = ? AND role = ?", spaceID, userID, repository.SpaceRoleAdmin).First(&member).Error == nil
}

func spaceRole(userID, spaceID uint) string {
	var space repository.Space
	if repository.Db.Where("id = ?", spaceID).First(&space).Error != nil {
		return ""
	}
	if space.OwnerUserID == userID {
		return repository.SpaceRoleAdmin
	}
	var member repository.SpaceMember
	if repository.Db.Where("space_id = ? AND user_id = ?", spaceID, userID).First(&member).Error != nil {
		return ""
	}
	return member.Role
}
func canEditSpace(userID, spaceID uint) bool {
	role := spaceRole(userID, spaceID)
	return role == repository.SpaceRoleAdmin || role == repository.SpaceRoleEditor
}

type groupRequest struct {
	Title    string `json:"title" binding:"required,max=50"`
	Icon     string `json:"icon"`
	ParentID *uint  `json:"parentId"`
}

func (r *SpaceRouter) CreateGroup(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canEditSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var req groupRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	if req.ParentID != nil {
		var parent repository.ItemIconGroup
		if repository.Db.Where("id = ? AND space_id = ?", *req.ParentID, id).First(&parent).Error != nil || *req.ParentID == 0 {
			response.ErrorParamFomat(c, "invalid parent group")
			return
		}
	}
	g := repository.ItemIconGroup{Title: req.Title, Icon: req.Icon, UserId: user.ID, SpaceID: id, ParentID: req.ParentID, Sort: 9999}
	if err := repository.Db.Create(&g).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, g)
}
func (r *SpaceRouter) UpdateGroup(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canEditSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	gid, err := strconv.ParseUint(c.Param("groupId"), 10, 32)
	if err != nil {
		response.ErrorParamFomat(c, "invalid groupId")
		return
	}
	var req groupRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	if req.ParentID != nil && (*req.ParentID == uint(gid) || !groupBelongsToSpace(*req.ParentID, id)) {
		response.ErrorParamFomat(c, "invalid parent group")
		return
	}
	if req.ParentID != nil && groupHasDescendant(id, uint(gid), *req.ParentID) {
		response.ErrorParamFomat(c, "group cycle detected")
		return
	}
	if err := repository.Db.Model(&repository.ItemIconGroup{}).Where("id = ? AND space_id = ?", gid, id).Updates(map[string]any{"title": req.Title, "icon": req.Icon, "parent_id": req.ParentID}).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func groupBelongsToSpace(groupID, spaceID uint) bool {
	var group repository.ItemIconGroup
	return repository.Db.Where("id = ? AND space_id = ?", groupID, spaceID).First(&group).Error == nil
}

func groupHasDescendant(spaceID, groupID, candidate uint) bool {
	current := candidate
	for current != 0 {
		if current == groupID {
			return true
		}
		var group repository.ItemIconGroup
		if repository.Db.Select("parent_id").Where("id = ? AND space_id = ?", current, spaceID).First(&group).Error != nil || group.ParentID == nil {
			return false
		}
		current = *group.ParentID
	}
	return false
}
func (r *SpaceRouter) DeleteGroup(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canEditSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	gid, err := strconv.ParseUint(c.Param("groupId"), 10, 32)
	if err != nil {
		response.ErrorParamFomat(c, "invalid groupId")
		return
	}
	err = repository.Db.Transaction(func(tx *gorm.DB) error {
		ids := []uint{uint(gid)}
		for i := 0; i < len(ids); i++ {
			var children []repository.ItemIconGroup
			if err := tx.Where("space_id = ? AND parent_id = ?", id, ids[i]).Find(&children).Error; err != nil {
				return err
			}
			for _, child := range children {
				ids = append(ids, child.ID)
			}
		}
		if err := tx.Where("space_id = ? AND item_icon_group_id IN ?", id, ids).Delete(&repository.ItemIcon{}).Error; err != nil {
			return err
		}
		if err := tx.Where("id IN ? AND space_id = ?", ids, id).Delete(&repository.ItemIconGroup{}).Error; err != nil {
			return err
		}
		return nil
	})
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func (r *SpaceRouter) Members(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canAccessSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var records []repository.SpaceMember
	if err := repository.Db.Where("space_id = ?", id).Find(&records).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	members := make([]map[string]any, 0, len(records))
	for _, record := range records {
		var memberUser repository.User
		repository.Db.First(&memberUser, record.UserID)
		members = append(members, map[string]any{"id": record.ID, "spaceId": record.SpaceID, "userId": record.UserID, "email": memberUser.Mail, "role": record.Role, "source": record.Source, "joinedAt": record.JoinedAt})
	}
	response.SuccessData(c, members)
}
func canAccessSpace(userID, spaceID uint) bool {
	var space repository.Space
	if repository.Db.Where("id = ?", spaceID).First(&space).Error != nil {
		return false
	}
	if space.OwnerUserID == userID {
		return true
	}
	var member repository.SpaceMember
	return repository.Db.Where("space_id = ? AND user_id = ?", spaceID, userID).First(&member).Error == nil
}
func (r *SpaceRouter) AddMember(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canManageSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var req memberRequest
	if err := c.ShouldBindJSON(&req); err != nil || !validSpaceRole(req.Role) {
		response.ErrorParamFomat(c, "invalid member request")
		return
	}
	var targetUser repository.User
	if repository.Db.Where("lower(mail) = ?", strings.ToLower(strings.TrimSpace(req.Email))).First(&targetUser).Error != nil {
		response.ErrorDataNotFound(c)
		return
	}
	m := repository.SpaceMember{SpaceID: id, UserID: targetUser.ID, Role: req.Role}
	if err := repository.Db.Where("space_id = ? AND user_id = ?", id, targetUser.ID).Assign(m).FirstOrCreate(&m).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, m)
}
func (r *SpaceRouter) UpdateMember(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canManageSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	uid, err := strconv.ParseUint(c.Param("userId"), 10, 32)
	if err != nil {
		response.ErrorParamFomat(c, "invalid userId")
		return
	}
	var req memberRequest
	if err := c.ShouldBindJSON(&req); err != nil || !validSpaceRole(req.Role) {
		response.ErrorParamFomat(c, "invalid role")
		return
	}
	if err := repository.Db.Model(&repository.SpaceMember{}).Where("space_id = ? AND user_id = ?", id, uid).Update("role", req.Role).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}
func (r *SpaceRouter) RemoveMember(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canManageSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	uid, err := strconv.ParseUint(c.Param("userId"), 10, 32)
	if err != nil {
		response.ErrorParamFomat(c, "invalid userId")
		return
	}
	if err := repository.Db.Where("space_id = ? AND user_id = ?", id, uid).Delete(&repository.SpaceMember{}).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}
func validSpaceRole(role string) bool {
	return role == repository.SpaceRoleAdmin || role == repository.SpaceRoleEditor || role == repository.SpaceRoleViewer
}
func (r *SpaceRouter) List(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	var spaces []repository.Space
	query := repository.Db.Where("owner_user_id = ? OR id IN (SELECT space_id FROM space_member WHERE user_id = ?)", user.ID, user.ID)
	if publicID, exists := c.Get("publicSpaceID"); exists {
		query = query.Where("id = ?", publicID)
	}
	err := query.Order("type, created_at").Find(&spaces).Error
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, spaces)
}

func (r *SpaceRouter) Groups(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canAccessSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var groups []repository.ItemIconGroup
	if err := repository.Db.Where("space_id = ?", id).Order("sort, created_at").Find(&groups).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, groups)
}

func (r *SpaceRouter) Items(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canAccessSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var items []repository.ItemIcon
	query := repository.Db.Where("space_id = ?", id).Order("sort, created_at")
	if groupID := c.Query("groupId"); groupID != "" {
		query = query.Where("item_icon_group_id = ?", groupID)
	}
	if page, _ := strconv.Atoi(c.Query("page")); page > 0 {
		pageSize, _ := strconv.Atoi(c.Query("pageSize"))
		if pageSize < 1 || pageSize > 200 {
			pageSize = 50
		}
		query = query.Offset((page - 1) * pageSize).Limit(pageSize)
	}
	if err := query.Find(&items).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	for i := range items {
		if items[i].IconJson != "" {
			_ = json.Unmarshal([]byte(items[i].IconJson), &items[i].Icon)
		}
	}
	response.SuccessData(c, items)
}

func (r *SpaceRouter) CreateItem(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canEditSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var item repository.ItemIcon
	if err := c.ShouldBindJSON(&item); err != nil || item.ItemIconGroupId == 0 {
		response.ErrorParamFomat(c, "invalid item or group")
		return
	}
	if !validItemTitle(item.Title) {
		response.ErrorParamFomat(c, "title must be at most 20 characters")
		return
	}
	ensureItemIcon(&item)
	var group repository.ItemIconGroup
	if repository.Db.Where("id = ? AND space_id = ?", item.ItemIconGroupId, id).First(&group).Error != nil {
		response.ErrorDataNotFound(c)
		return
	}
	item.UserId, item.SpaceID = user.ID, id
	iconJSON, err := json.Marshal(item.Icon)
	if err != nil {
		response.ErrorParamFomat(c, "invalid icon")
		return
	}
	item.IconJson = string(iconJSON)
	if err := repository.Db.Create(&item).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.SuccessData(c, item)
}
func (r *SpaceRouter) UpdateItem(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canEditSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	iid, err := strconv.ParseUint(c.Param("itemId"), 10, 32)
	if err != nil {
		response.ErrorParamFomat(c, "invalid itemId")
		return
	}
	var input repository.ItemIcon
	if err := c.ShouldBindJSON(&input); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	if input.ItemIconGroupId == 0 {
		response.ErrorParamFomat(c, "invalid groupId")
		return
	}
	if !validItemTitle(input.Title) {
		response.ErrorParamFomat(c, "title must be at most 20 characters")
		return
	}
	ensureItemIcon(&input)
	var group repository.ItemIconGroup
	if repository.Db.Where("id = ? AND space_id = ?", input.ItemIconGroupId, id).First(&group).Error != nil {
		response.ErrorDataNotFound(c)
		return
	}
	iconJSON, err := json.Marshal(input.Icon)
	if err != nil {
		response.ErrorParamFomat(c, "invalid icon")
		return
	}
	if err := repository.Db.Model(&repository.ItemIcon{}).Where("id = ? AND space_id = ?", iid, id).Updates(map[string]any{"icon_json": string(iconJSON), "title": input.Title, "url": input.Url, "lan_url": input.LanUrl, "description": input.Description, "open_method": input.OpenMethod, "sort": input.Sort, "item_icon_group_id": input.ItemIconGroupId}).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func ensureItemIcon(item *repository.ItemIcon) {
	if item.Icon.ItemType != 4 || item.Icon.Src != "" {
		return
	}
	title := []rune(strings.TrimSpace(item.Title))
	chinese := make([]rune, 0, 5)
	latin := make([]rune, 0, 8)
	for _, r := range title {
		if unicode.Is(unicode.Han, r) && len(chinese) < 5 {
			chinese = append(chinese, r)
		}
		if ((r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z')) && len(latin) < 8 {
			latin = append(latin, r)
		}
	}
	text := string(chinese)
	if text == "" {
		text = string(latin)
	}
	if text == "" {
		for _, r := range title {
			if !unicode.IsSpace(r) {
				text += string(r)
				break
			}
		}
	}
	if text == "" {
		text = "?"
	}
	item.Icon = repository.ItemIconIconInfo{ItemType: 1, Text: text, BackgroundColor: "#2a2a2a6b"}
}

func limitTitle(title string) string {
	runes := []rune(strings.TrimSpace(title))
	if len(runes) > 20 {
		runes = runes[:20]
	}
	return string(runes)
}

func validItemTitle(title string) bool {
	return len([]rune(strings.TrimSpace(title))) <= 20
}
func (r *SpaceRouter) DeleteItem(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canEditSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	iid, err := strconv.ParseUint(c.Param("itemId"), 10, 32)
	if err != nil {
		response.ErrorParamFomat(c, "invalid itemId")
		return
	}
	if err := repository.Db.Where("id = ? AND space_id = ?", iid, id).Delete(&repository.ItemIcon{}).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}

func (r *SpaceRouter) SortItems(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	id, err := spaceID(c)
	if err != nil || !canEditSpace(user.ID, id) {
		response.ErrorNoAccess(c)
		return
	}
	var req struct {
		GroupID   uint `json:"groupId"`
		SortItems []struct {
			ID   uint `json:"id"`
			Sort int  `json:"sort"`
		} `json:"sortItems"`
	}
	if c.ShouldBindJSON(&req) != nil || req.GroupID == 0 {
		response.ErrorParamFomat(c, "invalid sort request")
		return
	}
	err = repository.Db.Transaction(func(tx *gorm.DB) error {
		for _, item := range req.SortItems {
			if err := tx.Model(&repository.ItemIcon{}).Where("id = ? AND space_id = ? AND item_icon_group_id = ?", item.ID, id, req.GroupID).Update("sort", item.Sort).Error; err != nil {
				return err
			}
		}
		return nil
	})
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}
func (r *SpaceRouter) CreateTeam(c *gin.Context) {
	user, ok := base.GetCurrentUserInfo(c)
	if !ok {
		response.Error(c, "not logged in")
		return
	}
	var req createSpaceRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}
	err := repository.Db.Transaction(func(tx *gorm.DB) error {
		team := repository.Team{Name: req.Name, OwnerUserID: user.ID}
		if err := tx.Create(&team).Error; err != nil {
			return err
		}
		space := repository.Space{Type: repository.SpaceTypeTeam, Name: req.Name, OwnerUserID: user.ID, TeamID: &team.ID}
		if err := tx.Create(&space).Error; err != nil {
			return err
		}
		if err := tx.Create(&repository.SpaceMember{SpaceID: space.ID, UserID: user.ID, Role: repository.SpaceRoleAdmin}).Error; err != nil {
			return err
		}
		return tx.Create(&repository.ItemIconGroup{Title: "APP", Icon: "material-symbols:apps", Sort: 0, UserId: user.ID, SpaceID: space.ID}).Error
	})
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
}
