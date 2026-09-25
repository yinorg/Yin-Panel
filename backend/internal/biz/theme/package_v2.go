package theme

import (
	"archive/zip"
	"bytes"
	"crypto/ed25519"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/url"
	"path"
	"regexp"
	"sort"
	"strings"
	"sync"
	"time"

	jsonschema "github.com/santhosh-tekuri/jsonschema/v5"
	"github.com/yinorg/Yin-Panel/backend/internal/global"
)

const (
	PackageFormatVersionV2 = 2
	ThemeAPIVersionV1      = "1.0.0"
	MaxPackageV2Files      = 512
	MaxThemeSettingsSchema = 256 * 1024
)

type PackageManifestV2 struct {
	Format        string           `json:"format"`
	FormatVersion int              `json:"formatVersion"`
	ID            string           `json:"id"`
	Name          string           `json:"name"`
	Description   string           `json:"description,omitempty"`
	Version       string           `json:"version"`
	ThemeAPI      string           `json:"themeApi"`
	Core          string           `json:"core"`
	Author        string           `json:"author"`
	License       string           `json:"license"`
	Repository    string           `json:"repository,omitempty"`
	Tokens        TokenSetV2       `json:"tokens"`
	DefaultScheme string           `json:"defaultScheme"`
	Entrypoints   EntrypointsV2    `json:"entrypoints"`
	Runtime       RuntimeV2        `json:"runtime"`
	Contributes   ContributionsV2  `json:"contributes"`
	Permissions   PermissionsV2    `json:"permissions"`
	Settings      *ThemeSettingsV2 `json:"settings,omitempty"`
	Resources     []ResourceV2     `json:"resources"`
}

type TokenSetV2 struct {
	Format  string            `json:"format"`
	Version string            `json:"version"`
	Docs    map[string]string `json:"documents"`
}

type EntrypointsV2 struct {
	Script string   `json:"script,omitempty"`
	Styles []string `json:"styles,omitempty"`
}

type RuntimeV2 struct {
	SupportedModes []string `json:"supportedModes,omitempty"`
}

type ContributionsV2 struct {
	Views      []string `json:"views,omitempty"`
	Regions    []string `json:"regions,omitempty"`
	Components []string `json:"components,omitempty"`
}

type PermissionsV2 struct {
	Required []PermissionV2 `json:"required,omitempty"`
	Optional []PermissionV2 `json:"optional,omitempty"`
}

type PermissionV2 struct {
	Name    string   `json:"name"`
	Origins []string `json:"origins,omitempty"`
}

type ThemeSettingsV2 struct {
	Schema        string `json:"schema"`
	SchemaVersion int    `json:"schemaVersion"`
}

type ResourceV2 struct {
	Path      string `json:"path"`
	SHA256    string `json:"sha256"`
	MediaType string `json:"mediaType"`
	URL       string `json:"url,omitempty"`
}

type PackageV2 struct {
	Manifest PackageManifestV2
	Tokens   map[string]json.RawMessage
	Files    map[string]ResourceData
	Verified bool
	Revision string
}

var digestPatternV2 = regexp.MustCompile(`^[a-fA-F0-9]{64}$`)

var packagePreviewsV2 = struct {
	sync.Mutex
	items map[string]previewPackageV2
}{items: map[string]previewPackageV2{}}

type previewPackageV2 struct {
	packageData *PackageV2
	expiresAt   time.Time
}

func ParsePackageArchiveV2(data []byte, confirmUnverified bool) (*PackageV2, error) {
	if len(data) == 0 || len(data) > MaxArchive {
		return nil, errors.New("theme package must be between 1 byte and 32 MiB")
	}
	zr, err := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if err != nil {
		return nil, fmt.Errorf("invalid theme package archive: %w", err)
	}
	if len(zr.File) > MaxPackageV2Files {
		return nil, errors.New("theme package contains too many files")
	}
	files := make(map[string][]byte, len(zr.File))
	expanded := int64(0)
	for _, file := range zr.File {
		name := strings.TrimPrefix(file.Name, "./")
		if name == "" || strings.Contains(name, "\\") || path.IsAbs(name) || path.Clean(name) != name || name == ".." || strings.HasPrefix(name, "../") || file.Mode()&0o170000 == 0o120000 {
			return nil, fmt.Errorf("unsafe theme package path %q", file.Name)
		}
		if file.FileInfo().IsDir() {
			continue
		}
		if _, exists := files[name]; exists {
			return nil, fmt.Errorf("duplicate theme package path %q", file.Name)
		}
		expanded += int64(file.UncompressedSize64)
		if expanded > MaxExpanded {
			return nil, errors.New("expanded theme package exceeds 48 MiB")
		}
		reader, err := file.Open()
		if err != nil {
			return nil, err
		}
		content, readErr := io.ReadAll(io.LimitReader(reader, MaxExpanded+1))
		closeErr := reader.Close()
		if readErr != nil {
			return nil, readErr
		}
		if closeErr != nil {
			return nil, closeErr
		}
		if int64(len(content)) > MaxExpanded {
			return nil, errors.New("expanded theme package exceeds 48 MiB")
		}
		files[name] = content
	}
	manifestBytes, exists := files["manifest.json"]
	if !exists {
		return nil, errors.New("manifest.json is required")
	}
	var manifest PackageManifestV2
	if err := strictJSON(manifestBytes, &manifest); err != nil {
		return nil, fmt.Errorf("invalid v2 theme manifest: %w", err)
	}
	if err := ValidatePackageManifestV2(manifest, strings.TrimPrefix(global.VERSION, "v")); err != nil {
		return nil, err
	}

	pkg := &PackageV2{Manifest: manifest, Tokens: map[string]json.RawMessage{}, Files: map[string]ResourceData{}}
	used := map[string]bool{"manifest.json": true, "signature.ed25519": true}
	for scheme, name := range manifest.Tokens.Docs {
		doc, ok := files[name]
		if !ok {
			return nil, fmt.Errorf("missing %s token document %q", scheme, name)
		}
		if err := validateDTCGDocument202510(doc); err != nil {
			return nil, fmt.Errorf("invalid %s DTCG document: %w", scheme, err)
		}
		used[name] = true
		pkg.Tokens[scheme] = append(json.RawMessage(nil), doc...)
	}
	for _, resource := range manifest.Resources {
		content, ok := files[resource.Path]
		if !ok {
			return nil, fmt.Errorf("missing theme resource %q", resource.Path)
		}
		used[resource.Path] = true
		sum := sha256.Sum256(content)
		if !strings.EqualFold(hex.EncodeToString(sum[:]), resource.SHA256) {
			return nil, fmt.Errorf("resource digest mismatch for %q", resource.Path)
		}
		if !allowedPackageResourceV2(resource.MediaType, resource.Path) {
			return nil, fmt.Errorf("resource type is not allowed for %q", resource.Path)
		}
		pkg.Files[resource.Path] = ResourceData{MediaType: resource.MediaType, Content: content}
	}
	if settings := manifest.Settings; settings != nil {
		schemaResource, ok := pkg.Files[settings.Schema]
		if settings.SchemaVersion < 1 || !ok || schemaResource.MediaType != "application/schema+json" {
			return nil, errors.New("theme settings require a declared schema and positive schemaVersion")
		}
		if len(schemaResource.Content) > MaxThemeSettingsSchema {
			return nil, errors.New("theme settings schema exceeds 256 KiB")
		}
		if err := validateThemeSettingsSchemaV2(settings.Schema, schemaResource.Content); err != nil {
			return nil, fmt.Errorf("invalid theme settings schema: %w", err)
		}
	}
	for _, name := range append(append([]string{manifest.Entrypoints.Script}, manifest.Entrypoints.Styles...), settingsPathV2(manifest.Settings)) {
		if name == "" {
			continue
		}
		if !used[name] {
			return nil, fmt.Errorf("entrypoint %q is not a declared theme resource", name)
		}
	}
	for name := range files {
		if !used[name] {
			return nil, fmt.Errorf("undeclared theme package file %q", name)
		}
	}
	if len(manifest.Entrypoints.Script) != 0 {
		if !containsString(manifest.Runtime.SupportedModes, "sandbox") && !containsString(manifest.Runtime.SupportedModes, "trusted") {
			return nil, errors.New("script themes must declare a supported runtime mode")
		}
	}
	if signature, ok := files["signature.ed25519"]; ok {
		decoded, err := base64.StdEncoding.DecodeString(strings.TrimSpace(string(signature)))
		if err != nil || len(decoded) != 64 {
			return nil, errors.New("invalid theme signature encoding")
		}
		payload, err := signaturePayload(files)
		if err != nil || !verifyThemeSignature(payload, decoded) {
			return nil, errors.New("theme signature is invalid")
		}
		pkg.Verified = true
	} else if !confirmUnverified {
		return nil, errors.New("theme is unsigned; explicit confirmation is required")
	}
	pkg.Revision = packageRevisionV2(files)
	return pkg, nil
}

func SavePackagePreviewV2(pkg *PackageV2) (string, error) {
	if pkg == nil {
		return "", errors.New("invalid theme preview package")
	}
	key := make([]byte, 24)
	if _, err := rand.Read(key); err != nil {
		return "", err
	}
	token := hex.EncodeToString(key)
	packagePreviewsV2.Lock()
	defer packagePreviewsV2.Unlock()
	now := time.Now()
	for existing, preview := range packagePreviewsV2.items {
		if now.After(preview.expiresAt) {
			delete(packagePreviewsV2.items, existing)
		}
	}
	packagePreviewsV2.items[token] = previewPackageV2{packageData: pkg, expiresAt: now.Add(10 * time.Minute)}
	return token, nil
}

func GetPackagePreviewV2(token string) (*PackageV2, error) {
	packagePreviewsV2.Lock()
	defer packagePreviewsV2.Unlock()
	preview, ok := packagePreviewsV2.items[token]
	if !ok || time.Now().After(preview.expiresAt) {
		delete(packagePreviewsV2.items, token)
		return nil, errors.New("theme preview has expired")
	}
	return preview.packageData, nil
}

func PackagePreviewPublicV2(pkg *PackageV2, token string) PublicPackageV2 {
	result := PublicPackageV2{Manifest: pkg.Manifest, Tokens: pkg.Tokens, Verified: pkg.Verified, Revision: pkg.Revision}
	result.Manifest.Resources = append([]ResourceV2(nil), pkg.Manifest.Resources...)
	for i := range result.Manifest.Resources {
		segments := strings.Split(result.Manifest.Resources[i].Path, "/")
		for j := range segments {
			segments[j] = url.PathEscape(segments[j])
		}
		result.Manifest.Resources[i].URL = "/api/theme/v2/preview/" + url.PathEscape(token) + "/assets/" + strings.Join(segments, "/")
	}
	return result
}

func ValidatePackageManifestV2(m PackageManifestV2, coreVersion string) error {
	if m.Format != "yin-theme" || m.FormatVersion != PackageFormatVersionV2 || m.Tokens.Format != "DTCG" || m.Tokens.Version != DTCGVersion {
		return errors.New("unsupported theme package format or DTCG version")
	}
	if !idPattern.MatchString(m.ID) || isBuiltinThemeID(m.ID) || strings.TrimSpace(m.Name) == "" || len(m.Name) > 100 || !versionPattern.MatchString(m.Version) {
		return errors.New("invalid theme identity or reserved package ID")
	}
	if strings.TrimSpace(m.Author) == "" || strings.TrimSpace(m.License) == "" {
		return errors.New("theme author and license are required")
	}
	if m.ThemeAPI != "^1.0.0" {
		return errors.New("theme API range must declare ^1.0.0")
	}
	if !versionPattern.MatchString(coreVersion) || !themeCoreRangeSupports(m.Core, coreVersion) {
		return errors.New("theme package is incompatible with this Yin Core version")
	}
	if len(m.Tokens.Docs) != 2 || m.Tokens.Docs["light"] == "" || m.Tokens.Docs["dark"] == "" || m.DefaultScheme != "light" && m.DefaultScheme != "dark" {
		return errors.New("theme must provide light and dark DTCG documents and a valid default scheme")
	}
	paths := map[string]bool{}
	for scheme, name := range m.Tokens.Docs {
		if scheme != "light" && scheme != "dark" || !safePackagePath(name) || paths[name] {
			return fmt.Errorf("invalid or duplicate %s token document path", scheme)
		}
		paths[name] = true
	}
	resourceTypes := map[string]string{}
	for _, resource := range m.Resources {
		if !safePackagePath(resource.Path) || paths[resource.Path] || !digestPatternV2.MatchString(resource.SHA256) || !allowedPackageResourceV2(resource.MediaType, resource.Path) {
			return fmt.Errorf("invalid or duplicate resource metadata for %q", resource.Path)
		}
		paths[resource.Path] = true
		resourceTypes[resource.Path] = resource.MediaType
	}
	if m.Entrypoints.Script != "" && resourceTypes[m.Entrypoints.Script] != "text/javascript" || m.Entrypoints.Script != "" && !strings.HasSuffix(m.Entrypoints.Script, ".mjs") {
		return errors.New("theme script entrypoint must be a declared .mjs resource")
	}
	for _, style := range m.Entrypoints.Styles {
		if resourceTypes[style] != "text/css" {
			return fmt.Errorf("theme stylesheet %q must be a declared CSS resource", style)
		}
	}
	if settingsPath := settingsPathV2(m.Settings); settingsPath != "" && (m.Settings.SchemaVersion < 1 || resourceTypes[settingsPath] != "application/schema+json") {
		return errors.New("theme settings schema must be a declared schema resource with a positive schemaVersion")
	}
	if err := validatePermissionNamesV2(m.Permissions); err != nil {
		return err
	}
	for _, mode := range m.Runtime.SupportedModes {
		if mode != "sandbox" && mode != "trusted" {
			return fmt.Errorf("unsupported theme runtime mode %q", mode)
		}
	}
	if err := validateContributionsV2(m); err != nil {
		return err
	}
	return nil
}

func validateThemeSettingsSchemaV2(name string, content []byte) error {
	var document any
	if err := json.Unmarshal(content, &document); err != nil {
		return err
	}
	if err := rejectRemoteSchemaRefsV2(document); err != nil {
		return err
	}
	compiler := jsonschema.NewCompiler()
	resourceURL := "https://theme.invalid/" + name
	if err := compiler.AddResource(resourceURL, bytes.NewReader(content)); err != nil {
		return err
	}
	_, err := compiler.Compile(resourceURL)
	return err
}

func rejectRemoteSchemaRefsV2(value any) error {
	switch node := value.(type) {
	case map[string]any:
		if ref, ok := node["$ref"].(string); ok && !strings.HasPrefix(ref, "#") {
			return fmt.Errorf("external schema reference %q is not allowed", ref)
		}
		for _, child := range node {
			if err := rejectRemoteSchemaRefsV2(child); err != nil {
				return err
			}
		}
	case []any:
		for _, child := range node {
			if err := rejectRemoteSchemaRefsV2(child); err != nil {
				return err
			}
		}
	}
	return nil
}

func validateContributionsV2(m PackageManifestV2) error {
	roles := []struct {
		name    string
		values  []string
		allowed map[string]bool
	}{
		{"view", m.Contributes.Views, map[string]bool{"home": true, "public-home": true, "theme-settings": true, "theme-page": true}},
		{"region", m.Contributes.Regions, map[string]bool{"header": true, "navigation": true, "content": true, "monitor": true, "background": true, "footer": true}},
		{"component", m.Contributes.Components, map[string]bool{"app-icon": true, "item-card": true, "group": true, "space-switcher": true, "search-box": true, "system-monitor": true, "command-center": true, "item-editor": true, "button": true, "input": true, "menu": true, "tooltip": true, "dialog": true}},
	}
	for _, roleSet := range roles {
		seen := map[string]bool{}
		for _, role := range roleSet.values {
			if !roleSet.allowed[role] || seen[role] {
				return fmt.Errorf("unknown or duplicate theme %s contribution %q", roleSet.name, role)
			}
			seen[role] = true
		}
	}
	if len(m.Contributes.Views)+len(m.Contributes.Regions)+len(m.Contributes.Components) > 0 && m.Entrypoints.Script == "" {
		return errors.New("theme view, region, and component contributions require a script entrypoint")
	}
	return nil
}

func themeCoreRangeSupports(expression, current string) bool {
	parts := strings.Fields(expression)
	if len(parts) == 0 || len(parts) > 4 {
		return false
	}
	comparator := regexp.MustCompile(`^(>=|<=|>|<|=)(.+)$`)
	for _, part := range parts {
		match := comparator.FindStringSubmatch(part)
		if len(match) != 3 || !versionPattern.MatchString(match[2]) {
			return false
		}
		cmp := compareThemeVersion(current, match[2])
		switch match[1] {
		case ">":
			if cmp <= 0 {
				return false
			}
		case ">=":
			if cmp < 0 {
				return false
			}
		case "<":
			if cmp >= 0 {
				return false
			}
		case "<=":
			if cmp > 0 {
				return false
			}
		case "=":
			if cmp != 0 {
				return false
			}
		default:
			return false
		}
	}
	return true
}

func validatePermissionNamesV2(p PermissionsV2) error {
	allowed := map[string]bool{"spaces.read": true, "groups.read": true, "items.read": true, "items.write": true, "groups.write": true, "monitor.read": true, "preferences.read": true, "preferences.write": true, "theme.storage": true, "network.fetch": true, "media.remote": true}
	seen := map[string]bool{}
	for _, group := range []struct {
		name string
		list []PermissionV2
	}{{"required", p.Required}, {"optional", p.Optional}} {
		for _, permission := range group.list {
			if !allowed[permission.Name] || seen[permission.Name] {
				return fmt.Errorf("unknown or duplicate %s permission %q", group.name, permission.Name)
			}
			seen[permission.Name] = true
			for _, origin := range permission.Origins {
				if !validThemeOriginV2(origin) {
					return fmt.Errorf("invalid origin %q in permission %q", origin, permission.Name)
				}
			}
		}
	}
	return nil
}

func ValidateThemeGrantV2(manifest PackageManifestV2, executionMode string, permissions []string) error {
	if (executionMode != "sandbox" && executionMode != "trusted") || !containsString(manifest.Runtime.SupportedModes, executionMode) {
		return fmt.Errorf("theme does not support the %s runtime", executionMode)
	}
	allowed := map[string]bool{}
	for _, permission := range append(append([]PermissionV2(nil), manifest.Permissions.Required...), manifest.Permissions.Optional...) {
		allowed[permission.Name] = true
	}
	granted := map[string]bool{}
	for _, permission := range permissions {
		if !allowed[permission] || granted[permission] {
			return fmt.Errorf("theme permission %q is not declared or is duplicated", permission)
		}
		granted[permission] = true
	}
	for _, permission := range manifest.Permissions.Required {
		if !granted[permission.Name] {
			return fmt.Errorf("required theme permission %q must be granted", permission.Name)
		}
	}
	return nil
}

func validThemeOriginV2(value string) bool {
	return (strings.HasPrefix(value, "https://") || strings.HasPrefix(value, "http://")) && !strings.ContainsAny(value, "*/?#@") && !strings.HasSuffix(value, "/")
}

func allowedPackageResourceV2(mediaType, name string) bool {
	allowed := map[string][]string{
		"text/css": {".css"}, "text/javascript": {".mjs"}, "application/schema+json": {".json"},
		"image/png": {".png"}, "image/jpeg": {".jpg", ".jpeg"}, "image/webp": {".webp"}, "image/gif": {".gif"}, "image/svg+xml": {".svg"},
		"font/woff2": {".woff2"}, "video/mp4": {".mp4"}, "video/webm": {".webm"},
	}
	for _, extension := range allowed[mediaType] {
		if strings.EqualFold(path.Ext(name), extension) {
			return true
		}
	}
	return false
}

func settingsPathV2(value *ThemeSettingsV2) string {
	if value == nil {
		return ""
	}
	return value.Schema
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func packageRevisionV2(files map[string][]byte) string {
	h := sha256.New()
	names := make([]string, 0, len(files))
	for name := range files {
		if name != "signature.ed25519" {
			names = append(names, name)
		}
	}
	sort.Strings(names)
	for _, name := range names {
		sum := sha256.Sum256(files[name])
		_, _ = fmt.Fprintf(h, "%s\x00%x\n", name, sum)
	}
	return hex.EncodeToString(h.Sum(nil))
}

func verifyThemeSignature(payload, signature []byte) bool {
	return len(OfficialPublicKey) != 0 && ed25519.Verify(OfficialPublicKey, payload, signature)
}
