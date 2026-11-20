//! Glue from the front door to `wallpaperpack-rs`: read the request, call `wallpaperpack_rs::port`,
//! answer what the original answers (status, headers that matter, body).
//!
//! Ported from `ThemeRouter.WallpaperLibrary`
//! (`backend/internal/web/router/system/theme.go:1001`): the handler rewrites every
//! asset path under the library's immutable version prefix and wraps the result in
//! the Core's JSON envelope.

use std::collections::HashMap;

use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Map, Value, json};

/// `wallpaperLibraryRoot` in `theme.go`.
const LIBRARY_ROOT: &str = "/api/theme/v2/wallpaper/library/assets/";

/// `route` is the route as wired (`GET /users/{id}`, or `*`), `params` its path parameters.
pub async fn handle(_route: &str, _params: HashMap<String, String>, _req: Request) -> Response {
    let catalogue = wallpaperpack_rs::port::catalogue();
    let version = text(&catalogue, "version");

    let mut categories = Vec::new();
    for category in items(&catalogue, "categories") {
        let mut items_out = Vec::new();
        for item in items(&category, "items") {
            let mut entry = Map::new();
            entry.insert("id".into(), json!(text(&item, "id")));
            entry.insert("kind".into(), json!(text(&item, "kind")));
            entry.insert("name".into(), json!(text(&item, "name")));
            let poster = text(&item, "poster");
            if !poster.is_empty() {
                entry.insert("poster".into(), json!(format!("{LIBRARY_ROOT}{version}/{poster}")));
            }
            entry.insert(
                "source".into(),
                json!(format!("{LIBRARY_ROOT}{version}/{}", text(&item, "source"))),
            );
            items_out.push(Value::Object(entry));
        }
        let mut category_out = Map::new();
        category_out.insert("id".into(), json!(text(&category, "id")));
        category_out.insert("items".into(), Value::Array(items_out));
        category_out.insert("name".into(), json!(text(&category, "name")));
        categories.push(Value::Object(category_out));
    }

    // `response.SuccessData`, then the handler's own cache policy. The security
    // headers are what the Core's middleware adds to every response.
    let body = json!({
        "code": 0,
        "data": { "categories": categories, "version": version },
        "msg": "OK",
    });
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
            (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::X_FRAME_OPTIONS, "SAMEORIGIN"),
        ],
        body.to_string(),
    )
        .into_response()
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn items(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}
