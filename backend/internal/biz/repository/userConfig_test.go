package repository

import (
	"encoding/json"
	"testing"
)

func TestPanelConfigKeepsLegacyValuesWhenAdoptingThemeDefaults(t *testing.T) {
	const legacy = `{"iconTextColor":"#fa00aa","backgroundImageSrc":"/uploads/legacy.png"}`
	var config PanelConfig
	if err := json.Unmarshal([]byte(legacy), &config); err != nil {
		t.Fatal(err)
	}
	if config.IconTextColor != "#fa00aa" || config.BackgroundImageSrc != "/uploads/legacy.png" {
		t.Fatalf("legacy values were not loaded: %#v", config)
	}

	config.UseThemeDefaults = true
	encoded, err := json.Marshal(config)
	if err != nil {
		t.Fatal(err)
	}
	var restored PanelConfig
	if err := json.Unmarshal(encoded, &restored); err != nil {
		t.Fatal(err)
	}
	if !restored.UseThemeDefaults || restored.IconTextColor != config.IconTextColor || restored.BackgroundImageSrc != config.BackgroundImageSrc {
		t.Fatalf("adopting theme defaults changed persisted configuration: %#v", restored)
	}
}
