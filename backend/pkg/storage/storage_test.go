package storage

import (
	"bytes"
	"context"
	"strings"
	"testing"
)

func TestContentAddressedKey(t *testing.T) {
	digest := "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
	key, err := ContentAddressedKey(digest, "PNG")
	if err != nil {
		t.Fatal(err)
	}
	if want := "ab/cd/abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789.png"; key != want {
		t.Fatalf("key = %q, want %q", key, want)
	}
}

func TestLocalStorageRejectsTraversalAndRoundTrips(t *testing.T) {
	store, err := NewLocal(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	ctx := context.Background()
	if err := store.Upload(ctx, bytes.NewBufferString("content"), "ab/cd/file.png"); err != nil {
		t.Fatal(err)
	}
	exists, err := store.Exists(ctx, "ab/cd/file.png")
	if err != nil || !exists {
		t.Fatalf("exists = %v, %v", exists, err)
	}
	data, err := store.Get(ctx, "ab/cd/file.png")
	if err != nil || string(data) != "content" {
		t.Fatalf("get = %q, %v", data, err)
	}
	if _, err := store.Get(ctx, "../outside"); err == nil {
		t.Fatal("expected traversal key to be rejected")
	}
}

func TestLocalStorageMissingAndDelete(t *testing.T) {
	store, err := NewLocal(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	ctx := context.Background()
	exists, err := store.Exists(ctx, "wallpaper.webm")
	if err != nil || exists {
		t.Fatalf("missing object: exists=%v err=%v", exists, err)
	}
	if _, err := store.Get(ctx, "wallpaper.webm"); err == nil {
		t.Fatal("expected an error reading a missing object")
	}
	if err := store.Upload(ctx, strings.NewReader("video"), "wallpaper.webm"); err != nil {
		t.Fatal(err)
	}
	if err := store.Delete(ctx, "wallpaper.webm"); err != nil {
		t.Fatal(err)
	}
	exists, err = store.Exists(ctx, "wallpaper.webm")
	if err != nil || exists {
		t.Fatalf("deleted object: exists=%v err=%v", exists, err)
	}
}
