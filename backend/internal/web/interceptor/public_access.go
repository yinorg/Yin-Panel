package interceptor

import (
	"net/http"
	"strconv"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
)

func publicCodeRouteAllowed(c *gin.Context, publicSpaceID uint) bool {
	return PublicCodeRequestAllowed(c.Request.Method, c.FullPath(), c.Param("spaceId"), publicSpaceID)
}

// PublicCodeRequestAllowed applies the default-deny route policy to requests
// authenticated through a public space code. route must be Gin's matched path.
func PublicCodeRequestAllowed(method, route, spaceParam string, publicSpaceID uint) bool {
	if method != http.MethodGet {
		return false
	}
	switch route {
	case "/api/spaces", "/api/panel/userConfig/getConfig", "/api/theme/v2/current", "/api/theme/v2/effective", "/api/theme/v2/packages":
		return true
	case "/api/theme/v2/package/:id", "/api/theme/v2/revisions/:revision", "/api/theme/v2/preview/:token", "/api/theme/v2/preview/:token/assets/*name", "/api/theme/v2/assets/:revision/*name":
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
