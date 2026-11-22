//! Upload and stored-file routes.

use axum::extract::{FromRequest, Multipart, Path, Query, Request};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::json;

use crate::appcfg;
use crate::auth::{self, AuthError};
use crate::envelope::{self, error_code, success, success_data};

use super::*;

/// `FileRouter.GetList`, behind `AdminInterceptor`: optional `limit`/`page`,
/// newest first, and `list` is `null` when there are no rows.
pub(super) async fn file_list(
    headers: HeaderMap,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let user = match auth::user(&headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return error_code(1000),
        Err(AuthError::Disabled) => return error_code(1004),
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let limit: i64 = query
        .get("limit")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let page: i64 = query
        .get("page")
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
        .max(1);
    let (total, rows) =
        match crate::repository::file_list(pool, ctx.sqlx_driver(), limit, page).await {
            Ok(result) => result,
            Err(_) => (0, Vec::new()),
        };
    let prefix = if ctx.conf.base.url_prefix.is_empty() {
        "/uploads/"
    } else {
        ctx.conf.base.url_prefix.as_str()
    };
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "createTime": rfc3339(&row.created_at),
                "fileName": row.file_name,
                "id": row.id,
                "src": format!("{prefix}{}", row.file_name),
                "updateTime": rfc3339(&row.updated_at),
            })
        })
        .collect();
    let list = if list.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Array(list)
    };
    success_data(json!({"count": total, "list": list}))
}

/// `FileRouter.UploadImg`: MD5-named content, one `file` row per upload.
pub(super) async fn upload_image(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let mut multipart = match Multipart::from_request(request, &()).await {
        Ok(multipart) => multipart,
        Err(_) => return error_code(1300),
    };
    let mut uploaded: Option<(String, Vec<u8>)> = None;
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();
        if name != "imgfile" {
            continue;
        }
        let file_name = field.file_name().unwrap_or("").to_string();
        let data = match field.bytes().await {
            Ok(data) => data.to_vec(),
            Err(_) => return error_code(1300),
        };
        uploaded = Some((file_name, data));
        break;
    }
    let Some((original, content)) = uploaded else {
        return error_code(1300);
    };
    let ext = extension_of(&original);
    if !UPLOAD_EXTENSIONS.contains(&ext.as_str()) {
        return error_code(1301);
    }
    if ext == ".mp4" || ext == ".webm" {
        if content.len() > 20 << 20 {
            return error_code(1300);
        }
        let valid = if ext == ".mp4" {
            content.len() >= 8 && &content[4..8] == b"ftyp"
        } else {
            content.len() >= 4 && &content[..4] == b"\x1a\x45\xdf\xa3"
        };
        if !valid {
            return error_code(1301);
        }
    }
    let file_name = format!("{}{}", hex_digest(&content, false), ext);
    if store_object(&file_name, &content).is_err() {
        return error_code(1300);
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(_) => return error_code(1300),
    };
    if crate::repository::insert_file(
        pool,
        ctx.sqlx_driver(),
        user.id,
        &file_name,
        &now_like_go(),
    )
    .await
    .is_err()
    {
        return error_code(1300);
    }
    success_data(json!({
        "fileName": file_name,
        "imageUrl": format!("{}{}", upload_prefix(), file_name),
    }))
}

/// `FileRouter.Delete`: the row and the stored bytes.
pub(super) async fn delete_file(headers: HeaderMap, request: Request) -> Response {
    let actor = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if actor.role != 1 {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: FileDeleteRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    if req.id == 0 {
        return envelope::error(
            " [Key: 'RequestDeleteId.Id' Error:Field validation for 'Id' failed on the 'required' tag]",
        );
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let file_name = match crate::repository::file_name_by_id(pool, ctx.sqlx_driver(), req.id).await
    {
        Ok(Some(name)) => name,
        Ok(None) | Err(_) => return envelope::database_error(),
    };
    let _ = std::fs::remove_file(storage_dir().join(&file_name));
    match crate::repository::delete_file_row(pool, ctx.sqlx_driver(), req.id).await {
        Ok(()) => success(),
        Err(_) => envelope::database_error(),
    }
}

pub(super) async fn web_wallpaper_asset(
    headers: HeaderMap,
    Path((id, name)): Path<(String, String)>,
) -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row = crate::repository::web_wallpaper(pool, ctx.sqlx_driver(), &id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/web_wallpaper_asset", &error);
            None
        });
    let Some(record) = row else {
        return envelope::empty_not_found();
    };
    let (html, poster, poster_type) = (record.html, record.poster, record.poster_type);
    let (media_type, content) = if name == "index.html" {
        ("text/html".to_string(), html)
    } else {
        let expected = match poster_type.as_str() {
            "image/png" => "poster.png",
            "image/jpeg" => "poster.jpg",
            "image/webp" => "poster.webp",
            _ => "",
        };
        if name != expected {
            return envelope::empty_not_found();
        }
        (poster_type, poster)
    };
    // `serveThemeAsset`: HTML revalidates and carries the sandbox policy, every
    // other asset is immutable.
    serve_bytes(&headers, &media_type, &content)
}

/// `ThemeRouter.WallpaperLibraryAsset`: the packaged artwork, only under the
/// catalogue's own version prefix.
pub(super) async fn wallpaper_library_asset(
    headers: HeaderMap,
    Path((version, raw_name)): Path<(String, String)>,
) -> Response {
    let catalogue = wallpaperpack_rs::port::catalogue();
    let expected = catalogue
        .get("version")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    if version != expected {
        return envelope::empty_not_found();
    }
    let name = raw_name.trim_start_matches('/').to_string();
    match wallpaperpack_rs::port::asset(name) {
        Some((content, media_type)) => serve_bytes(&headers, &media_type, &content),
        None => envelope::empty_not_found(),
    }
}

// ------------------------------------------------------- Go router fallback

/// Go matches `/uploads/` against the wildcard route with an empty path, so the
/// handler answers the same 404 instead of Gin's NoRoute.
pub(super) async fn serve_stored_file_empty() -> Response {
    static_error(
        StatusCode::NOT_FOUND,
        "{\"error\":\"failed to get file\"}",
        false,
    )
}

/// `FileRouter.GetS3File`: an uploaded file from local storage, with the cache
/// policy its content-hash name allows.
pub(super) async fn serve_stored_file(headers: HeaderMap, Path(filepath): Path<String>) -> Response {
    serve_stored(&headers, filepath, false).await
}

/// The `/api` registration of the same handler; failures carry `no-store`.
pub(super) async fn serve_stored_file_api(headers: HeaderMap, Path(filepath): Path<String>) -> Response {
    serve_stored(&headers, filepath, true).await
}

/// `POST /api/theme/v2/wallpaper/web`: a self-contained web wallpaper archive.
pub(super) async fn upload_web_wallpaper(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let mut multipart = match Multipart::from_request(request, &()).await {
        Ok(multipart) => multipart,
        Err(_) => return envelope::error(" [web wallpaper package is required]"),
    };
    let mut data: Option<Vec<u8>> = None;
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        if field.name() == Some("package") {
            match field.bytes().await {
                Ok(bytes) => data = Some(bytes.to_vec()),
                Err(_) => return envelope::error(" [web wallpaper package is required]"),
            }
        }
    }
    let Some(data) = data else {
        return envelope::error(" [web wallpaper package is required]");
    };
    if data.len() > MAX_WEB_WALLPAPER {
        return envelope::error(" [web wallpaper package exceeds 6 MiB]");
    }
    let (html, poster, poster_type) = match parse_web_wallpaper(&data) {
        Ok(parts) => parts,
        Err(error) => return envelope::error(&format!(" [{error}]")),
    };
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:", user.id).as_bytes());
    hasher.update(&html);
    hasher.update(&poster);
    let id: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let exists = crate::repository::web_wallpaper_exists(pool, ctx.sqlx_driver(), &id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/upload_web_wallpaper", &error);
            false
        });
    if !exists
        && crate::repository::insert_web_wallpaper(
            pool,
            ctx.sqlx_driver(),
            &id,
            user.id,
            &html,
            &poster,
            &poster_type,
            &now_like_go(),
        )
        .await
        .is_err()
    {
        return envelope::database_error();
    }
    let poster_name = match poster_type.as_str() {
        "image/jpeg" => "poster.jpg",
        "image/webp" => "poster.webp",
        _ => "poster.png",
    };
    let root = format!("/api/theme/v2/wallpaper/web/{id}/");
    success_data(json!({
        "poster": format!("{root}{poster_name}"),
        "source": format!("{root}index.html"),
    }))
}
