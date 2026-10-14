package router

import (
	"errors"
	"net"
	"net/http"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/collectors"
	"github.com/prometheus/client_golang/prometheus/promhttp"

	"github.com/yinorg/Yin-Panel/backend/internal/global"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/config"
	"github.com/yinorg/Yin-Panel/backend/internal/infra/zaplog"
)

// DefaultMetricsPort is used when the configuration does not set one.
const DefaultMetricsPort = "9091"

// MetricsAddr returns the listen address of the metrics listener.
func MetricsAddr() string {
	port := ""
	if config.AppConfig != nil {
		port = config.AppConfig.Base.MetricsPort
	}
	if port == "" {
		port = DefaultMetricsPort
	}
	return ":" + port
}

// newMetricsHandler builds the unauthenticated Prometheus exposition handler.
// It is served on its own listener so the public HTTP port never exposes it.
func newMetricsHandler() http.Handler {
	registry := prometheus.NewRegistry()
	registry.MustRegister(
		collectors.NewGoCollector(),
		collectors.NewProcessCollector(collectors.ProcessCollectorOpts{}),
	)
	buildInfo := prometheus.NewGaugeVec(
		prometheus.GaugeOpts{
			Name: "yin_panel_build_info",
			Help: "Build information for this Yin-Panel instance, always 1.",
		},
		[]string{"version", "runmode"},
	)
	buildInfo.WithLabelValues(global.VERSION, global.RUNCODE).Set(1)
	registry.MustRegister(buildInfo)

	mux := http.NewServeMux()
	mux.Handle("/metrics", promhttp.HandlerFor(registry, promhttp.HandlerOpts{}))
	return mux
}

// StartMetricsServer serves Prometheus metrics on a separate listener. The
// socket is bound synchronously so a port conflict fails startup loudly, then
// the listener serves in the background for the life of the process.
func StartMetricsServer() error {
	listener, err := net.Listen("tcp", MetricsAddr())
	if err != nil {
		return err
	}
	server := &http.Server{
		Handler:           newMetricsHandler(),
		ReadHeaderTimeout: 5 * time.Second,
	}
	go func() {
		if serveErr := server.Serve(listener); serveErr != nil && !errors.Is(serveErr, http.ErrServerClosed) {
			zaplog.Logger.Errorf("metrics listener stopped: %v", serveErr)
		}
	}()
	zaplog.Logger.Info("Prometheus metrics listening on ", listener.Addr().String())
	return nil
}
