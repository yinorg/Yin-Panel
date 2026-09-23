package theme

import (
	"archive/zip"
	"bytes"
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"net/url"
	"path"
	"regexp"
	"sort"
	"strings"
	"unicode/utf8"
)

const (
	FormatVersion = 1
	APIVersion    = "2"
	DTCGVersion   = "2025.10"
	MaxArchive    = 32 << 20
	MaxExpanded   = 48 << 20
)

var requiredSlots = map[string]string{
	"canvas": "color", "surface": "color", "surfaceElevated": "color", "text": "color",
	"textMuted": "color", "border": "color", "primary": "color", "onPrimary": "color",
	"secondary": "color", "success": "color", "warning": "color", "danger": "color", "focusRing": "color",
}

var (
	idPattern      = regexp.MustCompile(`^[a-z0-9][a-z0-9.-]{1,127}$`)
	versionPattern = regexp.MustCompile(`^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$`)
)

// OfficialPublicKey is the compiled trust anchor. It is intentionally only a
// public key; official signing material is never shipped with Yin-Panel.
var OfficialPublicKey = mustDecodeKey("c57e23e3f7bafbe6a797e45d73931e4cd82f7e3479c2de5be8c4c74322871c78")

type Resource struct {
	Path      string `json:"path"`
	SHA256    string `json:"sha256"`
	MediaType string `json:"mediaType"`
	URL       string `json:"url,omitempty"`
}

type Wallpaper struct {
	Kind   string `json:"kind"`
	Source string `json:"source"`
	Poster string `json:"poster,omitempty"`
}

type FontResource struct {
	Family string `json:"family"`
	Path   string `json:"path"`
	Weight int    `json:"weight"`
	Style  string `json:"style"`
}

type Manifest struct {
	Format         string               `json:"format"`
	FormatVersion  int                  `json:"formatVersion"`
	ID             string               `json:"id"`
	Name           string               `json:"name"`
	PackageVersion string               `json:"packageVersion"`
	APIVersion     string               `json:"apiVersion"`
	DTCGVersion    string               `json:"dtcgVersion"`
	Schemes        []string             `json:"schemes"`
	Documents      map[string]string    `json:"documents"`
	Bindings       map[string]string    `json:"bindings"`
	Resources      []Resource           `json:"resources"`
	Wallpapers     map[string]Wallpaper `json:"wallpapers,omitempty"`
	Fonts          []FontResource       `json:"fonts,omitempty"`
}

type Package struct {
	Manifest  Manifest                   `json:"manifest"`
	Documents map[string]json.RawMessage `json:"documents"`
	Resources map[string]ResourceData    `json:"resources,omitempty"`
	Verified  bool                       `json:"verified"`
}

type ResourceData struct {
	MediaType string `json:"mediaType"`
	Content   []byte `json:"-"`
}

func mustDecodeKey(value string) ed25519.PublicKey {
	key, _ := hex.DecodeString(value)
	return ed25519.PublicKey(key)
}

func ParseArchive(data []byte, confirmUnverified bool) (*Package, error) {
	return parseArchive(data, confirmUnverified, OfficialPublicKey)
}

func parseArchive(data []byte, confirmUnverified bool, trustedKey ed25519.PublicKey) (*Package, error) {
	if len(data) == 0 || len(data) > MaxArchive {
		return nil, errors.New("theme archive must be between 1 byte and 32 MiB")
	}
	zr, err := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if err != nil {
		return nil, fmt.Errorf("invalid ZIP archive: %w", err)
	}
	if len(zr.File) > 64 {
		return nil, errors.New("theme archive contains too many files")
	}
	files := make(map[string][]byte, len(zr.File))
	expanded := int64(0)
	for _, file := range zr.File {
		name := strings.TrimPrefix(file.Name, "./")
		if name == "" || strings.Contains(name, "\\") || path.IsAbs(name) || path.Clean(name) != name || name == ".." || strings.HasPrefix(name, "../") || file.Mode()&0o170000 == 0o120000 {
			return nil, fmt.Errorf("unsafe theme archive path %q", file.Name)
		}
		if file.FileInfo().IsDir() {
			continue
		}
		if _, exists := files[name]; exists {
			return nil, fmt.Errorf("duplicate archive path %q", name)
		}
		expanded += int64(file.UncompressedSize64)
		if expanded > MaxExpanded {
			return nil, errors.New("expanded theme archive exceeds 48 MiB")
		}
		r, err := file.Open()
		if err != nil {
			return nil, err
		}
		content, readErr := io.ReadAll(io.LimitReader(r, MaxExpanded+1))
		closeErr := r.Close()
		if readErr != nil {
			return nil, readErr
		}
		if closeErr != nil {
			return nil, closeErr
		}
		if int64(len(content)) > MaxExpanded {
			return nil, errors.New("expanded theme archive exceeds 48 MiB")
		}
		files[name] = content
	}
	manifestBytes, ok := files["manifest.json"]
	if !ok {
		return nil, errors.New("manifest.json is required")
	}
	var manifest Manifest
	if err := strictJSON(manifestBytes, &manifest); err != nil {
		return nil, fmt.Errorf("invalid theme manifest: %w", err)
	}
	if err := validateManifest(manifest); err != nil {
		return nil, err
	}
	pkg := &Package{Manifest: manifest, Documents: map[string]json.RawMessage{}, Resources: map[string]ResourceData{}}
	used := map[string]bool{"manifest.json": true, "signature.ed25519": true}
	for _, scheme := range manifest.Schemes {
		name := manifest.Documents[scheme]
		doc, exists := files[name]
		if !exists {
			return nil, fmt.Errorf("missing token document %q", name)
		}
		used[name] = true
		if !json.Valid(doc) {
			return nil, fmt.Errorf("token document %q is not valid JSON", name)
		}
		pkg.Documents[scheme] = append(json.RawMessage(nil), doc...)
	}
	for _, resource := range manifest.Resources {
		content, exists := files[resource.Path]
		if !exists {
			return nil, fmt.Errorf("missing resource %q", resource.Path)
		}
		used[resource.Path] = true
		sum := sha256.Sum256(content)
		if !strings.EqualFold(hex.EncodeToString(sum[:]), resource.SHA256) {
			return nil, fmt.Errorf("resource digest mismatch for %q", resource.Path)
		}
		if !allowedResource(resource.MediaType, resource.Path) {
			return nil, fmt.Errorf("resource type is not allowed for %q", resource.Path)
		}
		pkg.Resources[resource.Path] = ResourceData{MediaType: resource.MediaType, Content: content}
	}
	for name := range files {
		if !used[name] {
			return nil, fmt.Errorf("undeclared theme archive file %q", name)
		}
	}
	if err := Validate(pkg); err != nil {
		return nil, err
	}
	if err := validateWallpaperAssets(pkg); err != nil {
		return nil, err
	}
	signed := false
	if signature, exists := files["signature.ed25519"]; exists {
		decoded, err := base64.StdEncoding.DecodeString(strings.TrimSpace(string(signature)))
		if err != nil || len(decoded) != ed25519.SignatureSize {
			return nil, errors.New("invalid official signature encoding")
		}
		payload, err := signaturePayload(files)
		if err != nil {
			return nil, err
		}
		if !ed25519.Verify(trustedKey, payload, decoded) {
			return nil, errors.New("theme signature is invalid")
		}
		signed = true
	}
	if !signed && !confirmUnverified {
		return nil, errors.New("theme is unverified; explicit confirmation is required")
	}
	pkg.Verified = signed
	return pkg, nil
}

func validateWallpaperAssets(pkg *Package) error {
	for _, font := range pkg.Manifest.Fonts {
		content := pkg.Resources[font.Path].Content
		if len(content) < 4 || string(content[:4]) != "wOF2" {
			return fmt.Errorf("font %q is not WOFF2", font.Path)
		}
	}
	for _, wallpaper := range pkg.Manifest.Wallpapers {
		if wallpaper.Kind != "externalUrl" && !validWallpaperBytes(pkg.Resources[wallpaper.Source]) {
			return fmt.Errorf("wallpaper source %q has invalid content", wallpaper.Source)
		}
		if wallpaper.Poster != "" && !validWallpaperBytes(pkg.Resources[wallpaper.Poster]) {
			return fmt.Errorf("wallpaper poster %q has invalid content", wallpaper.Poster)
		}
	}
	return nil
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

func validateManifest(m Manifest) error {
	if m.Format != "yin-theme" || m.FormatVersion != FormatVersion || (m.APIVersion != "1" && m.APIVersion != APIVersion) || m.DTCGVersion != DTCGVersion {
		return errors.New("unsupported theme format, package API, or DTCG version")
	}
	if !idPattern.MatchString(m.ID) || m.ID == "org.yin.default" || strings.TrimSpace(m.Name) == "" || len(m.Name) > 100 || !versionPattern.MatchString(m.PackageVersion) {
		return errors.New("invalid theme identity or reserved package ID")
	}
	if len(m.Schemes) == 0 || len(m.Schemes) > 2 || len(m.Documents) != len(m.Schemes) {
		return errors.New("theme must declare one or two complete schemes")
	}
	seen := map[string]bool{}
	for _, scheme := range m.Schemes {
		if (scheme != "light" && scheme != "dark") || seen[scheme] || m.Documents[scheme] == "" {
			return errors.New("theme schemes must be unique light/dark values with a document each")
		}
		seen[scheme] = true
	}
	expectedSlots := slotTypes(m.APIVersion)
	if len(m.Bindings) != len(expectedSlots) {
		return errors.New("theme must bind every required Yin semantic slot exactly once")
	}
	for slot, pointer := range m.Bindings {
		if _, ok := expectedSlots[slot]; !ok || !strings.HasPrefix(pointer, "/") {
			return fmt.Errorf("invalid semantic binding %q", slot)
		}
	}
	paths := map[string]bool{}
	for scheme, name := range m.Documents {
		if !safePackagePath(name) || paths[name] {
			return fmt.Errorf("invalid or duplicate token document path for %s", scheme)
		}
		paths[name] = true
	}
	resourceTypes := map[string]string{}
	for _, resource := range m.Resources {
		if !safePackagePath(resource.Path) || resource.URL != "" || paths[resource.Path] || len(resource.SHA256) != 64 || !regexp.MustCompile(`^[0-9a-fA-F]{64}$`).MatchString(resource.SHA256) {
			return fmt.Errorf("invalid or duplicate resource metadata for %q", resource.Path)
		}
		paths[resource.Path] = true
		resourceTypes[resource.Path] = resource.MediaType
	}
	if m.APIVersion == "1" && len(m.Wallpapers) != 0 {
		return errors.New("wallpapers require theme package API v2")
	}
	if len(m.Fonts) > 8 {
		return errors.New("theme contains too many font faces")
	}
	for _, font := range m.Fonts {
		if !safeFontName.MatchString(font.Family) || resourceTypes[font.Path] != "font/woff2" || font.Weight < 100 || font.Weight > 900 || font.Weight%100 != 0 || (font.Style != "normal" && font.Style != "italic") {
			return fmt.Errorf("invalid font resource %q", font.Path)
		}
	}
	if len(m.Wallpapers) != 0 && len(m.Wallpapers) != len(m.Schemes) {
		return errors.New("wallpaper defaults must cover every declared scheme")
	}
	for _, scheme := range m.Schemes {
		wallpaper, exists := m.Wallpapers[scheme]
		if !exists && len(m.Wallpapers) != 0 {
			return fmt.Errorf("wallpaper default for %s is missing", scheme)
		}
		if !exists {
			continue
		}
		if err := validateWallpaper(wallpaper, resourceTypes); err != nil {
			return fmt.Errorf("wallpaper %s: %w", scheme, err)
		}
	}
	return nil
}

func validateWallpaper(w Wallpaper, resources map[string]string) error {
	if w.Source == "" {
		return errors.New("source is required")
	}
	switch w.Kind {
	case "image":
		if !strings.HasPrefix(resources[w.Source], "image/") {
			return errors.New("image source must be a declared image")
		}
	case "video":
		if resources[w.Source] != "video/mp4" && resources[w.Source] != "video/webm" {
			return errors.New("video source must be a declared video")
		}
	case "webBundle":
		if resources[w.Source] != "text/html" {
			return errors.New("web wallpaper source must be declared HTML")
		}
	case "externalUrl":
		u, err := url.Parse(w.Source)
		if err != nil || u.Scheme != "https" || u.Hostname() == "" || u.User != nil || u.Fragment != "" || len(w.Source) > 2048 {
			return errors.New("external wallpaper must use a plain HTTPS URL")
		}
	default:
		return errors.New("unknown wallpaper kind")
	}
	if w.Poster == "" && w.Kind != "image" {
		return errors.New("dynamic wallpaper requires a poster")
	}
	if w.Poster != "" && !strings.HasPrefix(resources[w.Poster], "image/") {
		return errors.New("poster must be a declared image")
	}
	return nil
}

func safePackagePath(name string) bool {
	return name != "" && !strings.Contains(name, "\\") && !path.IsAbs(name) && path.Clean(name) == name && !strings.HasPrefix(name, "../") && name != "manifest.json" && name != "signature.ed25519"
}

func allowedResource(mediaType, name string) bool {
	allowed := map[string]string{"image/png": ".png", "image/jpeg": ".jpg", "image/webp": ".webp", "image/gif": ".gif", "font/woff2": ".woff2", "video/mp4": ".mp4", "video/webm": ".webm", "text/html": ".html"}
	return allowed[mediaType] != "" && strings.EqualFold(path.Ext(name), allowed[mediaType]) && ((mediaType != "text/html" && !strings.HasPrefix(mediaType, "video/")) || strings.HasPrefix(name, "wallpaper/"))
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

func Validate(pkg *Package) error {
	expectedSlots := slotTypes(pkg.Manifest.APIVersion)
	for _, scheme := range pkg.Manifest.Schemes {
		var doc any
		decoder := json.NewDecoder(bytes.NewReader(pkg.Documents[scheme]))
		decoder.UseNumber()
		if err := decoder.Decode(&doc); err != nil {
			return err
		}
		root, ok := doc.(map[string]any)
		if !ok || root["$schema"] != "https://design-tokens.github.io/community-group/format/2025.10/schema.json" {
			return fmt.Errorf("scheme %s must declare the DTCG 2025.10 schema", scheme)
		}
		flat := map[string]token{}
		flatten(doc, "", "", flat)
		for name := range flat {
			if _, err := resolveToken(name, flat, map[string]bool{}); err != nil {
				return fmt.Errorf("scheme %s: %w", scheme, err)
			}
		}
		resolvedSlots := map[string]any{}
		for slot, pointer := range pkg.Manifest.Bindings {
			name, err := pointerTokenName(doc, pointer)
			if err != nil {
				return fmt.Errorf("binding %s: %w", slot, err)
			}
			t, err := resolveToken(name, flat, map[string]bool{})
			if err != nil {
				return fmt.Errorf("binding %s: %w", slot, err)
			}
			if t.typ != expectedSlots[slot] {
				return fmt.Errorf("binding %s must reference a %s token", slot, expectedSlots[slot])
			}
			if t.typ == "color" {
				if _, err := parseColor(t.value); err != nil {
					return fmt.Errorf("binding %s: %w", slot, err)
				}
			} else if err := validateDesignSlot(slot, t.value); err != nil {
				return err
			}
			resolvedSlots[slot] = t.value
		}
		if pkg.Manifest.APIVersion == APIVersion {
			mobile, _, _ := dimension(resolvedSlots["breakpointMobile"])
			tablet, _, _ := dimension(resolvedSlots["breakpointTablet"])
			if mobile >= tablet {
				return fmt.Errorf("scheme %s has unordered breakpoints", scheme)
			}
		}
		if err := validateContrast(doc, flat, pkg.Manifest.Bindings); err != nil {
			return fmt.Errorf("scheme %s: %w", scheme, err)
		}
	}
	return nil
}

type token struct {
	typ   string
	value any
}

func flatten(value any, prefix, inherited string, out map[string]token) {
	obj, ok := value.(map[string]any)
	if !ok {
		return
	}
	typ := inherited
	if v, ok := obj["$type"].(string); ok {
		typ = v
	}
	if raw, hasValue := obj["$value"]; hasValue {
		out[prefix] = token{typ: typ, value: raw}
	}
	for key, child := range obj {
		if strings.HasPrefix(key, "$") {
			continue
		}
		part := strings.ReplaceAll(strings.ReplaceAll(key, "~", "~0"), "/", "~1")
		name := part
		if prefix != "" {
			name = prefix + "." + part
		}
		flatten(child, name, typ, out)
	}
}

func resolveToken(name string, tokens map[string]token, visiting map[string]bool) (token, error) {
	t, ok := tokens[name]
	if !ok {
		return token{}, fmt.Errorf("unknown token reference %q", name)
	}
	if visiting[name] {
		return token{}, fmt.Errorf("token reference cycle at %q", name)
	}
	visiting[name] = true
	defer delete(visiting, name)
	if ref, ok := referenceName(t.value); ok {
		resolved, err := resolveToken(ref, tokens, visiting)
		if err != nil {
			return token{}, err
		}
		if t.typ == "" {
			t.typ = resolved.typ
		}
		if t.typ != resolved.typ {
			return token{}, fmt.Errorf("reference %q changes token type", name)
		}
		t.value = resolved.value
	} else {
		value, err := resolveNestedReferences(t.value, tokens, visiting)
		if err != nil {
			return token{}, err
		}
		t.value = value
	}
	if t.typ == "" {
		return token{}, fmt.Errorf("token %q has no inherited $type", name)
	}
	if !validTypedValue(t.typ, t.value) {
		return token{}, fmt.Errorf("token %q has invalid %s value", name, t.typ)
	}
	return t, nil
}

func referenceName(value any) (string, bool) {
	ref, ok := value.(string)
	if !ok || len(ref) < 3 || ref[0] != '{' || ref[len(ref)-1] != '}' || strings.ContainsAny(ref[1:len(ref)-1], "{}") {
		return "", false
	}
	return strings.ReplaceAll(strings.TrimSpace(ref[1:len(ref)-1]), "/", "."), true
}

func resolveNestedReferences(value any, tokens map[string]token, visiting map[string]bool) (any, error) {
	switch current := value.(type) {
	case string:
		if name, ok := referenceName(current); ok {
			resolved, err := resolveToken(name, tokens, visiting)
			if err != nil {
				return nil, err
			}
			return resolved.value, nil
		}
		return current, nil
	case []any:
		result := make([]any, len(current))
		for i, item := range current {
			resolved, err := resolveNestedReferences(item, tokens, visiting)
			if err != nil {
				return nil, err
			}
			result[i] = resolved
		}
		return result, nil
	case map[string]any:
		result := make(map[string]any, len(current))
		for key, item := range current {
			resolved, err := resolveNestedReferences(item, tokens, visiting)
			if err != nil {
				return nil, err
			}
			result[key] = resolved
		}
		return result, nil
	default:
		return value, nil
	}
}

func validTypedValue(typ string, value any) bool {
	switch typ {
	case "color":
		_, err := parseColor(value)
		return err == nil
	case "dimension":
		v, ok := value.(map[string]any)
		if !ok {
			return false
		}
		_, n := v["value"].(json.Number)
		_, u := v["unit"].(string)
		return n && u
	case "number":
		_, ok := value.(json.Number)
		return ok
	case "fontFamily":
		_, ok := value.(string)
		if ok {
			return true
		}
		_, ok = value.([]any)
		return ok
	case "boolean":
		_, ok := value.(bool)
		return ok
	case "string", "asset":
		_, ok := value.(string)
		return ok
	case "fontWeight":
		switch v := value.(type) {
		case string:
			return v == "thin" || v == "hairline" || v == "extra-light" || v == "ultra-light" || v == "light" || v == "normal" || v == "regular" || v == "medium" || v == "semi-bold" || v == "demi-bold" || v == "bold" || v == "extra-bold" || v == "ultra-bold" || v == "black" || v == "heavy" || v == "extra-black" || v == "ultra-black"
		case json.Number:
			n, err := v.Float64()
			return err == nil && n > 0
		default:
			return false
		}
	case "duration":
		v, ok := value.(map[string]any)
		if !ok {
			return false
		}
		_, number := v["value"].(json.Number)
		unit, unitOK := v["unit"].(string)
		return number && unitOK && (unit == "ms" || unit == "s")
	case "cubicBezier":
		values, ok := value.([]any)
		if !ok || len(values) != 4 {
			return false
		}
		for _, component := range values {
			if _, ok := component.(json.Number); !ok {
				return false
			}
		}
		return true
	case "strokeStyle":
		v, ok := value.(string)
		if !ok {
			_, ok = value.(map[string]any)
			return ok
		}
		return v == "solid" || v == "dashed" || v == "dotted" || v == "double" || v == "groove" || v == "ridge" || v == "inset" || v == "outset"
	case "border", "transition", "typography":
		_, ok := value.(map[string]any)
		return ok
	case "shadow":
		switch value.(type) {
		case map[string]any, []any:
			return true
		default:
			return false
		}
	case "gradient":
		_, ok := value.([]any)
		return ok
	default:
		return false
	}
}

func pointerTokenName(doc any, pointer string) (string, error) {
	parts := strings.Split(strings.TrimPrefix(pointer, "/"), "/")
	current := doc
	decoded := make([]string, 0, len(parts))
	for _, part := range parts {
		part = strings.ReplaceAll(strings.ReplaceAll(part, "~1", "/"), "~0", "~")
		obj, ok := current.(map[string]any)
		if !ok {
			return "", errors.New("JSON Pointer does not identify a token")
		}
		current, ok = obj[part]
		if !ok {
			return "", errors.New("JSON Pointer does not identify a token")
		}
		decoded = append(decoded, strings.ReplaceAll(strings.ReplaceAll(part, "~1", "/"), "~0", "~"))
	}
	obj, ok := current.(map[string]any)
	if !ok {
		return "", errors.New("binding must point to a token object")
	}
	if _, ok := obj["$value"]; !ok {
		return "", errors.New("binding must point to a token object")
	}
	return strings.Join(decoded, "."), nil
}

func validateContrast(doc any, tokens map[string]token, bindings map[string]string) error {
	colors := map[string]color{}
	for _, slot := range []string{"text", "textMuted", "onPrimary", "canvas", "surface", "surfaceElevated", "primary", "secondary", "success", "warning", "danger", "focusRing"} {
		name, err := pointerTokenName(doc, bindings[slot])
		if err != nil {
			return err
		}
		resolved, err := resolveToken(name, tokens, map[string]bool{})
		if err != nil {
			return err
		}
		c, err := parseColor(resolved.value)
		if err != nil {
			return err
		}
		colors[slot] = c
	}
	for _, pair := range [][2]string{
		{"text", "canvas"}, {"textMuted", "canvas"}, {"text", "surface"}, {"textMuted", "surface"},
		{"text", "surfaceElevated"}, {"textMuted", "surfaceElevated"}, {"onPrimary", "primary"},
		{"primary", "canvas"}, {"secondary", "canvas"}, {"success", "canvas"}, {"warning", "canvas"},
		{"danger", "canvas"}, {"focusRing", "canvas"},
	} {
		if ratio(colors[pair[0]], colors[pair[1]]) < 4.5 {
			return fmt.Errorf("%s/%s contrast is below WCAG AA (4.5:1)", pair[0], pair[1])
		}
	}
	return nil
}

type color struct{ r, g, b float64 }

func parseColor(value any) (color, error) {
	if s, ok := value.(string); ok {
		if len(s) != 7 || s[0] != '#' {
			return color{}, errors.New("only opaque #RRGGBB colors are supported")
		}
		v, err := hex.DecodeString(s[1:])
		if err != nil {
			return color{}, err
		}
		return color{float64(v[0]) / 255, float64(v[1]) / 255, float64(v[2]) / 255}, nil
	}
	if object, ok := value.(map[string]any); ok {
		space, _ := object["colorSpace"].(string)
		components, _ := object["components"].([]any)
		alpha := 1.0
		if raw, exists := object["alpha"]; exists {
			switch v := raw.(type) {
			case json.Number:
				alpha, _ = v.Float64()
			case float64:
				alpha = v
			}
		}
		if space != "srgb" || len(components) != 3 || alpha != 1 {
			return color{}, errors.New("only opaque DTCG srgb colors are supported")
		}
		values := [3]float64{}
		for i, raw := range components {
			switch v := raw.(type) {
			case json.Number:
				values[i], _ = v.Float64()
			case float64:
				values[i] = v
			default:
				return color{}, errors.New("invalid DTCG srgb component")
			}
			if values[i] < 0 || values[i] > 1 {
				return color{}, errors.New("DTCG srgb components must be between 0 and 1")
			}
		}
		return color{values[0], values[1], values[2]}, nil
	}
	return color{}, errors.New("unsupported DTCG color value")
}
func ratio(a, b color) float64 {
	channel := func(v float64) float64 {
		if v <= .04045 {
			return v / 12.92
		}
		return math.Pow((v+.055)/1.055, 2.4)
	}
	lum := func(c color) float64 { return .2126*channel(c.r) + .7152*channel(c.g) + .0722*channel(c.b) }
	x, y := lum(a), lum(b)
	if x < y {
		x, y = y, x
	}
	return (x + .05) / (y + .05)
}

func Builtin() *Package {
	bindings := map[string]string{}
	names := []string{"canvas", "surface", "surfaceElevated", "text", "textMuted", "border", "primary", "onPrimary", "secondary", "success", "warning", "danger", "focusRing"}
	for _, name := range names {
		bindings[name] = "/color/" + name
	}
	for name := range v2Slots {
		bindings[name] = "/design/" + name
	}
	manifest := Manifest{Format: "yin-theme", FormatVersion: FormatVersion, ID: "org.yin.default", Name: "Yin Default", PackageVersion: "2.0.1", APIVersion: APIVersion, DTCGVersion: DTCGVersion, Schemes: []string{"light", "dark"}, Documents: map[string]string{"light": "tokens/light.json", "dark": "tokens/dark.json"}, Bindings: bindings}
	light := map[string]string{"canvas": "#ffffff", "surface": "#f3f6f8", "surfaceElevated": "#ffffff", "text": "#172126", "textMuted": "#53636a", "border": "#d5dfe2", "primary": "#075b68", "onPrimary": "#ffffff", "secondary": "#8b4412", "success": "#176b45", "warning": "#805200", "danger": "#a12627", "focusRing": "#075b68"}
	dark := map[string]string{"canvas": "#171d20", "surface": "#222a2e", "surfaceElevated": "#2b353a", "text": "#f1f5f6", "textMuted": "#b0bec3", "border": "#536168", "primary": "#72d6df", "onPrimary": "#102326", "secondary": "#f0a66d", "success": "#71d8a0", "warning": "#f2c46c", "danger": "#ff9792", "focusRing": "#72d6df"}
	return &Package{Manifest: manifest, Documents: map[string]json.RawMessage{"light": makeDocument(light, false), "dark": makeDocument(dark, true)}, Resources: map[string]ResourceData{}, Verified: true}
}

func makeDocument(palette map[string]string, dark bool) json.RawMessage {
	colors := map[string]any{"$type": "color"}
	for name, hexColor := range palette {
		parsed, _ := parseColor(hexColor)
		colors[name] = map[string]any{"$value": map[string]any{"colorSpace": "srgb", "components": []float64{parsed.r, parsed.g, parsed.b}, "alpha": 1}}
	}
	design := map[string]any{}
	for name, value := range builtinDesignValues(dark) {
		design[name] = map[string]any{"$type": v2Slots[name], "$value": value}
	}
	document := map[string]any{"$schema": "https://design-tokens.github.io/community-group/format/2025.10/schema.json", "color": colors, "design": design}
	raw, _ := json.Marshal(document)
	return raw
}
