package theme

import (
	"embed"
	"fmt"
)

//go:embed builtin_themes/shared/home.mjs builtin_themes/shared/home.css builtin_themes/yin/styles/theme-home.css builtin_themes/*/home/*.mjs builtin_themes/*/styles/home.css
var builtinThemeFiles embed.FS

// builtinHomeSpecV2 describes how one built-in home package is assembled. A
// package owns everything it renders — its shell, its layout module and its
// stylesheets all come from its own directory — so its revision, which is
// immutable once published, covers exactly the bytes it ships. Two packages
// sharing a source file would make that promise untrue: a change for one would
// move the other's version as well.
type builtinHomeSpecV2 struct {
	layout string   // layout module at <home>/home/<layout>.mjs, or "" for the shared view
	home   string   // source root holding this package's home/ and styles/
	styles []string // stylesheet sources, concatenated in order
}

var builtinHomeSpecsV2 = map[string]builtinHomeSpecV2{
	"org.yin.default": {
		layout: "standard",
		home:   "builtin_themes/yin",
		styles: []string{"builtin_themes/yin/styles/theme-home.css"},
	},
	"org.yin.directory": {
		layout: "directory",
		home:   "builtin_themes/directory",
		styles: []string{"builtin_themes/directory/styles/home.css"},
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
	view, err := builtinHomeViewV2(spec.home, spec.layout)
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

// builtinHomeViewV2 returns the shared view for an empty layout, and the
// package's own shell composed with its layout module otherwise.
func builtinHomeViewV2(home, layout string) ([]byte, error) {
	if layout == "" {
		return builtinThemeFiles.ReadFile("builtin_themes/shared/home.mjs")
	}
	sources := []string{home + "/home/shell.mjs", home + "/home/" + layout + ".mjs"}
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
