package router

import (
	"bytes"
	"compress/gzip"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/gin-gonic/gin"
)

// A response must never be labelled `Content-Encoding: gzip` unless its body
// really is gzip. gin-contrib/gzip's minimum-length path used to break that: a
// handler that set Content-Length below the threshold made the middleware skip
// compression after it had already announced gzip, and the plain body was
// committed with the header. Static handlers (`c.Data`, `http.ServeContent`)
// set Content-Length themselves, so small theme assets and uploads were
// unreadable for any client that offered gzip.
func TestAPICompressionNeverLabelsAPlainBody(t *testing.T) {
	gin.SetMode(gin.TestMode)

	payloads := map[string]string{
		"empty":  "",
		"tiny":   "body{color:red}",
		"under":  strings.Repeat("a", 1023),
		"at":     strings.Repeat("b", 1024),
		"large":  strings.Repeat("c", 4096),
		"binary": "\x00\x01\x02\x03",
	}

	engine := gin.New()
	group := engine.Group("/api")
	group.Use(noStoreAPI(), apiGzip())
	for name, body := range payloads {
		body := body
		// `c.Data` sets Content-Length before writing, which is what used to
		// trigger the broken path.
		group.GET("/"+name, func(c *gin.Context) {
			c.Data(http.StatusOK, "text/css", []byte(body))
		})
	}

	for name, body := range payloads {
		name, body := name, body
		t.Run(name, func(t *testing.T) {
			request := httptest.NewRequest(http.MethodGet, "/api/"+name, nil)
			request.Header.Set("Accept-Encoding", "gzip")
			recorder := httptest.NewRecorder()
			engine.ServeHTTP(recorder, request)

			response := recorder.Result()
			defer response.Body.Close()
			raw, err := io.ReadAll(response.Body)
			if err != nil {
				t.Fatalf("reading the body: %v", err)
			}

			encoding := response.Header.Get("Content-Encoding")
			if encoding == "" {
				if string(raw) != body {
					t.Fatalf("uncompressed body is %d bytes, want %d", len(raw), len(body))
				}
				return
			}
			if encoding != "gzip" {
				t.Fatalf("unexpected Content-Encoding %q", encoding)
			}
			reader, err := gzip.NewReader(bytes.NewReader(raw))
			if err != nil {
				t.Fatalf("a gzip-labelled body is not gzip (%d bytes): %v", len(raw), err)
			}
			decoded, err := io.ReadAll(reader)
			if err != nil {
				t.Fatalf("decoding the body: %v", err)
			}
			if err := reader.Close(); err != nil {
				t.Fatalf("closing the gzip reader: %v", err)
			}
			if string(decoded) != body {
				t.Fatalf("decoded body is %s, want %s", fmt.Sprintf("%d bytes", len(decoded)), fmt.Sprintf("%d bytes", len(body)))
			}
		})
	}
}
