package favicon

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestGetOneFaviconURLFallsBackToConventionalPath(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/favicon.ico" {
			w.Header().Set("Content-Type", "image/x-icon")
			_, _ = w.Write([]byte("icon"))
			return
		}
		w.Header().Set("Content-Type", "text/html")
		_, _ = w.Write([]byte("<!doctype html><html><head><title>test</title></head></html>"))
	}))
	defer server.Close()

	got, err := GetOneFaviconURL(server.URL + "/")
	if err != nil {
		t.Fatalf("GetOneFaviconURL() error = %v", err)
	}
	if want := server.URL + "/favicon.ico"; got != want {
		t.Fatalf("GetOneFaviconURL() = %q, want %q", got, want)
	}
}

func TestGetOneFaviconURLResolvesRelativePath(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html")
		_, _ = w.Write([]byte(`<link rel="icon" href="/assets/site.ico">`))
	}))
	defer server.Close()

	got, err := GetOneFaviconURL(server.URL + "/page")
	if err != nil {
		t.Fatalf("GetOneFaviconURL() error = %v", err)
	}
	if want := server.URL + "/assets/site.ico"; got != want {
		t.Fatalf("GetOneFaviconURL() = %q, want %q", got, want)
	}
}
