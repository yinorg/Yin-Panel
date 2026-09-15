// Package storage contains the storage contract shared by the Core and
// Enterprise distributions. Business code must depend on this interface, not
// on a particular storage backend.
package storage

import (
	"context"
	"io"
)

// Storage persists private upload objects addressed by their object key.
type Storage interface {
	Exists(ctx context.Context, key string) (bool, error)
	Upload(ctx context.Context, reader io.Reader, key string) error
	Delete(ctx context.Context, key string) error
	Get(ctx context.Context, key string) ([]byte, error)
}

// Factory is used by a distribution extension to supply its storage backend.
// It is called after Core configuration and database initialization.
type Factory func(context.Context) (Storage, error)
