package storage

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

// Local stores objects below a single local directory. It is the only storage
// implementation linked into the Core image.
type Local struct {
	root string
}

func NewLocal(root string) (*Local, error) {
	if strings.TrimSpace(root) == "" {
		return nil, fmt.Errorf("storage directory is required")
	}
	if err := os.MkdirAll(root, 0750); err != nil {
		return nil, fmt.Errorf("create storage directory: %w", err)
	}
	return &Local{root: root}, nil
}

func (s *Local) objectPath(key string) (string, error) {
	key = strings.TrimPrefix(filepath.ToSlash(key), "/")
	if key == "" || key == "." || strings.Contains(key, "..") {
		return "", fmt.Errorf("invalid object key")
	}
	path := filepath.Join(s.root, filepath.FromSlash(key))
	relative, err := filepath.Rel(s.root, path)
	if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(os.PathSeparator)) {
		return "", fmt.Errorf("invalid object key")
	}
	return path, nil
}

func (s *Local) Exists(_ context.Context, key string) (bool, error) {
	path, err := s.objectPath(key)
	if err != nil {
		return false, err
	}
	_, err = os.Stat(path)
	if os.IsNotExist(err) {
		return false, nil
	}
	return err == nil, err
}

func (s *Local) Upload(_ context.Context, reader io.Reader, key string) error {
	path, err := s.objectPath(key)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0750); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(filepath.Dir(path), ".upload-*")
	if err != nil {
		return err
	}
	tmpName := tmp.Name()
	defer os.Remove(tmpName)
	if _, err = io.Copy(tmp, reader); err != nil {
		tmp.Close()
		return err
	}
	if err = tmp.Close(); err != nil {
		return err
	}
	return os.Rename(tmpName, path)
}

func (s *Local) Delete(_ context.Context, key string) error {
	path, err := s.objectPath(key)
	if err != nil {
		return err
	}
	return os.Remove(path)
}

func (s *Local) Get(_ context.Context, key string) ([]byte, error) {
	path, err := s.objectPath(key)
	if err != nil {
		return nil, err
	}
	return os.ReadFile(path)
}
