package theme

import (
	"bytes"
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"path"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"
)

const (
	DTCGVersion = "2025.10"
	MaxArchive  = 32 << 20
	MaxExpanded = 48 << 20
)

var (
	OfficialPublicKey = mustDecodeKey("c57e23e3f7bafbe6a797e45d73931e4cd82f7e3479c2de5be8c4c74322871c78")
	idPattern         = regexp.MustCompile(`^[a-z0-9][a-z0-9.-]{1,127}$`)
	versionPattern    = regexp.MustCompile(`^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$`)
)

type ResourceData struct {
	MediaType string
	Content   []byte
}

func safePackagePath(name string) bool {
	return name != "" && !strings.Contains(name, "\\") && !path.IsAbs(name) && path.Clean(name) == name && !strings.HasPrefix(name, "../") && name != "manifest.json" && name != "signature.ed25519"
}

func compareThemeVersion(left, right string) int {
	parse := func(value string) [3]int {
		core := strings.SplitN(strings.SplitN(value, "+", 2)[0], "-", 2)[0]
		parts := strings.Split(core, ".")
		version := [3]int{}
		for i := 0; i < len(parts) && i < len(version); i++ {
			version[i], _ = strconv.Atoi(parts[i])
		}
		return version
	}
	a, b := parse(left), parse(right)
	for i := range a {
		if a[i] < b[i] {
			return -1
		}
		if a[i] > b[i] {
			return 1
		}
	}
	return 0
}

func mustDecodeKey(value string) ed25519.PublicKey {
	key, _ := hex.DecodeString(value)
	return ed25519.PublicKey(key)
}

func strictJSON(data []byte, out any) error {
	d := json.NewDecoder(bytes.NewReader(data))
	d.DisallowUnknownFields()
	if err := d.Decode(out); err != nil {
		return err
	}
	var trailing any
	if err := d.Decode(&trailing); err != io.EOF {
		return errors.New("unexpected trailing JSON")
	}
	return nil
}

func signaturePayload(files map[string][]byte) ([]byte, error) {
	names := make([]string, 0, len(files)-1)
	for name := range files {
		if name != "signature.ed25519" {
			names = append(names, name)
		}
	}
	sort.Strings(names)
	var payload bytes.Buffer
	for _, name := range names {
		sum := sha256.Sum256(files[name])
		if _, err := fmt.Fprintf(&payload, "%s\x00%x\n", name, sum); err != nil {
			return nil, err
		}
	}
	return payload.Bytes(), nil
}

func validWallpaperBytes(asset ResourceData) bool {
	data := asset.Content
	switch asset.MediaType {
	case "image/png":
		return len(data) >= 8 && bytes.Equal(data[:8], []byte("\x89PNG\r\n\x1a\n"))
	case "image/jpeg":
		return len(data) >= 3 && bytes.Equal(data[:3], []byte("\xff\xd8\xff"))
	case "image/gif":
		return len(data) >= 6 && (string(data[:6]) == "GIF87a" || string(data[:6]) == "GIF89a")
	case "image/webp":
		return len(data) >= 12 && string(data[:4]) == "RIFF" && string(data[8:12]) == "WEBP"
	case "video/mp4":
		return len(data) >= 12 && len(data) <= 20<<20 && string(data[4:8]) == "ftyp"
	case "video/webm":
		return len(data) >= 4 && len(data) <= 20<<20 && bytes.Equal(data[:4], []byte("\x1a\x45\xdf\xa3"))
	case "text/html":
		return len(data) > 0 && len(data) <= 5<<20 && utf8.Valid(data)
	default:
		return false
	}
}

func isBuiltinThemeID(id string) bool {
	switch id {
	case "org.yin.default", "org.yin.glass":
		return true
	default:
		return false
	}
}
