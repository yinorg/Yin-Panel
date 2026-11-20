mod appcfg;
mod bootstrap;
mod go_routes;
mod auth;
mod envelope;
mod monitor;
mod packages;
mod routes;
mod units;

use std::net::SocketAddr;

use axum::serve::ListenerExt;

/// `rust-anything app replay` measures the fallback rate by counting the lines
/// this build appends. Nothing is proxied any more, so the file stays empty.
fn report_fallbacks() {
    if let Some(path) = std::env::var_os("RUST_ANYTHING_FALLBACK_LOG") {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path);
    }
}

#[tokio::main]
async fn main() {
    report_fallbacks();
    // The Go build created and seeded the database at startup; this binary does
    // the same, so an empty directory is a working installation.
    let database = appcfg::ctx().database_file();
    if let Err(error) = bootstrap::prepare(&database).await {
        eprintln!("database startup failed: {error}");
        std::process::exit(1);
    }
    // `units/mod.rs` belongs to `app wire` and is rewritten on every wire; the
    // hand-ported routes are merged here so a wire cannot drop them. Every route
    // the Go build registered is now answered here, so there is no child process
    // and no proxy: the fallback is Gin's own 404/301 behaviour.
    let app = units::router()
        .merge(routes::router())
        .fallback(routes::go_fallback);
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port)))
        .await
        .expect("binding the port");
    let listener = listener.tap_io(|tcp| {
        let _ = tcp.set_nodelay(true);
    });
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serving");
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("a SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
