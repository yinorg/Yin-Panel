package interceptor

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/gin-gonic/gin"
)

// The session token must be unreadable by page scripts, so the cookie that
// carries it has to be httpOnly. This is the property the whole "same-origin
// theme cannot read the token" boundary rests on.
func TestSetAuthCookieMarksSessionTokenHttpOnly(t *testing.T) {
	gin.SetMode(gin.TestMode)
	w := httptest.NewRecorder()
	c, _ := gin.CreateTestContext(w)
	c.Request = httptest.NewRequest(http.MethodPost, "/api/login", nil)

	SetAuthCookie(c, "session-token-value")

	raw := w.Header().Get("Set-Cookie")
	if raw == "" {
		t.Fatal("SetAuthCookie did not emit a Set-Cookie header")
	}
	if !strings.Contains(raw, AuthCookieName+"=session-token-value") {
		t.Fatalf("Set-Cookie does not carry the session token: %q", raw)
	}
	for _, want := range []string{"HttpOnly", "Path=/", "SameSite=Lax"} {
		if !strings.Contains(raw, want) {
			t.Errorf("Set-Cookie missing %q: %q", want, raw)
		}
	}
}

func TestClearAuthCookieExpiresTheSession(t *testing.T) {
	gin.SetMode(gin.TestMode)
	w := httptest.NewRecorder()
	c, _ := gin.CreateTestContext(w)
	c.Request = httptest.NewRequest(http.MethodPost, "/api/logout", nil)

	ClearAuthCookie(c)

	raw := w.Header().Get("Set-Cookie")
	if !strings.Contains(raw, AuthCookieName+"=") || !strings.Contains(raw, "Max-Age=0") {
		t.Fatalf("ClearAuthCookie did not expire the cookie: %q", raw)
	}
}
