package system

import (
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/util/monitor"
	"github.com/yinorg/Yin-Panel/backend/internal/web/interceptor"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/param/systemApi"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/response"

	"github.com/gin-gonic/gin"
)

type MonitorRouter struct {
}

func monitorEnabled() bool { return global.Config != nil && global.Config.Base.EnableMonitor }

func NewMonitorRouter() *MonitorRouter {
	return &MonitorRouter{}
}

func (a *MonitorRouter) InitRouter(router *gin.RouterGroup) {
	r := router.Group("")
	r.Use(interceptor.Auth)
	{
		r.POST("/system/monitor/getDiskMountpoints", a.GetDiskMountpoints)
		r.POST("/system/monitor/getDiskStateByPath", a.GetDiskStateByPath)
		r.POST("/system/monitor/getSnapshot", a.GetSnapshot)
		r.POST("/system/monitor/getEnableStatus", a.GetEnableStatus)
	}
}

func (a *MonitorRouter) GetSnapshot(c *gin.Context) {
	if !monitorEnabled() {
		response.Error(c, "system monitor is disabled")
		return
	}
	state, err := global.CacheMonitor.GetSnapshot()
	if err != nil {
		response.Error(c, "failed")
		return
	}
	response.SuccessData(c, state)
}

func (a *MonitorRouter) GetNetState(c *gin.Context) {
	if !monitorEnabled() {
		response.Error(c, "system monitor is disabled")
		return
	}
	state, err := global.CacheMonitor.GetNetState()
	if err != nil {
		response.Error(c, "failed")
		return
	}
	response.SuccessData(c, state)
}

func (a *MonitorRouter) GetCpuState(c *gin.Context) {
	if !monitorEnabled() {
		response.Error(c, "system monitor is disabled")
		return
	}
	cpuInfo, err := global.CacheMonitor.GetCpuState()
	if err != nil {
		response.Error(c, "failed")
		return
	}

	response.SuccessData(c, cpuInfo)
}

func (a *MonitorRouter) GetMemonyState(c *gin.Context) {
	if !monitorEnabled() {
		response.Error(c, "system monitor is disabled")
		return
	}
	memoryInfo, err := global.CacheMonitor.GetMemonyState()
	if err != nil {
		response.Error(c, "failed")
		return
	}

	response.SuccessData(c, memoryInfo)
}

func (a *MonitorRouter) GetDiskStateByPath(c *gin.Context) {
	if !monitorEnabled() {
		response.Error(c, "system monitor is disabled")
		return
	}
	req := systemApi.MonitorGetDiskStateByPathReq{}
	if err := c.ShouldBind(&req); err != nil {
		response.ErrorParamFomat(c, err.Error())
		return
	}

	diskState, err := global.CacheMonitor.GetDiskStateByPath(req.Path)
	if err != nil {
		response.Error(c, "failed")
		return
	}

	response.SuccessData(c, diskState)
}

func (a *MonitorRouter) GetDiskMountpoints(c *gin.Context) {
	if !monitorEnabled() {
		response.Error(c, "system monitor is disabled")
		return
	}
	if list, err := monitor.GetDiskMountpoints(); err != nil {
		response.Error(c, err.Error())
	} else {
		response.SuccessData(c, list)
	}
}

// GetEnableStatus returns the enableMonitor configuration from conf.ini
func (a *MonitorRouter) GetEnableStatus(c *gin.Context) {
	response.SuccessData(c, gin.H{
		"enabled": global.Config.Base.EnableMonitor,
	})
}
