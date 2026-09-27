package theme

import (
	"embed"
	"fmt"
)

//go:embed builtin_themes/shared/home.mjs builtin_themes/shared/home.css builtin_themes/yin/home.mjs builtin_themes/yin/styles/theme-home.css builtin_themes/*/styles/home.css
var builtinThemeFiles embed.FS

func builtinHomeResourcesV2(id string) map[string]ResourceData {
	variant := id[len("org.yin."):]
	if variant == "default" {
		variant = "yin"
	}
	viewPath := "builtin_themes/shared/home.mjs"
	if id == "org.yin.default" {
		viewPath = "builtin_themes/yin/home.mjs"
	}
	view, err := builtinThemeFiles.ReadFile(viewPath)
	if err != nil {
		panic(fmt.Sprintf("missing built-in theme view %q: %v", variant, err))
	}
	stylePath := fmt.Sprintf("builtin_themes/%s/styles/home.css", variant)
	if id == "org.yin.default" {
		stylePath = "builtin_themes/yin/styles/theme-home.css"
	}
	style, err := builtinThemeFiles.ReadFile(stylePath)
	if err != nil {
		panic(fmt.Sprintf("missing built-in theme stylesheet %q: %v", variant, err))
	}
	if id != "org.yin.default" {
		baseStyle, err := builtinThemeFiles.ReadFile("builtin_themes/shared/home.css")
		if err != nil {
			panic(fmt.Sprintf("missing shared built-in theme stylesheet: %v", err))
		}
		style = append(append(baseStyle, '\n'), style...)
	}
	return map[string]ResourceData{
		"views/home.mjs":  {MediaType: "text/javascript", Content: view},
		"styles/home.css": {MediaType: "text/css", Content: style},
	}
}
