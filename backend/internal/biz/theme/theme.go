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
	FormatVersion    = 1
	APIVersion       = "3"
	DTCGVersion      = "2025.10"
	EngineVersion    = "1.0.0"
	MaxArchive       = 32 << 20
	MaxExpanded      = 48 << 20
	builtinDefaultID = "org.yin.default"
	builtinMistID    = "org.yin.mist"
	builtinHorizonID = "org.yin.horizon"
	builtinGlassID   = "org.yin.glass"
	builtinMinimalID = "org.yin.minimal"
	builtinCyberID   = "org.yin.cyber"
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
	Kind           string   `json:"kind"`
	Source         string   `json:"source"`
	Poster         string   `json:"poster,omitempty"`
	OverlayOpacity *float64 `json:"overlayOpacity,omitempty"`
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
	Compatibility  *Compatibility       `json:"compatibility,omitempty"`
	Resources      []Resource           `json:"resources"`
	Wallpapers     map[string]Wallpaper `json:"wallpapers,omitempty"`
	Fonts          []FontResource       `json:"fonts,omitempty"`
}

type Compatibility struct {
	Engine  string `json:"engine"`
	Minimum string `json:"minimum"`
	Maximum string `json:"maximum,omitempty"`
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
		if wallpaper.Kind != "externalUrl" && wallpaper.Kind != "imageUrl" && !validWallpaperBytes(pkg.Resources[wallpaper.Source]) {
			return fmt.Errorf("wallpaper source %q has invalid content", wallpaper.Source)
		}
		if wallpaper.Poster != "" && wallpaper.Kind != "imageUrl" && !validWallpaperBytes(pkg.Resources[wallpaper.Poster]) {
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
	if m.Format != "yin-theme" || m.FormatVersion != FormatVersion || (m.APIVersion != "1" && m.APIVersion != "2" && m.APIVersion != APIVersion) || m.DTCGVersion != DTCGVersion {
		return errors.New("unsupported theme format, package API, or DTCG version")
	}
	if !idPattern.MatchString(m.ID) || isBuiltinThemeID(m.ID) || strings.TrimSpace(m.Name) == "" || len(m.Name) > 100 || !versionPattern.MatchString(m.PackageVersion) {
		return errors.New("invalid theme identity or reserved package ID")
	}
	if m.APIVersion == "3" {
		if m.Compatibility == nil || m.Compatibility.Engine != "yin-theme-engine" || !versionPattern.MatchString(m.Compatibility.Minimum) || (m.Compatibility.Maximum != "" && !versionPattern.MatchString(m.Compatibility.Maximum)) {
			return errors.New("API v3 themes must declare valid engine compatibility")
		}
		if compareThemeVersion(EngineVersion, m.Compatibility.Minimum) < 0 || (m.Compatibility.Maximum != "" && compareThemeVersion(EngineVersion, m.Compatibility.Maximum) > 0) {
			return errors.New("theme package is incompatible with this Yin Theme Engine version")
		}
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

func compareThemeVersion(left, right string) int {
	parse := func(value string) [3]int {
		core := strings.SplitN(strings.SplitN(value, "+", 2)[0], "-", 2)[0]
		parts := strings.Split(core, ".")
		version := [3]int{}
		for i := 0; i < len(parts) && i < len(version); i++ {
			_, _ = fmt.Sscanf(parts[i], "%d", &version[i])
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
	case "imageUrl":
		u, err := url.Parse(w.Source)
		if err != nil || u.Scheme != "https" || u.Hostname() == "" || u.User != nil || u.Fragment != "" || len(w.Source) > 2048 {
			return errors.New("image wallpaper must use a plain HTTPS URL")
		}
	default:
		return errors.New("unknown wallpaper kind")
	}
	if w.OverlayOpacity != nil && (*w.OverlayOpacity < 0 || *w.OverlayOpacity > 1 || math.IsNaN(*w.OverlayOpacity) || math.IsInf(*w.OverlayOpacity, 0)) {
		return errors.New("wallpaper overlay opacity must be between 0 and 1")
	}
	if w.Poster == "" && w.Kind != "image" && w.Kind != "imageUrl" {
		return errors.New("dynamic wallpaper requires a poster")
	}
	if w.Poster != "" && w.Kind != "imageUrl" && !strings.HasPrefix(resources[w.Poster], "image/") {
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
			resolved, err := resolveToken(name, flat, map[string]bool{})
			if err != nil {
				return fmt.Errorf("scheme %s: %w", scheme, err)
			}
			if pkg.Manifest.APIVersion == "3" {
				if err := validateAPIV3Token(name, resolved, pkg.Manifest.Resources); err != nil {
					return fmt.Errorf("scheme %s: %w", scheme, err)
				}
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
		if pkg.Manifest.APIVersion == "2" {
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

func validateAPIV3Token(name string, value token, resources []Resource) error {
	invalid := func() error { return fmt.Errorf("token %q has an invalid %s value", name, value.typ) }
	switch value.typ {
	case "color":
		if _, err := parseColor(value.value); err != nil {
			return invalid()
		}
	case "dimension":
		n, unit, ok := dimension(value.value)
		if !ok || n < 0 || (unit != "px" && unit != "rem") || n > 4096 {
			return invalid()
		}
	case "number":
		n, ok := number(value.value)
		if !ok || math.Abs(n) > 10000 {
			return invalid()
		}
		lower := strings.ToLower(name)
		if strings.Contains(lower, "opacity") && (n < 0 || n > 1) {
			return invalid()
		}
		if strings.Contains(lower, "scale") && (n < .5 || n > 1.5) {
			return invalid()
		}
		if strings.HasSuffix(lower, "tiltdegrees") && (n < 0 || n > 12) {
			return invalid()
		}
	case "fontFamily":
		if err := validateDesignSlot("fontBody", value.value); err != nil {
			return invalid()
		}
	case "fontWeight":
		switch current := value.value.(type) {
		case json.Number:
			n, _ := current.Float64()
			if n < 100 || n > 900 {
				return invalid()
			}
		case string:
			if !regexp.MustCompile(`^(thin|hairline|extra-light|ultra-light|light|normal|regular|medium|semi-bold|demi-bold|bold|extra-bold|ultra-bold|black|heavy|extra-black|ultra-black)$`).MatchString(current) {
				return invalid()
			}
		default:
			return invalid()
		}
	case "duration":
		v, ok := value.value.(map[string]any)
		if !ok {
			return invalid()
		}
		n, numberOK := number(v["value"])
		unit, unitOK := v["unit"].(string)
		if !numberOK || n < 0 || n > 10000 || !unitOK || (unit != "ms" && unit != "s") {
			return invalid()
		}
	case "cubicBezier":
		points, ok := value.value.([]any)
		if !ok || len(points) != 4 {
			return invalid()
		}
		for i, point := range points {
			n, valid := number(point)
			if !valid || ((i == 0 || i == 2) && (n < 0 || n > 1)) || math.Abs(n) > 10 {
				return invalid()
			}
		}
	case "boolean":
		if _, ok := value.value.(bool); !ok {
			return invalid()
		}
	case "border":
		v, ok := value.value.(map[string]any)
		if !ok {
			return invalid()
		}
		if _, err := parseColor(v["color"]); err != nil {
			return invalid()
		}
		if _, _, ok := dimension(v["width"]); !ok {
			return invalid()
		}
		style, ok := v["style"].(string)
		if !ok || !regexp.MustCompile(`^(solid|dashed|dotted|double|groove|ridge|inset|outset)$`).MatchString(style) {
			return invalid()
		}
	case "typography":
		v, ok := value.value.(map[string]any)
		if !ok || validateDesignSlot("fontBody", v["fontFamily"]) != nil || validateAPIV3Token(name+".fontSize", token{typ: "dimension", value: v["fontSize"]}, resources) != nil || validateAPIV3Token(name+".fontWeight", token{typ: "fontWeight", value: v["fontWeight"]}, resources) != nil {
			return invalid()
		}
		if line, exists := v["lineHeight"]; exists {
			n, ok := number(line)
			if !ok || n < .5 || n > 5 {
				return invalid()
			}
		}
		if style, exists := v["fontStyle"]; exists {
			s, ok := style.(string)
			if !ok || (s != "normal" && s != "italic") {
				return invalid()
			}
		}
	case "transition":
		v, ok := value.value.(map[string]any)
		if !ok || validateAPIV3Token(name+".duration", token{typ: "duration", value: v["duration"]}, resources) != nil {
			return invalid()
		}
		if delay, exists := v["delay"]; exists && validateAPIV3Token(name+".delay", token{typ: "duration", value: delay}, resources) != nil {
			return invalid()
		}
		if timing, exists := v["timingFunction"]; exists && validateAPIV3Token(name+".timingFunction", token{typ: "cubicBezier", value: timing}, resources) != nil {
			return invalid()
		}
		if property, exists := v["property"]; exists {
			p, ok := property.(string)
			if !ok || !regexp.MustCompile(`^[\w-]+$`).MatchString(p) {
				return invalid()
			}
		}
	case "gradient":
		stops, ok := value.value.([]any)
		if !ok || len(stops) < 2 || len(stops) > 16 {
			return invalid()
		}
		for _, raw := range stops {
			stop, ok := raw.(map[string]any)
			if !ok {
				return invalid()
			}
			if _, err := parseColor(stop["color"]); err != nil {
				return invalid()
			}
			position, ok := number(stop["position"])
			if !ok || position < 0 || position > 1 {
				return invalid()
			}
		}
	case "string", "strokeStyle":
		v, ok := value.value.(string)
		if !ok || len(v) > 2048 || strings.ContainsAny(v, ";{}<>\r\n\x00") {
			return invalid()
		}
	case "asset":
		asset, ok := value.value.(string)
		if !ok || (asset != "" && !hasDeclaredResource(resources, asset)) {
			return invalid()
		}
	case "shadow":
		if err := validateShadow(value.value); err != nil {
			return invalid()
		}
	default:
		return invalid()
	}
	return nil
}

func hasDeclaredResource(resources []Resource, name string) bool {
	for _, resource := range resources {
		if resource.Path == name && strings.HasPrefix(resource.MediaType, "image/") {
			return true
		}
	}
	return false
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
	light := map[string]string{"canvas": "#ffffff", "surface": "#f3f6f8", "surfaceElevated": "#ffffff", "text": "#172126", "textMuted": "#53636a", "border": "#d5dfe2", "primary": "#075b68", "onPrimary": "#ffffff", "secondary": "#8b4412", "success": "#176b45", "warning": "#805200", "danger": "#a12627", "focusRing": "#075b68"}
	dark := map[string]string{"canvas": "#171d20", "surface": "#222a2e", "surfaceElevated": "#2b353a", "text": "#f1f5f6", "textMuted": "#b0bec3", "border": "#536168", "primary": "#72d6df", "onPrimary": "#102326", "secondary": "#f0a66d", "success": "#71d8a0", "warning": "#f2c46c", "danger": "#ff9792", "focusRing": "#72d6df"}
	return builtinPackage(builtinDefaultID, "Yin Default", "2.1.0", light, dark, "yin")
}

func BuiltinMist() *Package {
	light := map[string]string{"canvas": "#f4f7f6", "surface": "#ffffff", "surfaceElevated": "#ffffff", "text": "#172826", "textMuted": "#586966", "border": "#d6e2df", "primary": "#b43743", "onPrimary": "#ffffff", "secondary": "#08796a", "success": "#197349", "warning": "#785300", "danger": "#a52d34", "focusRing": "#b43743"}
	dark := map[string]string{"canvas": "#151d1c", "surface": "#202a29", "surfaceElevated": "#2a3634", "text": "#f0f6f3", "textMuted": "#b4c3be", "border": "#4b605a", "primary": "#f28b80", "onPrimary": "#311717", "secondary": "#74d4bd", "success": "#76d39b", "warning": "#f0c66f", "danger": "#ff9b95", "focusRing": "#f28b80"}
	return builtinPackage(builtinMistID, "Yin Mist", "1.1.0", light, dark, "mist")
}

func BuiltinHorizon() *Package {
	light := map[string]string{"canvas": "#f4f8fa", "surface": "#ffffff", "surfaceElevated": "#ffffff", "text": "#20282c", "textMuted": "#64737a", "border": "#d8e0e3", "primary": "#176b80", "onPrimary": "#ffffff", "secondary": "#b4543c", "success": "#28734d", "warning": "#805500", "danger": "#a63338", "focusRing": "#176b80"}
	dark := map[string]string{"canvas": "#171d20", "surface": "#22292c", "surfaceElevated": "#2b3438", "text": "#f2f5f6", "textMuted": "#b7c1c4", "border": "#536066", "primary": "#78c5d4", "onPrimary": "#14262a", "secondary": "#eea080", "success": "#79d4a1", "warning": "#f1c66d", "danger": "#ff9994", "focusRing": "#78c5d4"}
	return builtinPackage(builtinHorizonID, "Yin Horizon", "1.1.0", light, dark, "horizon")
}

func BuiltinGlass() *Package {
	light := map[string]string{"canvas": "#e7edf4", "surface": "#edf2f8", "surfaceElevated": "#f8fbff", "text": "#182535", "textMuted": "#53677c", "border": "#a9bed3", "primary": "#2666a6", "onPrimary": "#ffffff", "secondary": "#8059a8", "success": "#176b45", "warning": "#805200", "danger": "#a12627", "focusRing": "#2666a6"}
	dark := map[string]string{"canvas": "#101827", "surface": "#19263a", "surfaceElevated": "#25354c", "text": "#eef5ff", "textMuted": "#a6b8cd", "border": "#526c89", "primary": "#73b9ff", "onPrimary": "#10253d", "secondary": "#c6a4ff", "success": "#71d8a0", "warning": "#f2c46c", "danger": "#ff9792", "focusRing": "#73b9ff"}
	return builtinPackage(builtinGlassID, "Yin Glass", "1.0.0", light, dark, "glass")
}

func BuiltinMinimal() *Package {
	light := map[string]string{"canvas": "#ffffff", "surface": "#fafafa", "surfaceElevated": "#ffffff", "text": "#202020", "textMuted": "#666666", "border": "#e6e6e6", "primary": "#303030", "onPrimary": "#ffffff", "secondary": "#526b5d", "success": "#176b45", "warning": "#805200", "danger": "#a12627", "focusRing": "#526b5d"}
	dark := map[string]string{"canvas": "#141414", "surface": "#1b1b1b", "surfaceElevated": "#202020", "text": "#eeeeee", "textMuted": "#aaaaaa", "border": "#363636", "primary": "#d0d0d0", "onPrimary": "#181818", "secondary": "#9db7a7", "success": "#71d8a0", "warning": "#f2c46c", "danger": "#ff9792", "focusRing": "#9db7a7"}
	return builtinPackage(builtinMinimalID, "Yin Minimal", "1.0.0", light, dark, "minimal")
}

func BuiltinCyber() *Package {
	light := map[string]string{"canvas": "#f5f3ff", "surface": "#ffffff", "surfaceElevated": "#ffffff", "text": "#251847", "textMuted": "#65568a", "border": "#8f77c8", "primary": "#5a27d5", "onPrimary": "#ffffff", "secondary": "#007b83", "success": "#176b45", "warning": "#805200", "danger": "#a12627", "focusRing": "#5a27d5"}
	dark := map[string]string{"canvas": "#100b20", "surface": "#19112e", "surfaceElevated": "#24173e", "text": "#f5edff", "textMuted": "#b5a5d5", "border": "#6746a0", "primary": "#ed4bc5", "onPrimary": "#210c27", "secondary": "#42e5df", "success": "#71d8a0", "warning": "#f2c46c", "danger": "#ff9792", "focusRing": "#42e5df"}
	return builtinPackage(builtinCyberID, "Yin Cyber", "1.0.0", light, dark, "cyber")
}

func Builtins() []*Package {
	return []*Package{Builtin(), BuiltinMist(), BuiltinHorizon(), BuiltinGlass(), BuiltinMinimal(), BuiltinCyber()}
}

func isBuiltinThemeID(id string) bool {
	return id == builtinDefaultID || id == builtinMistID || id == builtinHorizonID || id == builtinGlassID || id == builtinMinimalID || id == builtinCyberID
}

func builtinPackage(id, name, version string, light, dark map[string]string, preset string) *Package {
	bindings := map[string]string{}
	names := []string{"canvas", "surface", "surfaceElevated", "text", "textMuted", "border", "primary", "onPrimary", "secondary", "success", "warning", "danger", "focusRing"}
	for _, name := range names {
		bindings[name] = "/semantic/color/" + name
	}
	manifest := Manifest{Format: "yin-theme", FormatVersion: FormatVersion, ID: id, Name: name, PackageVersion: version, APIVersion: APIVersion, DTCGVersion: DTCGVersion, Compatibility: &Compatibility{Engine: "yin-theme-engine", Minimum: "1.0.0"}, Schemes: []string{"light", "dark"}, Documents: map[string]string{"light": "tokens/light.json", "dark": "tokens/dark.json"}, Bindings: bindings}
	return &Package{Manifest: manifest, Documents: map[string]json.RawMessage{"light": makeDocument(light, false, preset), "dark": makeDocument(dark, true, preset)}, Resources: map[string]ResourceData{}, Verified: true}
}

func makeDocument(palette map[string]string, dark bool, preset string) json.RawMessage {
	primitiveColors := map[string]any{"$type": "color"}
	for name, hexColor := range palette {
		parsed, _ := parseColor(hexColor)
		primitiveColors[name] = map[string]any{"$value": map[string]any{"colorSpace": "srgb", "components": []float64{parsed.r, parsed.g, parsed.b}, "alpha": 1}}
	}
	semanticColors := map[string]any{"$type": "color"}
	for name := range palette {
		semanticColors[name] = map[string]any{"$value": "{primitive.color." + name + "}"}
	}
	px := func(value float64) map[string]any { return map[string]any{"value": value, "unit": "px"} }
	ms := func(value float64) map[string]any { return map[string]any{"value": value, "unit": "ms"} }
	shadowColor := "#24323b"
	fontBody := []string{"Inter", "system-ui", "sans-serif"}
	fontDisplay := []string{"Inter", "system-ui", "sans-serif"}
	baseRadius, cardPadding, groupGap, iconSize, duration, searchBlur := 8.0, 16.0, 20.0, 70.0, 180.0, 0.0
	surfaceStyle, density, borderStyle := "solid", "standard", "solid"
	if dark {
		shadowColor = "#080d12"
	}
	switch preset {
	case "glass":
		baseRadius, cardPadding, groupGap, iconSize, duration, searchBlur = 18, 22, 28, 76, 260, 18
		surfaceStyle, density, shadowColor = "glass", "comfortable", "#37628c"
	case "minimal":
		baseRadius, cardPadding, groupGap, iconSize, duration = 2, 22, 28, 64, 100
		surfaceStyle, density, shadowColor, borderStyle = "solid", "spacious", "#808080", "solid"
	case "cyber":
		baseRadius, cardPadding, groupGap, iconSize, duration = 1, 14, 16, 72, 90
		surfaceStyle, density, shadowColor, borderStyle = "gradient", "compact", "#ed4bc5", "double"
		fontBody = []string{"IBM Plex Mono", "monospace"}
		fontDisplay = []string{"Orbitron", "IBM Plex Mono", "monospace"}
	case "horizon":
		baseRadius, cardPadding, groupGap, iconSize, duration, searchBlur, surfaceStyle = 5, 16, 18, 64, 160, 10, "frosted"
	case "mist":
		baseRadius, cardPadding, groupGap, iconSize, duration, searchBlur = 10, 18, 24, 72, 220, 10
		surfaceStyle = "frosted"
	}
	if preset == "minimal" {
		shadowColor = "#000000"
	}
	headingWeight := 600
	if preset == "cyber" {
		headingWeight = 700
	}
	glow := 0.0
	if preset == "glass" || preset == "cyber" {
		glow = 22
	}
	tiltDegrees := 8
	if preset == "glass" {
		tiltDegrees = 3
	}
	if preset == "minimal" {
		tiltDegrees = 0
	}
	if preset == "cyber" {
		tiltDegrees = 6
	}
	texture := "none"
	if preset == "cyber" {
		texture = "grid"
	}
	zero := px(0)
	shadow := map[string]any{"color": shadowColor, "offsetX": zero, "offsetY": px(4), "blur": px(18), "spread": px(0)}
	component := map[string]any{
		"appIcon":       map[string]any{"$type": "dimension", "size": map[string]any{"$value": px(iconSize)}, "radius": map[string]any{"$value": px(baseRadius)}, "gap": map[string]any{"$value": px(8)}, "surface": map[string]any{"$type": "color", "$value": "{semantic.color.surfaceElevated}"}, "shadow": map[string]any{"$type": "shadow", "$value": shadow}},
		"card":          map[string]any{"$type": "dimension", "padding": map[string]any{"$value": px(cardPadding)}, "radius": map[string]any{"$value": px(baseRadius)}, "borderWidth": map[string]any{"$value": px(1)}, "borderStyle": map[string]any{"$type": "string", "$value": borderStyle}, "surfaceMode": map[string]any{"$type": "string", "$value": surfaceStyle}, "shadow": map[string]any{"$type": "shadow", "$value": shadow}},
		"group":         map[string]any{"$type": "dimension", "gap": map[string]any{"$value": px(groupGap)}, "sectionSpacing": map[string]any{"$value": px(groupGap)}, "padding": map[string]any{"$value": px(cardPadding)}},
		"searchBox":     map[string]any{"$type": "dimension", "height": map[string]any{"$value": px(44)}, "radius": map[string]any{"$value": px(baseRadius)}, "borderWidth": map[string]any{"$value": px(1)}, "surface": map[string]any{"$type": "color", "$value": "{semantic.color.surfaceElevated}"}, "surfaceMode": map[string]any{"$type": "string", "$value": surfaceStyle}, "blur": map[string]any{"$value": px(searchBlur)}, "shadow": map[string]any{"$type": "shadow", "$value": shadow}},
		"sidebar":       map[string]any{"$type": "dimension", "padding": map[string]any{"$value": px(cardPadding)}, "radius": map[string]any{"$value": px(baseRadius)}, "surface": map[string]any{"$type": "color", "$value": "{semantic.color.surfaceElevated}"}, "surfaceMode": map[string]any{"$type": "string", "$value": surfaceStyle}},
		"dialog":        map[string]any{"$type": "dimension", "padding": map[string]any{"$value": px(cardPadding)}, "radius": map[string]any{"$value": px(baseRadius)}, "surface": map[string]any{"$type": "color", "$value": "{semantic.color.surfaceElevated}"}, "surfaceMode": map[string]any{"$type": "string", "$value": surfaceStyle}, "shadow": map[string]any{"$type": "shadow", "$value": shadow}},
		"menu":          map[string]any{"$type": "dimension", "padding": map[string]any{"$value": px(6)}, "radius": map[string]any{"$value": px(baseRadius)}, "surface": map[string]any{"$type": "color", "$value": "{semantic.color.surfaceElevated}"}, "surfaceMode": map[string]any{"$type": "string", "$value": surfaceStyle}, "shadow": map[string]any{"$type": "shadow", "$value": shadow}},
		"button":        map[string]any{"$type": "dimension", "height": map[string]any{"$value": px(36)}, "paddingX": map[string]any{"$value": px(14)}, "radius": map[string]any{"$value": px(baseRadius)}, "borderWidth": map[string]any{"$value": px(1)}, "surfaceMode": map[string]any{"$type": "string", "$value": surfaceStyle}},
		"input":         map[string]any{"$type": "dimension", "height": map[string]any{"$value": px(36)}, "paddingX": map[string]any{"$value": px(12)}, "radius": map[string]any{"$value": px(baseRadius)}, "borderWidth": map[string]any{"$value": px(1)}, "surfaceMode": map[string]any{"$type": "string", "$value": surfaceStyle}},
		"tooltip":       map[string]any{"$type": "dimension", "padding": map[string]any{"$value": px(8)}, "radius": map[string]any{"$value": px(baseRadius)}, "surfaceMode": map[string]any{"$type": "string", "$value": surfaceStyle}, "shadow": map[string]any{"$type": "shadow", "$value": shadow}},
		"systemMonitor": map[string]any{"$type": "dimension", "gap": map[string]any{"$value": px(groupGap)}, "padding": map[string]any{"$value": px(cardPadding)}, "radius": map[string]any{"$value": px(baseRadius)}, "iconSize": map[string]any{"$value": px(28)}},
		"state":         map[string]any{"$type": "duration", "hoverDuration": map[string]any{"$value": ms(duration)}, "pressDuration": map[string]any{"$value": ms(80)}, "enterDuration": map[string]any{"$value": ms(duration)}, "easing": map[string]any{"$type": "cubicBezier", "$value": []float64{.2, .8, .2, 1}}, "hoverScale": map[string]any{"$type": "number", "$value": 1.02}, "pressScale": map[string]any{"$type": "number", "$value": .96}, "tiltDegrees": map[string]any{"$type": "number", "$value": tiltDegrees}, "spaceTransitionDuration": map[string]any{"$type": "duration", "$value": ms(duration * 5.5)}, "spaceTransitionEasing": map[string]any{"$type": "cubicBezier", "$value": []float64{.22, .61, .36, 1}}},
		"surface":       map[string]any{"$type": "number", "opacity": map[string]any{"$value": .88}, "blur": map[string]any{"$type": "dimension", "$value": px(16)}, "glow": map[string]any{"$type": "dimension", "$value": px(glow)}},
		"iconography":   map[string]any{"$type": "dimension", "size": map[string]any{"$value": px(20)}, "containerRadius": map[string]any{"$value": px(baseRadius)}, "strokeWidth": map[string]any{"$type": "number", "$value": 1.8}},
	}
	typography := map[string]any{
		"body": map[string]any{"$type": "fontFamily", "$value": fontBody}, "display": map[string]any{"$type": "fontFamily", "$value": fontDisplay},
		"bodySize": map[string]any{"$type": "dimension", "$value": px(14)}, "smallSize": map[string]any{"$type": "dimension", "$value": px(12)}, "headingSize": map[string]any{"$type": "dimension", "$value": px(24)},
		"bodyWeight": map[string]any{"$type": "fontWeight", "$value": 400}, "headingWeight": map[string]any{"$type": "fontWeight", "$value": headingWeight},
		"bodyLineHeight": map[string]any{"$type": "number", "$value": 1.5}, "headingLineHeight": map[string]any{"$type": "number", "$value": 1.25},
	}
	semantic := map[string]any{"color": semanticColors, "typography": typography, "surface": map[string]any{"$type": "color", "canvas": map[string]any{"$value": "{primitive.color.canvas}"}, "panel": map[string]any{"$value": "{primitive.color.surface}"}, "raised": map[string]any{"$value": "{primitive.color.surfaceElevated}"}}, "state": map[string]any{"$type": "color", "focus": map[string]any{"$value": "{primitive.color.focusRing}"}, "success": map[string]any{"$value": "{primitive.color.success}"}, "warning": map[string]any{"$value": "{primitive.color.warning}"}, "danger": map[string]any{"$value": "{primitive.color.danger}"}}}
	shape := map[string]any{"$type": "dimension", "control": map[string]any{"$value": px(baseRadius)}, "card": map[string]any{"$value": px(baseRadius)}, "dialog": map[string]any{"$value": px(baseRadius)}, "borderWidth": map[string]any{"$value": px(1)}}
	spacing := map[string]any{"$type": "dimension", "xs": map[string]any{"$value": px(4)}, "sm": map[string]any{"$value": px(8)}, "md": map[string]any{"$value": px(12)}, "lg": map[string]any{"$value": px(groupGap)}, "xl": map[string]any{"$value": px(groupGap * 1.6)}}
	densityToken := map[string]any{"$type": "string", "$value": density}
	elevation := map[string]any{"$type": "shadow", "card": map[string]any{"$value": shadow}, "popup": map[string]any{"$value": shadow}}
	motion := map[string]any{"$type": "duration", "hover": map[string]any{"$value": ms(duration)}, "press": map[string]any{"$value": ms(80)}, "enter": map[string]any{"$value": ms(duration)}, "easing": map[string]any{"$type": "cubicBezier", "$value": []float64{.2, .8, .2, 1}}}
	background := map[string]any{"$type": "number", "overlayOpacity": map[string]any{"$value": .18}, "texture": map[string]any{"$type": "string", "$value": texture}, "image": map[string]any{"$type": "asset", "$value": ""}}
	effect := map[string]any{"$type": "number", "glowOpacity": map[string]any{"$value": glow / 32}, "hoverOpacity": map[string]any{"$value": .12}, "focusWidth": map[string]any{"$type": "dimension", "$value": px(2)}, "textShadow": map[string]any{"$type": "shadow", "$value": shadow}}
	document := map[string]any{"$schema": "https://design-tokens.github.io/community-group/format/2025.10/schema.json", "primitive": map[string]any{"color": primitiveColors}, "semantic": semantic, "component": component, "shape": shape, "spacing": spacing, "density": map[string]any{"scale": densityToken}, "elevation": elevation, "motion": motion, "background": background, "effect": effect}
	raw, _ := json.Marshal(document)
	return raw
}
