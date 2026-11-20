mod appcfg;
mod bootstrap;
mod go_routes;
mod auth;
mod envelope;
mod monitor;
mod packages;
mod routes;
mod site;
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

fn main() -> std::process::ExitCode {
    // `migrate-db` copies a SQLite deployment into MySQL/PostgreSQL and exits,
    // exactly as the Go binary's subcommand does; it never starts the server.
    if std::env::args().nth(1).as_deref() == Some("migrate-db") {
        return migrate_db();
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a tokio runtime");
    runtime.block_on(serve());
    std::process::ExitCode::SUCCESS
}

/// `runMigrateDB`: `-source`/`-target`, the same messages and exit codes.
fn migrate_db() -> std::process::ExitCode {
    let mut source = "conf.yaml".to_string();
    let mut target = None;
    let mut args = std::env::args().skip(2);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-source" | "--source" => {
                if let Some(value) = args.next() {
                    source = value;
                }
            }
            "-target" | "--target" => target = args.next(),
            _ => {}
        }
    }
    let Some(target) = target else {
        println!("usage: yin-panel migrate-db -source conf.yaml -target conf.target.yaml");
        return std::process::ExitCode::from(2);
    };
    println!("Migrating from {source} to {target} ...");
    match yin_db_tool::migrate_db(&source, &target) {
        Ok(()) => {
            println!("Migration complete.");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("migrate-db failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn serve() {
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
        // The built frontend, when the configuration serves it.
        .merge(site::router())
        .fallback(routes::go_fallback)
        // Gin answers a method mismatch with the same NoRoute as an unknown
        // path; axum would answer 405 with an `Allow` header.
        .method_not_allowed_fallback(routes::go_fallback)
        .layer(axum::middleware::from_fn(routes::head_gate));
    let port = appcfg::ctx().listen_port();
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
