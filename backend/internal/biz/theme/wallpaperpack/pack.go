// Package wallpaperpack ships the Core's built-in wallpaper library.
//
// The library belongs to the Core rather than to a theme: every theme gets it
// for free, its assets are stored once in the binary instead of being copied
// into every theme revision row, and the picker's selection therefore keeps
// working when the user switches theme.
//
// `assets/` holds the catalogue and the artwork it names. The catalogue is
// content, not code: nothing here trusts a URL, and every entry is checked
// against the packaged files (and their media types) while loading, so an
// entry that is missing or mislabelled is dropped rather than served.
package wallpaperpack

import (
	"embed"
	"encoding/json"
	"errors"
	"io/fs"
	"path"
	"path/filepath"
	"strings"
	"sync"
)

//go:embed assets
var packFS embed.FS

const root = "assets"

// Entry is one wallpaper the picker can apply. `Kind` decides how the Core's
// wallpaper layer paints it: an `image` is drawn like any photo, a `page` is an
// HTML document mounted in a sandboxed frame (the asset route serves those with
// a `sandbox allow-scripts` policy).
type Entry struct {
	ID     string `json:"id"`
	Name   string `json:"name"`
	Kind   string `json:"kind"`
	Source string `json:"source"`
	Poster string `json:"poster"`
}

// Category groups entries the way the catalogue arranges them.
type Category struct {
	ID    string  `json:"id"`
	Name  string  `json:"name"`
	Items []Entry `json:"items"`
}

// Pack is the packaged index: a version (part of every asset URL, so the URLs
// stay immutable) and the categories with their entries.
type Pack struct {
	Version    string     `json:"version"`
	Categories []Category `json:"categories"`
}

var (
	once      sync.Once
	catalogue Pack
	assets    map[string]string
	loadErr   error
)

func load() {
	assets = map[string]string{}
	content, err := packFS.ReadFile(root + "/catalogue.json")
	if err != nil {
		loadErr = err
		return
	}
	var document Pack
	if err := json.Unmarshal(content, &document); err != nil {
		loadErr = err
		return
	}
	if document.Version == "" {
		loadErr = errors.New("wallpaper pack catalogue has no version")
		return
	}
	err = fs.WalkDir(packFS, root, func(name string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil || entry.IsDir() || name == root+"/catalogue.json" {
			return walkErr
		}
		mediaType := mediaTypeFor(entry.Name())
		if mediaType == "" {
			return nil
		}
		assets[strings.TrimPrefix(name, root+"/")] = mediaType
		return nil
	})
	if err != nil {
		loadErr = err
		return
	}
	kept := make([]Category, 0, len(document.Categories))
	for _, category := range document.Categories {
		items := make([]Entry, 0, len(category.Items))
		for _, item := range category.Items {
			if valid(item, assets) != nil {
				continue
			}
			items = append(items, item)
		}
		category.Items = items
		kept = append(kept, category)
	}
	document.Categories = kept
	catalogue = document
}

// valid accepts an entry only when both its files are packaged and their media
// types match the entry's kind, so a `page` can never point at an image (or the
// other way round) and nothing outside the pack can be named.
func valid(entry Entry, assets map[string]string) error {
	if entry.ID == "" || entry.Name == "" || entry.Source == "" {
		return errors.New("incomplete wallpaper entry")
	}
	switch entry.Kind {
	case "page":
		if assets[entry.Source] != "text/html" {
			return errors.New("wallpaper page is not packaged html")
		}
	case "image":
		if !strings.HasPrefix(assets[entry.Source], "image/") {
			return errors.New("wallpaper image is not packaged artwork")
		}
	default:
		return errors.New("unknown wallpaper kind")
	}
	if entry.Poster != "" && !strings.HasPrefix(assets[entry.Poster], "image/") {
		return errors.New("wallpaper poster is not packaged artwork")
	}
	return nil
}

func mediaTypeFor(name string) string {
	switch strings.ToLower(filepath.Ext(name)) {
	case ".jpg", ".jpeg":
		return "image/jpeg"
	case ".png":
		return "image/png"
	case ".html", ".htm":
		return "text/html"
	}
	return ""
}

// Catalogue returns the packaged index.
func Catalogue() (Pack, error) {
	once.Do(load)
	return catalogue, loadErr
}

// Find returns the packaged entry with this id. Callers use it to check a
// stored selection before saving it, so a config can never name an entry the
// library does not have.
func Find(id string) (Entry, bool) {
	once.Do(load)
	if loadErr != nil || id == "" {
		return Entry{}, false
	}
	for _, category := range catalogue.Categories {
		for _, item := range category.Items {
			if item.ID == id {
				return item, true
			}
		}
	}
	return Entry{}, false
}

// Asset returns one packaged file and the media type to serve it with. The name
// is resolved against the loaded index, so a traversal or an unpackaged path is
// simply absent.
func Asset(name string) ([]byte, string, bool) {
	once.Do(load)
	if loadErr != nil {
		return nil, "", false
	}
	name = strings.TrimPrefix(name, "/")
	if name == "" || path.Clean(name) != name || strings.HasPrefix(name, "../") || strings.Contains(name, "\\") {
		return nil, "", false
	}
	mediaType, ok := assets[name]
	if !ok {
		return nil, "", false
	}
	content, err := packFS.ReadFile(root + "/" + name)
	if err != nil {
		return nil, "", false
	}
	return content, mediaType, true
}
