// Package extension defines the public Core integration points used by the
// private Enterprise distribution. Core itself does not register extensions.
package extension

import (
	"context"
	"fmt"
	"sync"

	"github.com/gin-gonic/gin"
	"github.com/yinorg/Yin-Panel/backend/pkg/storage"
	"gorm.io/gorm"
)

type Capability struct {
	Name     string `json:"name"`
	ReadOnly bool   `json:"readOnly"`
}

// ActorContextKey contains an Actor set by Core authentication middleware.
// Extensions must treat it as optional because public requests are unauthenticated.
const ActorContextKey = "yin-panel.actor"

type Actor struct {
	ID       uint
	Username string
}

// Module is registered by a distribution entrypoint before app.Run. Init can
// validate a license or prepare migrations. A module that is not registered
// has no routes or APIs in the resulting binary.
type Module struct {
	Name           string
	Capabilities   []Capability
	Init           func(context.Context, Runtime) error
	StorageFactory storage.Factory
	Middleware     []gin.HandlerFunc
	RegisterRoutes func(*gin.RouterGroup)
}

// Runtime exposes the intentionally small set of Core services that an
// extension needs for its own migrations and persistence. Enterprise tables
// remain owned by the extension.
type Runtime struct {
	DB *gorm.DB
}

var registry struct {
	sync.RWMutex
	modules  []Module
	edition  string
	readOnly bool
}

// ConfigureEdition sets the distribution metadata returned by the capability
// endpoint. It does not grant access to any module; only Register does that.
func ConfigureEdition(edition string, readOnly bool) error {
	if edition == "" {
		return fmt.Errorf("edition is required")
	}
	registry.Lock()
	defer registry.Unlock()
	registry.edition = edition
	registry.readOnly = readOnly
	return nil
}

func Edition() (string, bool) {
	registry.RLock()
	defer registry.RUnlock()
	if registry.edition == "" {
		return "core", false
	}
	return registry.edition, registry.readOnly
}

func Register(module Module) error {
	if module.Name == "" {
		return fmt.Errorf("extension module name is required")
	}
	registry.Lock()
	defer registry.Unlock()
	for _, registered := range registry.modules {
		if registered.Name == module.Name {
			return fmt.Errorf("extension module %q already registered", module.Name)
		}
	}
	registry.modules = append(registry.modules, module)
	return nil
}

// Modules returns a snapshot so app startup cannot be affected by later
// registrations.
func Modules() []Module {
	registry.RLock()
	defer registry.RUnlock()
	return append([]Module(nil), registry.modules...)
}

func Capabilities() []Capability {
	modules := Modules()
	capabilities := make([]Capability, 0)
	for _, module := range modules {
		capabilities = append(capabilities, module.Capabilities...)
	}
	return capabilities
}
