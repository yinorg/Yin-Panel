package panel

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"regexp"
	"strconv"
	"strings"
	"yin-panel/internal/biz/repository"
	"yin-panel/internal/web/interceptor"
	"yin-panel/internal/web/model/base"
	"yin-panel/internal/web/model/response"

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
	updates := map[string]any{"public_enabled": req.Enabled, "public_id": req.PublicID, "public_mode": req.Mode}
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
	Title string `json:"title" binding:"required,max=50"`
	Icon  string `json:"icon"`
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
	g := repository.ItemIconGroup{Title: req.Title, Icon: req.Icon, UserId: user.ID, SpaceID: id, Sort: 9999}
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
	if err := repository.Db.Model(&repository.ItemIconGroup{}).Where("id = ? AND space_id = ?", gid, id).Updates(map[string]any{"title": req.Title, "icon": req.Icon}).Error; err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}
	response.Success(c)
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
		if err := tx.Where("id = ? AND space_id = ?", gid, id).Delete(&repository.ItemIconGroup{}).Error; err != nil {
			return err
		}
		return tx.Where("item_icon_group_id = ? AND space_id = ?", gid, id).Delete(&repository.ItemIcon{}).Error
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
	runes := []rune(strings.TrimSpace(item.Title))
	if len(runes) > 5 {
		runes = runes[:5]
	}
	text := string(runes)
	if text == "" {
		text = "?"
	}
	item.Icon = repository.ItemIconIconInfo{ItemType: 1, Text: text, BackgroundColor: "#2a2a2a6b"}
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
