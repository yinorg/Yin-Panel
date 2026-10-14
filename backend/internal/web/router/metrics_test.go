package router

import (
	"io"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
)

func TestMetricsHandlerExposesProcessMetrics(t *testing.T) {
	server := httptest.NewServer(newMetricsHandler())
	t.Cleanup(server.Close)

	response, err := server.Client().Get(server.URL + "/metrics")
	if err != nil {
		t.Fatalf("GET /metrics: %v", err)
	}
	defer response.Body.Close()
	if response.StatusCode != 200 {
		t.Fatalf("GET /metrics status = %d, want 200", response.StatusCode)
	}
	body, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatalf("read metrics body: %v", err)
	}
	text := string(body)
	for _, want := range []string{
		"go_goroutines",
		"go_memstats_heap_alloc_bytes",
		"yin_panel_build_info",
	} {
		if !strings.Contains(text, want) {
			t.Errorf("metrics payload is missing %q", want)
		}
	}
}

func TestMetricsAddrUsesConfiguredPort(t *testing.T) {
	previous := config.AppConfig
	t.Cleanup(func() { config.AppConfig = previous })

	config.AppConfig = &config.Config{}
	if got := MetricsAddr(); got != ":9091" {
		t.Errorf("MetricsAddr() with no configured port = %q, want %q", got, ":9091")
	}

	config.AppConfig = &config.Config{Base: config.BaseConfig{MetricsPort: "9999"}}
	if got := MetricsAddr(); got != ":9999" {
		t.Errorf("MetricsAddr() with metrics_port=9999 = %q, want %q", got, ":9999")
	}
}
