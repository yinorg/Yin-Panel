// The nested manifest types the bridge's generated wrapper refers to but does
// not copy. Kept in their own file so `rust-anything refresh` does not drop
// them; the JSON tags match `internal/biz/theme/package_v2.go`.
package theme

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
	Views      []string      `json:"views,omitempty"`
	Regions    []string      `json:"regions,omitempty"`
	Components []string      `json:"components,omitempty"`
	HomeLayout *HomeLayoutV2 `json:"homeLayout,omitempty"`
}

type HomeLayoutV2 struct {
	Kind     string `json:"kind"`
	Editable bool   `json:"editable,omitempty"`
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
