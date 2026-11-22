//! Minimal diagnostics.
//!
//! Most handlers intentionally collapse a failed read into the empty result the
//! Go build answers (a missing row and a broken database both became `null`).
//! That is behaviour parity, but it used to hide real database failures. Every
//! such collapse now records the error on stderr, and the HTTP boundary records
//! server-error responses, so an operator can still see what happened.
//!
//! When `base.enable_metrics` is set, a second listener exposes the counters
//! below in Prometheus text format, the same shape the Go build served.

use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Router;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::appcfg;

static REQUESTS: AtomicU64 = AtomicU64::new(0);
static FAILURES: AtomicU64 = AtomicU64::new(0);
static LOGINS: AtomicU64 = AtomicU64::new(0);
static STARTED: OnceLock<u64> = OnceLock::new();

fn epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Record the process start; called once before the listeners open.
pub fn mark_start() {
    let _ = STARTED.get_or_init(epoch_seconds);
}

/// Record an error that a handler is deliberately turning into an empty result.
pub fn warn(context: &str, error: &dyn Display) {
    eprintln!("warning: {context}: {error}");
}

/// Record a path and status for responses that indicate a server-side failure.
pub fn request_failure(method: &str, path: &str, status: u16) {
    eprintln!("request failed: {method} {path} -> {status}");
}

pub fn count_login() {
    LOGINS.fetch_add(1, Ordering::Relaxed);
}

/// Log 5xx responses and count every request for the metrics listener.
pub async fn log_failures(request: Request, next: axum::middleware::Next) -> Response {
    let method = request.method().as_str().to_string();
    let path = request.uri().path().to_string();
    let response = next.run(request).await;
    REQUESTS.fetch_add(1, Ordering::Relaxed);
    let status = response.status().as_u16();
    if status >= 500 {
        FAILURES.fetch_add(1, Ordering::Relaxed);
        request_failure(&method, &path, status);
    }
    response
}

fn metrics_text() -> String {
    let uptime = epoch_seconds().saturating_sub(STARTED.get().copied().unwrap_or(0));
    format!(
        "# HELP yin_panel_requests_total HTTP requests served.\n\
         # TYPE yin_panel_requests_total counter\n\
         yin_panel_requests_total {}\n\
         # HELP yin_panel_request_failures_total Responses with status >= 500.\n\
         # TYPE yin_panel_request_failures_total counter\n\
         yin_panel_request_failures_total {}\n\
         # HELP yin_panel_logins_total Successful password/OAuth logins.\n\
         # TYPE yin_panel_logins_total counter\n\
         yin_panel_logins_total {}\n\
         # HELP yin_panel_uptime_seconds Seconds since the process started.\n\
         # TYPE yin_panel_uptime_seconds gauge\n\
         yin_panel_uptime_seconds {}\n\
         # HELP yin_panel_build_info Build metadata.\n\
         # TYPE yin_panel_build_info gauge\n\
         yin_panel_build_info{{version=\"{}\",commit=\"{}\"}} 1\n",
        REQUESTS.load(Ordering::Relaxed),
        FAILURES.load(Ordering::Relaxed),
        LOGINS.load(Ordering::Relaxed),
        uptime,
        appcfg::version_name(),
        appcfg::build_commit(),
    )
}

async fn metrics() -> Response {
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        metrics_text(),
    )
        .into_response()
}

/// The unauthenticated metrics listener, started only when the configuration
/// asks for it. It is a separate listener on purpose: the endpoint must never
/// be reachable through the public application port.
pub async fn serve_metrics() {
    let port = appcfg::ctx().metrics_port();
    let address = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("metrics listener failed to bind {address}: {error}");
            return;
        }
    };
    let router = Router::new().route("/metrics", get(metrics));
    if let Err(error) = axum::serve(listener, router).await {
        eprintln!("metrics listener stopped: {error}");
    }
}
