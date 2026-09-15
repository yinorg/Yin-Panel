package system

import (
	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/response"
	"github.com/yinorg/Yin-Panel/backend/pkg/extension"
)

// CapabilitiesRouter exposes only capabilities compiled into the running
// backend. The frontend must use this endpoint for feature visibility; it is
// not an authorization mechanism.
type CapabilitiesRouter struct{}

func NewCapabilitiesRouter() *CapabilitiesRouter { return &CapabilitiesRouter{} }

func (a *CapabilitiesRouter) InitRouter(router *gin.RouterGroup) {
	router.GET("/system/capabilities", a.Get)
}

func (a *CapabilitiesRouter) Get(c *gin.Context) {
	edition, readOnly := extension.Edition()
	response.SuccessData(c, gin.H{
		"edition":      edition,
		"capabilities": extension.Capabilities(),
		"readOnly":     readOnly,
		"versionName":  global.VERSION,
	})
}
