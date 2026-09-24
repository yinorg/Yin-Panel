package theme

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

const DTCGSchema202510 = "https://www.designtokens.org/schemas/2025.10/format.json"

type dtcgTokenV2 struct {
	name     string
	group    string
	typeName string
	value    any
}

var aliasPatternV2 = regexp.MustCompile(`^\{[^{}]+\}$`)

func validateDTCGDocument202510(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	var document map[string]any
	if err := decoder.Decode(&document); err != nil {
		return err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return errors.New("DTCG document contains trailing JSON")
	}
	if document["$schema"] != DTCGSchema202510 {
		return errors.New("$schema must identify DTCG 2025.10")
	}
	tokens := map[string]dtcgTokenV2{}
	groups := map[string]string{}
	parents := map[string]string{}
	if err := collectDTCGTokens(document, "", "", "", tokens, groups, parents); err != nil {
		return err
	}
	if err := applyDTCGExtends(tokens, groups, parents); err != nil {
		return err
	}
	for name, token := range tokens {
		if token.typeName == "" {
			typeName, ok := effectiveDTCGGroupType(token.group, groups, parents, map[string]bool{})
			if !ok {
				return fmt.Errorf("token %q has no inherited $type", name)
			}
			token.typeName = typeName
			tokens[name] = token
		}
	}
	for name, token := range tokens {
		if !standardDTCGType(token.typeName) {
			return fmt.Errorf("token %q has unknown or missing DTCG type %q", name, token.typeName)
		}
		resolved, err := resolveDTCGToken(name, tokens, document, map[string]bool{})
		if err != nil {
			return err
		}
		if err := validateDTCGValue(resolved.typeName, resolved.value); err != nil {
			return fmt.Errorf("token %q: %w", name, err)
		}
	}
	return nil
}

func collectDTCGTokens(node map[string]any, prefix, inheritedType, group string, tokens map[string]dtcgTokenV2, groups, parents map[string]string) error {
	if err := validateDTCGMetadata(node); err != nil {
		return fmt.Errorf("group %q: %w", prefix, err)
	}
	typeName := inheritedType
	if declared, ok := node["$type"].(string); ok {
		typeName = declared
	}
	if parent, ok := node["$extends"]; ok {
		value, ok := parent.(string)
		if !ok || !aliasPatternV2.MatchString(value) {
			return fmt.Errorf("group %q has invalid $extends reference", prefix)
		}
		parents[prefix] = strings.Trim(value[1:len(value)-1], ".")
	}
	if _, hasRef := node["$ref"]; hasRef {
		if _, hasValue := node["$value"]; hasValue {
			return fmt.Errorf("token %q cannot contain both $ref and $value", prefix)
		}
		ref, ok := node["$ref"].(string)
		if !ok || !strings.HasPrefix(ref, "#/") || prefix == "" {
			return fmt.Errorf("token %q has invalid JSON Pointer $ref", prefix)
		}
		tokens[prefix] = dtcgTokenV2{name: prefix, group: group, typeName: typeName, value: map[string]any{"$ref": ref}}
		return nil
	}
	if raw, exists := node["$value"]; exists {
		if prefix == "" {
			prefix = "$root"
		}
		tokens[prefix] = dtcgTokenV2{name: prefix, group: group, typeName: typeName, value: raw}
		for key := range node {
			if !strings.HasPrefix(key, "$") {
				return fmt.Errorf("token %q contains non-metadata field %q", prefix, key)
			}
			if !allowedDTCGMetadata(key) {
				return fmt.Errorf("token %q contains unsupported metadata %q", prefix, key)
			}
		}
		return nil
	}
	if prefix != "" {
		groups[prefix] = typeName
	}
	for key, value := range node {
		if key == "$root" {
			child, ok := value.(map[string]any)
			if !ok {
				return fmt.Errorf("group %q $root must be a token object", prefix)
			}
			rootName := prefix
			if rootName == "" {
				rootName = "$root"
			}
			if err := collectDTCGTokens(child, rootName, typeName, prefix, tokens, groups, parents); err != nil {
				return err
			}
			continue
		}
		if strings.HasPrefix(key, "$") {
			if !allowedDTCGMetadata(key) {
				return fmt.Errorf("group %q contains unsupported metadata %q", prefix, key)
			}
			continue
		}
		if !validDTCGTokenName(key) {
			return fmt.Errorf("group %q contains invalid token name %q", prefix, key)
		}
		child, ok := value.(map[string]any)
		if !ok {
			return fmt.Errorf("group %q child %q must be an object", prefix, key)
		}
		name := key
		if prefix != "" {
			name = prefix + "." + key
		}
		childGroup := group
		if _, hasValue := child["$value"]; !hasValue {
			childGroup = name
		}
		if err := collectDTCGTokens(child, name, typeName, childGroup, tokens, groups, parents); err != nil {
			return err
		}
	}
	return nil
}

func allowedDTCGMetadata(key string) bool {
	switch key {
	case "$schema", "$type", "$value", "$description", "$extensions", "$deprecated", "$extends":
		return true
	default:
		return false
	}
}

func validateDTCGMetadata(node map[string]any) error {
	if value, exists := node["$description"]; exists {
		if _, ok := value.(string); !ok {
			return errors.New("$description must be a string")
		}
	}
	if value, exists := node["$deprecated"]; exists {
		if _, boolOK := value.(bool); !boolOK {
			if _, stringOK := value.(string); !stringOK {
				return errors.New("$deprecated must be a boolean or string")
			}
		}
	}
	if value, exists := node["$extensions"]; exists {
		if _, ok := value.(map[string]any); !ok {
			return errors.New("$extensions must be an object")
		}
	}
	return nil
}

func validDTCGTokenName(name string) bool {
	return name != "" && !strings.HasPrefix(name, "$") && !strings.ContainsAny(name, ".{}")
}

func applyDTCGExtends(tokens map[string]dtcgTokenV2, groups, parents map[string]string) error {
	resolved := map[string]map[string]dtcgTokenV2{}
	visiting := map[string]bool{}
	var expand func(string) (map[string]dtcgTokenV2, error)
	expand = func(group string) (map[string]dtcgTokenV2, error) {
		if value, ok := resolved[group]; ok {
			return value, nil
		}
		if visiting[group] {
			return nil, fmt.Errorf("DTCG group extension cycle at %q", group)
		}
		visiting[group] = true
		defer delete(visiting, group)
		result := map[string]dtcgTokenV2{}
		if parent, ok := parents[group]; ok {
			if _, exists := groups[parent]; !exists {
				return nil, fmt.Errorf("DTCG group %q extends unknown group %q", group, parent)
			}
			inherited, err := expand(parent)
			if err != nil {
				return nil, err
			}
			for relative, token := range inherited {
				token.name = joinDTCGPath(group, relative)
				if localType := groups[group]; localType != "" && token.typeName == "" {
					token.typeName = localType
				}
				result[relative] = token
			}
		}
		for name, token := range tokens {
			if token.group == group || strings.HasPrefix(token.group, group+".") {
				relative := strings.TrimPrefix(name, group+".")
				if name == group {
					relative = "$root"
				}
				result[relative] = token
			}
		}
		if typeName, ok := effectiveDTCGGroupType(group, groups, parents, map[string]bool{}); ok {
			for relative, token := range result {
				if token.typeName == "" {
					token.typeName = typeName
					result[relative] = token
				}
			}
		}
		resolved[group] = result
		return result, nil
	}
	groupsToExpand := make([]string, 0, len(parents))
	for group := range parents {
		groupsToExpand = append(groupsToExpand, group)
	}
	sort.Strings(groupsToExpand)
	for _, group := range groupsToExpand {
		inherited, err := expand(group)
		if err != nil {
			return err
		}
		for relative, token := range inherited {
			name := joinDTCGPath(group, relative)
			if _, exists := tokens[name]; !exists {
				token.name = name
				if token.group == "" {
					token.group = group
				}
				tokens[name] = token
			}
		}
	}
	return nil
}

func joinDTCGPath(group, relative string) string {
	if relative == "$root" {
		return group
	}
	return group + "." + relative
}

func effectiveDTCGGroupType(group string, groups, parents map[string]string, visiting map[string]bool) (string, bool) {
	if group == "" || visiting[group] {
		return "", false
	}
	visiting[group] = true
	defer delete(visiting, group)
	if value := groups[group]; value != "" {
		return value, true
	}
	if parent := parents[group]; parent != "" {
		if value, ok := effectiveDTCGGroupType(parent, groups, parents, visiting); ok {
			return value, true
		}
	}
	if split := strings.LastIndex(group, "."); split >= 0 {
		return effectiveDTCGGroupType(group[:split], groups, parents, visiting)
	}
	return "", false
}

func standardDTCGType(value string) bool {
	switch value {
	case "color", "dimension", "fontFamily", "fontWeight", "duration", "cubicBezier", "number", "strokeStyle", "border", "transition", "shadow", "gradient", "typography":
		return true
	default:
		return false
	}
}

func validateDTCGValue(typeName string, value any) error {
	invalid := func() error { return fmt.Errorf("invalid %s value", typeName) }
	switch typeName {
	case "color":
		object, ok := value.(map[string]any)
		if !ok {
			return invalid()
		}
		space, _ := object["colorSpace"].(string)
		components, ok := object["components"].([]any)
		length := map[string]int{"srgb": 3, "srgb-linear": 3, "display-p3": 3, "a98-rgb": 3, "prophoto-rgb": 3, "rec2020": 3, "xyz-d50": 3, "xyz-d65": 3, "hsl": 3, "hwb": 3, "lab": 3, "lch": 3, "oklab": 3, "oklch": 3}[space]
		if length == 0 || !ok || len(components) != length {
			return invalid()
		}
		for _, component := range components {
			if !finiteDTCGNumber(component) {
				if componentString, ok := component.(string); !ok || componentString != "none" {
					return invalid()
				}
			}
		}
		if hex, exists := object["hex"]; exists {
			text, ok := hex.(string)
			if !ok || !regexp.MustCompile(`^#[0-9a-fA-F]{6}$`).MatchString(text) {
				return invalid()
			}
		}
		for index, component := range components {
			if text, ok := component.(string); ok && text == "none" {
				continue
			}
			componentNumber, ok := component.(json.Number)
			if !ok {
				return invalid()
			}
			number, _ := componentNumber.Float64()
			minimum, maximum, bounded := dtcgColorComponentBounds(space, index)
			if bounded && (number < minimum || number > maximum || space == "hsl" && index == 0 && number == 360 || space == "hwb" && index == 0 && number == 360 || (space == "lch" || space == "oklch") && index == 2 && number == 360) || (space == "lch" || space == "oklch") && index == 1 && number < 0 {
				return invalid()
			}
		}
		if alpha, ok := object["alpha"]; ok {
			if !finiteDTCGNumber(alpha) {
				return invalid()
			}
			number, _ := alpha.(json.Number)
			value, _ := number.Float64()
			if value < 0 || value > 1 {
				return invalid()
			}
		}
	case "dimension", "duration":
		object, ok := value.(map[string]any)
		if !ok || !finiteDTCGNumber(object["value"]) {
			return invalid()
		}
		unit, _ := object["unit"].(string)
		if typeName == "dimension" && unit != "px" && unit != "rem" || typeName == "duration" && unit != "ms" && unit != "s" {
			return invalid()
		}
	case "fontFamily":
		if name, ok := value.(string); ok && strings.TrimSpace(name) != "" {
			return nil
		}
		families, ok := value.([]any)
		if !ok || len(families) == 0 {
			return invalid()
		}
		for _, family := range families {
			if name, ok := family.(string); !ok || strings.TrimSpace(name) == "" {
				return invalid()
			}
		}
	case "fontWeight":
		if number, ok := value.(json.Number); ok {
			n, err := number.Float64()
			if err == nil && n >= 1 && n <= 1000 {
				return nil
			}
		}
		if name, ok := value.(string); ok && map[string]bool{"thin": true, "hairline": true, "extra-light": true, "ultra-light": true, "light": true, "normal": true, "medium": true, "semi-bold": true, "demi-bold": true, "bold": true, "extra-bold": true, "ultra-bold": true, "black": true, "heavy": true, "extra-black": true, "ultra-black": true}[name] {
			return nil
		}
		return invalid()
	case "cubicBezier":
		points, ok := value.([]any)
		if !ok || len(points) != 4 {
			return invalid()
		}
		values := make([]float64, 4)
		for i, point := range points {
			number, ok := point.(json.Number)
			if !ok {
				return invalid()
			}
			n, err := number.Float64()
			if err != nil {
				return invalid()
			}
			values[i] = n
		}
		if values[0] < 0 || values[0] > 1 || values[2] < 0 || values[2] > 1 {
			return invalid()
		}
	case "number":
		if !finiteDTCGNumber(value) {
			return invalid()
		}
	case "strokeStyle":
		if name, ok := value.(string); ok && map[string]bool{"solid": true, "dashed": true, "dotted": true, "double": true, "groove": true, "ridge": true, "inset": true, "outset": true}[name] {
			return nil
		}
		object, ok := value.(map[string]any)
		if !ok {
			return invalid()
		}
		pattern, ok := object["dashArray"].([]any)
		lineCap, lineCapOK := object["lineCap"].(string)
		if !ok || len(pattern) == 0 || !lineCapOK || !map[string]bool{"butt": true, "round": true, "square": true}[lineCap] {
			return invalid()
		}
		for _, part := range pattern {
			if !validDTCGDimension(part) {
				return invalid()
			}
		}
	case "border":
		object, ok := value.(map[string]any)
		if !ok || validateDTCGValue("color", object["color"]) != nil || !validDTCGDimension(object["width"]) || validateDTCGValue("strokeStyle", object["style"]) != nil {
			return invalid()
		}
	case "transition":
		object, ok := value.(map[string]any)
		if !ok || validateDTCGValue("duration", object["duration"]) != nil || validateDTCGValue("duration", object["delay"]) != nil || validateDTCGValue("cubicBezier", object["timingFunction"]) != nil {
			return invalid()
		}
	case "shadow":
		layers := []any{value}
		if array, ok := value.([]any); ok {
			layers = array
		}
		if len(layers) == 0 {
			return invalid()
		}
		for _, layer := range layers {
			object, ok := layer.(map[string]any)
			if !ok || validateDTCGValue("color", object["color"]) != nil || !validDTCGDimension(object["offsetX"]) || !validDTCGDimension(object["offsetY"]) || !validDTCGDimension(object["blur"]) || !validDTCGDimension(object["spread"]) {
				return invalid()
			}
			if inset, exists := object["inset"]; exists {
				if _, ok := inset.(bool); !ok {
					return invalid()
				}
			}
		}
	case "gradient":
		stops, ok := value.([]any)
		if !ok || len(stops) < 1 {
			return invalid()
		}
		for _, stop := range stops {
			object, ok := stop.(map[string]any)
			if !ok || validateDTCGValue("color", object["color"]) != nil || !finiteDTCGNumber(object["position"]) {
				return invalid()
			}
		}
	case "typography":
		object, ok := value.(map[string]any)
		if !ok || validateDTCGValue("fontFamily", object["fontFamily"]) != nil || !validDTCGDimension(object["fontSize"]) || validateDTCGValue("fontWeight", object["fontWeight"]) != nil || !validDTCGDimension(object["letterSpacing"]) || !finiteDTCGNumber(object["lineHeight"]) {
			return invalid()
		}
	}
	return nil
}

func dtcgColorComponentBounds(space string, index int) (float64, float64, bool) {
	switch space {
	case "srgb", "srgb-linear", "display-p3", "a98-rgb", "prophoto-rgb", "rec2020", "xyz-d50", "xyz-d65":
		return 0, 1, true
	case "hsl", "hwb":
		if index == 0 {
			return 0, 360, true
		}
		return 0, 100, true
	case "lab", "lch":
		if index == 0 {
			return 0, 100, true
		}
		if space == "lch" && index == 1 {
			return 0, math.Inf(1), false
		}
		if space == "lch" && index == 2 {
			return 0, 360, true
		}
	case "oklab", "oklch":
		if index == 0 {
			return 0, 1, true
		}
		if space == "oklch" && index == 1 {
			return 0, math.Inf(1), false
		}
		if space == "oklch" && index == 2 {
			return 0, 360, true
		}
	}
	return 0, 0, false
}

func validDTCGDimension(value any) bool {
	object, ok := value.(map[string]any)
	if !ok || !finiteDTCGNumber(object["value"]) {
		return false
	}
	unit, _ := object["unit"].(string)
	return unit == "px" || unit == "rem"
}

func finiteDTCGNumber(value any) bool {
	number, ok := value.(json.Number)
	if !ok {
		return false
	}
	n, err := number.Float64()
	return err == nil && !math.IsNaN(n) && !math.IsInf(n, 0)
}

func resolveDTCGToken(name string, tokens map[string]dtcgTokenV2, document map[string]any, visiting map[string]bool) (dtcgTokenV2, error) {
	token, ok := tokens[name]
	if !ok {
		return dtcgTokenV2{}, fmt.Errorf("unknown DTCG reference %q", name)
	}
	if visiting[name] {
		return dtcgTokenV2{}, fmt.Errorf("DTCG reference cycle at %q", name)
	}
	visiting[name] = true
	defer delete(visiting, name)
	if reference, ok := token.value.(map[string]any); ok {
		if pointer, exists := reference["$ref"].(string); exists {
			value, err := resolveDTCGJSONPointer(document, pointer, map[string]bool{})
			if err != nil {
				return dtcgTokenV2{}, err
			}
			value, err = resolveDTCGCompositeValue(token.typeName, value, tokens, document, visiting)
			if err != nil {
				return dtcgTokenV2{}, err
			}
			token.value = value
			return token, nil
		}
	}
	if reference, ok := token.value.(string); ok && aliasPatternV2.MatchString(reference) {
		targetName := strings.ReplaceAll(strings.Trim(reference[1:len(reference)-1], "."), "/", ".")
		target, err := resolveDTCGToken(targetName, tokens, document, visiting)
		if err != nil {
			return dtcgTokenV2{}, err
		}
		if token.typeName != target.typeName {
			return dtcgTokenV2{}, fmt.Errorf("DTCG reference %q changes type", name)
		}
		token.value = target.value
		return token, nil
	}
	value, err := resolveDTCGCompositeValue(token.typeName, token.value, tokens, document, visiting)
	if err != nil {
		return dtcgTokenV2{}, err
	}
	token.value = value
	return token, nil
}

func resolveDTCGCompositeValue(typeName string, value any, tokens map[string]dtcgTokenV2, document map[string]any, visiting map[string]bool) (any, error) {
	compositeTypes := map[string]bool{"border": true, "transition": true, "shadow": true, "gradient": true, "typography": true, "strokeStyle": true}
	var walk func(any) (any, error)
	walk = func(value any) (any, error) {
		switch current := value.(type) {
		case string:
			if aliasPatternV2.MatchString(current) {
				if !compositeTypes[typeName] {
					return nil, errors.New("curly brace references are only valid in composite token values")
				}
				name := strings.ReplaceAll(strings.Trim(current[1:len(current)-1], "."), "/", ".")
				target, err := resolveDTCGToken(name, tokens, document, visiting)
				if err != nil {
					return nil, err
				}
				return target.value, nil
			}
			return current, nil
		case map[string]any:
			if pointer, ok := current["$ref"].(string); ok {
				value, err := resolveDTCGJSONPointer(document, pointer, map[string]bool{})
				if err != nil {
					return nil, err
				}
				return walk(value)
			}
			resolved := make(map[string]any, len(current))
			for key, child := range current {
				value, err := walk(child)
				if err != nil {
					return nil, err
				}
				resolved[key] = value
			}
			return resolved, nil
		case []any:
			resolved := make([]any, len(current))
			for i, child := range current {
				value, err := walk(child)
				if err != nil {
					return nil, err
				}
				resolved[i] = value
			}
			return resolved, nil
		}
		return value, nil
	}
	return walk(value)
}

func resolveDTCGJSONPointer(document map[string]any, reference string, visiting map[string]bool) (any, error) {
	if !strings.HasPrefix(reference, "#/") {
		return nil, fmt.Errorf("DTCG $ref %q must be a document-local JSON Pointer", reference)
	}
	if visiting[reference] {
		return nil, fmt.Errorf("DTCG JSON Pointer reference cycle at %q", reference)
	}
	visiting[reference] = true
	defer delete(visiting, reference)
	var current any = document
	for _, raw := range strings.Split(strings.TrimPrefix(reference, "#/"), "/") {
		for index := 0; index < len(raw); index++ {
			if raw[index] == '~' && (index+1 >= len(raw) || raw[index+1] != '0' && raw[index+1] != '1') {
				return nil, fmt.Errorf("DTCG $ref %q has malformed JSON Pointer escaping", reference)
			}
		}
		part := strings.ReplaceAll(strings.ReplaceAll(raw, "~1", "/"), "~0", "~")
		switch node := current.(type) {
		case map[string]any:
			var ok bool
			current, ok = node[part]
			if !ok {
				return nil, fmt.Errorf("DTCG $ref %q points to a missing value", reference)
			}
		case []any:
			index, err := strconv.Atoi(part)
			if err != nil || index < 0 || strconv.Itoa(index) != part || index >= len(node) {
				return nil, fmt.Errorf("DTCG $ref %q has an invalid array index", reference)
			}
			current = node[index]
		default:
			return nil, fmt.Errorf("DTCG $ref %q traverses a scalar", reference)
		}
	}
	if object, ok := current.(map[string]any); ok {
		if pointer, exists := object["$ref"].(string); exists {
			return resolveDTCGJSONPointer(document, pointer, visiting)
		}
	}
	return current, nil
}
