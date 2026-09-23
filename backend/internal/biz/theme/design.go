package theme

import (
	"encoding/json"
	"fmt"
	"math"
	"regexp"
	"strings"
)

var v2Slots = map[string]string{
	"fontBody": "fontFamily", "fontDisplay": "fontFamily",
	"fontBodySize": "dimension", "fontSmallSize": "dimension", "fontHeadingSize": "dimension",
	"fontBodyWeight": "fontWeight", "fontHeadingWeight": "fontWeight",
	"lineHeightBody": "number", "lineHeightHeading": "number",
	"spaceXs": "dimension", "spaceSm": "dimension", "spaceMd": "dimension",
	"spaceLg": "dimension", "spaceXl": "dimension",
	"radiusControl": "dimension", "radiusCard": "dimension", "radiusDialog": "dimension",
	"borderWidth": "dimension", "shadowCard": "shadow", "shadowPopup": "shadow",
	"controlHeight": "dimension", "iconSize": "dimension", "sidebarWidth": "dimension",
	"contentMaxWidth": "dimension", "pageGutter": "dimension",
	"breakpointMobile": "dimension", "breakpointTablet": "dimension",
	"layoutTemplate": "string", "homeColumns": "number",
}

var safeFontName = regexp.MustCompile(`^[\pL\pN _.-]{1,80}$`)

func slotTypes(version string) map[string]string {
	result := make(map[string]string, len(requiredSlots)+len(v2Slots))
	for name, typ := range requiredSlots {
		result[name] = typ
	}
	if version == APIVersion {
		for name, typ := range v2Slots {
			result[name] = typ
		}
	}
	return result
}

func number(value any) (float64, bool) {
	raw, ok := value.(json.Number)
	if !ok {
		return 0, false
	}
	n, err := raw.Float64()
	return n, err == nil && !math.IsNaN(n) && !math.IsInf(n, 0)
}

func dimension(value any) (float64, string, bool) {
	object, ok := value.(map[string]any)
	if !ok || len(object) != 2 {
		return 0, "", false
	}
	n, valid := number(object["value"])
	unit, unitOK := object["unit"].(string)
	return n, unit, valid && unitOK && (unit == "px" || unit == "rem")
}

func validateDesignSlot(name string, value any) error {
	switch name {
	case "fontBody", "fontDisplay":
		families := []string{}
		switch current := value.(type) {
		case string:
			families = append(families, current)
		case []any:
			for _, item := range current {
				family, ok := item.(string)
				if !ok {
					return fmt.Errorf("binding %s has invalid font family", name)
				}
				families = append(families, family)
			}
		}
		if len(families) == 0 || len(families) > 5 {
			return fmt.Errorf("binding %s has invalid font family", name)
		}
		for _, family := range families {
			if !safeFontName.MatchString(family) {
				return fmt.Errorf("binding %s has unsafe font family", name)
			}
		}
	case "fontBodyWeight", "fontHeadingWeight":
		if n, ok := number(value); ok && (n < 100 || n > 900) {
			return fmt.Errorf("binding %s weight must be 100 to 900", name)
		}
	case "lineHeightBody", "lineHeightHeading":
		n, ok := number(value)
		if !ok || n < 1 || n > 2.5 {
			return fmt.Errorf("binding %s line height is out of range", name)
		}
	case "homeColumns":
		n, ok := number(value)
		if !ok || n < 1 || n > 12 || math.Trunc(n) != n {
			return fmt.Errorf("binding %s must be an integer from 1 to 12", name)
		}
	case "layoutTemplate":
		if value != "centered" && value != "split" {
			return fmt.Errorf("binding %s must be centered or split", name)
		}
	case "shadowCard", "shadowPopup":
		if err := validateShadow(value); err != nil {
			return fmt.Errorf("binding %s: %w", name, err)
		}
	default:
		n, unit, ok := dimension(value)
		if !ok || n < 0 {
			return fmt.Errorf("binding %s requires a nonnegative px/rem dimension", name)
		}
		min, max := 0.0, 1920.0
		switch name {
		case "fontBodySize", "fontSmallSize", "fontHeadingSize":
			min, max = 10, 64
		case "spaceXs", "spaceSm", "spaceMd", "spaceLg", "spaceXl", "pageGutter":
			max = 96
		case "radiusControl", "radiusCard", "radiusDialog":
			max = 48
		case "borderWidth":
			max = 4
		case "controlHeight":
			min, max = 32, 96
		case "iconSize":
			min, max = 12, 96
		case "sidebarWidth":
			min, max = 160, 480
		case "contentMaxWidth":
			min, max = 480, 1920
		case "breakpointMobile":
			min, max = 320, 800
		case "breakpointTablet":
			min, max = 768, 1280
		}
		if unit != "px" && (name == "breakpointMobile" || name == "breakpointTablet") {
			return fmt.Errorf("binding %s must use px", name)
		}
		if unit == "rem" {
			n *= 16
		}
		if n < min || n > max {
			return fmt.Errorf("binding %s is out of range", name)
		}
	}
	return nil
}

func validateShadow(value any) error {
	shadows := []any{value}
	if array, ok := value.([]any); ok {
		shadows = array
	}
	if len(shadows) == 0 || len(shadows) > 3 {
		return fmt.Errorf("shadow must have 1 to 3 layers")
	}
	for _, layer := range shadows {
		object, ok := layer.(map[string]any)
		if !ok {
			return fmt.Errorf("shadow layer must be an object")
		}
		if _, err := parseColor(object["color"]); err != nil {
			return fmt.Errorf("shadow color is invalid")
		}
		for _, field := range []string{"offsetX", "offsetY", "blur", "spread"} {
			n, _, valid := dimension(object[field])
			if !valid || math.Abs(n) > 96 || (field == "blur" && n < 0) {
				return fmt.Errorf("shadow %s is invalid", field)
			}
		}
		if inset, exists := object["inset"]; exists {
			if _, ok := inset.(bool); !ok {
				return fmt.Errorf("shadow inset is invalid")
			}
		}
	}
	return nil
}

func isWallpaperResource(mediaType string) bool {
	return strings.HasPrefix(mediaType, "image/") || mediaType == "video/mp4" || mediaType == "video/webm" || mediaType == "text/html"
}

func builtinDesignValues(dark bool) map[string]any {
	px := func(value float64) map[string]any { return map[string]any{"value": value, "unit": "px"} }
	shadowColor, popupColor := "#d5dfe2", "#53636a"
	if dark {
		shadowColor, popupColor = "#111719", "#050809"
	}
	shadow := func(color string, y, blur float64) map[string]any {
		return map[string]any{"color": color, "offsetX": px(0), "offsetY": px(y), "blur": px(blur), "spread": px(0)}
	}
	return map[string]any{
		"fontBody":     []string{"Inter", "system-ui", "sans-serif"},
		"fontDisplay":  []string{"Inter", "system-ui", "sans-serif"},
		"fontBodySize": px(14), "fontSmallSize": px(12), "fontHeadingSize": px(24),
		"fontBodyWeight": 400, "fontHeadingWeight": 600,
		"lineHeightBody": 1.5, "lineHeightHeading": 1.25,
		"spaceXs": px(4), "spaceSm": px(8), "spaceMd": px(12),
		"spaceLg": px(20), "spaceXl": px(32),
		"radiusControl": px(4), "radiusCard": px(6), "radiusDialog": px(8),
		"borderWidth": px(1), "shadowCard": shadow(shadowColor, 2, 12),
		"shadowPopup":   shadow(popupColor, 6, 24),
		"controlHeight": px(36), "iconSize": px(24), "sidebarWidth": px(280),
		"contentMaxWidth": px(1200), "pageGutter": px(16),
		"breakpointMobile": px(640), "breakpointTablet": px(1024),
		"layoutTemplate": "centered", "homeColumns": 12,
	}
}
