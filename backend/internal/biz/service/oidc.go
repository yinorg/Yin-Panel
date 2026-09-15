package service

import (
	"context"
	"crypto"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/rsa"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math/big"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"golang.org/x/oauth2"

	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
)

// oauthLoginState is short lived and single use. Keeping the PKCE verifier on
// the server means it is never exposed to the browser or the identity provider.
type oauthLoginState struct {
	Provider     string
	Nonce        string
	PKCEVerifier string
}

type oidcDiscovery struct {
	Issuer                string `json:"issuer"`
	AuthorizationEndpoint string `json:"authorization_endpoint"`
	TokenEndpoint         string `json:"token_endpoint"`
	UserInfoEndpoint      string `json:"userinfo_endpoint"`
	JWKSURI               string `json:"jwks_uri"`
}

type jsonWebKeySet struct {
	Keys []jsonWebKey `json:"keys"`
}

type jsonWebKey struct {
	Kty string `json:"kty"`
	Kid string `json:"kid"`
	Use string `json:"use"`
	Alg string `json:"alg"`
	N   string `json:"n"`
	E   string `json:"e"`
	Crv string `json:"crv"`
	X   string `json:"x"`
	Y   string `json:"y"`
}

func (s *UserService) getOIDCLoginURL(provider config.OAuthProviderConfig, redirectURI string) (string, error) {
	metadata, err := s.discoverOIDCProvider(provider)
	if err != nil {
		return "", err
	}

	state, err := randomURLSafeString(32)
	if err != nil {
		return "", fmt.Errorf("generate OIDC state: %w", err)
	}
	nonce, err := randomURLSafeString(32)
	if err != nil {
		return "", fmt.Errorf("generate OIDC nonce: %w", err)
	}
	verifier, err := randomURLSafeString(32)
	if err != nil {
		return "", fmt.Errorf("generate PKCE verifier: %w", err)
	}

	s.oauthStates.Set(state, oauthLoginState{
		Provider:     provider.Name,
		Nonce:        nonce,
		PKCEVerifier: verifier,
	}, 5*time.Minute)

	oauthConfig := oauth2.Config{
		ClientID:    provider.ClientID,
		RedirectURL: redirectURI,
		Scopes:      oidcScopes(provider.Scopes),
		Endpoint: oauth2.Endpoint{
			AuthURL: metadata.AuthorizationEndpoint,
		},
	}

	return oauthConfig.AuthCodeURL(
		state,
		oauth2.S256ChallengeOption(verifier),
		oauth2.SetAuthURLParam("nonce", nonce),
	), nil
}

func (s *UserService) consumeOAuthState(provider, state string) (oauthLoginState, error) {
	if state == "" {
		return oauthLoginState{}, errors.New("missing OAuth state")
	}

	s.oauthStateMu.Lock()
	defer s.oauthStateMu.Unlock()
	loginState, ok := s.oauthStates.Get(state)
	if !ok {
		return oauthLoginState{}, errors.New("invalid or expired OAuth state")
	}
	// A state must not be accepted a second time, including if the code exchange
	// subsequently fails.
	s.oauthStates.Delete(state)
	if !strings.EqualFold(loginState.Provider, provider) {
		return oauthLoginState{}, errors.New("OAuth state belongs to another provider")
	}
	return loginState, nil
}

func (s *UserService) exchangeOIDCCode(provider config.OAuthProviderConfig, redirectURI, code string, loginState oauthLoginState) (map[string]any, error) {
	metadata, err := s.discoverOIDCProvider(provider)
	if err != nil {
		return nil, err
	}

	oauthConfig := oauth2.Config{
		ClientID:     provider.ClientID,
		ClientSecret: provider.ClientSecret,
		RedirectURL:  redirectURI,
		Endpoint: oauth2.Endpoint{
			AuthURL:  metadata.AuthorizationEndpoint,
			TokenURL: metadata.TokenEndpoint,
		},
	}
	ctx, cancel := s.createProxyContext(10 * time.Second)
	defer cancel()
	token, err := oauthConfig.Exchange(ctx, code, oauth2.VerifierOption(loginState.PKCEVerifier))
	if err != nil {
		return nil, fmt.Errorf("exchange OIDC authorization code: %w", err)
	}

	rawIDToken, ok := token.Extra("id_token").(string)
	if !ok || rawIDToken == "" {
		return nil, errors.New("OIDC token response did not include an id_token")
	}
	claims, err := s.verifyOIDCIDToken(ctx, rawIDToken, provider, metadata, loginState.Nonce)
	if err != nil {
		return nil, err
	}
	return claims, nil
}

func (s *UserService) discoverOIDCProvider(provider config.OAuthProviderConfig) (oidcDiscovery, error) {
	issuer := strings.TrimSpace(provider.IssuerURL)
	if issuer == "" {
		return oidcDiscovery{}, errors.New("OIDC issuer_url is required")
	}
	issuerURL, err := url.Parse(issuer)
	if err != nil {
		return oidcDiscovery{}, fmt.Errorf("invalid OIDC issuer_url: %w", err)
	}
	if issuerURL.Scheme == "" || issuerURL.Host == "" {
		return oidcDiscovery{}, errors.New("OIDC issuer_url must be an absolute URL")
	}

	discoveryURL := provider.DiscoveryURL
	if discoveryURL == "" {
		discoveryURL = strings.TrimRight(issuer, "/") + "/.well-known/openid-configuration"
	}
	ctx, cancel := s.createProxyContext(10 * time.Second)
	defer cancel()
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, discoveryURL, nil)
	if err != nil {
		return oidcDiscovery{}, fmt.Errorf("create OIDC discovery request: %w", err)
	}
	response, err := s.proxyClient.Do(request)
	if err != nil {
		return oidcDiscovery{}, fmt.Errorf("fetch OIDC discovery document: %w", err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return oidcDiscovery{}, fmt.Errorf("OIDC discovery returned HTTP %d", response.StatusCode)
	}

	var metadata oidcDiscovery
	if err := json.NewDecoder(io.LimitReader(response.Body, 1<<20)).Decode(&metadata); err != nil {
		return oidcDiscovery{}, fmt.Errorf("decode OIDC discovery document: %w", err)
	}
	if metadata.Issuer != issuer {
		return oidcDiscovery{}, errors.New("OIDC discovery issuer does not match issuer_url")
	}
	if metadata.AuthorizationEndpoint == "" || metadata.TokenEndpoint == "" || metadata.JWKSURI == "" {
		return oidcDiscovery{}, errors.New("OIDC discovery document is missing a required endpoint")
	}
	return metadata, nil
}

func (s *UserService) verifyOIDCIDToken(ctx context.Context, rawToken string, provider config.OAuthProviderConfig, metadata oidcDiscovery, nonce string) (map[string]any, error) {
	keys, err := s.fetchOIDCJWKS(ctx, metadata.JWKSURI)
	if err != nil {
		return nil, err
	}

	claims := jwt.MapClaims{}
	parsedToken, err := jwt.ParseWithClaims(rawToken, claims, func(token *jwt.Token) (any, error) {
		keyID, _ := token.Header["kid"].(string)
		key, err := keys.key(keyID)
		if err != nil {
			return nil, err
		}
		if !supportsKeyAndAlgorithm(key, token.Method) {
			return nil, errors.New("OIDC ID token signing algorithm does not match JWKS key")
		}
		return key, nil
	})
	if err != nil || !parsedToken.Valid {
		if err == nil {
			err = errors.New("OIDC ID token is invalid")
		}
		return nil, fmt.Errorf("verify OIDC ID token: %w", err)
	}

	expiresAt, err := claims.GetExpirationTime()
	if err != nil || expiresAt == nil || !expiresAt.Time.After(time.Now()) {
		return nil, errors.New("OIDC ID token is expired or missing exp")
	}
	issuer, err := claims.GetIssuer()
	if err != nil || issuer != strings.TrimSpace(provider.IssuerURL) {
		return nil, errors.New("OIDC ID token issuer does not match issuer_url")
	}
	audience, err := claims.GetAudience()
	if err != nil || !contains(audience, provider.ClientID) {
		return nil, errors.New("OIDC ID token audience does not include client_id")
	}
	tokenNonce, _ := claims["nonce"].(string)
	if tokenNonce == "" || tokenNonce != nonce {
		return nil, errors.New("OIDC ID token nonce does not match authorization request")
	}

	return map[string]any(claims), nil
}

func (s *UserService) fetchOIDCJWKS(ctx context.Context, jwksURL string) (jsonWebKeySet, error) {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, jwksURL, nil)
	if err != nil {
		return jsonWebKeySet{}, fmt.Errorf("create OIDC JWKS request: %w", err)
	}
	response, err := s.proxyClient.Do(request)
	if err != nil {
		return jsonWebKeySet{}, fmt.Errorf("fetch OIDC JWKS: %w", err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return jsonWebKeySet{}, fmt.Errorf("OIDC JWKS returned HTTP %d", response.StatusCode)
	}
	var keySet jsonWebKeySet
	if err := json.NewDecoder(io.LimitReader(response.Body, 1<<20)).Decode(&keySet); err != nil {
		return jsonWebKeySet{}, fmt.Errorf("decode OIDC JWKS: %w", err)
	}
	if len(keySet.Keys) == 0 {
		return jsonWebKeySet{}, errors.New("OIDC JWKS contains no keys")
	}
	return keySet, nil
}

func (set jsonWebKeySet) key(keyID string) (crypto.PublicKey, error) {
	for _, key := range set.Keys {
		if key.Kid == keyID {
			return key.publicKey()
		}
	}
	return nil, errors.New("OIDC ID token key ID was not found in JWKS")
}

func (key jsonWebKey) publicKey() (crypto.PublicKey, error) {
	switch key.Kty {
	case "RSA":
		modulus, err := decodeBase64URLInt(key.N)
		if err != nil {
			return nil, fmt.Errorf("decode RSA modulus: %w", err)
		}
		exponent, err := decodeBase64URLInt(key.E)
		if err != nil || !exponent.IsInt64() || exponent.Sign() <= 0 {
			return nil, errors.New("decode RSA exponent")
		}
		return &rsa.PublicKey{N: modulus, E: int(exponent.Int64())}, nil
	case "EC":
		curve := map[string]elliptic.Curve{
			"P-256": elliptic.P256(),
			"P-384": elliptic.P384(),
			"P-521": elliptic.P521(),
		}[key.Crv]
		if curve == nil {
			return nil, errors.New("unsupported EC curve in OIDC JWKS")
		}
		x, err := decodeBase64URLInt(key.X)
		if err != nil {
			return nil, fmt.Errorf("decode EC x coordinate: %w", err)
		}
		y, err := decodeBase64URLInt(key.Y)
		if err != nil || !curve.IsOnCurve(x, y) {
			return nil, errors.New("invalid EC public key in OIDC JWKS")
		}
		return &ecdsa.PublicKey{Curve: curve, X: x, Y: y}, nil
	default:
		return nil, errors.New("unsupported OIDC JWKS key type")
	}
}

func supportsKeyAndAlgorithm(key crypto.PublicKey, method jwt.SigningMethod) bool {
	switch key.(type) {
	case *rsa.PublicKey:
		if _, ok := method.(*jwt.SigningMethodRSA); ok {
			return true
		}
		_, ok := method.(*jwt.SigningMethodRSAPSS)
		return ok
	case *ecdsa.PublicKey:
		_, ok := method.(*jwt.SigningMethodECDSA)
		return ok
	default:
		return false
	}
}

func decodeBase64URLInt(value string) (*big.Int, error) {
	decoded, err := base64.RawURLEncoding.DecodeString(value)
	if err != nil || len(decoded) == 0 {
		return nil, errors.New("invalid base64url integer")
	}
	return new(big.Int).SetBytes(decoded), nil
}

func oidcScopes(scopes string) []string {
	result := strings.Fields(scopes)
	if len(result) == 0 {
		return []string{"openid", "profile", "email"}
	}
	for _, scope := range result {
		if scope == "openid" {
			return result
		}
	}
	return append([]string{"openid"}, result...)
}

func contains(values []string, expected string) bool {
	for _, value := range values {
		if value == expected {
			return true
		}
	}
	return false
}

func randomURLSafeString(byteLength int) (string, error) {
	value := make([]byte, byteLength)
	if _, err := rand.Read(value); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(value), nil
}
