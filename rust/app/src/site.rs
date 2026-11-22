//! The built frontend, served from `./web` when `enable_static_server` is on.
//!
//! Mirrors `registerStaticRoutes` in `backend/internal/web/router/router.go`:
//! hashed build assets are immutable, the entry documents and PWA control files
//! are never cached, and the SPA routes fall back to `index.html`.

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::appcfg;

/// `cacheStatic(2592000, true)` for `/assets` and the Workbox runtime.
const IMMUTABLE: &str = "public, max-age=2592000, immutable";
/// `cacheStatic(86400, false)` for `/custom`.
const CUSTOM: &str = "public, max-age=86400";

fn web_root() -> std::path::PathBuf {
    // `webPath := "./web"`, resolved from the working directory as Go does.
    std::path::PathBuf::from("./web")
}

pub fn router() -> Router<()> {
    if !appcfg::ctx().conf.base.enable_static_server {
        return Router::new();
    }
    Router::new()
        .route("/", get(root))
        .route("/index.html", get(root))
        .route("/login", get(root))
        .route("/oauth/callback", get(root))
        .route("/theme/{view}", get(root))
        .route("/clear.html", get(clear))
        .route("/registerSW.js", get(register_sw))
        .route("/sw.js", get(service_worker))
        .route("/manifest.webmanifest", get(manifest))
        .route("/workbox-{filename}", get(workbox))
        .route("/assets/{*path}", get(assets))
        .route("/custom/{*path}", get(custom))
        // Last: a public space link is an SPA route with one segment.
        .route("/{public_id}", get(root))
}

fn mime_type(name: &str) -> &'static str {
    let extension = name
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match extension.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "webmanifest" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "map" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        "wasm" => "application/wasm",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

/// `time.RFC1123` with `GMT`, as `http.ServeContent` writes it.
fn http_date(modified: std::time::SystemTime) -> String {
    let offset = time::UtcOffset::UTC;
    let datetime = time::OffsetDateTime::from(modified).to_offset(offset);
    let format = time::macros::format_description!(
        "[weekday repr:short], [day] [month repr:short] [year] [hour]:[minute]:[second] GMT"
    );
    datetime.format(&format).unwrap_or_default()
}

/// One file with the headers `gin.Context.File` (`http.ServeContent`) sets.
fn serve_file(
    request: &HeaderMap,
    relative: &str,
    cache: Option<&str>,
    content: Vec<u8>,
    modified: Option<std::time::SystemTime>,
) -> Response {
    let mut response = Response::new(axum::body::Body::empty());
    {
        let headers = response.headers_mut();
        for (key, value) in crate::envelope::headers() {
            headers.insert(key, value);
        }
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(mime_type(relative))
                .unwrap_or(HeaderValue::from_static("application/octet-stream")),
        );
        headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&content.len().to_string())
                .unwrap_or(HeaderValue::from_static("0")),
        );
        headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        if let Some(modified) = modified
            && let Ok(value) = HeaderValue::from_str(&http_date(modified))
        {
            headers.insert(header::LAST_MODIFIED, value);
        }
        match cache {
            None => {
                // `noCacheStatic`: entry documents must always be revalidated.
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-store, no-cache, must-revalidate"),
                );
                headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
                headers.insert(header::EXPIRES, HeaderValue::from_static("0"));
                if let Ok(value) = HeaderValue::from_str("no-store") {
                    headers.insert("cdn-cache-control", value.clone());
                    headers.insert("surrogate-control", value);
                }
            }
            Some(policy) => {
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_str(policy).unwrap_or(HeaderValue::from_static("no-store")),
                );
            }
        }
    }
    let (parts, _) = response.into_parts();
    crate::content::serve(request, parts, &content)
}

/// `gin.Dir(dir, false)` for a missing file: 404, an empty body and the headers
/// the group's middleware already set.
fn missing(cache: Option<&str>) -> Response {
    let mut response = (StatusCode::NOT_FOUND, "").into_response();
    {
        let headers = response.headers_mut();
        for (key, value) in crate::envelope::headers() {
            if key == header::CONTENT_TYPE {
                continue;
            }
            headers.insert(key, value);
        }
        headers.remove(header::CONTENT_TYPE);
        match cache {
            None => {
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-store, no-cache, must-revalidate"),
                );
                headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
                headers.insert(header::EXPIRES, HeaderValue::from_static("0"));
                if let Ok(value) = HeaderValue::from_str("no-store") {
                    headers.insert("cdn-cache-control", value.clone());
                    headers.insert("surrogate-control", value);
                }
            }
            Some(policy) => {
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_str(policy).unwrap_or(HeaderValue::from_static("no-store")),
                );
            }
        }
    }
    response
}

/// Large bodies are read off the async workers so a slow disk cannot stall a
/// runtime thread; small ones are read inline, because a `spawn_blocking`
/// round-trip costs more than the read does. The threshold was chosen by
/// measuring the entry document: offloading a 13 KB file added 66 µs of CPU
/// per request and did not move p99, while the theme assets that reach hundreds
/// of kilobytes are the reads that can actually block.
const OFFLOAD_THRESHOLD: u64 = 256 * 1024;

async fn read(request: &HeaderMap, relative: &str, cache: Option<&str>) -> Response {
    let path = web_root().join(relative);
    let probe = std::fs::metadata(&path).ok();
    let modified = probe.as_ref().and_then(|metadata| metadata.modified().ok());
    let size = probe.map(|metadata| metadata.len()).unwrap_or(0);
    let content = if size >= OFFLOAD_THRESHOLD {
        let path = path.clone();
        tokio::task::spawn_blocking(move || std::fs::read(&path))
            .await
            .ok()
            .and_then(|result| result.ok())
    } else {
        std::fs::read(&path).ok()
    };
    match content {
        Some(content) => serve_file(request, relative, cache, content, modified),
        None => missing(cache),
    }
}

async fn root(headers: HeaderMap) -> Response {
    read(&headers, "index.html", None).await
}

async fn register_sw(headers: HeaderMap) -> Response {
    read(&headers, "registerSW.js", None).await
}

async fn service_worker(headers: HeaderMap) -> Response {
    read(&headers, "sw.js", None).await
}

async fn manifest(headers: HeaderMap) -> Response {
    read(&headers, "manifest.webmanifest", None).await
}

/// `/clear.html` also expires the session cookie, which script cannot reach.
async fn clear(headers: HeaderMap) -> Response {
    let mut response = read(&headers, "clear.html", None).await;
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&crate::auth::cleared_cookie(crate::appcfg::ctx().secure_cookies()))
            .unwrap_or(HeaderValue::from_static("yin_token=; Path=/; Max-Age=0")),
    );
    response
}

/// `/workbox-:filename`: only a `.js` file in the web root.
async fn workbox(headers: HeaderMap, Path(filename): Path<String>) -> Response {
    if !filename.ends_with(".js") || filename.contains('/') {
        return (
            StatusCode::NOT_FOUND,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            )],
            "",
        )
            .into_response();
    }
    read(&headers, &format!("workbox-{filename}"), Some(IMMUTABLE)).await
}

async fn assets(headers: HeaderMap, Path(path): Path<String>) -> Response {
    read(
        &headers,
        &format!("assets/{}", path.trim_start_matches('/')),
        Some(IMMUTABLE),
    )
    .await
}

async fn custom(headers: HeaderMap, Path(path): Path<String>) -> Response {
    read(
        &headers,
        &format!("custom/{}", path.trim_start_matches('/')),
        Some(CUSTOM),
    )
    .await
}
