package storage

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
	"go.uber.org/zap"
)

func TestRcloneExistsTreatsMissingObjectAsAbsent(t *testing.T) {
	previousLogger := zaplog.Logger
	zaplog.Logger = zap.NewNop().Sugar()
	t.Cleanup(func() { zaplog.Logger = previousLogger })
	root := t.TempDir()
	configPath := filepath.Join(root, "conf.yaml")
	if err := os.WriteFile(configPath, []byte("rclone:\n  rclone.conf: |-\n    type = local\n"), 0600); err != nil {
		t.Fatal(err)
	}
	store, err := NewRcloneStorage(context.Background(), configPath, filepath.Join(root, "uploads"))
	if err != nil {
		t.Fatal(err)
	}
	exists, err := store.Exists(context.Background(), "wallpaper.webm")
	if err != nil || exists {
		t.Fatalf("missing object: exists=%v err=%v", exists, err)
	}
	if err := store.Upload(context.Background(), strings.NewReader("video"), "wallpaper.webm"); err != nil {
		t.Fatal(err)
	}
	exists, err = store.Exists(context.Background(), "wallpaper.webm")
	if err != nil || !exists {
		t.Fatalf("uploaded object: exists=%v err=%v", exists, err)
	}
}
