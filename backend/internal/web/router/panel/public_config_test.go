package panel

import (
	"testing"

	"github.com/yinorg/Yin-Panel/backend/internal/biz/repository"
)

func TestPublicPanelConfigProjectsDisplayPreferencesOnly(t *testing.T) {
	monitorEnabled := true
	searchVisible := true
	source := &repository.PanelConfig{
		HomeLayout:             "directory",
		LogoText:               "Public bookmarks",
		SearchBoxShow:          &searchVisible,
		SystemMonitorShow:      &monitorEnabled,
		SystemMonitorShowTitle: &monitorEnabled,
		FooterHtml:             `<script>localStorage.clear()</script>`,
	}

	projected := publicPanelConfig(source)
	if projected.HomeLayout != "directory" || projected.LogoText != "Public bookmarks" || projected.SearchBoxShow == nil || !*projected.SearchBoxShow {
		t.Fatalf("display preferences were not preserved: %+v", projected)
	}
	if projected.FooterHtml != "" {
		t.Fatal("public configuration retained executable footer HTML")
	}
	if projected.SystemMonitorShow == nil || *projected.SystemMonitorShow || projected.SystemMonitorShowTitle == nil || *projected.SystemMonitorShowTitle {
		t.Fatal("public configuration exposed system monitor state")
	}
}

func TestPublicPanelConfigHandlesMissingConfiguration(t *testing.T) {
	if got := publicPanelConfig(nil); got != nil {
		t.Fatalf("public panel config = %+v, want nil", got)
	}
}
