package service

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"go.uber.org/zap"
	"gorm.io/gorm"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"github.com/yinorg/Yin-Panel/backend/internal/web/model/param/commonApi"
)

func TestAuthentikOIDCAuthorizationCodeFlow(t *testing.T) {
	privateKey, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatal(err)
	}

	var issuer string
	var expectedNonce string
	var expectedVerifier string
	invalidNonce := false
	mockAuthentik := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		switch request.URL.Path {
		case "/.well-known/openid-configuration":
			writeJSON(writer, map[string]string{
				"issuer":                 issuer,
				"authorization_endpoint": issuer + "/authorize",
				"token_endpoint":         issuer + "/token",
				"jwks_uri":               issuer + "/jwks",
			})
		case "/jwks":
			writeJSON(writer, jsonWebKeySet{Keys: []jsonWebKey{{
				Kty: "RSA", Kid: "authentik-key", Alg: "RS256",
				N: base64.RawURLEncoding.EncodeToString(privateKey.PublicKey.N.Bytes()),
				E: base64.RawURLEncoding.EncodeToString([]byte{1, 0, 1}),
			}}})
		case "/token":
			if err := request.ParseForm(); err != nil {
				http.Error(writer, err.Error(), http.StatusBadRequest)
				return
			}
			if request.Form.Get("grant_type") != "authorization_code" || request.Form.Get("code") != "authentik-code" || request.Form.Get("code_verifier") != expectedVerifier {
				http.Error(writer, "invalid authorization-code request", http.StatusBadRequest)
				return
			}
			nonce := expectedNonce
			if invalidNonce {
				nonce = "wrong-nonce"
			}
			idToken := jwt.NewWithClaims(jwt.SigningMethodRS256, jwt.MapClaims{
				"iss":                issuer,
				"aud":                "yin-panel-client",
				"sub":                "authentik-user-42",
				"preferred_username": "alice",
				"name":               "Alice Example",
				"email":              "alice@example.test",
				"email_verified":     true,
				"nonce":              nonce,
				"exp":                time.Now().Add(time.Minute).Unix(),
			})
			idToken.Header["kid"] = "authentik-key"
			rawIDToken, err := idToken.SignedString(privateKey)
			if err != nil {
				http.Error(writer, err.Error(), http.StatusInternalServerError)
				return
			}
			writeJSON(writer, map[string]string{
				"access_token": "access-token",
				"token_type":   "Bearer",
				"id_token":     rawIDToken,
			})
		default:
			http.NotFound(writer, request)
		}
	}))
	defer mockAuthentik.Close()
	issuer = mockAuthentik.URL

	originalConfig := config.AppConfig
	originalLogger := zaplog.Logger
	zaplog.Logger = zap.NewNop().Sugar()
	config.AppConfig = &config.Config{OAuth: config.OAuthConfig{
		Enable: true,
		Providers: []config.OAuthProviderConfig{{
			Name:                    "authentik",
			ClientID:                "yin-panel-client",
			ClientSecret:            "yin-panel-secret",
			IssuerURL:               issuer,
			Scopes:                  "profile email",
			FieldMappingIdentifier:  "preferred_username",
			FieldMappingDisplayName: "name",
			FieldMappingEmail:       "email",
		}},
	}}
	t.Cleanup(func() {
		config.AppConfig = originalConfig
		zaplog.Logger = originalLogger
	})

	users := &memoryUserRepo{}
	service := NewUserService(users, memoryItemIconGroupRepo{})
	redirectURI := "https://panel.example.test/api/oauth/authentik/callback"
	authorizationURL, err := service.GetOAuthLoginURL("authentik", redirectURI)
	if err != nil {
		t.Fatalf("build authorization URL: %v", err)
	}

	query, err := url.Parse(authorizationURL)
	if err != nil {
		t.Fatal(err)
	}
	if got := query.Query().Get("scope"); !strings.Contains(got, "openid") {
		t.Fatalf("OIDC authorization request does not contain openid scope: %q", got)
	}
	if query.Query().Get("code_challenge_method") != "S256" {
		t.Fatalf("unexpected PKCE method: %q", query.Query().Get("code_challenge_method"))
	}
	state := query.Query().Get("state")
	loginState, exists := service.oauthStates.Get(state)
	if !exists || loginState.Nonce == "" || loginState.PKCEVerifier == "" {
		t.Fatal("OIDC state, nonce, or PKCE verifier was not retained")
	}
	challenge := sha256.Sum256([]byte(loginState.PKCEVerifier))
	if got := query.Query().Get("code_challenge"); got != base64.RawURLEncoding.EncodeToString(challenge[:]) {
		t.Fatal("OIDC authorization request has an invalid PKCE challenge")
	}
	expectedNonce = loginState.Nonce
	expectedVerifier = loginState.PKCEVerifier

	user, err := service.HandleOAuthCallback("authentik", "authentik-code", redirectURI, state)
	if err != nil {
		t.Fatalf("handle Authentik OIDC callback: %v", err)
	}
	if user.Username != "alice@example.test" || user.Name != "Alice Example" || user.Mail != "alice@example.test" {
		t.Fatalf("unexpected OIDC user: %#v", user)
	}
	if _, err := service.HandleOAuthCallback("authentik", "authentik-code", redirectURI, state); err == nil {
		t.Fatal("reused OIDC state was accepted")
	}

	authorizationURL, err = service.GetOAuthLoginURL("authentik", redirectURI)
	if err != nil {
		t.Fatal(err)
	}
	query, _ = url.Parse(authorizationURL)
	state = query.Query().Get("state")
	loginState, _ = service.oauthStates.Get(state)
	expectedNonce = loginState.Nonce
	expectedVerifier = loginState.PKCEVerifier
	invalidNonce = true
	if _, err := service.HandleOAuthCallback("authentik", "authentik-code", redirectURI, state); err == nil || !strings.Contains(err.Error(), "nonce") {
		t.Fatalf("invalid ID token nonce was accepted: %v", err)
	}
}

func writeJSON(writer http.ResponseWriter, value any) {
	writer.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(writer).Encode(value)
}

type memoryUserRepo struct {
	users []repository.User
}

func (r *memoryUserRepo) Get(id uint) (repository.User, error) {
	return repository.User{}, gorm.ErrRecordNotFound
}
func (r *memoryUserRepo) Count() (uint, error) { return uint(len(r.users)), nil }
func (r *memoryUserRepo) GetByUsernameAndPassword(string, string, string) (repository.User, error) {
	return repository.User{}, gorm.ErrRecordNotFound
}
func (r *memoryUserRepo) GetByOAuthID(provider, oauthID string) (repository.User, error) {
	for _, user := range r.users {
		if user.OauthProvider == provider && user.OauthID == oauthID {
			return user, nil
		}
	}
	return repository.User{}, gorm.ErrRecordNotFound
}

func (r *memoryUserRepo) GetByMail(mail string) (repository.User, error) {
	for _, user := range r.users {
		if user.Mail == mail {
			return user, nil
		}
	}
	return repository.User{}, gorm.ErrRecordNotFound
}
func (r *memoryUserRepo) GetList(repository.PagedParam) ([]repository.User, uint, error) {
	return r.users, uint(len(r.users)), nil
}
func (r *memoryUserRepo) Update(uint, *repository.User) error       { return nil }
func (r *memoryUserRepo) UpdateUserInfo(uint, map[string]any) error { return nil }
func (r *memoryUserRepo) Delete(uint) ([]string, error)             { return nil, nil }
func (r *memoryUserRepo) CheckUsernameExist(string, string) (repository.User, error) {
	return repository.User{}, gorm.ErrRecordNotFound
}
func (r *memoryUserRepo) GetByPubliccode(string) (repository.User, error) {
	return repository.User{}, gorm.ErrRecordNotFound
}
func (r *memoryUserRepo) InvalidateTokens(userID uint) error {
	for index := range r.users {
		if r.users[index].ID == userID {
			r.users[index].TokenVersion++
			return nil
		}
	}
	return gorm.ErrRecordNotFound
}
func (r *memoryUserRepo) Create(user *repository.User) error {
	user.ID = uint(len(r.users) + 1)
	r.users = append(r.users, *user)
	return nil
}

type memoryItemIconGroupRepo struct{}

func (memoryItemIconGroupRepo) Save(*repository.ItemIconGroup) error { return nil }
func (memoryItemIconGroupRepo) GetList(uint) ([]repository.ItemIconGroup, error) {
	return nil, nil
}
func (memoryItemIconGroupRepo) Count(uint) (int, error)    { return 0, nil }
func (memoryItemIconGroupRepo) Deletes(uint, []uint) error { return nil }
func (memoryItemIconGroupRepo) BatchSaveSort(uint, []commonApi.SortRequestItem) error {
	return nil
}

var _ repository.IUserRepo = (*memoryUserRepo)(nil)
var _ repository.IItemIconGroupRepo = memoryItemIconGroupRepo{}
