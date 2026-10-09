package interceptor

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"net/http"
	"net/url"
	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/constant"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"github.com/yinorg/Yin-Panel/backend/internal/util/jwt"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/base"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/response"
	"github.com/yinorg/Yin-Panel/backend/pkg/extension"
	"strings"

	"github.com/gin-gonic/gin"
)

// AuthCookieName is the httpOnly session cookie. The session token lives here so
// same-origin theme code cannot read it from localStorage; the Authorization
// header is still accepted for non-browser clients.
const AuthCookieName = "yin_token"

// SetAuthCookie stores the session token in an httpOnly cookie the frontend
// cannot read. SameSite=Lax blocks cross-site writes while still allowing the
// top-level OAuth redirect back into the app.
func SetAuthCookie(c *gin.Context, token string) {
	c.SetSameSite(http.SameSiteLaxMode)
	c.SetCookie(AuthCookieName, token, authCookieMaxAge(), "/", "", requestIsHTTPS(c), true)
}

// ClearAuthCookie expires the session cookie on logout.
func ClearAuthCookie(c *gin.Context) {
	c.SetSameSite(http.SameSiteLaxMode)
	c.SetCookie(AuthCookieName, "", -1, "/", "", requestIsHTTPS(c), true)
}

func authCookieMaxAge() int {
	hours := 0
	if config.AppConfig != nil {
		hours = config.AppConfig.JWT.Expire
	}
	if hours <= 0 {
		hours = jwt.DefaultExpireHours
	}
	return hours * 3600
}

func requestIsHTTPS(c *gin.Context) bool {
	if c.Request != nil && c.Request.TLS != nil {
		return true
	}
	if config.AppConfig == nil {
		return false
	}
	root, err := url.Parse(config.AppConfig.Base.RootURL)
	return err == nil && root.Scheme == "https"
}


// Auth 认证中间件
func Auth(c *gin.Context) {
	publiccode := c.GetHeader("publiccode")
	var userId uint
	var err error
	var claims *jwt.Claims
	authMethod := "publiccode"
	var publicSpaceID uint
	if publiccode != "" {
		var space repository.Space
		err = repository.Db.Where("public_id = ? AND public_enabled = ?", publiccode, true).First(&space).Error
		if err == nil && space.PublicMode == "code" {
			accessCode := c.GetHeader("Public-Access-Code")
			sum := sha256.Sum256([]byte(accessCode))
			if accessCode == "" || hex.EncodeToString(sum[:]) != space.PublicCodeHash {
				err = errors.New("invalid public access code")
			}
		}
		if err == nil {
			userId = space.OwnerUserID
			publicSpaceID = space.ID
		}
	} else {
		token := c.GetHeader("Authorization")
		if token == "" {
			token, _ = c.Cookie(AuthCookieName)
		}
		claims, err = ParseJwtClaims(token)
		if err == nil {
			userId = claims.UserID
			authMethod = "jwt"
		}
	}

	if err != nil {
		response.ErrorByCode(c, constant.CodeNotLogin)
		c.Abort()
		return
	}

	// 获取用户信息
	user, err := global.UserRepo.Get(userId)
	if err != nil {
		zaplog.Logger.Infof("user not exist. %v", err)
		response.ErrorByCode(c, constant.CodeNotLogin)
		c.Abort()
		return
	}

	// 检查用户状态
	if user.Status != 1 {
		response.ErrorByCode(c, constant.CodeStatusError)
		c.Abort()
		return
	}
	if claims != nil && (claims.TokenVersion == nil || *claims.TokenVersion != user.TokenVersion) {
		response.ErrorByCode(c, constant.CodeNotLogin)
		c.Abort()
		return
	}

	// 将用户信息存储到上下文
	userInfo := base.UserInfo{
		ID:    user.ID,
		Name:  user.Name,
		Role:  user.Role,
		Mail:  user.Mail,
		Token: user.Token,
	}
	c.Set("userInfo", userInfo)
	c.Set("authMethod", authMethod)
	c.Set(extension.ActorContextKey, extension.Actor{ID: user.ID, Username: user.Name})
	if publicSpaceID != 0 {
		c.Set("publicSpaceID", publicSpaceID)
		if !publicCodeRouteAllowed(c, publicSpaceID) {
			response.ErrorNoAccess(c)
			c.Abort()
			return
		}
	}
	c.Next()
}

// OptionalAuth resolves a session when one is present but never rejects the
// request. It is for endpoints that serve both signed-in users and anonymous
// visitors (theme resolution), where the difference only changes which theme is
// chosen, not whether the request may proceed.
func OptionalAuth(c *gin.Context) {
	token := c.GetHeader("Authorization")
	if token == "" {
		token, _ = c.Cookie(AuthCookieName)
	}
	if token == "" {
		c.Next()
		return
	}
	claims, err := ParseJwtClaims(token)
	if err != nil {
		c.Next()
		return
	}
	user, err := global.UserRepo.Get(claims.UserID)
	if err != nil || user.Status != 1 || claims.TokenVersion == nil || *claims.TokenVersion != user.TokenVersion {
		c.Next()
		return
	}
	c.Set("userInfo", base.UserInfo{ID: user.ID, Name: user.Name, Role: user.Role, Mail: user.Mail})
	c.Set("authMethod", "jwt")
	c.Next()
}

// ParseUserIdFromJwtToken 解析JWT Token，获取用户ID
func ParseUserIdFromJwtToken(authHeader string) (uint, error) {
	claims, err := ParseJwtClaims(authHeader)
	if err != nil {
		return 0, err
	}
	return claims.UserID, nil
}

func ParseJwtClaims(authHeader string) (*jwt.Claims, error) {
	if authHeader == "" {
		return nil, errors.New("authHeader is empty")
	}

	// 支持Bearer token
	parts := strings.SplitN(authHeader, " ", 2)
	if len(parts) == 2 && parts[0] == "Bearer" {
		authHeader = parts[1]
	}

	// 解析Token
	claims, err := jwt.ParseToken(authHeader)
	if err != nil {
		zaplog.Logger.Infof("invalid token. %v", err)
		return nil, errors.New("invalid token")
	}

	return claims, nil
}
