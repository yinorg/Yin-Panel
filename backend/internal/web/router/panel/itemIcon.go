package panel

import (
	"context"
	"encoding/json"
	"errors"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/constant"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"github.com/yinorg/Yin-Panel/backend/internal/util"
	"github.com/yinorg/Yin-Panel/backend/internal/util/favicon"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/param/panelApi"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/response"
	"net/url"
	"strings"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/gin-gonic/gin/binding"
)

type ItemIconRouter struct {
}

var urlPrefix string

// The page fetch and the icon download share one budget, so a slow host cannot
// outlive the request that asked for it.
const itemIconRequestTimeout = 15 * time.Second

func NewItemIconRouter() *ItemIconRouter {
	urlPrefix = global.Config.Base.URLPrefix
	return &ItemIconRouter{}
}

func (a *ItemIconRouter) InitRouter(router *gin.RouterGroup) {
	// 需要登录的路由组
	r := router.Group("")
	r.Use(interceptor.Auth)
	{
		r.POST("/panel/itemIcon/getSiteFavicon", a.GetSiteFavicon)
	}
}

func (a *ItemIconRouter) Edit(c *gin.Context) {
	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	itemIcon := repository.ItemIcon{}
	if err := c.ShouldBindBodyWith(&itemIcon, binding.JSON); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}

	if itemIcon.ItemIconGroupId == 0 {
		response.ErrorParamFomat(c, "Group is mandatory")
		return
	}

	itemIcon.UserId = userInfo.ID
	itemIcon.IconJson = util.ToJSONString(itemIcon.Icon)
	if err := global.ItemIconRepo.Save(&itemIcon); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}

	response.SuccessData(c, itemIcon)
}

func (a *ItemIconRouter) GetIcons(c *gin.Context) {
	type ParamsStruct struct {
		ItemIconGroupId uint `form:"itemIconGroupId" json:"itemIconGroupId"`
	}

	// 查询条件
	param := ParamsStruct{}
	if err := c.ShouldBind(&param); err != nil {
		response.ErrorParamFomat(c, err.Error())
		c.Abort()
		return
	}

	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	itemIcons, err := global.ItemIconRepo.GetList(userInfo.ID, param.ItemIconGroupId)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}

	for k, v := range itemIcons {
		json.Unmarshal([]byte(v.IconJson), &itemIcons[k].Icon)
	}

	response.SuccessListData(c, itemIcons, 0)
}

func (a *ItemIconRouter) Delete(c *gin.Context) {
	type RequestDeleteId struct {
		Id uint `json:"id" binding:"required"`
	}

	req := RequestDeleteId{}
	if err := c.ShouldBindBodyWith(&req, binding.JSON); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}

	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	item, err := global.ItemIconRepo.Get(userInfo.ID, req.Id)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}

	// Delete the actual file using storage interface
	var icon map[string]any
	if err := json.Unmarshal([]byte(item.IconJson), &icon); err == nil {
		// Check if the icon has a src field indicating a file path
		if fileName, ok := icon["fileName"].(string); ok {
			if err := global.Storage.Delete(c.Request.Context(), fileName); err != nil {
				zaplog.Logger.Warnf("Failed to delete file %s: %v", fileName, err)
			}
		}
	}

	if err := global.ItemIconRepo.Delete(userInfo.ID, req.Id); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}

	response.Success(c)
}

// 支持获取并直接下载对方网站图标到服务器
func (a *ItemIconRouter) GetSiteFavicon(c *gin.Context) {
	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	req := panelApi.ItemIconGetSiteFaviconReq{}
	if err := c.ShouldBind(&req); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}

	resp := panelApi.ItemIconGetSiteFaviconResp{}

	// One budget covers the page fetch and the icon download.
	ctx, cancel := context.WithTimeout(c.Request.Context(), itemIconRequestTimeout)
	defer cancel()

	fullUrl := ""
	if iconUrl, err := favicon.GetOneFaviconURL(ctx, req.Url); err != nil {
		if errors.Is(err, favicon.ErrIconUnavailable) {
			// The site simply has no icon. That is an answer the caller acts on
			// (it falls back to a text icon), not a fault that should fail the
			// whole bookmark import.
			response.SuccessData(c, resp)
			return
		}
		response.Error(c, "acquisition failed: get ico error:"+err.Error())
		return
	} else {
		fullUrl = iconUrl
	}

	parsedURL, err := url.Parse(req.Url)
	if err != nil {
		response.Error(c, "acquisition failed:"+err.Error())
		return
	}

	protocol := parsedURL.Scheme

	// 如果URL以双斜杠（//）开头，则使用当前页面协议
	if strings.HasPrefix(fullUrl, "//") {
		fullUrl = protocol + "://" + fullUrl[2:]
	} else if !strings.HasPrefix(fullUrl, "http://") && !strings.HasPrefix(fullUrl, "https://") {
		// 相对地址沿用页面自身的协议：https 页面不该被要求用 http 取图标
		fullUrl = protocol + "://" + fullUrl
	}

	// 去除图标的get参数
	{
		parsedIcoURL, err := url.Parse(fullUrl)
		if err != nil {
			response.Error(c, "acquisition failed: parsed ico URL :"+err.Error())
			return
		}
		fullUrl = parsedIcoURL.Scheme + "://" + parsedIcoURL.Host + parsedIcoURL.Path
	}

	// 下载图标
	fileName, err := favicon.DownloadImage(ctx, fullUrl)
	if err != nil {
		response.Error(c, "acquisition failed: download "+err.Error())
		return
	}

	// 保存到数据库
	if _, err := global.FileRepo.AddFile(userInfo.ID, fileName); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}

	resp.FileName = fileName
	resp.IconUrl = urlPrefix + fileName
	response.SuccessData(c, resp)
}

// SaveSort 保存排序
func (a *ItemIconRouter) SaveSort(c *gin.Context) {
	req := panelApi.ItemIconSaveSortRequest{}

	if err := c.ShouldBindBodyWith(&req, binding.JSON); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}

	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	err := global.ItemIconRepo.BatchSaveSort(userInfo.ID, req.ItemIconGroupId, req.SortItems)
	if err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}

	response.Success(c)
}
