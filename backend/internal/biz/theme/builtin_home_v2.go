package theme

import (
	"embed"
	"fmt"
)

//go:embed builtin_themes/shared/home.mjs builtin_themes/shared/home.css builtin_themes/*/styles/home.css
var builtinThemeFiles embed.FS

func builtinHomeResourcesV2(id string) map[string]ResourceData {
	variant := id[len("org.yin."):]
	if variant == "default" {
		variant = "yin"
	}
	view, err := builtinThemeFiles.ReadFile("builtin_themes/shared/home.mjs")
	if err != nil {
		panic(fmt.Sprintf("missing built-in theme view %q: %v", variant, err))
	}
	style, err := builtinThemeFiles.ReadFile(fmt.Sprintf("builtin_themes/%s/styles/home.css", variant))
	if err != nil {
		panic(fmt.Sprintf("missing built-in theme stylesheet %q: %v", variant, err))
	}
	baseStyle, err := builtinThemeFiles.ReadFile("builtin_themes/shared/home.css")
	if err != nil {
		panic(fmt.Sprintf("missing shared built-in theme stylesheet: %v", err))
	}
	return map[string]ResourceData{
		"views/home.mjs":  {MediaType: "text/javascript", Content: view},
		"styles/home.css": {MediaType: "text/css", Content: append(append(baseStyle, '\n'), style...)},
	}
}
