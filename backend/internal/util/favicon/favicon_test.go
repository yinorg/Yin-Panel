package favicon

import (
	"context"
	"errors"
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

	got, err := GetOneFaviconURL(context.Background(), server.URL+"/")
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

	got, err := GetOneFaviconURL(context.Background(), server.URL+"/page")
	if err != nil {
		t.Fatalf("GetOneFaviconURL() error = %v", err)
	}
	if want := server.URL + "/assets/site.ico"; got != want {
		t.Fatalf("GetOneFaviconURL() = %q, want %q", got, want)
	}
}

func TestGetOneFaviconURLKeepsThePageScheme(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html")
		_, _ = w.Write([]byte(`<link rel="icon" href="//cdn.example.com/site.ico">`))
	}))
	defer server.Close()

	got, err := GetOneFaviconURL(context.Background(), server.URL+"/page")
	if err != nil {
		t.Fatalf("GetOneFaviconURL() error = %v", err)
	}
	// A protocol-relative href must inherit the page's own scheme.
	if want := "http://cdn.example.com/site.ico"; got != want {
		t.Fatalf("GetOneFaviconURL() = %q, want %q", got, want)
	}
}

func TestGetOneFaviconURLReportsUnavailableForNonHTTPIcons(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html")
		_, _ = w.Write([]byte(`<link rel="icon" href="data:image/png;base64,iVBORw0KGgo=">`))
	}))
	defer server.Close()

	_, err := GetOneFaviconURL(context.Background(), server.URL+"/page")
	if !errors.Is(err, ErrIconUnavailable) {
		t.Fatalf("GetOneFaviconURL() error = %v, want ErrIconUnavailable", err)
	}
}

func TestGetOneFaviconURLFailsOnCancelledContext(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html")
		_, _ = w.Write([]byte(`<link rel="icon" href="/site.ico">`))
	}))
	defer server.Close()

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := GetOneFaviconURL(ctx, server.URL+"/page")
	if err == nil {
		t.Fatal("GetOneFaviconURL() error = nil, want a cancellation error")
	}
	// A cancelled fetch is a fault, not "this site has no icon".
	if errors.Is(err, ErrIconUnavailable) {
		t.Fatalf("GetOneFaviconURL() error = %v, must not be ErrIconUnavailable", err)
	}
}
