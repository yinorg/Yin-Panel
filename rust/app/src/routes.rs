//! The first ported Core routes: the ones that need only configuration, the
//! session and one user row. Each mirrors the Go handler in
//! `backend/internal/web/router/system`.

use axum::Router;
use axum::extract::{FromRequest, Multipart, Path, Query, Request};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::{get, post};
use md5::{Digest, Md5};
use serde::Serialize;
use serde_json::json;

use crate::appcfg;
use crate::auth::{self, AuthError};
use crate::envelope::{self, error_code, success, success_data};

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

/// `AboutRouter.Get`.
async fn about() -> Response {
    success_data(json!({
        "buildTime": appcfg::build_time(),
        "commit": appcfg::build_commit(),
        "versionName": appcfg::version_name(),
    }))
}

/// `CapabilitiesRouter.Get`: Core registers no extension modules (ADR-002).
async fn capabilities() -> Response {
    success_data(json!({
        "capabilities": [],
        "edition": "core",
        "readOnly": false,
        "versionName": appcfg::version_name(),
    }))
}

/// `OAuthRouter.GetConfig`: `providers` is a nil slice in Go, which encodes as
/// `null`, not `[]`.
async fn oauth_config() -> Response {
    let conf = &appcfg::ctx().conf;
    let providers: Option<Vec<String>> = if conf.oauth.providers.is_empty() {
        None
    } else {
        Some(
            conf.oauth
                .providers
                .iter()
                .map(|provider| provider.name.clone())
                .collect(),
        )
    };
    let mut data = serde_json::Map::new();
    data.insert("enabled".into(), json!(conf.oauth.enable));
    data.insert("providers".into(), json!(providers));
    success_data(serde_json::Value::Object(data))
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

/// `LoginRouter.Login`.
async fn login(request: Request) -> Response {
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let params: LoginParams = match serde_json::from_slice(&body) {
        Ok(params) => params,
        Err(error) => return envelope::error(&format!("invalid request body: {error}")),
    };
    let mail = params.mail.trim().to_lowercase();
    // `base.ValidateInputStruct` over `LoginLoginVerify`: fields are reported in
    // struct order (Mail, then Password) and joined with a space inside
    // `"[ " + errors + "]"`.
    let mut problems: Vec<&str> = Vec::new();
    if mail.is_empty() {
        problems.push("Mail为必填字段");
    } else if !is_email(&mail) {
        problems.push("Mail必须是一个有效的邮箱");
    }
    if params.password.is_empty() {
        problems.push("Password为必填字段");
    }
    if !problems.is_empty() {
        return envelope::error(&format!(" [ {}]", problems.join(" ")));
    }

    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let user = match crate::repository::user_by_credentials(
        pool,
        ctx.sqlx_driver(),
        &mail,
        &password_encryption(&params.password),
    )
    .await
    {
        Ok(user) => user,
        Err(_) => return error_code(1003),
    };
    let Some(user) = user else {
        return error_code(1003);
    };
    if user.status != 1 {
        return error_code(1004);
    }
    let token = match auth::issue_token(user.id, user.token_version) {
        Ok(token) => token,
        Err(error) => return envelope::error(&error),
    };
    let max_age = ctx.jwt_expire_hours() * 3600;
    let cookie = auth::session_cookie(&token, max_age);
    let response = envelope::success_struct(UserInfo {
        id: user.id,
        name: user.name,
        role: user.role,
        mail: user.mail,
        token: token.clone(),
    });
    envelope::with_header(
        response,
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static("")),
    )
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

/// `UserRouter.GetUser`: the caller's own row, with an empty token field.
async fn get_user(headers: HeaderMap) -> Response {
    match auth::user(&headers).await {
        Ok(user) => envelope::success_struct(UserInfo {
            id: user.id,
            name: user.name,
            role: user.role,
            mail: user.mail,
            token: String::new(),
        }),
        Err(AuthError::NotLoggedIn) => error_code(1000),
        Err(AuthError::Disabled) => error_code(1004),
    }
}

/// `LoginRouter.Logout`: revoke every token for the account, then clear the
/// cookie.
async fn logout(headers: HeaderMap) -> Response {
    let user = match auth::user(&headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return error_code(1000),
        Err(AuthError::Disabled) => return error_code(1004),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    if crate::repository::revoke_user_sessions(pool, ctx.sqlx_driver(), user.id)
        .await
        .is_err()
    {
        return envelope::error("退出登录失败");
    }
    envelope::with_header(
        success(),
        header::SET_COOKIE,
        HeaderValue::from_str(&auth::cleared_cookie())
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    )
}

/// `SpaceRouter.List`: the caller's own Yin spaces, each with its paired Yang
/// id and the caller's edit right.
async fn list_spaces(headers: HeaderMap) -> Response {
    let user = match auth::user(&headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return error_code(1000),
        Err(AuthError::Disabled) => return error_code(1004),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let rows = match crate::repository::space_list(pool, ctx.sqlx_driver(), user.id).await {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let items = rows
        .into_iter()
        .map(|row| SpaceItem {
            paired_space_id: row.paired_space_id,
            can_edit: row.can_edit,
            id: row.id,
            create_time: rfc3339(&row.created_at),
            update_time: rfc3339(&row.updated_at),
            kind: row.kind,
            name: row.name,
            owner_user_id: row.owner_user_id,
            pair_id: row.pair_id,
            side: row.side,
            public_enabled: row.public_enabled,
            public_id: row.public_id,
            public_mode: row.public_mode,
        })
        .collect::<Vec<_>>();
    envelope::success_struct(items)
}

/// `SpaceRouter.GetSearchConfig`: `null` data is omitted from the envelope, so
/// an unset configuration answers `{"code":0,"msg":"OK"}`.
async fn search_config(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let user = match auth::user(&headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return error_code(1000),
        Err(AuthError::Disabled) => return error_code(1004),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let found =
        match crate::repository::space_search_config(pool, ctx.sqlx_driver(), user.id, space_id)
            .await
        {
            Ok(Some(found)) => found,
            Ok(None) | Err(_) => return error_code(404),
        };
    if found.owner_user_id != user.id && !found.is_member {
        return error_code(1005);
    }
    if found.config_json.is_empty() {
        return success();
    }
    match serde_json::from_str::<serde_json::Value>(&found.config_json) {
        Ok(value) => success_data(value),
        Err(error) => envelope::error(&error.to_string()),
    }
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

/// `canAccessSpace`: the owner or any member, for the SQLite write paths not
/// yet migrated. Read routes use `repository::can_access_space`.
async fn can_access(pool: &sqlx::SqlitePool, user_id: i64, space_id: i64) -> bool {
    let owner: Option<(i64,)> = sqlx::query_as(r#"SELECT owner_user_id FROM "space" WHERE id = ?"#)
        .bind(space_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    match owner {
        Some((owner_id,)) if owner_id == user_id => true,
        Some(_) => sqlx::query_as::<_, (i64,)>(
            r#"SELECT id FROM space_member WHERE space_id = ? AND user_id = ? LIMIT 1"#,
        )
        .bind(space_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        .is_some(),
        None => false,
    }
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

/// `SpaceRouter.Groups`: one grouped count for the whole Space.
async fn groups(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard_shared(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let rows =
        match crate::repository::groups_for_space(&pool, appcfg::ctx().sqlx_driver(), space_id)
            .await
        {
            Ok(rows) => rows,
            Err(error) => return envelope::error(&error.to_string()),
        };
    let items = rows
        .into_iter()
        .map(|row| GroupItem {
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
        .collect::<Vec<_>>();
    envelope::success_struct(items)
}

/// `SpaceRouter.Items`: `sort, created_at, id` order, optional group filter and
/// the 200-row page cap.
async fn items(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let pool = match guard_shared(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let group = query.get("groupId").cloned().unwrap_or_default();
    let group_id = if group.is_empty() {
        None
    } else {
        Some(group.parse::<i64>().unwrap_or(-1))
    };
    let page: i64 = query
        .get("page")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let page_size: i64 = query
        .get("pageSize")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let rows = match crate::repository::items_for_space(
        &pool,
        appcfg::ctx().sqlx_driver(),
        space_id,
        group_id,
        page,
        page_size,
    )
    .await
    {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let items: Vec<ItemOut> = rows
        .into_iter()
        .map(|row| {
            let icon = if row.icon_json.is_empty() {
                IconInfo::default()
            } else {
                serde_json::from_str(&row.icon_json).unwrap_or_default()
            };
            ItemOut {
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
            }
        })
        .collect();
    envelope::success_struct(items)
}

/// `SpaceRouter.Members`: `map[string]any` in Go, so keys come out sorted.
async fn members(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard_shared(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let rows = match crate::repository::space_members(&pool, appcfg::ctx().sqlx_driver(), space_id)
        .await
    {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(json!({
            "email": row.email,
            "id": row.id,
            "joinedAt": rfc3339(&row.joined_at),
            "name": row.name,
            "role": row.role,
            "source": row.source,
            "spaceId": row.space_id,
            "userId": row.user_id,
        }));
    }
    success_data(serde_json::Value::Array(out))
}

#[derive(sqlx::FromRow)]
struct ItemRow {
    id: i64,
    created_at: String,
    updated_at: String,
    icon_json: String,
    title: String,
    url: String,
    lan_url: String,
    mobile_url: String,
    description: String,
    open_method: i64,
    sort: i64,
    item_icon_group_id: i64,
    user_id: i64,
    space_id: i64,
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
async fn can_manage(pool: &sqlx::SqlitePool, user_id: i64, space_id: i64) -> bool {
    let owner: Option<(i64,)> = sqlx::query_as(r#"SELECT owner_user_id FROM "space" WHERE id = ?"#)
        .bind(space_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    match owner {
        Some((owner_id,)) if owner_id == user_id => true,
        Some(_) => sqlx::query_as::<_, (i64,)>(
            r#"SELECT id FROM space_member WHERE space_id = ? AND user_id = ? AND role = 'admin' LIMIT 1"#,
        )
        .bind(space_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        .is_some(),
        None => false,
    }
}

/// `SpaceRouter.GetPublicConfig`.
async fn space_public(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let user = match auth::user(&headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return error_code(1000),
        Err(AuthError::Disabled) => return error_code(1004),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let row = match crate::repository::space_public_config(&pool, ctx.sqlx_driver(), space_id).await
    {
        Ok(Some(row)) => row,
        Ok(None) | Err(_) => return error_code(404),
    };
    success_data(json!({
        "enabled": row.enabled,
        "mode": row.mode,
        "publicId": row.public_id,
    }))
}

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

async fn set_space_public(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match auth::user(&headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return error_code(1000),
        Err(AuthError::Disabled) => return error_code(1004),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error(" [invalid public config]"),
    };
    let req: PublicConfigRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid public config]"),
    };
    let valid_public_id = !req.public_id.is_empty()
        && req.public_id.len() <= 64
        && req
            .public_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    if req.enabled && !valid_public_id {
        return envelope::error(" [invalid publicId]");
    }
    if req.mode != "direct" && req.mode != "code" {
        return envelope::error(" [invalid public mode]");
    }
    if req.mode == "code" && req.enabled && !(4..=12).contains(&req.access_code.len()) {
        return envelope::error(" [invalid access code]");
    }
    let public_id = if req.enabled {
        Some(req.public_id.as_str())
    } else {
        None
    };
    let code_hash = if !req.enabled {
        Some(String::new())
    } else if req.mode == "code" && !req.access_code.is_empty() {
        Some(hex_digest(req.access_code.as_bytes(), true))
    } else {
        None
    };
    match crate::repository::update_space_public_config(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        req.enabled,
        public_id,
        &req.mode,
        code_hash.as_deref(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&format!("save public config: {error}")),
    }
}

/// `ModuleConfigRouter.GetByName`: a missing row answers `"data": null`, which
/// Go produces by handing a nil map to a non-nil interface.
async fn module_config(
    headers: HeaderMap,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let user = match auth::user(&headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return error_code(1000),
        Err(AuthError::Disabled) => return error_code(1004),
    };
    let name = query.get("name").cloned().unwrap_or_default();
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row = crate::repository::module_config_value(pool, ctx.sqlx_driver(), user.id, &name)
        .await
        .unwrap_or(None);
    match row {
        None => (
            axum::http::StatusCode::OK,
            envelope::headers(),
            envelope::body(0, "OK", Some(serde_json::Value::Null)),
        )
            .into_response(),
        Some(value_json) => {
            let value: serde_json::Value =
                serde_json::from_str(&value_json).unwrap_or(serde_json::Value::Null);
            (
                axum::http::StatusCode::OK,
                envelope::headers(),
                envelope::body(0, "OK", Some(value)),
            )
                .into_response()
        }
    }
}

/// `FileRouter.GetList`, behind `AdminInterceptor`: optional `limit`/`page`,
/// newest first, and `list` is `null` when there are no rows.
async fn file_list(
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

/// `SpaceRouter.BookmarkTree`: groups ordered `sort, id`, items ordered
/// `sort, id`, folded into a tree capped at `bookmarkTreeMaxDepth`.
async fn bookmarks_tree(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard_shared(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let (groups, items) = match crate::repository::bookmark_tree(
        &pool,
        appcfg::ctx().sqlx_driver(),
        space_id,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let groups: Vec<(i64, String, String, Option<i64>)> = groups
        .into_iter()
        .map(|group| (group.id, group.title, group.icon, group.parent_id))
        .collect();
    let items: Vec<(i64, String, String)> = items
        .into_iter()
        .map(|item| (item.group_id, item.title, item.url))
        .collect();
    let known: std::collections::HashSet<i64> = groups.iter().map(|group| group.0).collect();
    let mut children: std::collections::HashMap<i64, Vec<&(i64, String, String, Option<i64>)>> =
        std::collections::HashMap::new();
    for group in &groups {
        let parent = match group.3 {
            Some(parent) if known.contains(&parent) => parent,
            _ => 0,
        };
        children.entry(parent).or_default().push(group);
    }
    let groups_json = build_tree(&children, &items, 0, 0);
    success_data(json!({ "groups": groups_json }))
}

const BOOKMARK_TREE_MAX_DEPTH: i64 = 64;

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

/// `UsersRouter.GetList`: `LIMIT 0` for an absent `limit`, so an unpaged call
/// answers an empty list (not null) with the real total.
async fn users_list(
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
        .unwrap_or(0);
    let (total, rows) =
        match crate::repository::user_list(pool, ctx.sqlx_driver(), limit, page).await {
            Ok(result) => result,
            Err(_) => (0, Vec::new()),
        };
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "createTime": rfc3339(&row.created_at),
                "updateTime": rfc3339(&row.updated_at),
                "password": "",
                "name": row.name,
                "headImage": row.head_image,
                "status": row.status,
                "role": row.role,
                "mail": row.mail,
                "token": "",
            })
        })
        .collect();
    success_data(json!({"count": total, "list": list}))
}

/// `UserConfigRouter.GetConfig`: a missing row is `404 Data not found`.
async fn user_config(headers: HeaderMap) -> Response {
    let user = match auth::user(&headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return error_code(1000),
        Err(AuthError::Disabled) => return error_code(1004),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row = crate::repository::user_config(pool, ctx.sqlx_driver(), user.id)
        .await
        .unwrap_or(None);
    let Some(config) = row else {
        return error_code(404);
    };
    let user_id = config.user_id;
    let panel_json = config.panel_json;
    let language = config.language;
    let mut panel: serde_json::Value = if panel_json.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(&panel_json).unwrap_or(serde_json::Value::Null)
    };
    if panel.is_null() {
        panel = json!({});
    }
    if let Some(fields) = panel.as_object_mut() {
        fields
            .entry("maxWidthUnit".to_string())
            .or_insert_with(|| json!(""));
        fields
            .entry("footerHtml".to_string())
            .or_insert_with(|| json!(""));
    }
    success_data(json!({"userId": user_id, "panel": panel, "language": language}))
}

/// `MonitorRouter.GetEnableStatus`.
async fn monitor_enable_status(headers: HeaderMap) -> Response {
    if let Err(response) = require_user(&headers).await {
        return response;
    }
    let conf = &appcfg::ctx().conf;
    let refresh = if conf.base.monitor_refresh_interval > 0 {
        conf.base.monitor_refresh_interval
    } else {
        10
    };
    success_data(json!({
        "enabled": conf.base.enable_monitor,
        "refresh_interval": refresh,
    }))
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

/// `MonitorRouter.GetSnapshot`.
async fn monitor_snapshot(headers: HeaderMap) -> Response {
    if let Err(response) = monitor_guard(&headers).await {
        return response;
    }
    let snapshot = crate::monitor::snapshot().await;
    if snapshot.is_empty() {
        return envelope::error("failed");
    }
    success_data(serde_json::Value::Object(snapshot.into_iter().collect()))
}

/// `MonitorRouter.GetDiskMountpoints`.
async fn monitor_disk_mountpoints(headers: HeaderMap) -> Response {
    if let Err(response) = monitor_guard(&headers).await {
        return response;
    }
    match crate::monitor::disk_mountpoints() {
        Ok(mountpoints) => success_data(json!(mountpoints)),
        Err(error) => envelope::error(&error),
    }
}

/// `MonitorRouter.GetDiskStateByPath`.
async fn monitor_disk_state(request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if let Err(response) = monitor_guard(&parts.headers).await {
        return response;
    }
    let bytes = match axum::body::to_bytes(body, 64 * 1024).await {
        Ok(bytes) => bytes,
        Err(error) => return envelope::error(&format!(" [invalid request: {error}]")),
    };
    let payload: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(payload) => payload,
        Err(_) => return error_code(1002),
    };
    let Some(path) = payload.get("path").and_then(serde_json::Value::as_str) else {
        return error_code(1002);
    };
    match crate::monitor::disk_state(path) {
        Ok(state) => success_data(json!(state)),
        Err(_) => envelope::error("failed"),
    }
}

/// `ThemeRouter.Preference`: the stored mode, or `auto` / `follow-space`.
async fn theme_preference(headers: HeaderMap) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row = crate::repository::user_theme_preference(pool, ctx.sqlx_driver(), user.id)
        .await
        .unwrap_or(None);
    let (mode, theme_mode) =
        row.unwrap_or_else(|| ("auto".to_string(), "follow-space".to_string()));
    let mode = if mode.is_empty() {
        "auto".to_string()
    } else {
        mode
    };
    let theme_mode = if theme_mode.is_empty() {
        "follow-space".to_string()
    } else {
        theme_mode
    };
    success_data(json!({"mode": mode, "themeMode": theme_mode}))
}

/// `ThemeRouter.SpaceThemeV2`: the Space default and the caller's override.
async fn theme_space(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    if let Err(response) = require_user(&headers).await {
        return response;
    }
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let space_theme: Option<(String, String)> = sqlx::query_as(
        "SELECT package_id, revision_id FROM space_theme_preference_v2 WHERE space_id = ?",
    )
    .bind(space_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    let override_theme: Option<(String, String)> = sqlx::query_as(
        "SELECT package_id, revision_id FROM user_space_theme_preference_v2 WHERE user_id = ? AND space_id = ?",
    )
    .bind(user.id)
    .bind(space_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    let theme = space_theme
        .map(|(package_id, revision)| json!({"packageId": package_id, "revision": revision}));
    let override_value = override_theme
        .map(|(package_id, revision)| json!({"packageId": package_id, "revision": revision}));
    success_data(json!({
        "override": override_value,
        "spaceId": space_id,
        "theme": theme,
    }))
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
    pool: &sqlx::SqlitePool,
    revision_id: &str,
) -> Result<serde_json::Value, String> {
    let row: Option<(String, String, i64)> = sqlx::query_as(
        "SELECT manifest_json, tokens_json, verified FROM revision_record_v2 WHERE id = ?",
    )
    .bind(revision_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| error.to_string())?;
    let Some((manifest_json, tokens_json, verified)) = row else {
        return Err("revision not found".to_string());
    };
    revision_public_from_parts(revision_id, &manifest_json, &tokens_json, verified)
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

async fn latest_revision_of(pool: &sqlx::SqlitePool, package_id: &str) -> Result<String, String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT latest_revision FROM package_record_v2 WHERE id = ? AND removed = 0",
    )
    .bind(package_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| error.to_string())?;
    row.map(|value| value.0)
        .ok_or_else(|| "package not found".to_string())
}

async fn instance_default_revision(pool: &sqlx::SqlitePool) -> Result<String, String> {
    latest_revision_of(pool, "org.yin.default").await
}

async fn instance_default_package(pool: &sqlx::SqlitePool) -> Result<serde_json::Value, String> {
    let row: Option<(String, String, String, i64)> = sqlx::query_as(
        r#"SELECT package_record_v2.latest_revision,
                  revision_record_v2.manifest_json,
                  revision_record_v2.tokens_json,
                  revision_record_v2.verified
           FROM package_record_v2
           JOIN revision_record_v2
             ON revision_record_v2.id = package_record_v2.latest_revision
           WHERE package_record_v2.id = 'org.yin.default'
             AND package_record_v2.removed = 0"#,
    )
    .fetch_optional(pool)
    .await
    .map_err(|error| error.to_string())?;
    let Some((revision, manifest, tokens, verified)) = row else {
        return Err("package not found".to_string());
    };
    revision_public_from_parts(&revision, &manifest, &tokens, verified)
}

async fn activation_revision(pool: &sqlx::SqlitePool, scope: &str) -> Option<String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT active_revision_id FROM activation_record_v2 WHERE scope = ?")
            .bind(scope)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    row.map(|value| value.0).filter(|value| !value.is_empty())
}

/// `ThemeRouter.CurrentV2`: the instance default package.
async fn theme_current() -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    match instance_default_package(pool).await {
        Ok(value) => success_data(value),
        Err(error) => envelope::error(&error),
    }
}

/// `ThemeRouter.PublicListV2`.
async fn theme_packages() -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let packages: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, name, latest_revision FROM package_record_v2 WHERE removed = 0 ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let mut out = Vec::with_capacity(packages.len());
    for (id, name, revision_id) in packages {
        let revision: Option<(String, i64, String)> = sqlx::query_as(
            "SELECT version, verified, created_at FROM revision_record_v2 WHERE id = ?",
        )
        .bind(&revision_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
        let (version, verified, created_at) = revision.unwrap_or_default();
        out.push(json!({
            "id": id,
            "name": name,
            "version": version,
            "revision": revision_id,
            "verified": verified != 0,
            "createdAt": rfc3339(&created_at),
        }));
    }
    success_data(serde_json::Value::Array(out))
}

/// `ThemeRouter.PackageByIDV2`.
async fn theme_package(Path(package_id): Path<String>) -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let revision = match latest_revision_of(pool, &package_id).await {
        Ok(revision) => revision,
        Err(_) => return envelope::empty_not_found(),
    };
    match revision_public(pool, &revision).await {
        Ok(value) => success_data(value),
        Err(_) => envelope::empty_not_found(),
    }
}

/// `ThemeRouter.Mine`: the revision the caller runs and their stored choice.
async fn theme_mine(headers: HeaderMap) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let instance = match instance_default_revision(pool).await {
        Ok(revision) => revision,
        Err(error) => return envelope::error(&error),
    };
    let system = activation_revision(pool, "instance")
        .await
        .unwrap_or(instance);
    let scope = format!("user:{}", user.id);
    let selected = activation_revision(pool, &scope).await;
    let revision = selected.clone().unwrap_or(system);
    let package = match revision_public(pool, &revision).await {
        Ok(value) => value,
        Err(error) => return envelope::error(&error),
    };
    let (mode, theme_mode) = user_theme_modes(pool, user.id).await;
    let package_id = sqlx::query_as::<_, (String,)>(
        "SELECT package_id FROM activation_record_v2 WHERE scope = ?",
    )
    .bind(&scope)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)
    .map(|value| value.0)
    .unwrap_or_default();
    success_data(json!({
        "package": package,
        "preference": {
            "mode": mode,
            "packageId": package_id,
            "revision": revision,
            "themeMode": theme_mode,
        },
    }))
}

async fn user_theme_modes(pool: &sqlx::SqlitePool, user_id: i64) -> (String, String) {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT mode, theme_mode FROM user_theme_preference_v2 WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
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

/// `ResolveThemeV2` and `ThemeRouter.EffectiveV2`.
async fn theme_effective(
    headers: HeaderMap,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let space_id: Option<i64> = query
        .get("spaceId")
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0);
    if query.get("spaceId").is_some() && space_id.is_none() {
        return envelope::error(" [spaceId must be a positive integer]");
    }
    let user = auth::user(&headers).await.ok();
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let instance = match instance_default_revision(pool).await {
        Ok(revision) => revision,
        Err(error) => return envelope::error(&error),
    };
    let system = activation_revision(pool, "instance")
        .await
        .unwrap_or(instance);

    let mut resolution: Option<(String, &str)> = None;
    if let (Some(user), Some(space)) = (&user, space_id) {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT revision_id FROM user_space_theme_preference_v2 WHERE user_id = ? AND space_id = ?",
        )
        .bind(user.id)
        .bind(space)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
        if let Some((revision,)) = row {
            resolution = Some((revision, "user-space"));
        }
    }
    if resolution.is_none() {
        if let Some(user) = &user {
            let (_, choice) = user_theme_modes(pool, user.id).await;
            if choice == "custom" {
                let scope = format!("user:{}", user.id);
                if let Some(revision) = activation_revision(pool, &scope).await {
                    resolution = Some((revision, "user"));
                }
            }
        }
    }
    if resolution.is_none() {
        if let Some(space) = space_id {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT revision_id FROM space_theme_preference_v2 WHERE space_id = ?",
            )
            .bind(space)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
            if let Some((revision,)) = row {
                resolution = Some((revision, "space"));
            }
        }
    }
    let (revision, source) = resolution.unwrap_or((system, "system"));
    let package = match revision_public(pool, &revision).await {
        Ok(value) => value,
        Err(error) => return envelope::error(&error),
    };
    let mode = match &user {
        Some(user) => user_theme_modes(pool, user.id).await.0,
        None => "auto".to_string(),
    };
    success_data(json!({
        "mode": mode,
        "package": package,
        "revision": revision,
        "source": source,
    }))
}

/// `ThemeRouter.AssetV2` plus `serveThemeAsset`: the stored bytes, the media
/// type, an ETag for HTML, and `http.ServeContent`'s conditional and range
/// behaviour.
async fn theme_asset(
    headers: HeaderMap,
    Path((revision, raw_name)): Path<(String, String)>,
) -> Response {
    let name = raw_name.trim_start_matches('/');
    if name.is_empty()
        || name.contains('\\')
        || name.starts_with("../")
        || name.split('/').any(|part| part == "." || part == "..")
        || name.contains("//")
    {
        return envelope::empty_not_found();
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row: Option<(String, Vec<u8>)> = sqlx::query_as(
        "SELECT media_type, content FROM asset_record_v2 WHERE revision_id = ? AND path = ?",
    )
    .bind(&revision)
    .bind(name)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    let Some((media_type, content)) = row else {
        return envelope::empty_not_found();
    };

    let is_html = media_type == "text/html";
    let etag = if is_html {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(&content);
        Some(format!(
            "\"{}\"",
            digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ))
    } else {
        None
    };

    let mut response = Response::new(axum::body::Body::empty());
    {
        let out = response.headers_mut();
        for (key, value) in envelope::headers() {
            out.insert(key, value);
        }
        out.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&media_type)
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
        if let Some(etag) = &etag {
            out.insert(
                header::ETAG,
                HeaderValue::from_str(etag).unwrap_or(HeaderValue::from_static("")),
            );
        }
        if is_html {
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

    // `http.ServeContent`: conditional requests and byte ranges.
    let (parts, _) = response.into_parts();
    crate::content::serve(&headers, parts, &content)
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

/// `ModuleConfigRouter.Save`: Go binds `Value` into `map[string]interface{}`, so
/// a non-object value is rejected with the decoder's own message.
async fn module_config_save(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(error) => return envelope::error(&format!(" [{error}]")),
    };
    let name = payload
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let value = payload
        .get("value")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    if !value.is_object() && !value.is_null() {
        let kind = match &value {
            serde_json::Value::String(_) => "string",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::Bool(_) => "bool",
            serde_json::Value::Array(_) => "array",
            _ => "value",
        };
        return envelope::error(&format!(
            " [json: cannot unmarshal {kind} into Go struct field ModuleConfig.value of type map[string]interface {{}}]"
        ));
    }
    let stored = if value.is_null() {
        "null".to_string()
    } else {
        value.to_string()
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    match crate::repository::save_module_config(
        pool,
        ctx.sqlx_driver(),
        user.id,
        &name,
        &stored,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `UserConfigRouter.SetConfig`: only a request that carries a panel rewrites
/// the stored one, as in `SaveUserConfig`.
async fn user_config_save(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    let language = payload
        .get("language")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    if !language.is_empty() && !supported_language(&language) {
        return envelope::error(" [invalid language]");
    }
    let panel = payload
        .get("panel")
        .cloned()
        .filter(|value| value.is_object());
    if let Some(panel) = &panel {
        let mode = panel
            .get("wallpaperMode")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if !mode.is_empty() && !matches!(mode, "theme" | "custom" | "library" | "none") {
            return envelope::error(" [invalid wallpaper mode]");
        }
        if mode == "library" {
            let entry = panel
                .get("wallpaperLibraryEntry")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let known = wallpaperpack_rs::port::find(entry.to_string())
                .get(1)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if !known && !theme_wallpaper_entry(entry) {
                return envelope::error(" [invalid wallpaper library entry]");
            }
        }
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let panel_json = panel.as_ref().map(|value| value.to_string());
    match crate::repository::save_user_config(
        pool,
        ctx.sqlx_driver(),
        user.id,
        panel_json.as_deref(),
        &language,
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
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

/// `spaceRole`: `admin` for the owner, otherwise the member row's role.
async fn space_role(pool: &sqlx::SqlitePool, user_id: i64, space_id: i64) -> String {
    let owner: Option<(i64,)> = sqlx::query_as(r#"SELECT owner_user_id FROM "space" WHERE id = ?"#)
        .bind(space_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    match owner {
        Some((owner_id,)) if owner_id == user_id => "admin".to_string(),
        Some(_) => sqlx::query_as::<_, (String,)>(
            "SELECT role FROM space_member WHERE space_id = ? AND user_id = ? LIMIT 1",
        )
        .bind(space_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        .map(|value| value.0)
        .unwrap_or_default(),
        None => String::new(),
    }
}

async fn can_edit(pool: &sqlx::SqlitePool, user_id: i64, space_id: i64) -> bool {
    matches!(
        space_role(pool, user_id, space_id).await.as_str(),
        "admin" | "editor"
    )
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

/// `SpaceRouter.CreateGroup`.
async fn create_group(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: GroupRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{error}]")),
    };
    if req.title.is_empty() {
        return envelope::error(" [ Title为必填字段]");
    }
    if req.title.chars().count() > 50 {
        return envelope::error(" [ Title长度不能超过50个字符]");
    }
    if let Some(parent) = req.parent_id {
        if parent == 0 {
            return envelope::error(" [invalid parent group]");
        }
        match crate::repository::group_belongs(&pool, ctx.sqlx_driver(), space_id, parent).await {
            Ok(true) => {}
            _ => return envelope::error(" [invalid parent group]"),
        }
    }
    let now = now_like_go();
    let id = match crate::repository::create_group(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        user.id,
        &req.icon,
        &req.title,
        req.parent_id,
        &now,
    )
    .await
    {
        Ok(id) => id,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let group = match crate::repository::group_by_id(&pool, ctx.sqlx_driver(), space_id, id).await {
        Ok(Some(row)) => group_item_json(row),
        _ => return envelope::error("group not found"),
    };
    success_data(group)
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

async fn group_belongs(pool: &sqlx::SqlitePool, group_id: i64, space_id: i64) -> bool {
    sqlx::query_as::<_, (i64,)>("SELECT id FROM item_icon_group WHERE id = ? AND space_id = ?")
        .bind(group_id)
        .bind(space_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        .is_some()
}

/// `SpaceRouter.UpdateGroup`.
async fn update_group(
    headers: HeaderMap,
    Path((space_id, group_id)): Path<(i64, String)>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let group_id: i64 = match group_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [invalid groupId]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: GroupRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{error}]")),
    };
    if req.title.is_empty() {
        return envelope::error(" [ Title为必填字段]");
    }
    if let Some(parent) = req.parent_id {
        let belongs = crate::repository::group_belongs(&pool, ctx.sqlx_driver(), space_id, parent)
            .await
            .unwrap_or(false);
        if parent == group_id || !belongs {
            return envelope::error(" [invalid parent group]");
        }
        if crate::repository::group_has_descendant(
            &pool,
            ctx.sqlx_driver(),
            space_id,
            group_id,
            parent,
        )
        .await
        .unwrap_or(false)
        {
            return envelope::error(" [group cycle detected]");
        }
    }
    let now = now_like_go();
    match crate::repository::update_group(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        group_id,
        &req.title,
        &req.icon,
        req.parent_id,
        &now,
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.DeleteGroup`: the group, its descendants and their items.
async fn delete_group(
    headers: HeaderMap,
    Path((space_id, group_id)): Path<(i64, String)>,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let group_id: i64 = match group_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [invalid groupId]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    match crate::repository::delete_group_tree(&pool, ctx.sqlx_driver(), space_id, group_id).await {
        Ok(()) => success(),
        Err(_) => envelope::error("delete failed"),
    }
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

/// `SpaceRouter.CreateItem`.
async fn create_item(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let mut input: ItemIn = match serde_json::from_slice(&body) {
        Ok(input) => input,
        Err(_) => return envelope::error(" [invalid item or group]"),
    };
    if input.item_icon_group_id == 0 {
        return envelope::error(" [invalid item or group]");
    }
    if !valid_item_title(&input.title) {
        return envelope::error(" [title must be at most 20 characters]");
    }
    ensure_item_icon(&input.title, &mut input.icon);
    let group_exists = crate::repository::group_belongs(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        input.item_icon_group_id,
    )
    .await
    .unwrap_or(false);
    if !group_exists {
        return error_code(404);
    }
    let now = now_like_go();
    let icon_json = serde_json::to_string(&input.icon).unwrap_or_else(|_| "{}".to_string());
    let record = crate::repository::ItemWriteRecord {
        icon_json,
        title: input.title,
        url: input.url,
        lan_url: input.lan_url,
        mobile_url: input.mobile_url,
        description: input.description,
        open_method: input.open_method,
        sort: input.sort,
        group_id: input.item_icon_group_id,
    };
    let id = match crate::repository::create_item(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        user.id,
        &record,
        &now,
    )
    .await
    {
        Ok(id) => id,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let row = match crate::repository::item_by_id(&pool, ctx.sqlx_driver(), space_id, id).await {
        Ok(Some(row)) => row,
        _ => return error_code(404),
    };
    success_data(item_json(row))
}

fn item_json(row: crate::repository::ItemListRecord) -> serde_json::Value {
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

async fn load_item(pool: &sqlx::SqlitePool, space_id: i64, id: i64) -> Option<serde_json::Value> {
    let row = sqlx::query_as::<_, ItemRow>(
        r#"SELECT id, created_at, updated_at, icon_json, title, url, lan_url, mobile_url,
                  description, open_method, sort, item_icon_group_id, user_id, space_id
           FROM item_icon WHERE id = ? AND space_id = ?"#,
    )
    .bind(id)
    .bind(space_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)?;
    let icon: IconInfo = serde_json::from_str(&row.icon_json).unwrap_or_default();
    Some(
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
        .unwrap_or(serde_json::Value::Null),
    )
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

/// `SpaceRouter.UpdateItem`.
async fn update_item(
    headers: HeaderMap,
    Path((space_id, item_id)): Path<(i64, String)>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let item_id: i64 = match item_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [invalid itemId]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let mut input: ItemIn = match serde_json::from_slice(&body) {
        Ok(input) => input,
        Err(error) => return envelope::error(&format!(" [{error}]")),
    };
    if input.item_icon_group_id == 0 {
        return envelope::error(" [invalid groupId]");
    }
    if !valid_item_title(&input.title) {
        return envelope::error(" [title must be at most 20 characters]");
    }
    ensure_item_icon(&input.title, &mut input.icon);
    let group_exists = crate::repository::group_belongs(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        input.item_icon_group_id,
    )
    .await
    .unwrap_or(false);
    if !group_exists {
        return error_code(404);
    }
    let icon_json = serde_json::to_string(&input.icon).unwrap_or_else(|_| "{}".to_string());
    let record = crate::repository::ItemWriteRecord {
        icon_json,
        title: input.title,
        url: input.url,
        lan_url: input.lan_url,
        mobile_url: input.mobile_url,
        description: input.description,
        open_method: input.open_method,
        sort: input.sort,
        group_id: input.item_icon_group_id,
    };
    match crate::repository::update_item(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        item_id,
        &record,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.DeleteItem`.
async fn delete_item(
    headers: HeaderMap,
    Path((space_id, item_id)): Path<(i64, String)>,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let item_id: i64 = match item_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [invalid itemId]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    match crate::repository::delete_item(&pool, ctx.sqlx_driver(), space_id, item_id).await {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `pairedIDs`: the Space itself plus its Yang twin, resolved through pair_id.
async fn paired_ids(pool: &sqlx::SqlitePool, space_id: i64) -> Vec<i64> {
    let row: Option<(i64,)> = sqlx::query_as(r#"SELECT pair_id FROM "space" WHERE id = ?"#)
        .bind(space_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    let pair_id = row.map(|value| value.0).unwrap_or(0);
    if pair_id == 0 {
        return vec![space_id];
    }
    let yang: Option<(i64,)> =
        sqlx::query_as(r#"SELECT id FROM "space" WHERE pair_id = ? AND side = 'yang' LIMIT 1"#)
            .bind(pair_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    match yang {
        Some((id,)) => vec![pair_id, id],
        None => vec![pair_id],
    }
}

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

/// `SpaceRouter.AddMember`: the member row exists in both paired Spaces.
async fn add_member(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: MemberAddRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid member request]"),
    };
    if !valid_space_role(&req.role) {
        return envelope::error(" [invalid member request]");
    }
    let email = req.email.trim().to_lowercase();
    let target = match crate::repository::user_id_by_mail(&pool, ctx.sqlx_driver(), &email).await {
        Ok(Some(target_id)) => target_id,
        Ok(None) | Err(_) => return error_code(404),
    };
    if crate::repository::space_owner(&pool, ctx.sqlx_driver(), space_id)
        .await
        .unwrap_or(None)
        .is_none()
    {
        return error_code(404);
    }
    let space_ids = match crate::repository::paired_space_ids(&pool, ctx.sqlx_driver(), space_id)
        .await
    {
        Ok(ids) => ids,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let now = now_like_go();
    match crate::repository::upsert_space_member(
        &pool,
        ctx.sqlx_driver(),
        &space_ids,
        target,
        &req.role,
        &now,
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.UpdateMember`.
async fn update_member(
    headers: HeaderMap,
    Path((space_id, user_id)): Path<(i64, String)>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let target: i64 = match user_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [invalid userId]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: MemberRoleRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid role]"),
    };
    if !valid_space_role(&req.role) {
        return envelope::error(" [invalid role]");
    }
    if crate::repository::space_owner(&pool, ctx.sqlx_driver(), space_id)
        .await
        .unwrap_or(None)
        .is_none()
    {
        return error_code(404);
    }
    let space_ids = match crate::repository::paired_space_ids(&pool, ctx.sqlx_driver(), space_id)
        .await
    {
        Ok(ids) => ids,
        Err(error) => return envelope::error(&error.to_string()),
    };
    match crate::repository::set_member_role(
        &pool,
        ctx.sqlx_driver(),
        &space_ids,
        target,
        &req.role,
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.RemoveMember`: the owner cannot be removed.
async fn remove_member(
    headers: HeaderMap,
    Path((space_id, user_id)): Path<(i64, String)>,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let target: i64 = match user_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [invalid userId]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let owner = match crate::repository::space_owner(&pool, ctx.sqlx_driver(), space_id).await {
        Ok(Some(owner)) => owner,
        Ok(None) | Err(_) => return error_code(404),
    };
    if target == owner {
        return envelope::error("the space owner cannot be removed; leave the space to delete it");
    }
    let space_ids = match crate::repository::paired_space_ids(&pool, ctx.sqlx_driver(), space_id)
        .await
    {
        Ok(ids) => ids,
        Err(error) => return envelope::error(&error.to_string()),
    };
    match crate::repository::remove_space_member(
        &pool,
        ctx.sqlx_driver(),
        &space_ids,
        target,
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

#[derive(serde::Deserialize)]
struct RenameRequest {
    #[serde(default)]
    name: String,
}

/// `SpaceRouter.Rename`: the Yang twin becomes `<name>-B`.
async fn rename_space(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: RenameRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    if req.name.is_empty() {
        return envelope::error(" [ Name为必填字段]");
    }
    if req.name.chars().count() > 100 {
        return envelope::error(" [ Name长度不能超过100个字符]");
    }
    if crate::repository::space_owner(&pool, ctx.sqlx_driver(), space_id)
        .await
        .unwrap_or(None)
        .is_none()
    {
        return error_code(404);
    }
    match crate::repository::rename_paired_space(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        &req.name,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.SetSearchConfig`.
async fn set_search_config(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let value: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return envelope::error(" [invalid search config]"),
    };
    let url = value
        .get("currentSearchEngine")
        .and_then(|engine| engine.get("url"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if url.is_empty() {
        return envelope::error(" [invalid search config]");
    }
    match crate::repository::update_space_search_config(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        &value.to_string(),
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.GetWallpaperConfig`.
async fn get_wallpaper(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard_shared(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let stored =
        match crate::repository::space_wallpaper_json(&pool, appcfg::ctx().sqlx_driver(), space_id)
            .await
        {
            Ok(Some(stored)) => stored,
            Ok(None) | Err(_) => return error_code(404),
        };
    if stored.is_empty() {
        return success();
    }
    match serde_json::from_str::<SpaceWallpaper>(&stored) {
        Ok(value) => match serde_json::to_value(value) {
            Ok(value) => success_data(value),
            Err(error) => envelope::error(&error.to_string()),
        },
        Err(error) => envelope::error(&error.to_string()),
    }
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

/// `SpaceRouter.SetWallpaperConfig`: `none` stores an empty column.
async fn set_wallpaper(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let value: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return envelope::error(" [invalid wallpaper config]"),
    };
    let mode = value
        .get("mode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if !valid_wallpaper_mode(mode) {
        return envelope::error(" [invalid wallpaper config]");
    }
    let stored = if mode == "none" {
        String::new()
    } else {
        value.to_string()
    };
    match crate::repository::update_space_wallpaper(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        &stored,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.OIDCGroups` (the list the routes expose alongside add/delete).
async fn list_oidc_groups(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard_shared(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let rows =
        match crate::repository::oidc_groups(&pool, appcfg::ctx().sqlx_driver(), space_id).await {
            Ok(rows) => rows,
            Err(error) => return envelope::error(&error.to_string()),
        };
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "createTime": rfc3339(&row.created_at),
                "updateTime": rfc3339(&row.updated_at),
                "spaceId": space_id,
                "provider": row.provider,
                "groupName": row.group_name,
                "role": row.role,
            })
        })
        .collect();
    success_data(serde_json::Value::Array(list))
}

/// `SpaceRouter.AddOIDCGroup`: a rule may grant editor or viewer, never admin.
async fn add_oidc_group(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !can_manage(&pool, user.id, space_id).await {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: OidcGroupRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid OIDC group rule]"),
    };
    if req.provider.is_empty()
        || req.group_name.is_empty()
        || !valid_space_role(&req.role)
        || req.role == "admin"
    {
        return envelope::error(" [invalid OIDC group rule]");
    }
    let now = now_like_go();
    let result = sqlx::query(
        "INSERT INTO space_o_id_c_group (created_at, updated_at, space_id, provider, group_name, role) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(space_id)
    .bind(&req.provider)
    .bind(&req.group_name)
    .bind(&req.role)
    .execute(&pool)
    .await;
    let id = match result {
        Ok(result) => result.last_insert_rowid(),
        Err(error) => return envelope::error(&error.to_string()),
    };
    success_data(json!({
        "id": id,
        "createTime": rfc3339(&now),
        "updateTime": rfc3339(&now),
        "spaceId": space_id,
        "provider": req.provider,
        "groupName": req.group_name,
        "role": req.role,
    }))
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

/// `SpaceRouter.DeleteOIDCGroup`.
async fn delete_oidc_group(
    headers: HeaderMap,
    Path((space_id, rule_id)): Path<(i64, String)>,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let rule_id: i64 = match rule_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [invalid ruleId]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !can_manage(&pool, user.id, space_id).await {
        return error_code(1005);
    }
    match sqlx::query("DELETE FROM space_o_id_c_group WHERE id = ? AND space_id = ?")
        .bind(rule_id)
        .bind(space_id)
        .execute(&pool)
        .await
    {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

#[derive(serde::Deserialize)]
struct TransferRequest {
    #[serde(default)]
    email: String,
}

/// `SpaceRouter.Transfer`: the new owner is made admin, the old one editor.
async fn transfer_space(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: TransferRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid userId]"),
    };
    let email = req.email.trim().to_lowercase();
    let target_id = match crate::repository::user_id_by_mail(&pool, ctx.sqlx_driver(), &email).await
    {
        Ok(Some(id)) => id,
        Ok(None) | Err(_) => return error_code(404),
    };
    match crate::repository::transfer_space(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        user.id,
        target_id,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(_) => envelope::error("transfer failed"),
    }
}

/// `SpaceRouter.ClearSpace`: items and groups, in one transaction.
async fn clear_space(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    match crate::repository::clear_space(&pool, ctx.sqlx_driver(), space_id).await {
        Ok(()) => success(),
        Err(_) => envelope::error("clear failed"),
    }
}

/// `SpaceRouter.CreateSpace`: the Yin Space, its Yang twin, both admin rows and
/// the default `APP` group in each.
async fn create_space(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: RenameRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    if req.name.is_empty() {
        return envelope::error(" [ Name为必填字段]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    match crate::repository::create_paired_space(
        &pool,
        ctx.sqlx_driver(),
        user.id,
        &req.name,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
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

/// `SpaceRouter.SortItems`.
async fn sort_items(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: ItemSortRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid sort request]"),
    };
    if req.group_id == 0 {
        return envelope::error(" [invalid sort request]");
    }
    let entries: Vec<(i64, i64)> = req
        .sort_items
        .iter()
        .map(|entry| (entry.id, entry.sort))
        .collect();
    match crate::repository::sort_items(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        req.group_id,
        &entries,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(_) => envelope::error("sort failed"),
    }
}

#[derive(serde::Deserialize)]
struct GroupSortRequest {
    #[serde(default, rename = "parentId")]
    parent_id: Option<i64>,
    #[serde(default, rename = "sortGroups")]
    sort_groups: Vec<SortEntry>,
}

/// `SpaceRouter.SortGroups`: every sibling must appear exactly once with a
/// distinct, positive sort value.
async fn sort_groups(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: GroupSortRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid group sort request]"),
    };
    if req.sort_groups.is_empty() {
        return envelope::error(" [invalid group sort request]");
    }
    if let Some(parent) = req.parent_id {
        let belongs = crate::repository::group_belongs(&pool, ctx.sqlx_driver(), space_id, parent)
            .await
            .unwrap_or(false);
        if !belongs {
            return envelope::error(" [invalid parent group]");
        }
    }
    let siblings =
        match crate::repository::group_siblings(&pool, ctx.sqlx_driver(), space_id, req.parent_id)
            .await
        {
            Ok(siblings) => siblings,
            Err(error) => return envelope::error(&error.to_string()),
        };
    if siblings.is_empty() || siblings.len() != req.sort_groups.len() {
        return envelope::error(" [group sort must include every sibling]");
    }
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_sorts = std::collections::HashSet::new();
    for entry in &req.sort_groups {
        if entry.id == 0
            || entry.sort < 1
            || !siblings.contains(&entry.id)
            || !seen_ids.insert(entry.id)
            || !seen_sorts.insert(entry.sort)
        {
            return envelope::error(" [group sort must include every sibling]");
        }
    }
    let entries: Vec<(i64, i64)> = req
        .sort_groups
        .iter()
        .map(|entry| (entry.id, entry.sort))
        .collect();
    match crate::repository::sort_groups(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        &entries,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(_) => envelope::error("sort failed"),
    }
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

/// `SpaceRouter.ArrangeLayout`: items may change group and sort, groups sort.
async fn arrange_layout(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !can_edit(&pool, user.id, space_id).await {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: ArrangeRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid layout request]"),
    };
    if req.items.is_empty() && req.groups.is_empty() {
        return envelope::error(" [invalid layout request]");
    }
    let mut seen_items = std::collections::HashSet::new();
    for item in &req.items {
        if item.id == 0 || item.item_icon_group_id == 0 {
            return envelope::error(" [invalid item entry]");
        }
        if !seen_items.insert(item.id) {
            return envelope::error(" [duplicate item entry]");
        }
    }
    let mut seen_groups = std::collections::HashSet::new();
    for group in &req.groups {
        if group.id == 0 {
            return envelope::error(" [invalid group entry]");
        }
        if !seen_groups.insert(group.id) {
            return envelope::error(" [duplicate group entry]");
        }
    }
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    for item in &req.items {
        let exists: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM item_icon WHERE id = ? AND space_id = ?")
                .bind(item.id)
                .bind(space_id)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or(None);
        if exists.is_none() {
            return envelope::error(&format!(" [item {} is not in this space]", item.id));
        }
        let group: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM item_icon_group WHERE id = ? AND space_id = ?")
                .bind(item.item_icon_group_id)
                .bind(space_id)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or(None);
        if group.is_none() {
            return envelope::error(&format!(
                " [target group {} is not in this space]",
                item.item_icon_group_id
            ));
        }
        if sqlx::query(
            "UPDATE item_icon SET item_icon_group_id = ?, sort = ?, updated_at = ? WHERE id = ?",
        )
        .bind(item.item_icon_group_id)
        .bind(item.sort)
        .bind(now_like_go())
        .bind(item.id)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return envelope::error("arrange failed");
        }
    }
    for group in &req.groups {
        let exists: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM item_icon_group WHERE id = ? AND space_id = ?")
                .bind(group.id)
                .bind(space_id)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or(None);
        if exists.is_none() {
            return envelope::error(&format!(" [group {} is not in this space]", group.id));
        }
        if sqlx::query("UPDATE item_icon_group SET sort = ?, updated_at = ? WHERE id = ?")
            .bind(group.sort)
            .bind(now_like_go())
            .bind(group.id)
            .execute(&mut *tx)
            .await
            .is_err()
        {
            return envelope::error("arrange failed");
        }
    }
    match tx.commit().await {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.DeleteSpace` plus `deleteSpaceData`: the rows go in one
/// transaction, and a stored file is removed only when nothing else references
/// it. Storage failures after the commit are logged, never rolled back.
async fn delete_space(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let (owner, kind) =
        match crate::repository::space_owner_and_type(&pool, ctx.sqlx_driver(), space_id).await {
            Ok(Some(space)) => space,
            Ok(None) | Err(_) => return error_code(404),
        };
    if owner != user.id {
        return error_code(1005);
    }
    if kind == "personal" {
        return envelope::error("a personal space cannot be deleted");
    }
    let ids = match crate::repository::paired_space_ids(&pool, ctx.sqlx_driver(), space_id).await {
        Ok(ids) => ids,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let removed = match crate::repository::delete_paired_space(&pool, ctx.sqlx_driver(), &ids).await
    {
        Ok(removed) => removed,
        Err(_) => return envelope::error("delete failed"),
    };

    let bucket = if ctx.conf.rclone.bucket.is_empty() {
        "uploads".to_string()
    } else {
        ctx.conf.rclone.bucket.clone()
    };
    let dir = if std::path::Path::new(&bucket).is_absolute() {
        std::path::PathBuf::from(&bucket)
    } else {
        ctx.dir.join(&bucket)
    };
    for name in removed {
        let _ = std::fs::remove_file(dir.join(&name));
    }
    success()
}

const ICON_EXTENSIONS: [&str; 7] = [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".ico"];
const UPLOAD_EXTENSIONS: [&str; 9] = [
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".ico", ".mp4", ".webm",
];

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

/// `FileRouter.UploadImg`: MD5-named content, one `file` row per upload.
async fn upload_image(headers: HeaderMap, request: Request) -> Response {
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

/// `SpaceRouter.CreateItemWithIcon`: the item arrives as a JSON form field and
/// the icon as an optional SHA-256-named file beside it.
async fn create_item_with_icon(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    mut multipart: Multipart,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !can_edit(&pool, user.id, space_id).await {
        return error_code(1005);
    }
    let mut item_json: Option<String> = None;
    let mut icon: Option<(String, Vec<u8>)> = None;
    // `c.FormFile` fails the whole handler when the part cannot be read, so a
    // multipart error is an answer, not a reason to drop the icon silently.
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => return envelope::error(" [invalid multipart upload]"),
        };
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "item" => {
                item_json = match field.text().await {
                    Ok(text) => Some(text),
                    Err(_) => return envelope::error(" [invalid multipart upload]"),
                };
            }
            "imgfile" => {
                let file_name = field.file_name().unwrap_or("").to_string();
                match field.bytes().await {
                    Ok(data) => icon = Some((file_name, data.to_vec())),
                    Err(_) => return envelope::error(" [icon too large]"),
                }
            }
            _ => {}
        }
    }
    let Some(item_json) = item_json else {
        return envelope::error(" [invalid item or group]");
    };
    let mut input: ItemIn = match serde_json::from_str(&item_json) {
        Ok(input) => input,
        Err(_) => return envelope::error(" [invalid item or group]"),
    };
    if input.item_icon_group_id == 0 || !valid_item_title(&input.title) {
        return envelope::error(" [invalid item or group]");
    }
    let group: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM item_icon_group WHERE id = ? AND space_id = ?")
            .bind(input.item_icon_group_id)
            .bind(space_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    if group.is_none() {
        return error_code(404);
    }

    let mut stored: Option<String> = None;
    if let Some((original, content)) = icon {
        if content.len() > 5 << 20 {
            return envelope::error(" [icon too large]");
        }
        let ext = extension_of(&original);
        if !ICON_EXTENSIONS.contains(&ext.as_str()) {
            return envelope::error(" [unsupported icon]");
        }
        let file_name = format!("{}{}", hex_digest(&content, true), ext);
        if store_object(&file_name, &content).is_err() {
            return error_code(1300);
        }
        input.icon = IconInfo {
            item_type: 2,
            src: format!("{}{}", upload_prefix(), file_name),
            file_name: file_name.clone(),
            text: String::new(),
            background_color: String::new(),
        };
        stored = Some(file_name);
    } else {
        ensure_item_icon(&input.title, &mut input.icon);
    }
    let icon_json = serde_json::to_string(&input.icon).unwrap_or_else(|_| "{}".to_string());
    let now = now_like_go();
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    if let Some(name) = &stored {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM file WHERE user_id = ? AND file_name = ?")
                .bind(user.id)
                .bind(name)
                .fetch_one(&mut *tx)
                .await
                .unwrap_or((0,));
        if count.0 == 0
            && sqlx::query(
                "INSERT INTO file (created_at, updated_at, user_id, file_name) VALUES (?, ?, ?, ?)",
            )
            .bind(&now)
            .bind(&now)
            .bind(user.id)
            .bind(name)
            .execute(&mut *tx)
            .await
            .is_err()
        {
            return envelope::error("create file record");
        }
    }
    let inserted = sqlx::query(
        "INSERT INTO item_icon (created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(&icon_json)
    .bind(&input.title)
    .bind(&input.url)
    .bind(&input.lan_url)
    .bind(&input.mobile_url)
    .bind(&input.description)
    .bind(input.open_method)
    .bind(input.sort)
    .bind(input.item_icon_group_id)
    .bind(user.id)
    .bind(space_id)
    .execute(&mut *tx)
    .await;
    let id = match inserted {
        Ok(result) => result.last_insert_rowid(),
        Err(error) => return envelope::error(&error.to_string()),
    };
    if let Err(error) = tx.commit().await {
        return envelope::error(&error.to_string());
    }
    match load_item(&pool, space_id, id).await {
        Some(item) => success_data(item),
        None => error_code(404),
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
    tx: &'a mut sqlx::Transaction<'_, sqlx::Sqlite>,
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
        let found: Option<(i64,)> = match parent_id {
            None => sqlx::query_as(
                "SELECT id FROM item_icon_group WHERE space_id = ? AND title = ? AND parent_id IS NULL LIMIT 1",
            )
            .bind(space_id)
            .bind(&title)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|error| error.to_string())?,
            Some(parent) => sqlx::query_as(
                "SELECT id FROM item_icon_group WHERE space_id = ? AND title = ? AND parent_id = ? LIMIT 1",
            )
            .bind(space_id)
            .bind(&title)
            .bind(parent)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|error| error.to_string())?,
        };
        let group_id = match found {
            Some((id,)) => id,
            None => {
                let inserted = sqlx::query(
                    "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES (?, ?, ?, ?, '', ?, ?, ?, ?)",
                )
                .bind(now_like_go())
                .bind(now_like_go())
                .bind(&node.icon)
                .bind(&title)
                .bind(index)
                .bind(user_id)
                .bind(space_id)
                .bind(parent_id)
                .execute(&mut **tx)
                .await
                .map_err(|error| error.to_string())?;
                summary.groups += 1;
                inserted.last_insert_rowid()
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
            let existing: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM item_icon WHERE space_id = ? AND item_icon_group_id = ? AND url = ?",
            )
            .bind(space_id)
            .bind(group_id)
            .bind(&bookmark.url)
            .fetch_one(&mut **tx)
            .await
            .map_err(|error| error.to_string())?;
            if existing.0 > 0 {
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

/// `SpaceRouter.ImportBookmarksBatch`.
async fn import_bookmarks(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !can_edit(&pool, user.id, space_id).await {
        return error_code(1005);
    }

    let mut multipart = match Multipart::from_request(request, &()).await {
        Ok(multipart) => multipart,
        Err(_) => return envelope::error(" [invalid bookmarks]"),
    };

    let mut bookmarks_json: Option<String> = None;
    let mut files: std::collections::HashMap<String, (String, Vec<u8>)> =
        std::collections::HashMap::new();
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();
        if name == "bookmarks" {
            bookmarks_json = field.text().await.ok();
            continue;
        }
        if name.trim().is_empty() {
            return envelope::error(" [invalid file]");
        }
        let file_name = field.file_name().unwrap_or("").to_string();
        if file_name.is_empty() {
            continue;
        }
        match field.bytes().await {
            Ok(data) => {
                files.insert(name, (file_name, data.to_vec()));
            }
            Err(_) => return envelope::error(" [invalid file]"),
        }
    }
    let Some(bookmarks_json) = bookmarks_json else {
        return envelope::error(" [invalid bookmarks]");
    };
    let batch: BookmarkBatch = match serde_json::from_str(&bookmarks_json) {
        Ok(batch) => batch,
        Err(_) => return envelope::error(" [invalid bookmarks]"),
    };

    let mut icons: std::collections::HashMap<String, IconInfo> = std::collections::HashMap::new();
    for (key, (original, content)) in files {
        if content.len() > 5 << 20 {
            return envelope::error(" [icon too large]");
        }
        let ext = extension_of(&original);
        if !ICON_EXTENSIONS.contains(&ext.as_str()) {
            return envelope::error(" [unsupported icon]");
        }
        let name = format!("{}{}", hex_digest(&content, true), ext);
        if store_object(&name, &content).is_err() {
            return error_code(1300);
        }
        icons.insert(
            key,
            IconInfo {
                item_type: 2,
                src: format!("{}{}", upload_prefix(), name),
                file_name: name,
                text: String::new(),
                background_color: String::new(),
            },
        );
    }

    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let now = now_like_go();
    for icon in icons.values() {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM file WHERE user_id = ? AND file_name = ?")
                .bind(user.id)
                .bind(&icon.file_name)
                .fetch_one(&mut *tx)
                .await
                .unwrap_or((0,));
        if count.0 == 0
            && sqlx::query(
                "INSERT INTO file (created_at, updated_at, user_id, file_name) VALUES (?, ?, ?, ?)",
            )
            .bind(&now)
            .bind(&now)
            .bind(user.id)
            .bind(&icon.file_name)
            .execute(&mut *tx)
            .await
            .is_err()
        {
            return envelope::error("create file record");
        }
    }
    let mut summary = ImportSummary {
        groups: 0,
        skipped: 0,
        seen: std::collections::HashSet::new(),
        items: Vec::new(),
    };
    for (index, node) in batch.groups.iter().enumerate() {
        if let Err(error) = import_node(
            &mut tx,
            &icons,
            space_id,
            user.id,
            None,
            node,
            index as i64,
            &mut summary,
        )
        .await
        {
            return envelope::error(&error);
        }
    }
    for (icon_json, title, group_id, sort, space, owner, url) in &summary.items {
        if sqlx::query(
            "INSERT INTO item_icon (created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES (?, ?, ?, ?, ?, '', '', '', 2, ?, ?, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .bind(icon_json)
        .bind(title)
        .bind(url)
        .bind(sort)
        .bind(group_id)
        .bind(owner)
        .bind(space)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return envelope::error("create item");
        }
    }
    if let Err(error) = tx.commit().await {
        return envelope::error(&error.to_string());
    }
    let files_json: serde_json::Map<String, serde_json::Value> = icons
        .iter()
        .map(|(key, icon)| (key.clone(), serde_json::to_value(icon).unwrap_or_default()))
        .collect();
    success_data(json!({
        "files": serde_json::Value::Object(files_json),
        "groups": summary.groups,
        "skipped": summary.skipped,
    }))
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

/// `UserService.CreateUser`: the account, its personal pair, both memberships
/// and the default group in one transaction.
async fn create_user(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let input: UserIn = match serde_json::from_slice(&body) {
        Ok(input) => input,
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    let mail = input.mail.trim().to_lowercase();
    if mail.len() < 5 {
        return envelope::error(" [The account must be no less than 5 characters long]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    match crate::repository::user_mail_exists(&pool, ctx.sqlx_driver(), &mail, None).await {
        Ok(true) => return error_code(1008),
        Ok(false) => {}
        Err(error) => return envelope::error(&error.to_string()),
    }
    let id = match crate::repository::create_user(
        &pool,
        ctx.sqlx_driver(),
        &mail,
        &password_encryption(&input.password),
        &input.name,
        &input.head_image,
        input.role,
        &now_like_go(),
    )
    .await
    {
        Ok(id) => id,
        Err(error) => return envelope::error(&error.to_string()),
    };
    success_data(json!({"userId": id}))
}

/// `UsersRouter.Update`: an empty password keeps the stored one, a new one is
/// hashed, and a renamed account renames its personal pair.
async fn update_user(headers: HeaderMap, request: Request) -> Response {
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
    let input: UserIn = match serde_json::from_slice(&body) {
        Ok(input) => input,
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let (existing_password, existing_name) =
        match crate::repository::user_password_and_name(&pool, ctx.sqlx_driver(), input.id).await {
            Ok(Some(pair)) => pair,
            Ok(None) | Err(_) => return envelope::error(" [record not found]"),
        };
    let mut password = input.password.clone();
    if password.is_empty() {
        password = existing_password.clone();
    }
    let mail = input.mail.trim().to_lowercase();
    if mail.len() < 3 {
        return envelope::error(" [The account must be no less than 3 characters long]");
    }
    match crate::repository::user_mail_exists(&pool, ctx.sqlx_driver(), &mail, Some(input.id)).await
    {
        Ok(true) => return error_code(1008),
        Ok(false) => {}
        Err(error) => return envelope::error(&error.to_string()),
    }
    let password_changed = password != existing_password;
    if password_changed {
        password = password_encryption(&password);
    }
    let rename = (!input.name.is_empty() && input.name != existing_name).then(|| {
        (input.name.clone(), format!("{}-B", input.name))
    });
    let renamed = rename.as_ref().map(|(yin, yang)| (yin.as_str(), yang.as_str()));
    match crate::repository::update_user_admin(
        &pool,
        ctx.sqlx_driver(),
        input.id,
        &mail,
        (!input.name.is_empty()).then_some(input.name.as_str()),
        (!input.head_image.is_empty()).then_some(input.head_image.as_str()),
        (input.status != 0).then_some(input.status),
        (input.role != 0).then_some(input.role),
        password_changed.then_some(password.as_str()),
        &now_like_go(),
        renamed,
    )
    .await
    {
        Ok(()) => {}
        Err(_) => return envelope::error("update failed"),
    }
    let stored = match crate::repository::admin_user_view(&pool, ctx.sqlx_driver(), input.id).await
    {
        Ok(Some(user)) => user,
        _ => return envelope::database_error(),
    };
    envelope::success_struct(AdminUser {
        id: stored.id,
        create_time: rfc3339(&stored.created_at),
        update_time: rfc3339(&stored.updated_at),
        password: stored.password,
        name: stored.name,
        head_image: stored.head_image,
        status: stored.status,
        role: stored.role,
        mail: stored.mail,
        token: String::new(),
    })
}

#[derive(serde::Deserialize)]
struct UserDeleteRequest {
    #[serde(default, rename = "userId")]
    user_id: i64,
    #[serde(default)]
    force: bool,
}

/// `UserRepo.DeleteWithSpaces`.
async fn delete_user(headers: HeaderMap, request: Request) -> Response {
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
    let req: UserDeleteRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let owned = match crate::repository::spaces_owned_by(&pool, ctx.sqlx_driver(), req.user_id)
        .await
    {
        Ok(owned) => owned,
        Err(error) => return envelope::error(&error.to_string()),
    };
    if !req.force && owned.iter().any(|(_, kind)| kind == "shared") {
        return envelope::error("user owns shared spaces; transfer them before deletion");
    }
    let mut ids: Vec<i64> = owned.iter().map(|(id, _)| *id).collect();
    if !ids.is_empty() {
        match crate::repository::paired_ids_of(&pool, ctx.sqlx_driver(), &ids).await {
            Ok(paired) => ids.extend(paired),
            Err(error) => return envelope::error(&error.to_string()),
        }
    }
    match crate::repository::delete_user(&pool, ctx.sqlx_driver(), req.user_id, &ids).await {
        Ok(()) => success(),
        Err(_) => envelope::error("delete failed"),
    }
}

#[derive(serde::Deserialize)]
struct FileDeleteRequest {
    #[serde(default)]
    id: i64,
}

/// `FileRouter.Delete`: the row and the stored bytes.
async fn delete_file(headers: HeaderMap, request: Request) -> Response {
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
async fn select_user_theme(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    package_id: &str,
    revision_id: &str,
    mode: &str,
) -> Result<(), String> {
    let package: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM package_record_v2 WHERE id = ? AND removed = 0")
            .bind(package_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    if package.is_none() {
        return Err("record not found".to_string());
    }
    let revision: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM revision_record_v2 WHERE id = ? AND package_id = ?")
            .bind(revision_id)
            .bind(package_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    if revision.is_none() {
        return Err("record not found".to_string());
    }
    let scope = format!("user:{user_id}");
    let now = now_like_go();
    sqlx::query(
        "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at)
         VALUES (?, ?, ?, ?, '', NULL, ?)
         ON CONFLICT(scope) DO UPDATE SET package_id = excluded.package_id, active_revision_id = excluded.active_revision_id, last_good_revision_id = excluded.last_good_revision_id, pending_revision_id = '', trial_started_at = NULL, updated_at = excluded.updated_at",
    )
    .bind(&scope)
    .bind(package_id)
    .bind(revision_id)
    .bind(revision_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;
    sqlx::query(
        "INSERT INTO user_theme_preference_v2 (user_id, mode, theme_mode, updated_at)
         VALUES (?, ?, 'custom', ?)
         ON CONFLICT(user_id) DO UPDATE SET mode = excluded.mode, theme_mode = 'custom', updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(mode)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;
    sqlx::query(
        "INSERT INTO audit_record (actor_id, action, package_id, created_at) VALUES (?, 'select', ?, ?)",
    )
    .bind(user_id)
    .bind(package_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// `ThemeRouter.SetPreference`.
async fn set_theme_preference(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: PreferenceRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !req.revision.is_empty() || !req.package_id.is_empty() || !req.mode.is_empty() {
        let mode = if req.mode.is_empty() {
            "auto".to_string()
        } else {
            req.mode.clone()
        };
        if !matches!(mode.as_str(), "light" | "dark" | "auto") {
            return envelope::error(" [mode must be light, dark, or auto]");
        }
        let revision_id = if !req.revision.is_empty() {
            req.revision.clone()
        } else if !req.package_id.is_empty() {
            match crate::repository::latest_revision(
                &pool,
                ctx.sqlx_driver(),
                &req.package_id,
            )
            .await
            {
                Ok(Some(id)) => id,
                _ => return error_code(404),
            }
        } else {
            let instance = match crate::repository::latest_revision(
                &pool,
                ctx.sqlx_driver(),
                "org.yin.default",
            )
            .await
            {
                Ok(Some(id)) => id,
                _ => return envelope::error("package not found"),
            };
            match crate::repository::activation_revision(
                &pool,
                ctx.sqlx_driver(),
                "instance",
            )
            .await
            {
                Ok(Some(active)) => active,
                _ => instance,
            }
        };
        let package_id =
            match crate::repository::package_id_for_revision(&pool, ctx.sqlx_driver(), &revision_id)
                .await
            {
                Ok(Some(id)) => id,
                _ => return error_code(404),
            };
        if let Err(error) = crate::repository::select_user_theme(
            &pool,
            ctx.sqlx_driver(),
            user.id,
            &package_id,
            &revision_id,
            &mode,
            &now_like_go(),
        )
        .await
        {
            if error == "record not found" {
                return error_code(404);
            }
            return envelope::error(&error);
        }
    }
    if !req.theme_mode.is_empty() {
        if !matches!(req.theme_mode.as_str(), "custom" | "follow-space") {
            return envelope::error(" [themeMode must be custom or follow-space]");
        }
        let mode = crate::repository::user_theme_preference(&pool, ctx.sqlx_driver(), user.id)
            .await
            .ok()
            .flatten()
            .map(|(mode, _)| mode)
            .filter(|mode| !mode.is_empty())
            .unwrap_or_else(|| "auto".to_string());
        if let Err(error) = crate::repository::save_theme_preference(
            &pool,
            ctx.sqlx_driver(),
            user.id,
            &mode,
            &req.theme_mode,
            &now_like_go(),
        )
        .await
        {
            return envelope::error(&error.to_string());
        }
    }
    success()
}

/// `ThemeRouter.RevisionsV2`.
async fn admin_revisions(headers: HeaderMap) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let state = match crate::repository::theme_admin_state(pool, ctx.sqlx_driver()).await {
        Ok(state) => state,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let package_list: Vec<serde_json::Value> = state
        .packages
        .into_iter()
        .map(|package| {
            json!({
                "id": package.id,
                "name": package.name,
                "version": package.version,
                "revision": package.latest_revision,
                "verified": package.verified != 0,
                "createdAt": rfc3339(&package.created_at),
            })
        })
        .collect();
    let revision_list: Vec<serde_json::Value> = state
        .revisions
        .into_iter()
        .map(|revision| {
            json!({
                "createdAt": rfc3339(&revision.created_at),
                "id": revision.id,
                "packageId": revision.package_id,
                "verified": revision.verified != 0,
                "version": revision.version,
            })
        })
        .collect();
    success_data(json!({
        "activeRevision": state.active_revision,
        "defaultPackage": state.default_package,
        "lastGoodRevision": state.last_good_revision,
        "packages": package_list,
        "pendingRevision": state.pending_revision,
        "revisions": revision_list,
        "trialStartedAt": state.trial_started_at.as_deref().map(rfc3339),
    }))
}

/// `ThemeRouter.Audit`: the newest 200 theme actions.
async fn admin_audit(headers: HeaderMap) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let rows = match crate::repository::audit_entries(pool, ctx.sqlx_driver()).await {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "actorId": row.actor_id,
                "action": row.action,
                "packageId": row.package_id,
                "createdAt": rfc3339(&row.created_at),
            })
        })
        .collect();
    success_data(serde_json::Value::Array(list))
}

/// `ThemeRouter.TrustedRuntimePolicyV2`.
async fn admin_trusted(headers: HeaderMap, Path(revision): Path<String>) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let exists = match crate::repository::package_id_for_revision(
        pool,
        ctx.sqlx_driver(),
        &revision,
    )
    .await
    {
        Ok(Some(_)) => true,
        _ => false,
    };
    if !exists {
        return error_code(404);
    }
    let enabled = crate::repository::trusted_enabled(pool, ctx.sqlx_driver(), &revision)
        .await
        .unwrap_or(None)
        .unwrap_or(false);
    success_data(json!({
        "enabled": enabled,
        "revision": revision,
    }))
}

async fn web_wallpaper_asset(
    headers: HeaderMap,
    Path((id, name)): Path<(String, String)>,
) -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row: Option<(Vec<u8>, Vec<u8>, String)> =
        sqlx::query_as("SELECT html, poster, poster_type FROM web_wallpaper_record WHERE id = ?")
            .bind(&id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    let Some((html, poster, poster_type)) = row else {
        return envelope::empty_not_found();
    };
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

/// `ThemeRouter.PackageV2`: one revision's public package.
async fn theme_revision(Path(revision): Path<String>) -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    match revision_public(pool, &revision).await {
        Ok(value) => success_data(value),
        Err(_) => envelope::empty_not_found(),
    }
}

#[derive(serde::Deserialize)]
struct TrustedRequest {
    #[serde(default)]
    enabled: bool,
}

/// `ThemeRouter.SetTrustedRuntimePolicyV2`: an administrator switches the
/// trusted runtime on or off for one revision.
async fn set_trusted(
    headers: HeaderMap,
    Path(revision): Path<String>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: TrustedRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let manifest_json =
        match crate::repository::revision_manifest(pool, ctx.sqlx_driver(), &revision).await {
            Ok(Some(manifest)) => manifest,
            Ok(None) | Err(_) => return error_code(404),
        };
    if req.enabled {
        let manifest: serde_json::Value = serde_json::from_str(&manifest_json).unwrap_or_default();
        if !supported_modes(&manifest)
            .iter()
            .any(|mode| mode == "trusted")
        {
            return envelope::error(" [theme does not declare support for the trusted runtime]");
        }
    }
    match crate::repository::save_trusted_policy(
        pool,
        ctx.sqlx_driver(),
        &revision,
        req.enabled,
        user.id,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => {
            if !req.enabled {
                let _ = crate::repository::withdraw_trusted_grants(
                    pool,
                    ctx.sqlx_driver(),
                    &revision,
                )
                .await;
            }
            success()
        }
        Err(error) => envelope::error(&error.to_string()),
    }
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
    pool: &sqlx::SqlitePool,
    revision: &str,
) -> Option<Vec<String>> {
    let row = sqlx::query_as::<_, (String, i64)>(
        "SELECT manifest_json, verified FROM revision_record_v2 WHERE id = ?",
    )
    .bind(revision)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)?;
    let (manifest_json, verified) = row;
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

/// `ThemeRouter.ThemeGrantV2`.
async fn theme_grant(
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row: Option<(String,)> =
        sqlx::query_as("SELECT manifest_json FROM revision_record_v2 WHERE id = ?")
            .bind(&revision)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    let Some((manifest_json,)) = row else {
        return error_code(404);
    };
    let manifest: serde_json::Value = serde_json::from_str(&manifest_json).unwrap_or_default();
    let available = supported_modes(&manifest)
        .iter()
        .any(|mode| mode == &execution_mode);
    let trusted_enabled: Option<(i64,)> =
        sqlx::query_as("SELECT enabled FROM trusted_runtime_policy_v2 WHERE revision_id = ?")
            .bind(&revision)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    let available = if execution_mode == "trusted" {
        available && trusted_enabled.map(|value| value.0 != 0).unwrap_or(false)
    } else {
        available
    };
    let grant: Option<(String, String, String)> = sqlx::query_as(
        "SELECT revision_id, execution_mode, permissions_json FROM grant_record_v2 WHERE user_id = ? AND revision_id = ? AND execution_mode = ?",
    )
    .bind(user.id)
    .bind(&revision)
    .bind(&execution_mode)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    if let Some((revision_id, mode, permissions_json)) = grant {
        let permissions: Vec<String> = serde_json::from_str(&permissions_json).unwrap_or_default();
        return success_data(json!({
            "available": available,
            "executionMode": mode,
            "granted": available,
            "permissions": permissions,
            "revision": revision_id,
        }));
    }
    if let Some(permissions) = implicit_builtin_permissions(pool, &revision).await {
        let permissions = if execution_mode == "sandbox" {
            permissions
        } else {
            Vec::new()
        };
        let granted = execution_mode == "sandbox" && available;
        return success_data(json!({
            "available": available,
            "executionMode": execution_mode,
            "granted": granted,
            "permissions": permissions,
            "revision": revision,
        }));
    }
    success_data(json!({
        "available": available,
        "executionMode": execution_mode,
        "granted": false,
        "permissions": Vec::<String>::new(),
        "revision": revision,
    }))
}

#[derive(serde::Deserialize)]
struct GrantRequest {
    #[serde(default, rename = "executionMode")]
    execution_mode: String,
    #[serde(default)]
    permissions: Vec<String>,
}

/// `ThemeRouter.SetThemeGrantV2`.
async fn set_theme_grant(
    headers: HeaderMap,
    Path(revision): Path<String>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: GrantRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    let execution_mode = if req.execution_mode.is_empty() {
        "sandbox".to_string()
    } else {
        req.execution_mode.clone()
    };
    if !matches!(execution_mode.as_str(), "sandbox" | "trusted") {
        return envelope::error(" [executionMode must be sandbox or trusted]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row: Option<(String,)> =
        sqlx::query_as("SELECT manifest_json FROM revision_record_v2 WHERE id = ?")
            .bind(&revision)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    let Some((manifest_json,)) = row else {
        return error_code(404);
    };
    let manifest: serde_json::Value = match serde_json::from_str(&manifest_json) {
        Ok(value) => value,
        Err(_) => return envelope::database_error(),
    };
    if let Err(error) = validate_grant(&manifest, &execution_mode, &req.permissions) {
        return envelope::error(&format!(" [{error}]"));
    }
    if execution_mode == "trusted" {
        let enabled: Option<(i64,)> = sqlx::query_as(
            "SELECT enabled FROM trusted_runtime_policy_v2 WHERE revision_id = ? AND enabled = 1",
        )
        .bind(&revision)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
        if enabled.is_none() {
            return error_code(1005);
        }
    }
    let permissions = serde_json::to_string(&req.permissions).unwrap_or_else(|_| "[]".to_string());
    match sqlx::query(
        "INSERT INTO grant_record_v2 (user_id, revision_id, execution_mode, permissions_json, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, revision_id, execution_mode) DO UPDATE SET permissions_json = excluded.permissions_json, updated_at = excluded.updated_at",
    )
    .bind(user.id)
    .bind(&revision)
    .bind(&execution_mode)
    .bind(&permissions)
    .bind(now_like_go())
    .execute(pool)
    .await
    {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `ThemeRouter.RevokeThemeGrantV2`: a built-in sandbox grant becomes an empty
/// explicit grant instead of being deleted, because the built-in would
/// otherwise be granted implicitly again.
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    if execution_mode == "sandbox"
        && implicit_builtin_permissions(pool, &revision)
            .await
            .is_some()
    {
        let result = sqlx::query(
            "INSERT INTO grant_record_v2 (user_id, revision_id, execution_mode, permissions_json, updated_at)
             VALUES (?, ?, 'sandbox', '[]', ?)
             ON CONFLICT(user_id, revision_id, execution_mode) DO UPDATE SET permissions_json = '[]', updated_at = excluded.updated_at",
        )
        .bind(user.id)
        .bind(&revision)
        .bind(now_like_go())
        .execute(pool)
        .await;
        return match result {
            Ok(_) => success(),
            Err(error) => envelope::error(&error.to_string()),
        };
    }
    match sqlx::query(
        "DELETE FROM grant_record_v2 WHERE user_id = ? AND revision_id = ? AND execution_mode = ?",
    )
    .bind(user.id)
    .bind(&revision)
    .bind(&execution_mode)
    .execute(pool)
    .await
    {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

// ------------------------------------------------------------ theme packages

/// `POST /api/theme/v2/admin/preview`.
async fn theme_preview_upload(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let mut multipart = match Multipart::from_request(request, &()).await {
        Ok(multipart) => multipart,
        Err(_) => return envelope::error(" [invalid multipart theme upload]"),
    };
    let mut data: Option<Vec<u8>> = None;
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        if field.name() == Some("package") {
            match field.bytes().await {
                Ok(bytes) => data = Some(bytes.to_vec()),
                Err(_) => return envelope::error(" [invalid multipart theme upload]"),
            }
        }
    }
    let Some(data) = data else {
        return envelope::error(" [theme package file is required]");
    };
    if data.len() > crate::packages::MAX_ARCHIVE {
        return envelope::error(" [theme package exceeds 32 MiB]");
    }
    let package = match crate::packages::parse_archive(&data, true) {
        Ok(package) => package,
        Err(error) => return envelope::error(&format!(" [{error}]")),
    };
    let token = match crate::packages::save_preview(package.clone()) {
        Ok(token) => token,
        Err(error) => return envelope::error(&error),
    };
    success_data(json!({
        "package": package.preview_public(&token),
        "token": token,
    }))
}

/// `POST /api/theme/v2/admin/install`.
async fn theme_install(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let mut multipart = match Multipart::from_request(request, &()).await {
        Ok(multipart) => multipart,
        Err(_) => return envelope::error(" [invalid multipart theme upload]"),
    };
    let mut data: Option<Vec<u8>> = None;
    let mut confirm = false;
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        match field.name() {
            Some("package") => match field.bytes().await {
                Ok(bytes) => data = Some(bytes.to_vec()),
                Err(_) => return envelope::error(" [invalid multipart theme upload]"),
            },
            Some("confirmUnverified") => {
                let value = field.text().await.unwrap_or_default();
                confirm = value.eq_ignore_ascii_case("true");
            }
            _ => {}
        }
    }
    let Some(data) = data else {
        return envelope::error(" [theme package file is required]");
    };
    if data.len() > crate::packages::MAX_ARCHIVE {
        return envelope::error(" [theme package exceeds 32 MiB]");
    }
    let package = match crate::packages::parse_archive(&data, confirm) {
        Ok(package) => package,
        Err(error) => return envelope::error(&format!(" [{error}]")),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if crate::repository::install_package(&pool, ctx.sqlx_driver(), user.id, &package)
        .await
        .is_err()
    {
        // `InstallPackageV2` failures go through `response.ErrorDatabase`, which
        // answers code 1200 and drops the underlying text.
        return envelope::database_error();
    }
    success_data(json!({
        "id": package.manifest.id,
        "preview": false,
        "revision": package.revision,
        "verified": package.verified,
    }))
}

/// `GET /api/theme/v2/preview/:token`.
async fn theme_preview_get(Path(token): Path<String>) -> Response {
    match crate::packages::get_preview(&token) {
        Some(package) => success_data(package.preview_public(&token)),
        None => envelope::empty_not_found(),
    }
}

/// `GET /api/theme/v2/preview/:token/assets/*name`.
async fn theme_preview_asset(
    headers: HeaderMap,
    Path((token, raw_name)): Path<(String, String)>,
) -> Response {
    let name = raw_name.trim_start_matches('/');
    let Some(package) = crate::packages::get_preview(&token) else {
        return envelope::empty_not_found();
    };
    let Some((media_type, content)) = package.files.get(name) else {
        return envelope::empty_not_found();
    };
    serve_bytes(&headers, media_type, content)
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

/// `RemovePackageV2`: built-in themes are protected; a removed package's
/// activations fall back to the seeded Yin package.
async fn remove_package(headers: HeaderMap, Path(package_id): Path<String>) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    if is_builtin_theme(&package_id) {
        return envelope::error(" [built-in themes cannot be removed]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    match crate::repository::package_removed(&pool, ctx.sqlx_driver(), &package_id).await {
        Ok(Some(0)) => {}
        Ok(_) | Err(_) => return error_code(404),
    }
    match crate::repository::latest_revision(&pool, ctx.sqlx_driver(), "org.yin.default").await {
        Ok(Some(_)) => {}
        _ => return envelope::database_error(),
    }
    match crate::repository::remove_theme_package(&pool, ctx.sqlx_driver(), user.id, &package_id)
        .await
    {
        Ok(()) => success(),
        Err(_) => envelope::error("remove failed"),
    }
}

/// `SetDefaultV2`: activates a revision for the whole instance.
async fn set_default_package(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: RevisionRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) if error.is_eof() => RevisionRequest {
            revision: String::new(),
            package_id: String::new(),
        },
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    if req.revision.is_empty() && req.package_id.is_empty() {
        return envelope::error(" [revision or packageId is required]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let revision_id = if !req.revision.is_empty() {
        req.revision.clone()
    } else {
        match crate::repository::latest_revision(&pool, ctx.sqlx_driver(), &req.package_id).await {
            Ok(Some(id)) => id,
            _ => return error_code(404),
        }
    };
    let package_id =
        match crate::repository::revision_package_id(&pool, ctx.sqlx_driver(), &revision_id).await {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => return error_code(404),
        };
    match crate::repository::set_default_theme(
        &pool,
        ctx.sqlx_driver(),
        &package_id,
        &revision_id,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `BeginTrialV2`: marks a revision pending for the 30 second trial window.
async fn begin_trial(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: RevisionRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) if error.is_eof() => RevisionRequest {
            revision: String::new(),
            package_id: String::new(),
        },
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    if req.revision.is_empty() {
        return envelope::error(" [revision is required]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let package_id =
        match crate::repository::revision_package_id(&pool, ctx.sqlx_driver(), &req.revision).await
        {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => return error_code(404),
        };
    match crate::repository::package_removed(&pool, ctx.sqlx_driver(), &package_id).await {
        Ok(Some(0)) => {}
        Ok(_) | Err(_) => return error_code(404),
    }
    let now = now_like_go();
    match crate::repository::begin_theme_trial(
        &pool,
        ctx.sqlx_driver(),
        &package_id,
        &req.revision,
        &now,
    )
    .await
    {
        Ok(()) => success_data(json!({
            "expiresInSeconds": 30,
            "revision": req.revision,
        })),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `ConfirmTrialV2`: promotes the pending revision and marks it healthy.
async fn confirm_trial(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: RevisionRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) if error.is_eof() => RevisionRequest {
            revision: String::new(),
            package_id: String::new(),
        },
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    if req.revision.is_empty() {
        return envelope::error(" [revision is required]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let pending = match crate::repository::instance_pending(&pool, ctx.sqlx_driver()).await {
        Ok(Some(pending)) => pending,
        Ok(None) | Err(_) => return envelope::error(" [record not found]"),
    };
    if pending.is_empty() || pending != req.revision {
        return envelope::error(" [no matching theme activation trial]");
    }
    let package_id =
        match crate::repository::revision_package_id(&pool, ctx.sqlx_driver(), &req.revision).await
        {
            Ok(Some(id)) => id,
            _ => return envelope::error(" [record not found]"),
        };
    match crate::repository::confirm_theme_trial(
        &pool,
        ctx.sqlx_driver(),
        &package_id,
        &req.revision,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `RollbackV2`: back to the last good revision, or the seeded default.
async fn rollback_theme(headers: HeaderMap) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let fallback = match crate::repository::latest_revision(
        &pool,
        ctx.sqlx_driver(),
        "org.yin.default",
    )
    .await
    {
        Ok(Some(revision)) => revision,
        _ => return envelope::database_error(),
    };
    let last_good = match crate::repository::instance_last_good(&pool, ctx.sqlx_driver()).await {
        Ok(Some(last_good)) => last_good,
        Ok(None) | Err(_) => return envelope::database_error(),
    };
    let revision_id = if last_good.is_empty() {
        fallback
    } else {
        last_good
    };
    let package_id =
        match crate::repository::revision_package_id(&pool, ctx.sqlx_driver(), &revision_id).await
        {
            Ok(Some(id)) => id,
            _ => return envelope::database_error(),
        };
    match crate::repository::rollback_theme(
        &pool,
        ctx.sqlx_driver(),
        &package_id,
        &revision_id,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
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

/// `ItemIconRouter.GetSiteFavicon`: discover the icon, download it, store it and
/// record the file row. The name is content-addressed by time in Go, so it is
/// reproduced the same way rather than being made deterministic.
async fn get_site_favicon(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: FaviconRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    if req.url.trim().is_empty() {
        return success_data(json!({"iconUrl": "", "fileName": ""}));
    }
    let icon_url = match discover_icon(req.url.trim()).await {
        Ok(url) => url,
        Err(error) => {
            return envelope::error(&format!("acquisition failed: get ico error:{error}"));
        }
    };
    let parsed = match reqwest::Url::parse(req.url.trim()) {
        Ok(url) => url,
        Err(error) => return envelope::error(&format!("acquisition failed:{error}")),
    };
    // The icon URL loses its query string, as the Go handler does.
    let icon = match reqwest::Url::parse(&icon_url) {
        Ok(mut url) => {
            url.set_query(None);
            url.set_fragment(None);
            url
        }
        Err(error) => {
            return envelope::error(&format!("acquisition failed: parsed ico URL :{error}"));
        }
    };
    let _ = &parsed;

    let client = match reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => return envelope::error(&format!("acquisition failed: download {error}")),
    };
    let response = match client.get(icon.clone()).send().await {
        Ok(response) => response,
        Err(error) => return envelope::error(&format!("acquisition failed: download {error}")),
    };
    if !response.status().is_success() {
        return envelope::error(&format!(
            "acquisition failed: download HTTP request failed, status code: {}",
            response.status().as_u16()
        ));
    }
    let content = match response.bytes().await {
        Ok(bytes) => bytes.to_vec(),
        Err(error) => return envelope::error(&format!("acquisition failed: download {error}")),
    };
    if content.len() > 10 * 1024 * 1024 {
        return envelope::error("acquisition failed: download 文件太大，不下载");
    }

    let url_file_name = icon
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .unwrap_or("")
        .to_string();
    let extension = match url_file_name.rfind('.') {
        Some(index) => url_file_name[index..].to_string(),
        None => ".ico".to_string(),
    };
    let extension = if extension.is_empty() {
        ".ico".to_string()
    } else {
        extension
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| format!("{}", value.as_nanos()))
        .unwrap_or_default();
    let file_name = format!(
        "{}{}",
        hex_digest(format!("{url_file_name}{now}").as_bytes(), false),
        extension
    );
    if store_object(&file_name, &content).is_err() {
        return envelope::error("acquisition failed: download failed to upload file");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let now = now_like_go();
    if sqlx::query(
        "INSERT INTO file (created_at, updated_at, user_id, file_name) VALUES (?, ?, ?, ?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(user.id)
    .bind(&file_name)
    .execute(pool)
    .await
    .is_err()
    {
        return envelope::database_error();
    }
    // `ItemIconGetSiteFaviconResp` keeps `iconUrl` before `fileName`.
    success_data(json!({
        "iconUrl": format!("{}{}", upload_prefix(), file_name),
        "fileName": file_name,
    }))
}

/// `ThemeRouter.WallpaperLibraryAsset`: the packaged artwork, only under the
/// catalogue's own version prefix.
async fn wallpaper_library_asset(
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

/// `UserRouter.UpdateInfo`: a 3–15 character nickname and an optional avatar.
async fn update_user_info(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: UpdateInfoRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    let length = req.name.chars().count();
    if req.name.is_empty() {
        return envelope::error(" [ Name为必填字段]");
    }
    if length < 3 {
        return envelope::error(" [ Name长度必须至少为3个字符]");
    }
    if length > 15 {
        return envelope::error(" [ Name长度不能超过15个字符]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if crate::repository::user_password_and_name(&pool, ctx.sqlx_driver(), user.id)
        .await
        .map(|pair| pair.is_none())
        .unwrap_or(true)
    {
        return envelope::database_error();
    }
    match crate::repository::update_user_profile(
        &pool,
        ctx.sqlx_driver(),
        user.id,
        &req.name,
        &req.head_image,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(_) => envelope::database_error(),
    }
}

#[derive(serde::Deserialize)]
struct UpdatePasswordRequest {
    #[serde(default, rename = "oldPassword")]
    old_password: String,
    #[serde(default, rename = "newPassword")]
    new_password: String,
}

/// `UserRouter.UpdatePasssword`: check the old password, store the new hash and
/// revoke every issued token.
async fn update_password(headers: HeaderMap, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: UpdatePasswordRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let stored = match crate::repository::user_password_and_name(&pool, ctx.sqlx_driver(), user.id)
        .await
        .unwrap_or(None)
    {
        Some((password, _)) => password,
        None => return envelope::error(" [record not found]"),
    };
    if stored != password_encryption(&req.old_password) {
        return error_code(1007);
    }
    match crate::repository::update_user_password(
        &pool,
        ctx.sqlx_driver(),
        user.id,
        &stored,
        &password_encryption(&req.new_password),
        &now_like_go(),
    )
    .await
    {
        Ok(true) => success(),
        Ok(false) => error_code(1007),
        Err(_) => envelope::database_error(),
    }
}

/// `FileRouter.GetS3File`: an uploaded file from local storage, with the cache
/// policy its content-hash name allows.
async fn serve_stored_file(headers: HeaderMap, Path(filepath): Path<String>) -> Response {
    serve_stored(&headers, filepath, false).await
}

/// The `/api` registration of the same handler; failures carry `no-store`.
async fn serve_stored_file_api(headers: HeaderMap, Path(filepath): Path<String>) -> Response {
    serve_stored(&headers, filepath, true).await
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

/// `SetUserSpaceThemeV2`: a per-Space override for the caller.
async fn set_user_space_theme(
    headers: HeaderMap,
    Path(space_id): Path<String>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let space_id: i64 = match space_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [spaceId must be a positive integer]"),
    };
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: RevisionRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let revision_id = if !req.revision.is_empty() {
        req.revision.clone()
    } else if !req.package_id.is_empty() {
        match latest_revision_of(&pool, &req.package_id).await {
            Ok(id) => id,
            Err(_) => return error_code(404),
        }
    } else {
        return envelope::error(" [packageId or revision is required]");
    };
    let revision: Option<(String,)> =
        sqlx::query_as("SELECT package_id FROM revision_record_v2 WHERE id = ?")
            .bind(&revision_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    let Some((package_id,)) = revision else {
        return error_code(404);
    };
    match sqlx::query(
        "INSERT INTO user_space_theme_preference_v2 (user_id, space_id, package_id, revision_id, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, space_id) DO UPDATE SET package_id = excluded.package_id, revision_id = excluded.revision_id, updated_at = excluded.updated_at",
    )
    .bind(user.id)
    .bind(space_id)
    .bind(&package_id)
    .bind(&revision_id)
    .bind(now_like_go())
    .execute(&pool)
    .await
    {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SetSpaceThemeV2`: the default theme for a Space, managed by its admin.
async fn set_space_theme(
    headers: HeaderMap,
    Path(space_id): Path<String>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let space_id: i64 = match space_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [spaceId must be a positive integer]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let exists: Option<(i64,)> = sqlx::query_as(r#"SELECT id FROM "space" WHERE id = ?"#)
        .bind(space_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
    if exists.is_none() {
        return error_code(404);
    }
    if !can_manage(&pool, user.id, space_id).await {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: RevisionRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    let revision_id = if !req.revision.is_empty() {
        req.revision
    } else if !req.package_id.is_empty() {
        match latest_revision_of(&pool, &req.package_id).await {
            Ok(id) => id,
            Err(_) => return error_code(404),
        }
    } else {
        return envelope::error(" [packageId or revision is required]");
    };
    let package: Option<(String,)> =
        sqlx::query_as("SELECT package_id FROM revision_record_v2 WHERE id = ?")
            .bind(&revision_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    let Some((package_id,)) = package else {
        return error_code(404);
    };
    match sqlx::query(
        "INSERT INTO space_theme_preference_v2 (space_id, package_id, revision_id, updated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(space_id) DO UPDATE SET package_id = excluded.package_id, revision_id = excluded.revision_id, updated_at = excluded.updated_at",
    )
    .bind(space_id)
    .bind(package_id)
    .bind(revision_id)
    .bind(now_like_go())
    .execute(&pool)
    .await
    {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `ClearSpaceThemeV2`: remove the Space-level default theme selection.
async fn clear_space_theme(headers: HeaderMap, Path(space_id): Path<String>) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let space_id: i64 = match space_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [spaceId must be a positive integer]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let exists: Option<(i64,)> = sqlx::query_as(r#"SELECT id FROM "space" WHERE id = ?"#)
        .bind(space_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
    if exists.is_none() {
        return error_code(404);
    }
    if !can_manage(&pool, user.id, space_id).await {
        return error_code(1005);
    }
    match sqlx::query("DELETE FROM space_theme_preference_v2 WHERE space_id = ?")
        .bind(space_id)
        .execute(&pool)
        .await
    {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `ClearUserSpaceThemeV2`.
async fn clear_user_space_theme(headers: HeaderMap, Path(space_id): Path<String>) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let space_id: i64 = match space_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [spaceId must be a positive integer]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    match sqlx::query(
        "DELETE FROM user_space_theme_preference_v2 WHERE user_id = ? AND space_id = ?",
    )
    .bind(user.id)
    .bind(space_id)
    .execute(pool)
    .await
    {
        Ok(_) => success(),
        Err(_) => envelope::database_error(),
    }
}

/// `SpaceRouter.Copy`: a new shared Space holding copies of every group and item.
async fn copy_space(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_access_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or(false)
    {
        return error_code(1005);
    }
    let source_name = match crate::repository::space_name(&pool, ctx.sqlx_driver(), space_id).await
    {
        Ok(Some(name)) => name,
        Ok(None) | Err(_) => return error_code(404),
    };
    let body = axum::body::to_bytes(request.into_body(), 1 << 20)
        .await
        .unwrap_or_default();
    let req: RenameRequest = serde_json::from_slice(&body).unwrap_or(RenameRequest {
        name: String::new(),
    });
    let name = if req.name.is_empty() {
        format!("{source_name} Copy")
    } else {
        req.name
    };
    match crate::repository::copy_space(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        user.id,
        &name,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(_) => envelope::database_error(),
    }
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
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
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

/// `POST /api/theme/v2/wallpaper/web`: a self-contained web wallpaper archive.
async fn upload_web_wallpaper(headers: HeaderMap, request: Request) -> Response {
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let exists: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM web_wallpaper_record WHERE id = ?")
        .bind(&id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    if exists.is_none() {
        if sqlx::query(
            "INSERT INTO web_wallpaper_record (id, owner_id, html, poster, poster_type, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(user.id)
        .bind(&html)
        .bind(&poster)
        .bind(&poster_type)
        .bind(now_like_go())
        .execute(pool)
        .await
        .is_err()
        {
            return envelope::database_error();
        }
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
