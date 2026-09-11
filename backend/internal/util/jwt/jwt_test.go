package jwt

import (
	"testing"
	"time"

	jwtv5 "github.com/golang-jwt/jwt/v5"
)

func TestNonPositiveExpirationUsesNinetyDayDefault(t *testing.T) {
	originalExpireHours := expireHours
	t.Cleanup(func() { expireHours = originalExpireHours })
	InitSecret("test-secret")

	SetExpire(0)
	token, err := GenerateToken(1, 0)
	if err != nil {
		t.Fatal(err)
	}
	claims, err := ParseToken(token)
	if err != nil {
		t.Fatal(err)
	}
	want := time.Now().Add(DefaultExpireHours * time.Hour)
	if difference := claims.ExpiresAt.Time.Sub(want); difference < -2*time.Second || difference > 2*time.Second {
		t.Fatalf("default token lifetime is %s, want %s", claims.ExpiresAt.Time.Sub(time.Now()), DefaultExpireHours*time.Hour)
	}

	SetExpire(-1)
	if expireHours != DefaultExpireHours {
		t.Fatalf("negative expiry set %d hours, want default %d", expireHours, DefaultExpireHours)
	}
}

func TestTokenVersionAndSigningAlgorithmAreValidated(t *testing.T) {
	InitSecret("test-secret")
	SetExpire(1)

	token, err := GenerateToken(7, 3)
	if err != nil {
		t.Fatal(err)
	}
	claims, err := ParseToken(token)
	if err != nil {
		t.Fatal(err)
	}
	if claims.UserID != 7 || claims.TokenVersion == nil || *claims.TokenVersion != 3 {
		t.Fatalf("unexpected token claims: %#v", claims)
	}

	wrongAlgorithm := jwtv5.NewWithClaims(jwtv5.SigningMethodHS384, Claims{UserID: 7})
	wrongToken, err := wrongAlgorithm.SignedString([]byte("test-secret"))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := ParseToken(wrongToken); err == nil {
		t.Fatal("HS384 token was accepted")
	}
}
