package system

import (
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/constant"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/response"

	"github.com/gin-gonic/gin/binding"

	"github.com/gin-gonic/gin"
)

type ModuleConfigRouter struct{}

func NewModuleConfigRouter() *ModuleConfigRouter {
	return &ModuleConfigRouter{}
}

func (a *ModuleConfigRouter) InitRouter(router *gin.RouterGroup) {
	r := router.Group("")
	r.Use(interceptor.Auth)
	{
		r.GET("/system/moduleConfig/getByName", a.GetByName)
		r.POST("/system/moduleConfig/save", a.Save)
	}
}

func (a *ModuleConfigRouter) GetByName(c *gin.Context) {
	req := repository.ModuleConfig{}
	if err := c.ShouldBind(&req); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}

	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	if cfg, err := global.ModuleConfigRepo.GetModuleConfigByUserIdAndName(userInfo.ID, req.Name); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	} else {
		response.SuccessData(c, cfg)
		return
	}
}

func (a *ModuleConfigRouter) Save(c *gin.Context) {
	req := repository.ModuleConfig{}
	if err := c.ShouldBindWith(&req, binding.JSON); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}

	userInfo, exist := base.GetCurrentUserInfo(c)
	if !exist || userInfo.ID == 0 {
		response.ErrorByCode(c, constant.CodeNotLogin)
		return
	}

	config := repository.ModuleConfig{
		UserId: userInfo.ID,
		Value:  req.Value,
		Name:   req.Name,
	}

	if err := global.ModuleConfigRepo.SaveModuleConfig(&config); err != nil {
		response.ErrorDatabase(c, err.Error())
		return
	}

	response.Success(c)
}
