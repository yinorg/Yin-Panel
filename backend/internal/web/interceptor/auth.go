package interceptor

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"strings"
	"yin-panel/internal/biz/repository"
	"yin-panel/internal/constant"
	"yin-panel/internal/global"
	"yin-panel/internal/infra/zaplog"
	"yin-panel/internal/util/jwt"
	"yin-panel/internal/util/publiccode"
	"yin-panel/internal/web/model/base"
	"yin-panel/internal/web/model/response"

	"github.com/gin-gonic/gin"
)

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
		claims, err = ParseJwtClaims(c.GetHeader("Authorization"))
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
		ID:         user.ID,
		Name:       user.Name,
		Role:       user.Role,
		Username:   user.Username,
		Publiccode: user.Publiccode,
		Token:      user.Token,
	}
	c.Set("userInfo", userInfo)
	c.Set("authMethod", authMethod)
	if publicSpaceID != 0 {
		c.Set("publicSpaceID", publicSpaceID)
	}
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

// ParseUserIdFromPubliccode 解析公开访问代码，获取用户ID
func ParseUserIdFromPubliccode(code string) (uint, error) {
	// 解析公开访问代码，获取用户ID
	userID, err := publiccode.ParseCode(code)
	if err != nil {
		zaplog.Logger.Infof("invalid public access code. %v", err)
		return 0, errors.New("invalid public access code")
	}

	return userID, nil
}
