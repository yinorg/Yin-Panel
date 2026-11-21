//! `http.ServeContent` semantics for the responses that use it.
//!
//! Go serves uploaded media, theme assets and the built frontend through
//! `http.ServeContent`, which supports byte ranges, `If-None-Match` on an ETag
//! the handler set, and `If-Modified-Since` when the file has a modification
//! time. The Rust routes previously sent the whole body every time, so a video
//! upload could not be scrubbed and a revalidated document was re-sent in full.
//!
//! The caller builds the response head (status 200 and every header the route
//! sets) and hands the body here; this module only adjusts what a conditional
//! or ranged request changes.
//!
//! One deliberate difference: `http.ServeContent` answers a multi-range request
//! with `multipart/byteranges`. This serves the whole body instead. Browsers and
//! players send a single range, and the recorded contract has no multi-range
//! case; the difference is stated rather than implied.

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;

/// Go's `parseRange` for one range; `None` means "ignore the Range header".
enum ParsedRange {
    Single(usize, usize),
    Unsatisfiable,
    Multiple,
}

fn parse_range(value: &str, size: usize) -> Option<ParsedRange> {
    let list = value.strip_prefix("bytes=")?;
    if list.contains(',') {
        return Some(ParsedRange::Multiple);
    }
    let (start, end) = list.split_once('-')?;
    let start = start.trim();
    let end = end.trim();
    if start.is_empty() {
        // `-n`: the last n bytes.
        let suffix: usize = end.parse().ok()?;
        if suffix == 0 {
            return Some(ParsedRange::Unsatisfiable);
        }
        let begin = size.saturating_sub(suffix);
        return Some(ParsedRange::Single(begin, size.saturating_sub(1)));
    }
    let begin: usize = start.parse().ok()?;
    if begin >= size {
        return Some(ParsedRange::Unsatisfiable);
    }
    let last = if end.is_empty() {
        size.saturating_sub(1)
    } else {
        let requested: usize = end.parse().ok()?;
        requested.min(size.saturating_sub(1))
    };
    if last < begin {
        return Some(ParsedRange::Unsatisfiable);
    }
    Some(ParsedRange::Single(begin, last))
}

/// "Sun, 06 Nov 1994 08:49:37 GMT" to Unix seconds.
fn http_date_to_unix(value: &str) -> Option<i64> {
    let format = time::macros::format_description!(
        "[weekday repr:short], [day] [month repr:short] [year] [hour]:[minute]:[second] GMT"
    );
    let parsed = time::PrimitiveDateTime::parse(value.trim(), &format).ok()?;
    Some(parsed.assume_utc().unix_timestamp())
}

fn modified_unix(parts: &axum::http::response::Parts) -> Option<i64> {
    let value = parts.headers.get(header::LAST_MODIFIED)?.to_str().ok()?;
    http_date_to_unix(value)
}

/// Weak comparison of one entity tag, as a conditional GET uses.
fn etag_matches(candidate: &str, ours: &str) -> bool {
    let strip = |tag: &str| tag.trim().trim_start_matches("W/").trim().to_string();
    strip(candidate) == strip(ours)
}

fn not_modified(request: &HeaderMap, parts: &axum::http::response::Parts) -> bool {
    if let (Some(if_none_match), Some(etag)) = (
        request
            .get(header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok()),
        parts
            .headers
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok()),
    ) {
        if if_none_match.trim() == "*" {
            return true;
        }
        return if_none_match
            .split(',')
            .any(|candidate| etag_matches(candidate, etag));
    }
    // If-None-Match takes precedence; the modification time is only consulted
    // when the client did not send a tag.
    if let (Some(if_modified_since), Some(modified)) = (
        request
            .get(header::IF_MODIFIED_SINCE)
            .and_then(|v| v.to_str().ok()),
        modified_unix(parts),
    ) {
        if let Some(since) = http_date_to_unix(if_modified_since) {
            return modified <= since;
        }
    }
    false
}

/// `If-Range` decides whether the Range header applies at all.
fn if_range_allows(request: &HeaderMap, parts: &axum::http::response::Parts) -> bool {
    let Some(value) = request.get(header::IF_RANGE).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    if let Some(etag) = parts
        .headers
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
    {
        return etag_matches(value.trim(), etag);
    }
    if let Some(modified) = modified_unix(parts) {
        return http_date_to_unix(value.trim())
            .map(|date| date == modified)
            .unwrap_or(false);
    }
    // No ETag and no modification time: Go's `checkIfRange` cannot confirm the
    // representation is unchanged, so it drops the Range and sends the whole
    // body. Treating this as "allow the range" would return 206 where Go
    // returns 200, which is what the head-to-head check caught.
    false
}

/// Serve `content` with the preconditions and range a client asked for.
pub fn serve(
    request: &HeaderMap,
    mut parts: axum::http::response::Parts,
    content: &[u8],
) -> Response {
    parts
        .headers
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if not_modified(request, &parts) {
        parts.status = StatusCode::NOT_MODIFIED;
        // `writeNotModified` removes the body headers and keeps the validators.
        parts.headers.remove(header::CONTENT_TYPE);
        parts.headers.remove(header::CONTENT_LENGTH);
        parts.headers.remove(header::CONTENT_ENCODING);
        return Response::from_parts(parts, Body::empty());
    }

    let range = match request.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        Some(value) if if_range_allows(request, &parts) => parse_range(value, content.len()),
        _ => None,
    };

    match range {
        Some(ParsedRange::Unsatisfiable) => {
            parts.status = StatusCode::RANGE_NOT_SATISFIABLE;
            parts.headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{}", content.len()))
                    .unwrap_or(HeaderValue::from_static("bytes */0")),
            );
            parts.headers.remove(header::CONTENT_TYPE);
            parts.headers.remove(header::CONTENT_LENGTH);
            // `http.Error` writes this exact message, so a byte-level comparison
            // of the two builds sees the same body.
            let mut response =
                Response::from_parts(parts, Body::from("invalid range: failed to overlap\n"));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            );
            response.headers_mut().insert(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            );
            response
        }
        Some(ParsedRange::Single(begin, end)) => {
            parts.status = StatusCode::PARTIAL_CONTENT;
            let slice = &content[begin..=end];
            parts.headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {begin}-{end}/{}", content.len()))
                    .unwrap_or(HeaderValue::from_static("bytes 0-0/0")),
            );
            parts.headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&slice.len().to_string())
                    .unwrap_or(HeaderValue::from_static("0")),
            );
            Response::from_parts(parts, Body::from(slice.to_vec()))
        }
        // `Multiple` keeps the whole body, as stated at the top of the module.
        _ => {
            parts.headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&content.len().to_string())
                    .unwrap_or(HeaderValue::from_static("0")),
            );
            Response::from_parts(parts, Body::from(content.to_vec()))
        }
    }
}
