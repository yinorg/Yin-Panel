package service

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"strconv"
	"strings"
	"sync"
	"time"

	"golang.org/x/oauth2"
	"gorm.io/gorm"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/kvcache"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"github.com/yinorg/Yin-Panel/backend/internal/util"
)

type UserService struct {
	itemGroupRepo repository.IItemIconGroupRepo
	userRepo      repository.IUserRepo
	proxyClient   *http.Client
	oauthStates   *kvcache.LocalCache[oauthLoginState]
	oauthStateMu  sync.Mutex
}

type IUserService interface {
	CreateUser(user *repository.User) error
	GetOAuthLoginURL(provider string, redirectURI string) (string, error)
	HandleOAuthCallback(provider, code, redirectURI, state string) (*repository.User, error)
}

func NewUserService(userRepo repository.IUserRepo, itemGroupRepo repository.IItemIconGroupRepo) *UserService {
	service := &UserService{
		userRepo:      userRepo,
		itemGroupRepo: itemGroupRepo,
		oauthStates:   kvcache.NewLocalCache[oauthLoginState](5*time.Minute, time.Minute),
	}

	// 初始化代理客户端
	service.initProxyClient()
	return service
}

// initProxyClient 初始化代理客户端
func (s *UserService) initProxyClient() {
	var transport *http.Transport
	// 检查是否启用了节点代理功能，并且能获取到 NODE_IP 环境变量
	nodeIP := os.Getenv("NODE_IP")
	// 添加对 AppConfig 的空指针检查
	if config.AppConfig.Base.EnableNodeProxy && nodeIP != "" {
		// 如果设置了 NODE_IP，为特定域名创建代理
		proxyFunc := func(req *http.Request) (*url.URL, error) {
			// 检查请求的主机名
			host := req.URL.Host
			// 只为 Google OAuth 相关域名使用代理
			if strings.Contains(host, "googleapis.com") ||
				strings.Contains(host, "google.com") {
				zaplog.Logger.Info("Using proxy for host: " + host + " via node: " + nodeIP)
				proxyURL := fmt.Sprintf("http://%s:7890", nodeIP)
				return url.Parse(proxyURL)
			}
			// 其他请求不使用代理
			return nil, nil
		}

		// 创建透明代理
		transport = &http.Transport{
			Proxy: proxyFunc,
		}

		zaplog.Logger.Info("Node proxy enabled, using node: " + nodeIP)
	} else {
		zaplog.Logger.Info("Node proxy not enabled")
	}

	s.proxyClient = &http.Client{Timeout: 30 * time.Second}
	if transport != nil {
		s.proxyClient.Transport = transport
	}
}

// createProxyContext 创建一个带有代理客户端的上下文
func (s *UserService) createProxyContext(timeout time.Duration) (context.Context, context.CancelFunc) {
	// 创建上下文
	ctx, cancel := context.WithTimeout(context.Background(), timeout)

	// 在 OAuth2 上下文中使用代理客户端
	ctx = context.WithValue(ctx, oauth2.HTTPClient, s.proxyClient)
	return ctx, cancel
}

func (s *UserService) CreateUser(user *repository.User) error {
	if err := s.userRepo.Create(user); err != nil {
		return err
	}

	personalSpace := repository.Space{}
	if repository.Db != nil {
		personalSpace = repository.Space{
			Type:        repository.SpaceTypePersonal,
			Name:        user.Name,
			OwnerUserID: user.ID,
		}
		if err := repository.Db.Create(&personalSpace).Error; err != nil {
			return err
		}
		member := repository.SpaceMember{
			SpaceID:  personalSpace.ID,
			UserID:   user.ID,
			Role:     repository.SpaceRoleAdmin,
			JoinedAt: time.Now(),
		}
		if err := repository.Db.Create(&member).Error; err != nil {
			return err
		}
	}

	defaultGroup := repository.ItemIconGroup{
		Title:   "APP",
		UserId:  user.ID,
		SpaceID: personalSpace.ID,
		Icon:    "material-symbols:ad-group-outline",
	}

	if err := s.itemGroupRepo.Save(&defaultGroup); err != nil {
		return err
	}

	return nil
}

// GetOAuthLoginURL generates the OAuth login URL for the specified provider
func (s *UserService) GetOAuthLoginURL(provider string, redirectURI string) (string, error) {
	providerConfig, err := s.getOAuthProvider(provider)
	if err != nil {
		return "", err
	}

	if providerConfig.IssuerURL != "" {
		return s.getOIDCLoginURL(*providerConfig, redirectURI)
	}

	state, err := randomURLSafeString(32)
	if err != nil {
		return "", fmt.Errorf("generate OAuth state: %w", err)
	}
	s.oauthStates.Set(state, oauthLoginState{Provider: provider}, 5*time.Minute)

	// Build OAuth URL
	authURL, err := url.Parse(providerConfig.AuthURL)
	if err != nil {
		return "", err
	}

	q := authURL.Query()
	q.Set("client_id", providerConfig.ClientID)
	q.Set("redirect_uri", redirectURI)
	q.Set("scope", providerConfig.Scopes)
	q.Set("response_type", "code")
	q.Set("state", state)

	authURL.RawQuery = q.Encode()
	return authURL.String(), nil
}

// HandleOAuthCallback processes the OAuth callback and returns or creates a user
func (s *UserService) HandleOAuthCallback(provider, code, redirectURI, state string) (*repository.User, error) {
	providerConfig, err := s.getOAuthProvider(provider)
	if err != nil {
		return nil, err
	}

	loginState, err := s.consumeOAuthState(provider, state)
	if err != nil {
		return nil, err
	}
	if code == "" {
		return nil, errors.New("authorization response did not include a code")
	}

	if providerConfig.IssuerURL != "" {
		userInfo, err := s.exchangeOIDCCode(*providerConfig, redirectURI, code, loginState)
		if err != nil {
			return nil, err
		}
		return s.findOrCreateOAuthUser(provider, *providerConfig, userInfo)
	}

	// Exchange code for token
	accessToken, err := s.exchangeCodeForToken(*providerConfig, code, redirectURI)
	if err != nil {
		zaplog.Logger.Error("Failed to exchange code for token: " + err.Error())
		return nil, err
	}

	// Get user info using the token
	userInfo, err := s.fetchUserInfo(*providerConfig, accessToken)
	if err != nil {
		zaplog.Logger.Error("Failed to fetch user info: " + err.Error())
		return nil, err
	}

	return s.findOrCreateOAuthUser(provider, *providerConfig, userInfo)
}

func (s *UserService) getOAuthProvider(provider string) (*config.OAuthProviderConfig, error) {
	for _, candidate := range config.AppConfig.OAuth.Providers {
		if strings.EqualFold(candidate.Name, provider) {
			return &candidate, nil
		}
	}
	return nil, errors.New("unsupported OAuth provider")
}

func (s *UserService) findOrCreateOAuthUser(provider string, providerConfig config.OAuthProviderConfig, userInfo map[string]any) (*repository.User, error) {
	// OIDC subject is the stable identity key; email is only used for merging.
	identifier, ok := userInfo["sub"].(string)
	if !ok || identifier == "" {
		return nil, errors.New("failed to get user identifier from OAuth provider")
	}
	email, _ := userInfo["email"].(string)
	email = strings.ToLower(strings.TrimSpace(email))
	verified, _ := userInfo["email_verified"].(bool)
	if email == "" || !verified {
		return nil, errors.New("OAuth provider did not return a verified email")
	}

	// Check if user already exists
	user, err := s.userRepo.GetByOAuthID(provider, identifier)
	if err == nil {
		if user.Status != 1 {
			return nil, errors.New("user account is disabled or inactive")
		}
		if email != "" && user.Mail != email {
			if err := s.userRepo.UpdateUserInfo(user.ID, map[string]any{"mail": email}); err != nil {
				return nil, err
			}
			user.Mail = email
		}
		s.syncOIDCGroups(user.ID, provider, userInfo)
		return &user, nil
	}

	// A new provider identity is automatically merged by verified email.
	user, err = s.userRepo.GetByMail(email)
	if err == nil {
		identity := &repository.OAuthIdentity{UserID: user.ID, Provider: provider, Subject: identifier, Email: email}
		if repository.Db != nil {
			if err := repository.Db.Create(identity).Error; err != nil {
				return nil, err
			}
		}
		return &user, nil
	}

	// User doesn't exist, create a new one.
	displayNameField := providerConfig.FieldMappingDisplayName
	if displayNameField == "" {
		displayNameField = "name"
	}
	emailField := providerConfig.FieldMappingEmail
	if emailField == "" {
		emailField = "email"
	}
	displayName, _ := userInfo[displayNameField].(string)
	email, _ = userInfo[emailField].(string)
	email = strings.ToLower(strings.TrimSpace(email))

	// OAuth users don't need a password as they authenticate through the provider
	newUser := &repository.User{
		Password:      "", // No password needed for OAuth users
		Name:          displayName,
		Mail:          email,
		Status:        1, // Active
		Role:          2, // Regular user
		OauthProvider: provider,
		OauthID:       identifier,
		// SQLite treats the empty string as a value, so multiple OAuth users
		// would violate the unique publiccode index. The provider identifier is
		// stable and unique enough for the initial value; users can regenerate it.
		Publiccode: util.GenerateRandomString(10),
	}

	if err := s.CreateUser(newUser); err != nil {
		return nil, err
	}
	if repository.Db != nil {
		if err := repository.Db.Create(&repository.OAuthIdentity{UserID: newUser.ID, Provider: provider, Subject: identifier, Email: email}).Error; err != nil {
			return nil, err
		}
	}
	s.syncOIDCGroups(newUser.ID, provider, userInfo)

	return newUser, nil
}

// syncOIDCGroups grants configured space roles from the provider's groups claim.
// It only adds or upgrades access and never removes manual permissions.
func (s *UserService) syncOIDCGroups(userID uint, provider string, userInfo map[string]any) {
	if repository.Db == nil {
		return
	}
	groups := make(map[string]bool)
	switch raw := userInfo["groups"].(type) {
	case []any:
		for _, value := range raw {
			if group, ok := value.(string); ok {
				groups[group] = true
			}
		}
	case []string:
		for _, group := range raw {
			groups[group] = true
		}
	default:
		return
	}
	if len(groups) == 0 {
		return
	}
	var rules []repository.SpaceOIDCGroup
	if repository.Db.Where("provider = ?", provider).Find(&rules).Error != nil {
		return
	}
	for _, rule := range rules {
		if !groups[rule.GroupName] {
			continue
		}
		var member repository.SpaceMember
		err := repository.Db.Where("space_id = ? AND user_id = ?", rule.SpaceID, userID).First(&member).Error
		if errors.Is(err, gorm.ErrRecordNotFound) {
			repository.Db.Create(&repository.SpaceMember{SpaceID: rule.SpaceID, UserID: userID, Role: rule.Role, Source: "oidc"})
		} else if err == nil && member.Source == "oidc" && roleRank(rule.Role) > roleRank(member.Role) {
			repository.Db.Model(&member).Update("role", rule.Role)
		}
	}
}

func roleRank(role string) int {
	switch role {
	case repository.SpaceRoleAdmin:
		return 3
	case repository.SpaceRoleEditor:
		return 2
	case repository.SpaceRoleViewer:
		return 1
	default:
		return 0
	}
}

// exchangeCodeForToken exchanges the authorization code for an access token
func (s *UserService) exchangeCodeForToken(config config.OAuthProviderConfig, code, redirectURI string) (string, error) {
	// 创建 OAuth2 配置
	conf := &oauth2.Config{
		ClientID:     config.ClientID,
		ClientSecret: config.ClientSecret,
		RedirectURL:  redirectURI,
		Endpoint: oauth2.Endpoint{
			TokenURL:  config.TokenURL,
			AuthURL:   config.AuthURL,
			AuthStyle: oauth2.AuthStyleInParams,
		},
	}

	// 创建带有代理的上下文
	ctx, cancel := s.createProxyContext(10 * time.Second)
	defer cancel()

	// 交换 code 获取 token
	token, err := conf.Exchange(ctx, code)
	if err != nil {
		zaplog.Logger.Error("Failed to exchange token: " + err.Error())
		return "", errors.New("failed to exchange code for token")
	}

	// 获取 access_token
	var accessToken string

	// 首先检查 token.AccessToken
	if token.AccessToken != "" {
		accessToken = token.AccessToken
	} else {
		// 尝试从 Extra 中获取
		if tokenStr, ok := token.Extra("access_token").(string); ok && tokenStr != "" {
			accessToken = tokenStr
		} else {
			zaplog.Logger.Error("No access_token found in token response")
			return "", errors.New("no access_token in response")
		}
	}

	zaplog.Logger.Info("Token exchange successful")
	return accessToken, nil
}

// fetchUserInfo fetches user information from the OAuth provider
func (s *UserService) fetchUserInfo(config config.OAuthProviderConfig, accessToken string) (map[string]interface{}, error) {
	// 创建 OAuth2 token
	token := &oauth2.Token{
		AccessToken: accessToken,
		TokenType:   "Bearer",
	}

	// 创建 OAuth2 配置
	conf := &oauth2.Config{
		ClientID:     config.ClientID,
		ClientSecret: config.ClientSecret,
		Endpoint: oauth2.Endpoint{
			TokenURL: config.TokenURL,
			AuthURL:  config.AuthURL,
		},
	}

	// 创建带有代理的上下文
	ctx, cancel := s.createProxyContext(10 * time.Second)
	defer cancel()

	// 创建带有 token 的客户端
	client := conf.Client(ctx, token)

	// 发送请求获取用户信息
	resp, err := client.Get(config.UserInfoURL)
	if err != nil {
		zaplog.Logger.Error("Failed to send user info request: " + err.Error())
		return nil, err
	}
	defer resp.Body.Close()

	// 读取响应
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		zaplog.Logger.Error("Failed to read user info response: " + err.Error())
		return nil, err
	}

	// 检查响应状态
	if resp.StatusCode != http.StatusOK {
		zaplog.Logger.Error("User info request failed with status code: " + strconv.Itoa(resp.StatusCode))
		zaplog.Logger.Error("Response body: " + string(body))
		return nil, errors.New("failed to fetch user info: " + string(body))
	}

	// 解析 JSON 响应
	var userInfo map[string]any
	if err := json.Unmarshal(body, &userInfo); err != nil {
		zaplog.Logger.Error("Failed to parse user info response as JSON: " + err.Error())
		return nil, err
	}

	return userInfo, nil
}
