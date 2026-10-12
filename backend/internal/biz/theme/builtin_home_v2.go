package theme

import (
	"embed"
	"fmt"
)

//go:embed builtin_themes/shared/home.mjs builtin_themes/shared/home.css builtin_themes/yin/home/*.mjs builtin_themes/yin/styles/theme-home.css builtin_themes/yin/styles/directory.css builtin_themes/*/styles/home.css
var builtinThemeFiles embed.FS

// builtinHomeSpecV2 describes how one built-in home package is assembled. The
// two Yin packages share a single layout-agnostic shell and differ only in the
// layout module concatenated after it, so the default package never ships the
// directory layout code and the directory package never ships the standard one.
type builtinHomeSpecV2 struct {
	layout string   // layout module under builtin_themes/yin/home/, or "" for the shared view
	styles []string // stylesheet sources, concatenated in order
}

var builtinHomeSpecsV2 = map[string]builtinHomeSpecV2{
	"org.yin.default": {
		layout: "standard",
		styles: []string{"builtin_themes/yin/styles/theme-home.css"},
	},
	"org.yin.directory": {
		layout: "directory",
		styles: []string{"builtin_themes/yin/styles/theme-home.css", "builtin_themes/yin/styles/directory.css"},
	},
	"org.yin.glass": {
		styles: []string{"builtin_themes/shared/home.css", "builtin_themes/glass/styles/home.css"},
	},
}

func builtinHomeResourcesV2(id string) map[string]ResourceData {
	spec, ok := builtinHomeSpecsV2[id]
	if !ok {
		panic(fmt.Sprintf("unknown built-in theme %q", id))
	}
	view, err := builtinHomeViewV2(spec.layout)
	if err != nil {
		panic(fmt.Sprintf("missing built-in theme view for %q: %v", id, err))
	}
	style, err := builtinHomeStyleV2(spec.styles)
	if err != nil {
		panic(fmt.Sprintf("missing built-in theme stylesheet for %q: %v", id, err))
	}
	resources := map[string]ResourceData{
		"views/home.mjs":  {MediaType: "text/javascript", Content: view},
		"styles/home.css": {MediaType: "text/css", Content: style},
	}
	return resources
}


// builtinHomeViewV2 returns the shared view for an empty layout, and the shared
// shell composed with the layout's own modules otherwise.
func builtinHomeViewV2(layout string) ([]byte, error) {
	if layout == "" {
		return builtinThemeFiles.ReadFile("builtin_themes/shared/home.mjs")
	}
	sources := []string{"builtin_themes/yin/home/shell.mjs", "builtin_themes/yin/home/" + layout + ".mjs"}
	var view []byte
	for index, path := range sources {
		part, err := builtinThemeFiles.ReadFile(path)
		if err != nil {
			return nil, err
		}
		if index > 0 {
			view = append(view, '\n')
		}
		view = append(view, part...)
	}
	view = append(view, []byte("\nexport default createYinHome(yinHomeLayout)\n")...)
	return view, nil
}

func builtinHomeStyleV2(paths []string) ([]byte, error) {
	var style []byte
	for index, path := range paths {
		part, err := builtinThemeFiles.ReadFile(path)
		if err != nil {
			return nil, err
		}
		if index > 0 {
			style = append(style, '\n')
		}
		style = append(style, part...)
	}
	return style, nil
}
