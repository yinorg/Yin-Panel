//! The first ported Core routes: the ones that need only configuration, the
//! session and one user row. Each mirrors the Go handler in
//! `backend/internal/web/router/system`.

use axum::Router;
use axum::extract::{Path, Query, Request};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::{get, post};
use md5::{Digest, Md5};
use serde::Serialize;
use serde_json::json;

use crate::appcfg;
use crate::auth::{self, AuthError};
use crate::envelope::{self, error_code, success};

fn go_json_error(error: &serde_json::Error) -> String {
    if error.is_eof() {
        "unexpected EOF".to_string()
    } else {
        error.to_string()
    }
}

pub fn router() -> Router<()> {
    Router::new()
        .route("/api/about", get(about))
        .route("/api/system/capabilities", get(capabilities))
        .route("/api/oauth/config", get(oauth_config))
        .route("/api/oauth/{provider}", get(oauth_login))
        .route("/api/oauth/{provider}/callback", get(oauth_callback))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/user/getUser", get(get_user))
        .route("/api/spaces", get(list_spaces))
        .route("/api/spaces/{space_id}/search-config", get(search_config))
        .route("/api/spaces/{space_id}/groups", get(groups))
        .route("/api/spaces/{space_id}/items", get(items))
        .route("/api/spaces/{space_id}/members", get(members))
        .route(
            "/api/spaces/{space_id}/public",
            get(space_public).post(set_space_public),
        )
        .route("/api/system/moduleConfig/getByName", get(module_config))
        .route("/api/file/getList", get(file_list))
        .route("/api/file/uploadImg", post(upload_image))
        .route("/api/panel/itemIcon/getSiteFavicon", post(get_site_favicon))
        .route("/api/user/updateInfo", post(update_user_info))
        .route("/api/user/updatePassword", post(update_password))
        .route("/api/file/s3/{*filepath}", get(serve_stored_file_api))
        .route("/uploads/", get(serve_stored_file_empty))
        .route("/uploads/{*filepath}", get(serve_stored_file))
        .route("/api/spaces/{space_id}/bookmarks/tree", get(bookmarks_tree))
        .route("/api/panel/users/getList", get(users_list))
        .route("/api/panel/users/create", post(create_user))
        .route("/api/panel/users/update", post(update_user))
        .route("/api/panel/users/delete", post(delete_user))
        .route("/api/file/delete", post(delete_file))
        .route("/api/panel/userConfig/getConfig", get(user_config))
        .route(
            "/api/system/monitor/getEnableStatus",
            post(monitor_enable_status),
        )
        .route("/api/system/monitor/getSnapshot", post(monitor_snapshot))
        .route(
            "/api/system/monitor/getDiskMountpoints",
            post(monitor_disk_mountpoints),
        )
        .route(
            "/api/system/monitor/getDiskStateByPath",
            post(monitor_disk_state),
        )
        .route(
            "/api/theme/v2/preference",
            get(theme_preference).post(set_theme_preference),
        )
        .route("/api/theme/v2/admin/revisions", get(admin_revisions))
        .route("/api/theme/v2/admin/packages", get(admin_revisions))
        .route("/api/theme/v2/admin/audit", get(admin_audit))
        .route(
            "/api/theme/v2/admin/trusted/{revision}",
            get(admin_trusted).put(set_trusted),
        )
        .route("/api/theme/v2/revisions/{revision}", get(theme_revision))
        .route("/api/theme/v2/admin/preview", post(theme_preview_upload))
        .route("/api/theme/v2/admin/install", post(theme_install))
        .route(
            "/api/theme/v2/admin/packages/{package_id}",
            axum::routing::delete(remove_package),
        )
        .route("/api/theme/v2/admin/default", post(set_default_package))
        .route("/api/theme/v2/admin/trial", post(begin_trial))
        .route("/api/theme/v2/admin/confirm", post(confirm_trial))
        .route("/api/theme/v2/admin/rollback", post(rollback_theme))
        .route(
            "/api/theme/v2/space/{space_id}/mine",
            axum::routing::put(set_user_space_theme).delete(clear_user_space_theme),
        )
        .route("/api/spaces/{space_id}/copy", post(copy_space))
        .route("/api/theme/v2/wallpaper/web", post(upload_web_wallpaper))
        .route(
            "/api/theme/v2/wallpaper/library/assets/{version}/{*name}",
            get(wallpaper_library_asset),
        )
        .route("/api/theme/v2/preview/{token}", get(theme_preview_get))
        .route(
            "/api/theme/v2/preview/{token}/assets/{*name}",
            get(theme_preview_asset),
        )
        .route(
            "/api/theme/v2/grants/{revision}",
            get(theme_grant)
                .post(set_theme_grant)
                .delete(revoke_theme_grant),
        )
        .route(
            "/api/theme/v2/wallpaper/web/{id}/{name}",
            get(web_wallpaper_asset),
        )
        .route(
            "/api/theme/v2/space/{space_id}",
            get(theme_space)
                .put(set_space_theme)
                .delete(clear_space_theme),
        )
        .route("/api/theme/v2/current", get(theme_current))
        .route("/api/theme/v2/packages", get(theme_packages))
        .route("/api/theme/v2/package/{package_id}", get(theme_package))
        .route("/api/theme/v2/mine", get(theme_mine))
        .route("/api/theme/v2/effective", get(theme_effective))
        .route("/api/theme/v2/assets/{revision}/{*name}", get(theme_asset))
        .route("/api/system/moduleConfig/save", post(module_config_save))
        .route("/api/panel/userConfig/setConfig", post(user_config_save))
        .route("/api/spaces/{space_id}/groups", post(create_group))
        .route(
            "/api/spaces/{space_id}/groups/{group_id}",
            post(update_group).delete(delete_group).put(update_group),
        )
        .route("/api/spaces/{space_id}/members", post(add_member))
        .route(
            "/api/spaces/{space_id}/members/{user_id}",
            axum::routing::put(update_member)
                .post(update_member)
                .delete(remove_member),
        )
        .route(
            "/api/spaces/{space_id}/members/{user_id}/delete",
            post(remove_member),
        )
        .route(
            "/api/spaces/{space_id}/search-config",
            post(set_search_config),
        )
        .route(
            "/api/spaces/{space_id}/wallpaper",
            get(get_wallpaper).post(set_wallpaper),
        )
        .route(
            "/api/spaces/{space_id}/oidc-groups",
            get(list_oidc_groups).post(add_oidc_group),
        )
        .route(
            "/api/spaces/{space_id}/oidc-groups/{rule_id}",
            axum::routing::delete(delete_oidc_group),
        )
        .route("/api/spaces/{space_id}/transfer", post(transfer_space))
        .route("/api/spaces/{space_id}/clear", post(clear_space))
        .route("/api/spaces/{space_id}/items", post(create_item))
        .route(
            "/api/spaces/{space_id}/items/with-icon",
            post(create_item_with_icon),
        )
        .route(
            "/api/spaces/{space_id}/bookmarks/import-batch",
            post(import_bookmarks),
        )
        .route("/api/spaces/{space_id}/items/sort", post(sort_items))
        .route("/api/spaces/{space_id}/groups/sort", post(sort_groups))
        .route(
            "/api/spaces/{space_id}/layout/arrange",
            post(arrange_layout),
        )
        .route(
            "/api/spaces/{space_id}",
            axum::routing::delete(delete_space)
                .post(rename_space)
                .put(rename_space),
        )
        .route("/api/spaces/{space_id}/delete", post(delete_space))
        .route("/api/spaces/shared", post(create_space))
        .route("/api/spaces/teams", post(create_space))
        .route(
            "/api/spaces/{space_id}/items/{item_id}",
            axum::routing::put(update_item).delete(delete_item),
        )
        .route(
            "/api/spaces/{space_id}/items/{item_id}/update",
            post(update_item),
        )
        .route(
            "/api/spaces/{space_id}/items/{item_id}/delete",
            post(delete_item),
        )
        .route(
            "/api/spaces/{space_id}/groups/{group_id}/update",
            post(update_group),
        )
        .route(
            "/api/spaces/{space_id}/groups/{group_id}/delete",
            post(delete_group),
        )
}

#[derive(serde::Deserialize)]
struct OauthCallbackQuery {
    #[serde(default)]
    code: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    error: Option<String>,
}

fn cookie_named(headers: &HeaderMap, name: &str) -> Option<String> {
    for value in headers.get_all(header::COOKIE) {
        let Ok(text) = value.to_str() else { continue };
        for part in text.split(';') {
            let part = part.trim();
            if let Some((cookie_name, cookie_value)) = part.split_once('=')
                && cookie_name == name
                && !cookie_value.is_empty()
            {
                return Some(cookie_value.to_string());
            }
        }
    }
    None
}

/// A `302` with zero or more `Set-Cookie` headers, the shape both OAuth routes
/// answer with.
fn redirect_with_cookies(location: &str, cookies: &[String]) -> Response {
    let location = HeaderValue::from_str(location)
        .unwrap_or_else(|_| HeaderValue::from_static("/oauth/callback"));
    let mut response = (StatusCode::FOUND, [(header::LOCATION, location)]).into_response();
    for cookie in cookies {
        if let Ok(value) = HeaderValue::from_str(cookie) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

fn oauth_failure(provider: &str, reason: &str) -> Response {
    let location = format!(
        "{}/oauth/callback?error={}",
        appcfg::ctx().root_url(),
        urlencoding::encode(reason)
    );
    redirect_with_cookies(&location, &[crate::oauth::clear_state_cookie(provider)])
}

/// `UserInfo` in `backend/internal/web/model/base`, whose field order is the
/// order the API answers with.
#[derive(Serialize)]
pub struct UserInfo {
    pub id: i64,
    pub name: String,
    pub role: i64,
    pub mail: String,
    pub token: String,
}

#[derive(serde::Deserialize)]
struct LoginParams {
    #[serde(default)]
    mail: String,
    #[serde(default)]
    password: String,
}

/// `util.PasswordEncryption`: MD5 applied three times, hex, lowercase.
fn password_encryption(password: &str) -> String {
    let once = hex_md5(password.as_bytes());
    let twice = hex_md5(once.as_bytes());
    hex_md5(twice.as_bytes())
}

/// The `email` validator's shape: `local@label(.label)+`, no spaces.
fn is_email(value: &str) -> bool {
    if value.contains(char::is_whitespace) || value.matches('@').count() != 1 {
        return false;
    }
    let (local, domain) = value.split_once('@').unwrap_or_default();
    if local.is_empty() || domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }
    domain.contains('.') && domain.split('.').all(|label| !label.is_empty())
}

fn hex_md5(bytes: &[u8]) -> String {
    Md5::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Go stores `time.Time` in SQLite as `YYYY-MM-DD HH:MM:SS(.frac)(offset)` and
/// encodes it as RFC 3339, which only differs by the separator.
fn rfc3339(stored: &str) -> String {
    match stored.split_once(' ') {
        Some((date, rest)) => format!("{date}T{rest}"),
        None => stored.to_string(),
    }
}

/// `spaceListDTO`: the Space fields in Go's order, then `canEdit`.
#[derive(Serialize)]
struct SpaceItem {
    id: i64,
    #[serde(rename = "createTime")]
    create_time: String,
    #[serde(rename = "updateTime")]
    update_time: String,
    #[serde(rename = "type")]
    kind: String,
    name: String,
    #[serde(rename = "ownerUserId")]
    owner_user_id: i64,
    #[serde(rename = "pairId")]
    pair_id: i64,
    side: String,
    #[serde(rename = "pairedSpaceId", skip_serializing_if = "Option::is_none")]
    paired_space_id: Option<i64>,
    #[serde(rename = "publicEnabled")]
    public_enabled: bool,
    #[serde(rename = "publicId", skip_serializing_if = "Option::is_none")]
    public_id: Option<String>,
    #[serde(rename = "publicMode")]
    public_mode: String,
    #[serde(rename = "canEdit")]
    can_edit: bool,
}

async fn guard_shared(headers: &HeaderMap, space_id: i64) -> Result<sqlx::AnyPool, Response> {
    let user = match auth::user(headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return Err(error_code(1000)),
        Err(AuthError::Disabled) => return Err(error_code(1004)),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return Err(envelope::error(&error)),
    };
    match crate::repository::can_access_space(&pool, ctx.sqlx_driver(), user.id, space_id).await {
        Ok(true) => Ok(pool),
        Ok(false) | Err(_) => Err(error_code(1005)),
    }
}

#[derive(Serialize)]
struct GroupItem {
    id: i64,
    #[serde(rename = "createTime")]
    create_time: String,
    #[serde(rename = "updateTime")]
    update_time: String,
    icon: String,
    title: String,
    description: String,
    sort: i64,
    #[serde(rename = "userId")]
    user_id: i64,
    #[serde(rename = "spaceId")]
    space_id: i64,
    #[serde(rename = "parentId", skip_serializing_if = "Option::is_none")]
    parent_id: Option<i64>,
    #[serde(rename = "itemCount", skip_serializing_if = "is_zero")]
    item_count: i64,
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}

#[derive(Default, Serialize, serde::Deserialize)]
struct IconInfo {
    #[serde(rename = "itemType", default)]
    item_type: i64,
    #[serde(default)]
    src: String,
    #[serde(rename = "fileName", default)]
    file_name: String,
    #[serde(default)]
    text: String,
    #[serde(rename = "backgroundColor", default)]
    background_color: String,
}

#[derive(Serialize)]
struct ItemOut {
    id: i64,
    #[serde(rename = "createTime")]
    create_time: String,
    #[serde(rename = "updateTime")]
    update_time: String,
    icon: IconInfo,
    title: String,
    url: String,
    #[serde(rename = "lanUrl")]
    lan_url: String,
    #[serde(rename = "mobileUrl")]
    mobile_url: String,
    description: String,
    #[serde(rename = "openMethod")]
    open_method: i64,
    sort: i64,
    #[serde(rename = "itemIconGroupId")]
    item_icon_group_id: i64,
    #[serde(rename = "userId")]
    user_id: i64,
    #[serde(rename = "spaceId")]
    space_id: i64,
}

/// `canManageSpace`: the owner or a member whose role is `admin`.

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublicConfigRequest {
    enabled: bool,
    #[serde(default)]
    public_id: String,
    mode: String,
    #[serde(default)]
    access_code: String,
}

fn build_tree(
    children: &std::collections::HashMap<i64, Vec<&(i64, String, String, Option<i64>)>>,
    items: &[(i64, String, String)],
    parent: i64,
    depth: i64,
) -> Vec<serde_json::Value> {
    let Some(list) = children.get(&parent) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(list.len());
    for group in list {
        let group_items: Vec<serde_json::Value> = items
            .iter()
            .filter(|item| item.0 == group.0)
            .map(|item| json!({ "title": item.1, "url": item.2 }))
            .collect();
        let nested = if depth < BOOKMARK_TREE_MAX_DEPTH {
            build_tree(children, items, group.0, depth + 1)
        } else {
            Vec::new()
        };
        out.push(json!({
            "children": nested,
            "icon": group.2,
            "items": group_items,
            "parentId": group.3,
            "title": group.1,
        }));
    }
    out
}

async fn monitor_guard(headers: &HeaderMap) -> Result<(), Response> {
    if let Err(response) = require_user(headers).await {
        return Err(response);
    }
    if !appcfg::ctx().conf.base.enable_monitor {
        return Err(envelope::error("system monitor is disabled"));
    }
    Ok(())
}

async fn require_user(headers: &HeaderMap) -> Result<auth::UserRow, Response> {
    match auth::user(headers).await {
        Ok(user) => Ok(user),
        Err(AuthError::NotLoggedIn) => Err(error_code(1000)),
        Err(AuthError::Disabled) => Err(error_code(1004)),
    }
}

/// `PackageRevisionPublicV2`: the stored manifest and tokens pass through in
/// their stored order, with the asset URL added to each resource.
async fn revision_public(
    pool: &sqlx::AnyPool,
    driver: crate::db::Driver,
    revision_id: &str,
) -> Result<serde_json::Value, String> {
    let parts = crate::repository::revision_parts(pool, driver, revision_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "revision not found".to_string())?;
    revision_public_from_parts(
        revision_id,
        &parts.manifest_json,
        &parts.tokens_json,
        parts.verified,
    )
}

fn revision_public_from_parts(
    revision_id: &str,
    manifest_json: &str,
    tokens_json: &str,
    verified: i64,
) -> Result<serde_json::Value, String> {
    let mut manifest: serde_json::Value =
        serde_json::from_str(manifest_json).map_err(|error| error.to_string())?;
    if let Some(resources) = manifest
        .get_mut("resources")
        .and_then(serde_json::Value::as_array_mut)
    {
        for resource in resources {
            let path = resource
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            let escaped: Vec<String> = path.split('/').map(path_escape).collect();
            resource["url"] = json!(format!(
                "/api/theme/v2/assets/{}/{}",
                path_escape(revision_id),
                escaped.join("/")
            ));
        }
    }
    let tokens: serde_json::Value =
        serde_json::from_str(tokens_json).map_err(|error| error.to_string())?;
    Ok(json!({
        "manifest": manifest,
        "tokens": tokens,
        "verified": verified != 0,
        "revision": revision_id,
    }))
}

/// `url.PathEscape` for one path segment.
fn path_escape(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        let keep = byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_'
                    | b'.'
                    | b'~'
                    | b'+'
                    | b'$'
                    | b'&'
                    | b'!'
                    | b'*'
                    | b'\''
                    | b'('
                    | b')'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
            );
        if keep {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

async fn instance_default_package(
    pool: &sqlx::AnyPool,
    driver: crate::db::Driver,
) -> Result<serde_json::Value, String> {
    let revision = crate::repository::latest_revision(pool, driver, "org.yin.default")
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "package not found".to_string())?;
    revision_public(pool, driver, &revision).await
}

async fn user_theme_modes(
    pool: &sqlx::AnyPool,
    driver: crate::db::Driver,
    user_id: i64,
) -> (String, String) {
    let row = crate::repository::user_theme_preference(pool, driver, user_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/user_theme_modes", &error);
            None
        });
    let (mode, theme_mode) =
        row.unwrap_or_else(|| ("auto".to_string(), "follow-space".to_string()));
    (
        if mode.is_empty() { "auto".into() } else { mode },
        if theme_mode.is_empty() {
            "follow-space".into()
        } else {
            theme_mode
        },
    )
}

/// The timestamps GORM writes: local time, nanosecond fraction, numeric offset.
fn now_like_go() -> String {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let now = time::OffsetDateTime::now_utc().to_offset(offset);
    let format = time::macros::format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:9][offset_hour sign:mandatory]:[offset_minute]"
    );
    now.format(&format).unwrap_or_default()
}

fn supported_language(language: &str) -> bool {
    matches!(
        language,
        "auto"
            | "en-US"
            | "zh-CN"
            | "zh-TW"
            | "ja-JP"
            | "ko-KR"
            | "de-DE"
            | "fr-FR"
            | "es-ES"
            | "pt-BR"
            | "ru-RU"
            | "id-ID"
            | "tr-TR"
            | "pl-PL"
            | "it-IT"
            | "nl-NL"
            | "hi-IN"
            | "th-TH"
    )
}

/// `themeWallpaperEntryV2`: `<packageId>:<entryId>`, both dot/dash/underscore-safe.
fn theme_wallpaper_entry(value: &str) -> bool {
    let Some((package, entry)) = value.split_once(':') else {
        return false;
    };
    let valid = |part: &str| {
        !part.is_empty()
            && part.len() <= 64
            && part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
            && !part.starts_with(['.', '-', '_'])
    };
    valid(package) && valid(entry)
}

#[derive(serde::Deserialize)]
struct GroupRequest {
    #[serde(default)]
    title: String,
    #[serde(default)]
    icon: String,
    #[serde(default, rename = "parentId")]
    parent_id: Option<i64>,
}

fn group_item_json(row: crate::repository::GroupListRecord) -> serde_json::Value {
    serde_json::to_value(GroupItem {
        id: row.id,
        create_time: rfc3339(&row.created_at),
        update_time: rfc3339(&row.updated_at),
        icon: row.icon,
        title: row.title,
        description: row.description,
        sort: row.sort,
        user_id: row.user_id,
        space_id: row.space_id,
        parent_id: row.parent_id,
        item_count: row.item_count,
    })
    .unwrap_or(serde_json::Value::Null)
}

fn valid_item_title(title: &str) -> bool {
    title.trim().chars().count() <= 20
}

/// `ensureItemIcon`: a text icon with no source borrows its glyph from the
/// title — up to five Han characters, else up to eight Latin letters, else the
/// first non-space character.
fn ensure_item_icon(title: &str, icon: &mut IconInfo) {
    if icon.item_type != 4 || !icon.src.is_empty() {
        return;
    }
    let trimmed = title.trim();
    let mut han = String::new();
    let mut latin = String::new();
    let mut han_count = 0;
    let mut latin_count = 0;
    for ch in trimmed.chars() {
        let code = ch as u32;
        let is_han = (0x4E00..=0x9FFF).contains(&code)
            || (0x3400..=0x4DBF).contains(&code)
            || (0xF900..=0xFAFF).contains(&code);
        if is_han && han_count < 5 {
            han.push(ch);
            han_count += 1;
        }
        if ch.is_ascii_alphabetic() && latin_count < 8 {
            latin.push(ch);
            latin_count += 1;
        }
    }
    let mut text = if !han.is_empty() { han } else { latin };
    if text.is_empty() {
        text = trimmed
            .chars()
            .find(|ch| !ch.is_whitespace())
            .map(String::from)
            .unwrap_or_default();
    }
    if text.is_empty() {
        text = "?".to_string();
    }
    *icon = IconInfo {
        item_type: 1,
        src: String::new(),
        file_name: String::new(),
        text,
        background_color: "#2a2a2a6b".to_string(),
    };
}

fn item_json_value(row: crate::repository::ItemListRecord) -> serde_json::Value {
    let icon = if row.icon_json.is_empty() {
        IconInfo::default()
    } else {
        serde_json::from_str(&row.icon_json).unwrap_or_default()
    };
    serde_json::to_value(ItemOut {
        id: row.id,
        create_time: rfc3339(&row.created_at),
        update_time: rfc3339(&row.updated_at),
        icon,
        title: row.title,
        url: row.url,
        lan_url: row.lan_url,
        mobile_url: row.mobile_url,
        description: row.description,
        open_method: row.open_method,
        sort: row.sort,
        item_icon_group_id: row.item_icon_group_id,
        user_id: row.user_id,
        space_id: row.space_id,
    })
    .unwrap_or(serde_json::Value::Null)
}

#[derive(serde::Deserialize)]
struct ItemIn {
    #[serde(default)]
    icon: IconInfo,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default, rename = "lanUrl")]
    lan_url: String,
    #[serde(default, rename = "mobileUrl")]
    mobile_url: String,
    #[serde(default)]
    description: String,
    #[serde(default, rename = "openMethod")]
    open_method: i64,
    #[serde(default)]
    sort: i64,
    #[serde(default, rename = "itemIconGroupId")]
    item_icon_group_id: i64,
}

/// `pairedIDs`: the Space itself plus its Yang twin, resolved through pair_id.
fn valid_space_role(role: &str) -> bool {
    matches!(role, "admin" | "editor" | "viewer")
}

#[derive(serde::Deserialize)]
struct MemberAddRequest {
    #[serde(default)]
    email: String,
    #[serde(default)]
    role: String,
}

#[derive(serde::Deserialize)]
struct MemberRoleRequest {
    #[serde(default)]
    role: String,
}

#[derive(serde::Deserialize)]
struct RenameRequest {
    #[serde(default)]
    name: String,
}

/// `SpaceWallpaperConfig` in Go: `mode` then the optional fields.
#[derive(serde::Serialize, serde::Deserialize)]
struct SpaceWallpaper {
    #[serde(default)]
    mode: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    poster: String,
    #[serde(
        default,
        rename = "libraryEntry",
        skip_serializing_if = "String::is_empty"
    )]
    library_entry: String,
}

fn valid_wallpaper_mode(mode: &str) -> bool {
    matches!(mode, "theme" | "library" | "custom" | "none")
}

#[derive(serde::Deserialize)]
struct OidcGroupRequest {
    #[serde(default)]
    provider: String,
    #[serde(default, rename = "groupName")]
    group_name: String,
    #[serde(default)]
    role: String,
}

#[derive(serde::Deserialize)]
struct TransferRequest {
    #[serde(default)]
    email: String,
}

#[derive(serde::Deserialize)]
struct ItemSortRequest {
    #[serde(default, rename = "groupId")]
    group_id: i64,
    #[serde(default, rename = "sortItems")]
    sort_items: Vec<SortEntry>,
}

#[derive(serde::Deserialize)]
struct SortEntry {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    sort: i64,
}

#[derive(serde::Deserialize)]
struct GroupSortRequest {
    #[serde(default, rename = "parentId")]
    parent_id: Option<i64>,
    #[serde(default, rename = "sortGroups")]
    sort_groups: Vec<SortEntry>,
}

#[derive(serde::Deserialize)]
struct ArrangeRequest {
    #[serde(default)]
    items: Vec<ArrangeItem>,
    #[serde(default)]
    groups: Vec<SortEntry>,
}

#[derive(serde::Deserialize)]
struct ArrangeItem {
    #[serde(default)]
    id: i64,
    #[serde(default, rename = "itemIconGroupId")]
    item_icon_group_id: i64,
    #[serde(default)]
    sort: i64,
}

fn upload_prefix() -> String {
    let prefix = &appcfg::ctx().conf.base.url_prefix;
    if prefix.is_empty() {
        "/uploads/".to_string()
    } else {
        prefix.clone()
    }
}

fn storage_dir() -> std::path::PathBuf {
    let ctx = appcfg::ctx();
    let bucket = if ctx.conf.rclone.bucket.is_empty() {
        "uploads".to_string()
    } else {
        ctx.conf.rclone.bucket.clone()
    };
    let path = std::path::Path::new(&bucket);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        ctx.dir.join(path)
    }
}

/// `storage.Local.Upload`: the object is written once and left in place when it
/// already exists, because the name is its content hash.
fn store_object(name: &str, content: &[u8]) -> Result<(), String> {
    if name.is_empty() || name.contains("..") {
        return Err("invalid object key".to_string());
    }
    let path = storage_dir().join(name);
    if path.is_file() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&path, content).map_err(|error| error.to_string())
}

fn extension_of(name: &str) -> String {
    match name.rfind('.') {
        Some(index) => name[index..].to_lowercase(),
        None => String::new(),
    }
}

fn hex_digest(bytes: &[u8], sha256: bool) -> String {
    if sha256 {
        use sha2::{Digest, Sha256};
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    } else {
        Md5::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

#[derive(serde::Deserialize)]
struct BookmarkBatch {
    #[serde(default)]
    groups: Vec<BookmarkNode>,
}

#[derive(serde::Deserialize)]
struct BookmarkNode {
    #[serde(default)]
    title: String,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    items: Vec<BookmarkItem>,
    #[serde(default)]
    children: Vec<BookmarkNode>,
}

#[derive(serde::Deserialize)]
struct BookmarkItem {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default, rename = "uploadKey")]
    upload_key: String,
    #[serde(default)]
    icon: IconInfo,
}

struct ImportSummary {
    groups: i64,
    skipped: i64,
    seen: std::collections::HashSet<String>,
    items: Vec<(String, String, i64, i64, i64, i64, String)>,
}

fn limit_title(title: &str) -> String {
    title.trim().chars().take(20).collect()
}

/// `importBookmarkNode`: find or create the folder, then add its links, skipping
/// a link that is already stored or repeated in the same file.
fn import_node<'a>(
    tx: &'a mut sqlx::Transaction<'_, sqlx::Any>,
    driver: crate::db::Driver,
    icons: &'a std::collections::HashMap<String, IconInfo>,
    space_id: i64,
    user_id: i64,
    parent_id: Option<i64>,
    node: &'a BookmarkNode,
    index: i64,
    summary: &'a mut ImportSummary,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        let title = node.title.trim().to_string();
        if title.is_empty() {
            return Ok(());
        }
        let found: Option<i64> = {
            use sqlx::Row;
            let (sql, bind_parent) = match parent_id {
                None => (
                    "SELECT id FROM item_icon_group WHERE space_id = ? AND title = ? AND parent_id IS NULL LIMIT 1".to_string(),
                    false,
                ),
                Some(_) => (
                    "SELECT id FROM item_icon_group WHERE space_id = ? AND title = ? AND parent_id = ? LIMIT 1".to_string(),
                    true,
                ),
            };
            let sql = crate::db::query(driver, &sql);
            let mut statement = sqlx::query(&sql).bind(space_id).bind(&title);
            if bind_parent {
                statement = statement.bind(parent_id);
            }
            statement
                .fetch_optional(&mut **tx)
                .await
                .map_err(|error| error.to_string())?
                .map(|row| row.try_get::<i64, _>("id").unwrap_or(0))
        };
        let group_id = match found {
            Some(id) => id,
            None => {
                let now = now_like_go();
                let id = crate::repository::insert_group_in_tx(
                    tx,
                    driver,
                    space_id,
                    user_id,
                    &node.icon,
                    &title,
                    index,
                    parent_id,
                    &now,
                )
                .await
                .map_err(|error| error.to_string())?;
                summary.groups += 1;
                id
            }
        };

        for (item_index, bookmark) in node.items.iter().enumerate() {
            if bookmark.url.trim().is_empty() {
                continue;
            }
            let key = format!("{group_id}\t{}", bookmark.url);
            if summary.seen.contains(&key) {
                summary.skipped += 1;
                continue;
            }
            let existing: i64 = {
                use sqlx::Row;
                let sql = crate::db::query(
                    driver,
                    "SELECT COUNT(*) AS total FROM item_icon WHERE space_id = ? AND item_icon_group_id = ? AND url = ?",
                );
                let row = sqlx::query(&sql)
                    .bind(space_id)
                    .bind(group_id)
                    .bind(&bookmark.url)
                    .fetch_one(&mut **tx)
                    .await
                    .map_err(|error| error.to_string())?;
                row.try_get("total").unwrap_or(0)
            };
            if existing > 0 {
                summary.seen.insert(key);
                summary.skipped += 1;
                continue;
            }
            summary.seen.insert(key);

            let mut icon = if bookmark.upload_key.is_empty() {
                IconInfo {
                    item_type: bookmark.icon.item_type,
                    src: bookmark.icon.src.clone(),
                    file_name: bookmark.icon.file_name.clone(),
                    text: bookmark.icon.text.clone(),
                    background_color: bookmark.icon.background_color.clone(),
                }
            } else {
                match icons.get(&bookmark.upload_key) {
                    Some(uploaded) => IconInfo {
                        item_type: uploaded.item_type,
                        src: uploaded.src.clone(),
                        file_name: uploaded.file_name.clone(),
                        text: String::new(),
                        background_color: String::new(),
                    },
                    None => return Err("invalid uploadKey".to_string()),
                }
            };
            if icon.item_type == 0 {
                icon.item_type = 4;
            }
            // `importBookmarkNode` stores the icon it picked *before*
            // `ensureItemIcon` runs, so a text icon is persisted with
            // `itemType: 4` and an empty glyph rather than the derived letter.
            let icon_json = serde_json::to_string(&icon).unwrap_or_else(|_| "{}".to_string());
            summary.items.push((
                icon_json,
                limit_title(&bookmark.title),
                group_id,
                item_index as i64,
                space_id,
                user_id,
                bookmark.url.clone(),
            ));
        }

        for (child_index, child) in node.children.iter().enumerate() {
            import_node(
                tx,
                driver,
                icons,
                space_id,
                user_id,
                Some(group_id),
                child,
                child_index as i64,
                summary,
            )
            .await?;
        }
        Ok(())
    })
}

#[derive(serde::Deserialize)]
struct UserIn {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    password: String,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "headImage")]
    head_image: String,
    #[serde(default)]
    status: i64,
    #[serde(default)]
    role: i64,
    #[serde(default)]
    mail: String,
}

#[derive(serde::Deserialize)]
struct UserDeleteRequest {
    #[serde(default, rename = "userId")]
    user_id: i64,
    #[serde(default)]
    force: bool,
}

#[derive(serde::Deserialize)]
struct FileDeleteRequest {
    #[serde(default)]
    id: i64,
}

/// `repository.User` as the API returns it.
#[derive(Serialize)]
struct AdminUser {
    id: i64,
    #[serde(rename = "createTime")]
    create_time: String,
    #[serde(rename = "updateTime")]
    update_time: String,
    password: String,
    name: String,
    #[serde(rename = "headImage")]
    head_image: String,
    status: i64,
    role: i64,
    mail: String,
    token: String,
}

#[derive(serde::Deserialize)]
struct PreferenceRequest {
    #[serde(default)]
    revision: String,
    #[serde(default, rename = "packageId")]
    package_id: String,
    #[serde(default)]
    mode: String,
    #[serde(default, rename = "themeMode")]
    theme_mode: String,
}

/// `SetUserThemeSelectionV2`: the user's activation and colour scheme.

#[derive(serde::Deserialize)]
struct TrustedRequest {
    #[serde(default)]
    enabled: bool,
}

fn manifest_field<'a>(
    manifest: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    let mut current = manifest;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn supported_modes(manifest: &serde_json::Value) -> Vec<String> {
    manifest_field(manifest, &["runtime", "supportedModes"])
        .and_then(serde_json::Value::as_array)
        .map(|modes| {
            modes
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn declared_permissions(manifest: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["required", "optional"] {
        if let Some(list) =
            manifest_field(manifest, &["permissions", key]).and_then(serde_json::Value::as_array)
        {
            for permission in list {
                if let Some(name) = permission.get("name").and_then(serde_json::Value::as_str) {
                    out.push(name.to_string());
                }
            }
        }
    }
    out
}

fn required_permissions(manifest: &serde_json::Value) -> Vec<String> {
    manifest_field(manifest, &["permissions", "required"])
        .and_then(serde_json::Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|permission| permission.get("name"))
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// `ValidateThemeGrantV2`.
fn validate_grant(
    manifest: &serde_json::Value,
    execution_mode: &str,
    permissions: &[String],
) -> Result<(), String> {
    if !matches!(execution_mode, "sandbox" | "trusted")
        || !supported_modes(manifest)
            .iter()
            .any(|mode| mode == execution_mode)
    {
        return Err(format!(
            "theme does not support the {execution_mode} runtime"
        ));
    }
    let allowed = declared_permissions(manifest);
    let mut granted = std::collections::HashSet::new();
    for permission in permissions {
        if !allowed.iter().any(|name| name == permission) || !granted.insert(permission.clone()) {
            return Err(format!(
                "theme permission {permission:?} is not declared or is duplicated"
            ));
        }
    }
    for required in required_permissions(manifest) {
        if !granted.contains(&required) {
            return Err(format!(
                "required theme permission {required:?} must be granted"
            ));
        }
    }
    Ok(())
}

fn is_builtin_theme(id: &str) -> bool {
    matches!(
        id,
        "org.yin.default" | "org.yin.glass" | "org.yin.directory"
    )
}

/// `ImplicitBuiltinPermissionsV2`: a verified built-in theme needs no grant.
async fn implicit_builtin_permissions(
    pool: &sqlx::AnyPool,
    driver: crate::db::Driver,
    revision: &str,
) -> Option<Vec<String>> {
    let (manifest_json, verified) =
        crate::repository::revision_verified_manifest(pool, driver, revision)
            .await
            .ok()??;
    if verified == 0 {
        return None;
    }
    let manifest: serde_json::Value = serde_json::from_str(&manifest_json).ok()?;
    if !is_builtin_theme(
        manifest
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
    ) {
        return None;
    }
    Some(required_permissions(&manifest))
}

fn execution_mode_from(query: &std::collections::HashMap<String, String>) -> (String, bool) {
    let mode = query
        .get("executionMode")
        .cloned()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "sandbox".to_string());
    let valid = matches!(mode.as_str(), "sandbox" | "trusted");
    (mode, valid)
}

#[derive(serde::Deserialize)]
struct GrantRequest {
    #[serde(default, rename = "executionMode")]
    execution_mode: String,
    #[serde(default)]
    permissions: Vec<String>,
}

/// `ThemeRouter.RevokeThemeGrantV2`.
async fn revoke_theme_grant(
    headers: HeaderMap,
    Path(revision): Path<String>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let (execution_mode, valid) = execution_mode_from(&query);
    if !valid {
        return envelope::error(" [executionMode must be sandbox or trusted]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let driver = ctx.sqlx_driver();
    if execution_mode == "sandbox"
        && implicit_builtin_permissions(pool, driver, &revision)
            .await
            .is_some()
    {
        return match crate::repository::save_grant(
            pool,
            driver,
            user.id,
            &revision,
            "sandbox",
            "[]",
            &now_like_go(),
        )
        .await
        {
            Ok(()) => success(),
            Err(error) => envelope::error(&error.to_string()),
        };
    }
    match crate::repository::delete_grant(pool, driver, user.id, &revision, &execution_mode).await {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `serveThemeAsset` for an in-memory asset: nosniff, the immutable policy, an
/// ETag on HTML and the document sandbox policy.
fn serve_bytes(headers: &HeaderMap, media_type: &str, content: &[u8]) -> Response {
    let is_html = media_type == "text/html";
    let mut response = Response::new(axum::body::Body::empty());
    {
        let out = response.headers_mut();
        for (key, value) in envelope::headers() {
            out.insert(key, value);
        }
        out.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(media_type)
                .unwrap_or(HeaderValue::from_static("application/octet-stream")),
        );
        out.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        let cache = if is_html {
            "public, max-age=0, must-revalidate, no-transform"
        } else {
            "public, max-age=31536000, immutable, no-transform"
        };
        out.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
        if is_html {
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(content);
            let etag = format!(
                "\"{}\"",
                digest
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            );
            out.insert(
                header::ETAG,
                HeaderValue::from_str(&etag).unwrap_or(HeaderValue::from_static("")),
            );
            out.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline' https://static.cloudflareinsights.com; style-src 'unsafe-inline'; img-src data:; connect-src https://cloudflareinsights.com; font-src 'none'; media-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'"));
            out.insert(
                header::REFERRER_POLICY,
                HeaderValue::from_static("no-referrer"),
            );
        } else if media_type == "image/svg+xml" {
            out.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox; default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; object-src 'none'; form-action 'none'; base-uri 'none'"));
            out.insert(
                header::REFERRER_POLICY,
                HeaderValue::from_static("no-referrer"),
            );
        }
    }
    let (parts, _) = response.into_parts();
    crate::content::serve(headers, parts, content)
}

#[derive(serde::Deserialize)]
struct RevisionRequest {
    #[serde(default, rename = "packageId")]
    package_id: String,
    #[serde(default)]
    revision: String,
}

#[derive(serde::Deserialize)]
struct FaviconRequest {
    #[serde(default)]
    url: String,
}

/// Every `<link>` tag's attributes, in document order.
fn link_attributes(html: &str) -> Vec<Vec<(String, String)>> {
    let mut out = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut cursor = 0;
    while let Some(start) = lower[cursor..].find("<link") {
        let start = cursor + start;
        let Some(end) = lower[start..].find('>') else {
            break;
        };
        let end = start + end;
        let tag = &html[start..end];
        let mut attributes = Vec::new();
        let bytes = tag.as_bytes();
        let mut index = 5; // after "<link"
        while index < tag.len() {
            while index < tag.len() && (bytes[index] as char).is_whitespace() {
                index += 1;
            }
            let name_start = index;
            while index < tag.len()
                && !matches!(bytes[index] as char, '=' | ' ' | '\t' | '\n' | '\r' | '/')
            {
                index += 1;
            }
            if name_start == index {
                index += 1;
                continue;
            }
            let name = tag[name_start..index].to_ascii_lowercase();
            while index < tag.len() && (bytes[index] as char).is_whitespace() {
                index += 1;
            }
            if index >= tag.len() || bytes[index] != b'=' {
                attributes.push((name, String::new()));
                continue;
            }
            index += 1;
            while index < tag.len() && (bytes[index] as char).is_whitespace() {
                index += 1;
            }
            let value = if index < tag.len() && (bytes[index] == b'"' || bytes[index] == b'\'') {
                let quote = bytes[index];
                index += 1;
                let value_start = index;
                while index < tag.len() && bytes[index] != quote {
                    index += 1;
                }
                let value = tag[value_start..index].to_string();
                index += 1;
                value
            } else {
                let value_start = index;
                while index < tag.len() && !(bytes[index] as char).is_whitespace() {
                    index += 1;
                }
                tag[value_start..index].to_string()
            };
            attributes.push((name, value));
        }
        out.push(attributes);
        cursor = end + 1;
    }
    out
}

/// `favicon.GetOneFaviconURL`: the first icon a page declares, else the
/// conventional `/favicon.ico`.
async fn discover_icon(page_url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(page_url)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3",
        )
        .send()
        .await
        .map_err(|error| go_http_get_error(page_url, &error))?;
    if !response.status().is_success() {
        return Err(format!(
            "HTTP request failed with status code {}",
            response.status().as_u16()
        ));
    }
    let html = response.text().await.map_err(|error| error.to_string())?;
    let page = reqwest::Url::parse(page_url).map_err(|error| error.to_string())?;
    for attributes in link_attributes(&html) {
        let mut rel = String::new();
        let mut href = String::new();
        for (name, value) in attributes {
            match name.as_str() {
                "rel" => rel = value,
                "href" => href = value,
                _ => {}
            }
        }
        if !rel.contains("icon") || href.is_empty() {
            continue;
        }
        let Ok(icon) = page.join(href.trim()) else {
            continue;
        };
        if matches!(icon.scheme(), "http" | "https") {
            return Ok(icon.to_string());
        }
    }
    // Many sites serve the conventional icon without declaring it. The authority
    // keeps a non-default port, which `host_str` alone drops.
    let host = match page.port() {
        Some(port) => format!("{}:{port}", page.host_str().unwrap_or("")),
        None => page.host_str().unwrap_or("").to_string(),
    };
    Ok(format!("{}://{host}/favicon.ico", page.scheme()))
}

fn go_http_get_error(url: &str, error: &reqwest::Error) -> String {
    let cause = std::iter::successors(std::error::Error::source(error), |cause| {
        std::error::Error::source(*cause)
    })
    .map(|cause| cause.to_string())
    .collect::<Vec<_>>()
    .join(": ");
    if error.is_connect() && cause.to_ascii_lowercase().contains("connection refused") {
        if let Ok(parsed) = reqwest::Url::parse(url) {
            let host = parsed.host_str().unwrap_or("");
            let address = parsed
                .port_or_known_default()
                .map(|port| format!("{host}:{port}"))
                .unwrap_or_else(|| host.to_string());
            return format!("Get \"{url}\": dial tcp {address}: connect: connection refused");
        }
    }
    error.to_string()
}

fn path_segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

/// Gin's tree: static segments, `:param` for one segment and `*name` for the
/// rest of the path.
pub fn pattern_matches(pattern: &str, path: &str) -> bool {
    let expected = path_segments(pattern);
    let actual = path_segments(path);
    let mut index = 0;
    for segment in &expected {
        if segment.starts_with('*') {
            return true;
        }
        if segment.starts_with(':') {
            if index >= actual.len() || actual[index].is_empty() {
                return false;
            }
            index += 1;
            continue;
        }
        if index >= actual.len() || actual[index] != *segment {
            return false;
        }
        index += 1;
    }
    index == actual.len()
}

/// The fallback behind every Rust route. A path the Go build registers is
/// proxied, because that route is not ported yet. A path it does not register
/// answers exactly what Gin's NoRoute answers, and a trailing slash on a path
/// whose trimmed form would match a `:param` route answers Gin's redirect.
pub async fn go_fallback(req: Request) -> Response {
    let mut response = go_fallback_inner(req).await;
    // Only the parity probe sets this: replay and production must see exactly
    // the bytes Gin produced, so the marker header is opt-in.
    if std::env::var_os("YIN_UNIMPLEMENTED_MARKER").is_some() {
        response
            .headers_mut()
            .insert("x-yin-unimplemented", HeaderValue::from_static("1"));
    }
    response
}

async fn go_fallback_inner(req: Request) -> Response {
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        let trimmed = &path[..path.len() - 1];
        let candidate = format!("{trimmed}/x");
        let redirects = crate::go_routes::GO_ROUTES
            .iter()
            .any(|(route_method, pattern)| {
                *route_method == method
                    && !crate::go_routes::conditional_disabled(route_method, pattern)
                    && pattern
                        .rsplit('/')
                        .next()
                        .is_some_and(|segment| segment.starts_with(':'))
                    && pattern_matches(pattern, &candidate)
            });
        if redirects {
            let body = format!("<a href=\"{trimmed}\">Moved Permanently</a>.\n\n");
            return (
                StatusCode::MOVED_PERMANENTLY,
                [
                    (
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("text/html; charset=utf-8"),
                    ),
                    (
                        header::LOCATION,
                        HeaderValue::from_str(trimmed).unwrap_or(HeaderValue::from_static("/")),
                    ),
                ],
                body,
            )
                .into_response();
        }
    }
    // `gin.Engine`'s default NoRoute: no middleware, no security headers.
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"))],
        "404 page not found",
    )
        .into_response()
}

#[derive(serde::Deserialize)]
struct UpdateInfoRequest {
    #[serde(default, rename = "headImage")]
    head_image: String,
    #[serde(default)]
    name: String,
}

#[derive(serde::Deserialize)]
struct UpdatePasswordRequest {
    #[serde(default, rename = "oldPassword")]
    old_password: String,
    #[serde(default, rename = "newPassword")]
    new_password: String,
}

async fn serve_stored(headers: &HeaderMap, filepath: String, api_group: bool) -> Response {
    let name = filepath.trim_start_matches('/').to_string();
    if name.is_empty() {
        return static_error(
            StatusCode::BAD_REQUEST,
            "{\"error\":\"file path is required\"}",
            api_group,
        );
    }
    let path = storage_dir().join(&name);
    // Uploads can be tens of megabytes; a small image is cheaper to read inline.
    let size = std::fs::metadata(&path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let read = if size >= 256 * 1024 {
        let path = path.clone();
        tokio::task::spawn_blocking(move || std::fs::read(&path))
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result.map_err(|error| error.to_string()))
    } else {
        std::fs::read(&path).map_err(|error| error.to_string())
    };
    let content = match read {
        Ok(content) => content,
        Err(_) => {
            return static_error(
                StatusCode::NOT_FOUND,
                "{\"error\":\"failed to get file\"}",
                api_group,
            );
        }
    };
    let extension = extension_of(&name);
    let content_type = match extension.as_str() {
        ".jpg" | ".jpeg" => "image/jpeg",
        ".png" => "image/png",
        ".gif" => "image/gif",
        ".svg" => "image/svg+xml",
        ".webp" => "image/webp",
        ".mp4" => "video/mp4",
        ".webm" => "video/webm",
        _ => "application/octet-stream",
    };
    let base = name.rsplit('/').next().unwrap_or(&name).to_string();
    let mut response = Response::new(axum::body::Body::from(content.clone()));
    {
        let out = response.headers_mut();
        out.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(content_type)
                .unwrap_or(HeaderValue::from_static("application/octet-stream")),
        );
        out.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("inline; filename={base}"))
                .unwrap_or(HeaderValue::from_static("inline")),
        );
        out.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
        out.insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        );
        out.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        out.insert(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("SAMEORIGIN"),
        );
    }
    // `GetS3File` sends only mp4/webm through `http.ServeContent`; an image is
    // written with `c.Data` and therefore has no ranges.
    if matches!(extension.as_str(), ".mp4" | ".webm") {
        let (parts, _) = response.into_parts();
        return crate::content::serve(headers, parts, &content);
    }
    *response.body_mut() = axum::body::Body::from(content);
    response
}

/// A JSON error from the static file routes, with the middleware headers the
/// matched route would add.
fn static_error(status: StatusCode, body: &'static str, api_group: bool) -> Response {
    let mut response = (
        status,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        )],
        body,
    )
        .into_response();
    {
        let headers = response.headers_mut();
        headers.insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        );
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        headers.insert(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("SAMEORIGIN"),
        );
        if api_group {
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        }
    }
    response
}

/// `validWallpaperBytes`: the media type must match the bytes.
fn valid_wallpaper_bytes(media_type: &str, data: &[u8]) -> bool {
    match media_type {
        "image/png" => {
            data.len() >= 8 && data[..8] == [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']
        }
        "image/jpeg" => data.len() >= 3 && data[..3] == [0xff, 0xd8, 0xff],
        "image/gif" => data.len() >= 6 && (&data[..6] == b"GIF87a" || &data[..6] == b"GIF89a"),
        "image/webp" => data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP",
        "video/mp4" => data.len() >= 12 && data.len() <= 20 << 20 && &data[4..8] == b"ftyp",
        "video/webm" => {
            data.len() >= 4 && data.len() <= 20 << 20 && data[..4] == [0x1a, 0x45, 0xdf, 0xa3]
        }
        "text/html" => {
            !data.is_empty() && data.len() <= 5 << 20 && std::str::from_utf8(data).is_ok()
        }
        _ => false,
    }
}

const MAX_WEB_WALLPAPER: usize = 6 << 20;

/// `ParseWebWallpaper`: exactly `index.html` and one `poster.*` at the archive
/// root.
fn parse_web_wallpaper(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>, String), String> {
    if data.is_empty() || data.len() > MAX_WEB_WALLPAPER {
        return Err("web wallpaper package exceeds 6 MiB".to_string());
    }
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|error| error.to_string())?;
    if archive.len() != 2 {
        return Err("web wallpaper must contain index.html and one poster".to_string());
    }
    let mut html: Option<Vec<u8>> = None;
    let mut poster: Option<(Vec<u8>, String)> = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry.name().to_string();
        if entry.is_dir()
            || name.contains('/')
            || name.contains('\\')
            || entry.size() > MAX_WEB_WALLPAPER as u64
        {
            return Err(format!("unsafe web wallpaper file {name:?}"));
        }
        let mut content = Vec::new();
        let mut limited = std::io::Read::take(entry, MAX_WEB_WALLPAPER as u64 + 1);
        std::io::Read::read_to_end(&mut limited, &mut content)
            .map_err(|_| "invalid web wallpaper content".to_string())?;
        if content.len() > MAX_WEB_WALLPAPER {
            return Err("invalid web wallpaper content".to_string());
        }
        match name.as_str() {
            "index.html" => html = Some(content),
            "poster.png" => poster = Some((content, "image/png".to_string())),
            "poster.jpg" => poster = Some((content, "image/jpeg".to_string())),
            "poster.webp" => poster = Some((content, "image/webp".to_string())),
            other => return Err(format!("unexpected web wallpaper file {other:?}")),
        }
    }
    let html =
        html.ok_or_else(|| "web wallpaper must contain index.html and one poster".to_string())?;
    let (poster, poster_type) =
        poster.ok_or_else(|| "web wallpaper must contain index.html and one poster".to_string())?;
    if html.len() + poster.len() > MAX_WEB_WALLPAPER
        || !valid_wallpaper_bytes("text/html", &html)
        || !valid_wallpaper_bytes(&poster_type, &poster)
    {
        return Err("invalid web wallpaper or poster".to_string());
    }
    Ok((html, poster, poster_type))
}

/// Gin registers `HEAD` only where it also registers `GET` for a *file*: the
/// static routes. Everywhere else a `HEAD` request hits NoRoute, while axum
/// would answer it from the `GET` handler. This restores Gin's answer.
pub async fn head_gate(req: Request, next: axum::middleware::Next) -> Response {
    if req.method() == axum::http::Method::HEAD {
        let path = req.uri().path();
        let served = matches!(
            path,
            "/" | "/index.html"
                | "/login"
                | "/oauth/callback"
                | "/clear.html"
                | "/registerSW.js"
                | "/sw.js"
                | "/manifest.webmanifest"
        ) || path.starts_with("/assets/")
            || path.starts_with("/custom/");
        if !served {
            return (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"))],
                "404 page not found",
            )
                .into_response();
        }
    }
    next.run(req).await
}

/// `apiGzip` from `backend/internal/web/router/router.go`.
///
/// gin-contrib/gzip compresses every `/api` response when the client offers
/// gzip: there is no minimum length, the content type is not inspected, the four
/// extensions it excludes are already-compressed images, error statuses stay
/// plain, and a strong ETag is weakened to `W/`. Reproducing those rules rather
/// than dropping in a generic compression layer keeps the header and the body
/// in agreement, which is what `TestAPICompressionNeverLabelsAPlainBody`
/// protects: a response must never be labelled gzip without being gzip.
pub async fn api_gzip(req: Request, next: axum::middleware::Next) -> Response {
    const EXCLUDED_EXTENSIONS: [&str; 4] = ["png", "gif", "jpeg", "jpg"];
    let path = req.uri().path().to_string();
    let offered = req
        .headers()
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("gzip"))
        .unwrap_or(false);
    let upgrading = req
        .headers()
        .get(header::CONNECTION)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("upgrade"))
        .unwrap_or(false);
    if !offered || upgrading || !path.starts_with("/api/") {
        return next.run(req).await;
    }
    let extension = path
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    if EXCLUDED_EXTENSIONS.contains(&extension.as_str()) {
        return next.run(req).await;
    }

    let response = next.run(req).await;
    if response.status().as_u16() >= 400
        || response.headers().contains_key(header::CONTENT_ENCODING)
    {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "").into_response(),
    };
    // gin-contrib only starts compressing once the handler writes a body, so an
    // empty 204 stays empty rather than carrying a gzip header with no stream.
    if bytes.is_empty() {
        return Response::from_parts(parts, axum::body::Body::from(bytes));
    }
    use std::io::Write as _;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    if encoder.write_all(&bytes).is_err() {
        return Response::from_parts(parts, axum::body::Body::from(bytes));
    }
    let compressed = match encoder.finish() {
        Ok(compressed) => compressed,
        Err(_) => return Response::from_parts(parts, axum::body::Body::from(bytes)),
    };

    parts.headers.remove(header::CONTENT_LENGTH);
    parts
        .headers
        .insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    let vary = parts
        .headers
        .get(header::VARY)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .unwrap_or_default();
    let vary = if vary.is_empty() {
        "Accept-Encoding".to_string()
    } else if vary.contains("Accept-Encoding") {
        vary
    } else {
        format!("{vary}, Accept-Encoding")
    };
    if let Ok(value) = HeaderValue::from_str(&vary) {
        parts.headers.insert(header::VARY, value);
    }
    if let Some(etag) = parts
        .headers
        .get(header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        && !etag.starts_with("W/")
        && let Ok(value) = HeaderValue::from_str(&format!("W/{etag}"))
    {
        parts.headers.insert(header::ETAG, value);
    }
    Response::from_parts(parts, axum::body::Body::from(compressed))
}

const UPLOAD_EXTENSIONS: [&str; 9] = [
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".ico", ".mp4", ".webm",
];

const BOOKMARK_TREE_MAX_DEPTH: i64 = 64;

mod system;
mod space;
mod file;
mod theme;
mod user;

use system::*;
use space::*;
use file::*;
use theme::*;
use user::*;
