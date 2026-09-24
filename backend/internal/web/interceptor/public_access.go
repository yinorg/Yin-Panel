package interceptor

import (
	"net/http"
	"strconv"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
)

func publicCodeRouteAllowed(c *gin.Context, publicSpaceID uint) bool {
	return publicCodeRequestAllowed(c.Request.Method, c.FullPath(), c.Param("spaceId"), publicSpaceID)
}

func publicCodeRequestAllowed(method, route, spaceParam string, publicSpaceID uint) bool {
	if method != http.MethodGet {
		return false
	}
	switch route {
	case "/api/spaces", "/api/panel/userConfig/getConfig", "/api/theme/current", "/api/theme/packages":
		return true
	case "/api/theme/packages/:id", "/api/theme/preview/:token", "/api/theme/preview/:token/assets/*name", "/api/theme/v2/current", "/api/theme/v2/packages/:revision", "/api/theme/v2/assets/:revision/*name":
		return true
	case "/api/spaces/:spaceId/groups", "/api/spaces/:spaceId/items", "/api/spaces/:spaceId/search-config":
		targetID, err := strconv.ParseUint(spaceParam, 10, 32)
		if err != nil || targetID == 0 {
			return false
		}
		return publicSpaceIDAllowsTarget(publicSpaceID, uint(targetID))
	default:
		return false
	}
}

func publicSpaceIDAllowsTarget(publicSpaceID, targetID uint) bool {
	if publicSpaceID == 0 || targetID == 0 {
		return false
	}
	if targetID == publicSpaceID {
		return true
	}
	var paired repository.Space
	return repository.Db.Select("id").Where("id = ? AND pair_id = ? AND side = ?", targetID, publicSpaceID, "yang").First(&paired).Error == nil
}
