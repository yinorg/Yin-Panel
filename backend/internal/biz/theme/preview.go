package theme

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"net/url"
	"strings"
	"sync"
	"time"
)

type previewEntry struct {
	packageData *Package
	expires     time.Time
}

var previewStore = struct {
	sync.Mutex
	entries map[string]previewEntry
}{entries: map[string]previewEntry{}}

func SavePreview(pkg *Package) (string, PublicPackage, error) {
	bytes := make([]byte, 24)
	if _, err := rand.Read(bytes); err != nil {
		return "", PublicPackage{}, err
	}
	token := hex.EncodeToString(bytes)
	previewStore.Lock()
	defer previewStore.Unlock()
	for key, entry := range previewStore.entries {
		if time.Now().After(entry.expires) {
			delete(previewStore.entries, key)
		}
	}
	previewStore.entries[token] = previewEntry{packageData: pkg, expires: time.Now().Add(15 * time.Minute)}
	manifest := pkg.Manifest
	manifest.Resources = append([]Resource(nil), pkg.Manifest.Resources...)
	for i := range manifest.Resources {
		parts := strings.Split(manifest.Resources[i].Path, "/")
		for j := range parts {
			parts[j] = url.PathEscape(parts[j])
		}
		manifest.Resources[i].URL = "/api/theme/preview/assets/" + token + "/" + strings.Join(parts, "/")
	}
	return token, PublicPackage{Manifest: manifest, Documents: pkg.Documents, Verified: pkg.Verified}, nil
}

func PreviewAsset(token, name string) (ResourceData, error) {
	previewStore.Lock()
	defer previewStore.Unlock()
	entry, exists := previewStore.entries[token]
	if !exists || time.Now().After(entry.expires) {
		delete(previewStore.entries, token)
		return ResourceData{}, errors.New("preview expired")
	}
	asset, exists := entry.packageData.Resources[name]
	if !exists {
		return ResourceData{}, errors.New("preview asset is missing")
	}
	return asset, nil
}

func PreviewPackage(token string) (PublicPackage, error) {
	previewStore.Lock()
	defer previewStore.Unlock()
	entry, exists := previewStore.entries[token]
	if !exists || time.Now().After(entry.expires) {
		delete(previewStore.entries, token)
		return PublicPackage{}, errors.New("preview expired")
	}
	manifest := entry.packageData.Manifest
	manifest.Resources = append([]Resource(nil), manifest.Resources...)
	for i := range manifest.Resources {
		parts := strings.Split(manifest.Resources[i].Path, "/")
		for j := range parts {
			parts[j] = url.PathEscape(parts[j])
		}
		manifest.Resources[i].URL = "/api/theme/preview/assets/" + token + "/" + strings.Join(parts, "/")
	}
	return PublicPackage{Manifest: manifest, Documents: entry.packageData.Documents, Verified: entry.packageData.Verified}, nil
}
