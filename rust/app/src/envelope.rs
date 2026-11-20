//! The Core's JSON envelope and the headers every API response carries.
//!
//! Go builds these bodies from maps (`gin.H`), which `encoding/json` emits in
//! sorted key order, and from structs, which keep field order. `serde_json`
//! sorts map keys the same way, so map-shaped bodies are built with `json!`
//! and struct-shaped bodies with a `Serialize` type.

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

/// Error messages from `backend/internal/web/model/response/ErrorCode.go`.
fn error_message(code: i64) -> Option<&'static str> {
    match code {
        1000 => Some("Not logged in yet"),
        1003 => Some("Incorrect username or password"),
        1004 => Some("Account disabled or not activated"),
        1005 => Some("No current permission for operation"),
        1200 => Some("Server error"),
        _ => None,
    }
}

pub fn headers() -> [(header::HeaderName, HeaderValue); 5] {
    [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        ),
        (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
        (
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ),
        (
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
        (
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("SAMEORIGIN"),
        ),
    ]
}

/// `response.ApiReturn`: the envelope, with `data` present only when non-null.
pub fn body(code: i64, msg: &str, data: Option<Value>) -> String {
    match data {
        Some(data) => json!({"code": code, "data": data, "msg": msg}).to_string(),
        None => json!({"code": code, "msg": msg}).to_string(),
    }
}

/// A body whose `data` is a struct: serialising the struct directly keeps its
/// field order, which is what Go emits for struct-shaped payloads. Going through
/// `Value` first would sort the keys.
#[derive(serde::Serialize)]
struct StructEnvelope<T> {
    code: i64,
    data: T,
    msg: &'static str,
}

pub fn success_struct<T: serde::Serialize>(data: T) -> Response {
    let body = serde_json::to_string(&StructEnvelope {
        code: 0,
        data,
        msg: "OK",
    })
    .unwrap_or_else(|_| body(-1, "Server error", None));
    (StatusCode::OK, headers(), body).into_response()
}

pub fn success_data(data: Value) -> Response {
    (StatusCode::OK, headers(), body(0, "OK", Some(data))).into_response()
}

pub fn success() -> Response {
    (StatusCode::OK, headers(), body(0, "OK", None)).into_response()
}

pub fn error_code(code: i64) -> Response {
    let msg = error_message(code).unwrap_or("Server error");
    (StatusCode::OK, headers(), body(code, msg, None)).into_response()
}

pub fn error(msg: &str) -> Response {
    (StatusCode::OK, headers(), body(-1, msg, None)).into_response()
}

/// A response with an extra header, used where the Core sets a cookie.
pub fn with_header(mut response: Response, name: header::HeaderName, value: HeaderValue) -> Response {
    response.headers_mut().insert(name, value);
    response
}
