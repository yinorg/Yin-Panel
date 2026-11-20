//! The first ported Core routes: the ones that need only configuration, the
//! session and one user row. Each mirrors the Go handler in
//! `backend/internal/web/router/system`.

use axum::Router;
use axum::extract::{Multipart, Path, Query, Request};
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
        .route("/api/spaces/{space_id}/public", get(space_public))
        .route("/api/system/moduleConfig/getByName", get(module_config))
        .route("/api/file/getList", get(file_list))
        .route("/api/file/uploadImg", post(upload_image))
        .route(
            "/api/panel/itemIcon/getSiteFavicon",
            post(get_site_favicon),
        )
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
        .route("/api/system/monitor/getEnableStatus", post(monitor_enable_status))
        .route("/api/system/monitor/getSnapshot", post(monitor_disabled))
        .route("/api/system/monitor/getDiskMountpoints", post(monitor_disabled))
        .route("/api/system/monitor/getDiskStateByPath", post(monitor_disabled))
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
        .route(
            "/api/theme/v2/revisions/{revision}",
            get(theme_revision),
        )
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
            get(theme_grant).post(set_theme_grant).delete(revoke_theme_grant),
        )
        .route(
            "/api/theme/v2/wallpaper/web/{id}/{name}",
            get(web_wallpaper_asset),
        )
        .route("/api/theme/v2/space/{space_id}", get(theme_space))
        .route("/api/theme/v2/current", get(theme_current))
        .route("/api/theme/v2/packages", get(theme_packages))
        .route("/api/theme/v2/package/{package_id}", get(theme_package))
        .route("/api/theme/v2/mine", get(theme_mine))
        .route("/api/theme/v2/effective", get(theme_effective))
        .route(
            "/api/theme/v2/assets/{revision}/{*name}",
            get(theme_asset),
        )
        .route("/api/system/moduleConfig/save", post(module_config_save))
        .route("/api/panel/userConfig/setConfig", post(user_config_save))
        .route(
            "/api/spaces/{space_id}/groups",
            post(create_group),
        )
        .route(
            "/api/spaces/{space_id}/groups/{group_id}",
            post(update_group).delete(delete_group).put(update_group),
        )
        .route(
            "/api/spaces/{space_id}/members",
            post(add_member),
        )
        .route(
            "/api/spaces/{space_id}/members/{user_id}",
            axum::routing::put(update_member).delete(remove_member),
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
            axum::routing::delete(delete_space).post(rename_space),
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let user: Option<PasswordUser> = sqlx::query_as(
        r#"SELECT id, name, role, mail, status, token_version FROM "user" WHERE mail = ? AND password = ?"#,
    )
    .bind(&mail)
    .bind(password_encryption(&params.password))
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
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

#[derive(sqlx::FromRow)]
struct PasswordUser {
    id: i64,
    name: String,
    role: i64,
    mail: String,
    status: i64,
    token_version: i64,
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let result = sqlx::query(r#"UPDATE "user" SET token_version = token_version + 1 WHERE id = ?"#)
        .bind(user.id)
        .execute(pool)
        .await;
    if result.is_err() {
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let rows: Vec<SpaceRow> = match sqlx::query_as(
        r#"SELECT id, created_at, updated_at, type, name, owner_user_id, pair_id, side,
                  public_enabled, public_id, public_mode
           FROM "space"
           WHERE (owner_user_id = ? OR id IN (SELECT space_id FROM space_member WHERE user_id = ?))
             AND side = 'yin'
           ORDER BY type, created_at"#,
    )
    .bind(user.id)
    .bind(user.id)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };

    // `List` resolves the paired space and the caller's role in two batched
    // queries rather than two per space, which matters once an account owns or
    // joins many Spaces.
    let mut paired_by_id: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut role_by_id: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    if !rows.is_empty() {
        let placeholders = vec!["?"; rows.len()].join(",");
        let paired_sql = format!(
            r#"SELECT id, pair_id FROM "space" WHERE pair_id IN ({placeholders}) AND side = 'yang'"#
        );
        let mut query = sqlx::query_as::<_, (i64, i64)>(&paired_sql);
        for row in &rows {
            query = query.bind(row.id);
        }
        if let Ok(paired) = query.fetch_all(pool).await {
            for (id, pair_id) in paired {
                paired_by_id.insert(pair_id, id);
            }
        }
        let member_sql = format!(
            "SELECT space_id, role FROM space_member WHERE user_id = ? AND space_id IN ({placeholders})"
        );
        let mut query = sqlx::query_as::<_, (i64, String)>(&member_sql).bind(user.id);
        for row in &rows {
            query = query.bind(row.id);
        }
        if let Ok(members) = query.fetch_all(pool).await {
            for (space_id, role) in members {
                role_by_id.insert(space_id, role);
            }
        }
    }

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let role = match role_by_id.get(&row.id) {
            Some(role) => role.clone(),
            None => String::new(),
        };
        // The owner is always an administrator, whatever the member row says.
        let can_edit = row.owner_user_id == user.id
            || role == "admin"
            || role == "editor";
        items.push(SpaceItem {
            paired_space_id: paired_by_id.get(&row.id).copied(),
            can_edit,
            id: row.id,
            create_time: rfc3339(&row.created_at),
            update_time: rfc3339(&row.updated_at),
            kind: row.kind,
            name: row.name,
            owner_user_id: row.owner_user_id,
            pair_id: row.pair_id,
            side: row.side,
            public_enabled: row.public_enabled != 0,
            public_id: row.public_id,
            public_mode: row.public_mode,
        });
    }
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let found: Option<(i64, String)> =
        sqlx::query_as(r#"SELECT owner_user_id, search_config_json FROM "space" WHERE id = ?"#)
            .bind(space_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    let Some((owner, config)) = found else {
        return error_code(404);
    };
    let member: Option<(String,)> = sqlx::query_as(
        r#"SELECT role FROM space_member WHERE space_id = ? AND user_id = ? LIMIT 1"#,
    )
    .bind(space_id)
    .bind(user.id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    if owner != user.id && member.is_none() {
        return error_code(1005);
    }
    if config.is_empty() {
        return success();
    }
    match serde_json::from_str::<serde_json::Value>(&config) {
        Ok(value) => success_data(value),
        Err(error) => envelope::error(&error.to_string()),
    }
}

#[derive(sqlx::FromRow)]
struct SpaceRow {
    id: i64,
    created_at: String,
    updated_at: String,
    #[sqlx(rename = "type")]
    kind: String,
    name: String,
    owner_user_id: i64,
    pair_id: i64,
    side: String,
    public_enabled: i64,
    public_id: Option<String>,
    public_mode: String,
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

/// `canAccessSpace`: the owner or any member.
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

async fn guard(headers: &HeaderMap, space_id: i64) -> Result<sqlx::SqlitePool, Response> {
    let user = match auth::user(headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return Err(error_code(1000)),
        Err(AuthError::Disabled) => return Err(error_code(1004)),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return Err(envelope::error(&error)),
    };
    if !can_access(&pool, user.id, space_id).await {
        return Err(error_code(1005));
    }
    Ok(pool)
}

/// `SpaceRouter.Groups`: one grouped count for the whole Space.
async fn groups(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let rows: Vec<GroupRow> = match sqlx::query_as(
        r#"SELECT id, created_at, updated_at, icon, title, description, sort, user_id, space_id,
                  parent_id
           FROM item_icon_group WHERE space_id = ? ORDER BY sort, created_at"#,
    )
    .bind(space_id)
    .fetch_all(&pool)
    .await
    {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let counts: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT item_icon_group_id, COUNT(*) FROM item_icon WHERE space_id = ? GROUP BY item_icon_group_id",
    )
    .bind(space_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let count = counts
            .iter()
            .find(|(group, _)| *group == row.id)
            .map(|(_, total)| *total)
            .unwrap_or(0);
        items.push(GroupItem {
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
            item_count: count,
        });
    }
    envelope::success_struct(items)
}

/// `SpaceRouter.Items`: `sort, created_at, id` order, optional group filter and
/// the 200-row page cap.
async fn items(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let pool = match guard(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let mut sql = String::from(
        r#"SELECT id, created_at, updated_at, icon_json, title, url, lan_url, mobile_url,
                  description, open_method, sort, item_icon_group_id, user_id, space_id
           FROM item_icon WHERE space_id = ?"#,
    );
    let group = query.get("groupId").cloned().unwrap_or_default();
    if !group.is_empty() {
        sql.push_str(" AND item_icon_group_id = ?");
    }
    sql.push_str(" ORDER BY sort, created_at, id");
    let mut binder = sqlx::query_as::<_, ItemRow>(&sql).bind(space_id);
    if !group.is_empty() {
        binder = binder.bind(group.parse::<i64>().unwrap_or(-1));
    }
    let page: i64 = query
        .get("page")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    if page > 0 {
        let mut size: i64 = query
            .get("pageSize")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        if size < 1 {
            size = 50;
        }
        if size > 200 {
            size = 200;
        }
        sql.clear();
        sql.push_str(
            r#"SELECT id, created_at, updated_at, icon_json, title, url, lan_url, mobile_url,
                      description, open_method, sort, item_icon_group_id, user_id, space_id
               FROM item_icon WHERE space_id = ?"#,
        );
        if !group.is_empty() {
            sql.push_str(" AND item_icon_group_id = ?");
        }
        sql.push_str(" ORDER BY sort, created_at, id LIMIT ? OFFSET ?");
        let mut paged = sqlx::query_as::<_, ItemRow>(&sql).bind(space_id);
        if !group.is_empty() {
            paged = paged.bind(group.parse::<i64>().unwrap_or(-1));
        }
        paged = paged.bind(size).bind((page - 1) * size);
        binder = paged;
    }
    let rows = match binder.fetch_all(&pool).await {
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
    let pool = match guard(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let rows: Vec<MemberRow> = match sqlx::query_as(
        r#"SELECT id, space_id, user_id, role, source, joined_at
           FROM space_member WHERE space_id = ? ORDER BY id"#,
    )
    .bind(space_id)
    .fetch_all(&pool)
    .await
    {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let account: Option<(String, String)> =
            sqlx::query_as(r#"SELECT mail, name FROM "user" WHERE id = ?"#)
                .bind(row.user_id)
                .fetch_optional(&pool)
                .await
                .unwrap_or(None);
        let (email, name) = account.unwrap_or_default();
        out.push(json!({
            "email": email,
            "id": row.id,
            "joinedAt": rfc3339(&row.joined_at),
            "name": name,
            "role": row.role,
            "source": row.source,
            "spaceId": row.space_id,
            "userId": row.user_id,
        }));
    }
    success_data(serde_json::Value::Array(out))
}

#[derive(sqlx::FromRow)]
struct GroupRow {
    id: i64,
    created_at: String,
    updated_at: String,
    icon: String,
    title: String,
    description: String,
    sort: i64,
    user_id: i64,
    space_id: i64,
    parent_id: Option<i64>,
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

#[derive(sqlx::FromRow)]
struct MemberRow {
    id: i64,
    space_id: i64,
    user_id: i64,
    role: String,
    source: String,
    joined_at: String,
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !can_manage(&pool, user.id, space_id).await {
        return error_code(1005);
    }
    let row: Option<(i64, Option<String>, String)> = sqlx::query_as(
        r#"SELECT public_enabled, public_id, public_mode FROM "space" WHERE id = ?"#,
    )
    .bind(space_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);
    let Some((enabled, public_id, mode)) = row else {
        return error_code(404);
    };
    success_data(json!({
        "enabled": enabled != 0,
        "mode": mode,
        "publicId": public_id,
    }))
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row: Option<(String,)> = sqlx::query_as(
        r#"SELECT value_json FROM module_config WHERE user_id = ? AND name = ? LIMIT 1"#,
    )
    .bind(user.id)
    .bind(&name)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    match row {
        None => (axum::http::StatusCode::OK, envelope::headers(), envelope::body(0, "OK", Some(serde_json::Value::Null))).into_response(),
        Some((value_json,)) => {
            let value: serde_json::Value =
                serde_json::from_str(&value_json).unwrap_or(serde_json::Value::Null);
            (axum::http::StatusCode::OK, envelope::headers(), envelope::body(0, "OK", Some(value))).into_response()
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM file")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));
    let limit: i64 = query.get("limit").and_then(|value| value.parse().ok()).unwrap_or(0);
    let rows: Vec<FileRow> = if limit > 0 {
        let page: i64 = query.get("page").and_then(|value| value.parse().ok()).unwrap_or(1).max(1);
        let limit = limit.min(200);
        sqlx::query_as(
            "SELECT id, created_at, updated_at, file_name FROM file ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind((page - 1) * limit)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as("SELECT id, created_at, updated_at, file_name FROM file ORDER BY created_at DESC")
            .fetch_all(pool)
            .await
            .unwrap_or_default()
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
    success_data(json!({"count": total.0, "list": list}))
}

#[derive(sqlx::FromRow)]
struct FileRow {
    id: i64,
    created_at: String,
    updated_at: String,
    file_name: String,
}

/// `SpaceRouter.BookmarkTree`: groups ordered `sort, id`, items ordered
/// `sort, id`, folded into a tree capped at `bookmarkTreeMaxDepth`.
async fn bookmarks_tree(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let groups: Vec<(i64, String, String, Option<i64>)> = match sqlx::query_as(
        r#"SELECT id, title, icon, parent_id FROM item_icon_group WHERE space_id = ? ORDER BY sort, id"#,
    )
    .bind(space_id)
    .fetch_all(&pool)
    .await
    {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let items: Vec<(i64, String, String)> = match sqlx::query_as(
        "SELECT item_icon_group_id, title, url FROM item_icon WHERE space_id = ? ORDER BY sort, id",
    )
    .bind(space_id)
    .fetch_all(&pool)
    .await
    {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let total: (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM "user""#)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));
    let limit: i64 = query.get("limit").and_then(|value| value.parse().ok()).unwrap_or(0);
    let page: i64 = query.get("page").and_then(|value| value.parse().ok()).unwrap_or(0);
    let rows: Vec<UserRowOut> = if limit > 0 {
        let offset = limit * (page.max(1) - 1);
        sqlx::query_as(
            r#"SELECT id, created_at, updated_at, name, head_image, status, role, mail,
                      token_version FROM "user" ORDER BY id LIMIT ? OFFSET ?"#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        Vec::new()
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
    success_data(json!({"count": total.0, "list": list}))
}

#[derive(sqlx::FromRow)]
struct UserRowOut {
    id: i64,
    created_at: String,
    updated_at: String,
    name: String,
    head_image: String,
    status: i64,
    role: i64,
    mail: String,
    token_version: i64,
}

/// `UserConfigRouter.GetConfig`: a missing row is `404 Data not found`.
async fn user_config(headers: HeaderMap) -> Response {
    let user = match auth::user(&headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return error_code(1000),
        Err(AuthError::Disabled) => return error_code(1004),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row: Option<(String, String)> =
        sqlx::query_as(r#"SELECT panel_json, language FROM user_config WHERE user_id = ?"#)
            .bind(user.id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    let Some((panel_json, language)) = row else {
        return error_code(404);
    };
    let panel: serde_json::Value = if panel_json.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(&panel_json).unwrap_or(serde_json::Value::Null)
    };
    success_data(json!({"panel": panel, "language": language}))
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

/// `MonitorRouter.GetSnapshot` and the disk readers while the monitor is off.
async fn monitor_disabled(headers: HeaderMap) -> Response {
    if let Err(response) = require_user(&headers).await {
        return response;
    }
    if !appcfg::ctx().conf.base.enable_monitor {
        return envelope::error("system monitor is disabled");
    }
    // The Go service answers from a snapshot refreshed every three seconds; the
    // port collects one per request, which is slower but always current.
    let snapshot = crate::monitor::snapshot().await;
    if snapshot.is_empty() {
        return envelope::error("failed");
    }
    success_data(serde_json::Value::Object(snapshot.into_iter().collect()))
}

/// `ThemeRouter.Preference`: the stored mode, or `auto` / `follow-space`.
async fn theme_preference(headers: HeaderMap) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT mode, theme_mode FROM user_theme_preference_v2 WHERE user_id = ?",
    )
    .bind(user.id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    let (mode, theme_mode) = row.unwrap_or_else(|| ("auto".to_string(), "follow-space".to_string()));
    let mode = if mode.is_empty() { "auto".to_string() } else { mode };
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
    let theme = space_theme.map(|(package_id, revision)| json!({"packageId": package_id, "revision": revision}));
    let override_value = override_theme.map(|(package_id, revision)| json!({"packageId": package_id, "revision": revision}));
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
async fn revision_public(pool: &sqlx::SqlitePool, revision_id: &str) -> Result<serde_json::Value, String> {
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
    let mut manifest: serde_json::Value =
        serde_json::from_str(&manifest_json).map_err(|error| error.to_string())?;
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
        serde_json::from_str(&tokens_json).map_err(|error| error.to_string())?;
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
            || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'+' | b'$' | b'&' | b'!' | b'*' | b'\'' | b'(' | b')' | b',' | b';' | b'=' | b':' | b'@');
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

async fn activation_revision(pool: &sqlx::SqlitePool, scope: &str) -> Option<String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT active_revision_id FROM activation_record_v2 WHERE scope = ?",
    )
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
    let revision = match instance_default_revision(pool).await {
        Ok(revision) => revision,
        Err(error) => return envelope::error(&error),
    };
    match revision_public(pool, &revision).await {
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
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT mode, theme_mode FROM user_theme_preference_v2 WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    let (mode, theme_mode) = row.unwrap_or_else(|| ("auto".to_string(), "follow-space".to_string()));
    (
        if mode.is_empty() { "auto".into() } else { mode },
        if theme_mode.is_empty() { "follow-space".into() } else { theme_mode },
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
            let row: Option<(String,)> =
                sqlx::query_as("SELECT revision_id FROM space_theme_preference_v2 WHERE space_id = ?")
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
        Some(format!("\"{}\"", digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>()))
    } else {
        None
    };

    let mut response = Response::new(axum::body::Body::empty());
    {
        let out = response.headers_mut();
        for (key, value) in envelope::headers() {
            out.insert(key, value);
        }
        out.insert(header::CONTENT_TYPE, HeaderValue::from_str(&media_type).unwrap_or(HeaderValue::from_static("application/octet-stream")));
        out.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
        let cache = if is_html {
            "public, max-age=0, must-revalidate, no-transform"
        } else {
            "public, max-age=31536000, immutable, no-transform"
        };
        out.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
        if let Some(etag) = &etag {
            out.insert(header::ETAG, HeaderValue::from_str(etag).unwrap_or(HeaderValue::from_static("")));
        }
        if is_html {
            out.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline' https://static.cloudflareinsights.com; style-src 'unsafe-inline'; img-src data:; connect-src https://cloudflareinsights.com; font-src 'none'; media-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'"));
            out.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
        } else if media_type == "image/svg+xml" {
            out.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox; default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; object-src 'none'; form-action 'none'; base-uri 'none'"));
            out.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
        }
    }

    // `http.ServeContent`: a matching If-None-Match turns the response into a 304.
    if let (Some(etag), Some(if_none_match)) = (&etag, headers.get(header::IF_NONE_MATCH)) {
        if if_none_match.to_str().map(str::trim).ok() == Some(etag.as_str()) {
            *response.status_mut() = axum::http::StatusCode::NOT_MODIFIED;
            return response;
        }
    }

    let total = content.len() as u64;
    if let Some(range) = headers.get(header::RANGE).and_then(|value| value.to_str().ok()) {
        if let Some((start, end)) = parse_range(range, total) {
            let slice = content[start as usize..=end as usize].to_vec();
            *response.status_mut() = axum::http::StatusCode::PARTIAL_CONTENT;
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {start}-{end}/{total}"))
                    .unwrap_or(HeaderValue::from_static("")),
            );
            *response.body_mut() = axum::body::Body::from(slice);
            return response;
        }
        if range.starts_with("bytes=") {
            *response.status_mut() = axum::http::StatusCode::RANGE_NOT_SATISFIABLE;
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{total}")).unwrap_or(HeaderValue::from_static("")),
            );
            return response;
        }
    }

    *response.body_mut() = axum::body::Body::from(content);
    response
}

/// A single `bytes=start-end|start-|-suffix` range, as `http.ServeContent`
/// accepts it.
fn parse_range(value: &str, total: u64) -> Option<(u64, u64)> {
    let spec = value.strip_prefix("bytes=")?;
    if spec.contains(',') || total == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    if start.is_empty() {
        let suffix: u64 = end.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        let length = suffix.min(total);
        return Some((total - length, total - 1));
    }
    let start: u64 = start.parse().ok()?;
    if start >= total {
        return None;
    }
    let end = if end.is_empty() {
        total - 1
    } else {
        end.parse::<u64>().ok()?.min(total - 1)
    };
    if end < start {
        return None;
    }
    Some((start, end))
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
    let value = payload.get("value").cloned().unwrap_or(serde_json::Value::Null);
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let now = now_like_go();
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM module_config WHERE user_id = ? AND name = ? LIMIT 1",
    )
    .bind(user.id)
    .bind(&name)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    let result = match existing {
        Some((id,)) => {
            sqlx::query("UPDATE module_config SET value_json = ?, updated_at = ? WHERE id = ?")
                .bind(&stored)
                .bind(&now)
                .bind(id)
                .execute(pool)
                .await
        }
        None => {
            sqlx::query(
                "INSERT INTO module_config (created_at, updated_at, user_id, name, value_json) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&now)
            .bind(&now)
            .bind(user.id)
            .bind(&name)
            .bind(&stored)
            .execute(pool)
            .await
        }
    };
    match result {
        Ok(_) => success(),
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
        Err(error) => return envelope::error(&format!(" [{error}]")),
    };
    let language = payload
        .get("language")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    if !language.is_empty() && !supported_language(&language) {
        return envelope::error(" [invalid language]");
    }
    let panel = payload.get("panel").cloned().filter(|value| value.is_object());
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let existing: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM user_config WHERE user_id = ? LIMIT 1")
            .bind(user.id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    let result = match existing {
        Some(_) => match &panel {
            Some(panel) => {
                sqlx::query("UPDATE user_config SET panel_json = ?, language = ? WHERE user_id = ?")
                    .bind(panel.to_string())
                    .bind(&language)
                    .bind(user.id)
                    .execute(pool)
                    .await
            }
            None => {
                sqlx::query("UPDATE user_config SET language = ? WHERE user_id = ?")
                    .bind(&language)
                    .bind(user.id)
                    .execute(pool)
                    .await
            }
        },
        None => {
            sqlx::query(
                "INSERT INTO user_config (user_id, panel_json, language) VALUES (?, ?, ?)",
            )
            .bind(user.id)
            .bind(panel.as_ref().map(|value| value.to_string()).unwrap_or_default())
            .bind(&language)
            .execute(pool)
            .await
        }
    };
    match result {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

fn supported_language(language: &str) -> bool {
    matches!(
        language,
        "auto" | "en-US" | "zh-CN" | "zh-TW" | "ja-JP" | "ko-KR" | "de-DE" | "fr-FR"
            | "es-ES" | "pt-BR" | "ru-RU" | "id-ID" | "tr-TR" | "pl-PL" | "it-IT"
            | "nl-NL" | "hi-IN" | "th-TH"
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
    matches!(space_role(pool, user_id, space_id).await.as_str(), "admin" | "editor")
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
async fn create_group(
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
        let exists: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM item_icon_group WHERE id = ? AND space_id = ?",
        )
        .bind(parent)
        .bind(space_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
        if exists.is_none() {
            return envelope::error(" [invalid parent group]");
        }
    }
    let now = now_like_go();
    let result = sqlx::query(
        "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES (?, ?, ?, ?, '', 9999, ?, ?, ?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(&req.icon)
    .bind(&req.title)
    .bind(user.id)
    .bind(space_id)
    .bind(req.parent_id)
    .execute(&pool)
    .await;
    let id = match result {
        Ok(result) => result.last_insert_rowid(),
        Err(error) => return envelope::error(&error.to_string()),
    };
    let group = match load_group(&pool, space_id, id).await {
        Some(group) => group,
        None => return envelope::error("group not found"),
    };
    success_data(group)
}

async fn load_group(pool: &sqlx::SqlitePool, space_id: i64, id: i64) -> Option<serde_json::Value> {
    let row = sqlx::query_as::<_, GroupRow>(
        r#"SELECT id, created_at, updated_at, icon, title, description, sort, user_id, space_id,
                  parent_id FROM item_icon_group WHERE id = ? AND space_id = ?"#,
    )
    .bind(id)
    .bind(space_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)?;
    Some(serde_json::to_value(GroupItem {
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
        item_count: 0,
    })
    .unwrap_or(serde_json::Value::Null))
}

async fn group_belongs(pool: &sqlx::SqlitePool, group_id: i64, space_id: i64) -> bool {
    sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM item_icon_group WHERE id = ? AND space_id = ?",
    )
    .bind(group_id)
    .bind(space_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)
    .is_some()
}

/// `groupHasDescendant`: walking up from the candidate must not reach the group.
async fn group_has_descendant(pool: &sqlx::SqlitePool, space_id: i64, group_id: i64, candidate: i64) -> bool {
    let mut current = candidate;
    let mut hops = 0;
    while current != 0 && hops < 256 {
        if current == group_id {
            return true;
        }
        let parent: Option<(Option<i64>,)> = sqlx::query_as(
            "SELECT parent_id FROM item_icon_group WHERE id = ? AND space_id = ?",
        )
        .bind(current)
        .bind(space_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
        match parent.and_then(|value| value.0) {
            Some(next) => current = next,
            None => return false,
        }
        hops += 1;
    }
    false
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
    let req: GroupRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{error}]")),
    };
    if req.title.is_empty() {
        return envelope::error(" [ Title为必填字段]");
    }
    if let Some(parent) = req.parent_id {
        if parent == group_id || !group_belongs(&pool, parent, space_id).await {
            return envelope::error(" [invalid parent group]");
        }
        if group_has_descendant(&pool, space_id, group_id, parent).await {
            return envelope::error(" [group cycle detected]");
        }
    }
    let result = sqlx::query(
        "UPDATE item_icon_group SET title = ?, icon = ?, parent_id = ?, updated_at = ? WHERE id = ? AND space_id = ?",
    )
    .bind(&req.title)
    .bind(&req.icon)
    .bind(req.parent_id)
    .bind(now_like_go())
    .bind(group_id)
    .bind(space_id)
    .execute(&pool)
    .await;
    match result {
        Ok(_) => success(),
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !can_edit(&pool, user.id, space_id).await {
        return error_code(1005);
    }
    let mut ids = vec![group_id];
    let mut index = 0;
    while index < ids.len() {
        let children: Vec<(i64,)> = sqlx::query_as(
            "SELECT id FROM item_icon_group WHERE space_id = ? AND parent_id = ?",
        )
        .bind(space_id)
        .bind(ids[index])
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        for (child,) in children {
            ids.push(child);
        }
        index += 1;
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let delete_items = format!(
        "DELETE FROM item_icon WHERE space_id = ? AND item_icon_group_id IN ({placeholders})"
    );
    let mut statement = sqlx::query(&delete_items).bind(space_id);
    for id in &ids {
        statement = statement.bind(id);
    }
    if statement.execute(&mut *tx).await.is_err() {
        return envelope::error("delete failed");
    }
    let delete_groups = format!(
        "DELETE FROM item_icon_group WHERE space_id = ? AND id IN ({placeholders})"
    );
    let mut statement = sqlx::query(&delete_groups).bind(space_id);
    for id in &ids {
        statement = statement.bind(id);
    }
    if statement.execute(&mut *tx).await.is_err() {
        return envelope::error("delete failed");
    }
    match tx.commit().await {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
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
        text = trimmed.chars().find(|ch| !ch.is_whitespace()).map(String::from).unwrap_or_default();
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
async fn create_item(
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
    let group: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM item_icon_group WHERE id = ? AND space_id = ?",
    )
    .bind(input.item_icon_group_id)
    .bind(space_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);
    if group.is_none() {
        return error_code(404);
    }
    let now = now_like_go();
    let icon_json = serde_json::to_string(&input.icon).unwrap_or_else(|_| "{}".to_string());
    let result = sqlx::query(
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
    .execute(&pool)
    .await;
    let id = match result {
        Ok(result) => result.last_insert_rowid(),
        Err(error) => return envelope::error(&error.to_string()),
    };
    let row = match load_item(&pool, space_id, id).await {
        Some(row) => row,
        None => return error_code(404),
    };
    success_data(row)
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
    Some(serde_json::to_value(ItemOut {
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
    .unwrap_or(serde_json::Value::Null))
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
    let group: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM item_icon_group WHERE id = ? AND space_id = ?",
    )
    .bind(input.item_icon_group_id)
    .bind(space_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);
    if group.is_none() {
        return error_code(404);
    }
    let icon_json = serde_json::to_string(&input.icon).unwrap_or_else(|_| "{}".to_string());
    let result = sqlx::query(
        "UPDATE item_icon SET icon_json = ?, title = ?, url = ?, lan_url = ?, mobile_url = ?, description = ?, open_method = ?, sort = ?, item_icon_group_id = ?, updated_at = ? WHERE id = ? AND space_id = ?",
    )
    .bind(&icon_json)
    .bind(&input.title)
    .bind(&input.url)
    .bind(&input.lan_url)
    .bind(&input.mobile_url)
    .bind(&input.description)
    .bind(input.open_method)
    .bind(input.sort)
    .bind(input.item_icon_group_id)
    .bind(now_like_go())
    .bind(item_id)
    .bind(space_id)
    .execute(&pool)
    .await;
    match result {
        Ok(_) => success(),
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !can_edit(&pool, user.id, space_id).await {
        return error_code(1005);
    }
    match sqlx::query("DELETE FROM item_icon WHERE id = ? AND space_id = ?")
        .bind(item_id)
        .bind(space_id)
        .execute(&pool)
        .await
    {
        Ok(_) => success(),
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
    let yang: Option<(i64,)> = sqlx::query_as(
        r#"SELECT id FROM "space" WHERE pair_id = ? AND side = 'yang' LIMIT 1"#,
    )
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
async fn add_member(
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
    let req: MemberAddRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid member request]"),
    };
    if !valid_space_role(&req.role) {
        return envelope::error(" [invalid member request]");
    }
    let email = req.email.trim().to_lowercase();
    let target: Option<(i64,)> =
        sqlx::query_as(r#"SELECT id FROM "user" WHERE mail = ?"#)
            .bind(&email)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    let Some((target_id,)) = target else {
        return error_code(404);
    };
    let exists: Option<(i64,)> = sqlx::query_as(r#"SELECT id FROM "space" WHERE id = ?"#)
        .bind(space_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
    if exists.is_none() {
        return error_code(404);
    }
    let now = now_like_go();
    for sid in paired_ids(&pool, space_id).await {
        let found: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM space_member WHERE space_id = ? AND user_id = ? LIMIT 1",
        )
        .bind(sid)
        .bind(target_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
        let result = match found {
            Some((id,)) => {
                sqlx::query("UPDATE space_member SET role = ? WHERE id = ?")
                    .bind(&req.role)
                    .bind(id)
                    .execute(&pool)
                    .await
            }
            None => {
                sqlx::query(
                    "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES (?, ?, ?, ?, ?, 'manual', ?)",
                )
                .bind(&now)
                .bind(&now)
                .bind(sid)
                .bind(target_id)
                .bind(&req.role)
                .bind(&now)
                .execute(&pool)
                .await
            }
        };
        if let Err(error) = result {
            return envelope::error(&error.to_string());
        }
    }
    success()
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
    let req: MemberRoleRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid role]"),
    };
    if !valid_space_role(&req.role) {
        return envelope::error(" [invalid role]");
    }
    let exists: Option<(i64,)> = sqlx::query_as(r#"SELECT id FROM "space" WHERE id = ?"#)
        .bind(space_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
    if exists.is_none() {
        return error_code(404);
    }
    let placeholders = {
        let ids = paired_ids(&pool, space_id).await;
        vec!["?"; ids.len()].join(",")
    };
    let sql = format!(
        "UPDATE space_member SET role = ? WHERE space_id IN ({placeholders}) AND user_id = ?"
    );
    let mut statement = sqlx::query(&sql).bind(&req.role);
    for sid in paired_ids(&pool, space_id).await {
        statement = statement.bind(sid);
    }
    match statement.bind(target).execute(&pool).await {
        Ok(_) => success(),
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !can_manage(&pool, user.id, space_id).await {
        return error_code(1005);
    }
    let space: Option<(i64,)> = sqlx::query_as(r#"SELECT owner_user_id FROM "space" WHERE id = ?"#)
        .bind(space_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
    let Some((owner,)) = space else {
        return error_code(404);
    };
    if target == owner {
        return envelope::error("the space owner cannot be removed; leave the space to delete it");
    }
    let placeholders = {
        let ids = paired_ids(&pool, space_id).await;
        vec!["?"; ids.len()].join(",")
    };
    let sql = format!("DELETE FROM space_member WHERE space_id IN ({placeholders}) AND user_id = ?");
    let mut statement = sqlx::query(&sql);
    for sid in paired_ids(&pool, space_id).await {
        statement = statement.bind(sid);
    }
    match statement.bind(target).execute(&pool).await {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

#[derive(serde::Deserialize)]
struct RenameRequest {
    #[serde(default)]
    name: String,
}

/// `SpaceRouter.Rename`: the Yang twin becomes `<name>-B`.
async fn rename_space(
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
    let req: RenameRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid name]"),
    };
    if req.name.is_empty() {
        return envelope::error(" [ Name为必填字段]");
    }
    if req.name.chars().count() > 100 {
        return envelope::error(" [ Name长度不能超过100个字符]");
    }
    let exists: Option<(i64,)> = sqlx::query_as(r#"SELECT id FROM "space" WHERE id = ?"#)
        .bind(space_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
    if exists.is_none() {
        return error_code(404);
    }
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    if sqlx::query(r#"UPDATE "space" SET name = ?, updated_at = ? WHERE id = ?"#)
        .bind(&req.name)
        .bind(now_like_go())
        .bind(space_id)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        return envelope::error("rename failed");
    }
    for sid in paired_ids(&pool, space_id).await {
        if sid == space_id {
            continue;
        }
        if sqlx::query(r#"UPDATE "space" SET name = ?, updated_at = ? WHERE id = ?"#)
            .bind(format!("{}-B", req.name))
            .bind(now_like_go())
            .bind(sid)
            .execute(&mut *tx)
            .await
            .is_err()
        {
            return envelope::error("rename failed");
        }
    }
    match tx.commit().await {
        Ok(_) => success(),
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
    match sqlx::query(r#"UPDATE "space" SET search_config_json = ?, updated_at = ? WHERE id = ?"#)
        .bind(value.to_string())
        .bind(now_like_go())
        .bind(space_id)
        .execute(&pool)
        .await
    {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.GetWallpaperConfig`.
async fn get_wallpaper(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let row: Option<(String,)> =
        sqlx::query_as(r#"SELECT wallpaper_json FROM "space" WHERE id = ?"#)
            .bind(space_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    let Some((stored,)) = row else {
        return error_code(404);
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
    #[serde(default, rename = "libraryEntry", skip_serializing_if = "String::is_empty")]
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
    match sqlx::query(r#"UPDATE "space" SET wallpaper_json = ?, updated_at = ? WHERE id = ?"#)
        .bind(&stored)
        .bind(now_like_go())
        .bind(space_id)
        .execute(&pool)
        .await
    {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.OIDCGroups` (the list the routes expose alongside add/delete).
async fn list_oidc_groups(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let rows: Vec<(i64, String, String, String, String, String)> = match sqlx::query_as(
        "SELECT id, created_at, updated_at, provider, group_name, role FROM space_o_id_c_group WHERE space_id = ? ORDER BY id",
    )
    .bind(space_id)
    .fetch_all(&pool)
    .await
    {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.0,
                "createTime": rfc3339(&row.1),
                "updateTime": rfc3339(&row.2),
                "spaceId": space_id,
                "provider": row.3,
                "groupName": row.4,
                "role": row.5,
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
    let req: TransferRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid userId]"),
    };
    let email = req.email.trim().to_lowercase();
    let target: Option<(i64,)> = sqlx::query_as(r#"SELECT id FROM "user" WHERE mail = ?"#)
        .bind(&email)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
    let Some((target_id,)) = target else {
        return error_code(404);
    };
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let now = now_like_go();
    let steps = [
        sqlx::query(r#"UPDATE "space" SET owner_user_id = ?, updated_at = ? WHERE id = ?"#)
            .bind(target_id)
            .bind(&now)
            .bind(space_id)
            .execute(&mut *tx),
    ];
    for step in steps {
        if step.await.is_err() {
            return envelope::error("transfer failed");
        }
    }
    if sqlx::query(
        "UPDATE space_member SET role = 'editor' WHERE space_id = ? AND user_id = ?",
    )
    .bind(space_id)
    .bind(user.id)
    .execute(&mut *tx)
    .await
    .is_err()
    {
        return envelope::error("transfer failed");
    }
    // The target may not have a row yet; make one, or promote the existing one.
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM space_member WHERE space_id = ? AND user_id = ? LIMIT 1",
    )
    .bind(space_id)
    .bind(target_id)
    .fetch_optional(&mut *tx)
    .await
    .unwrap_or(None);
    let result = match existing {
        Some((id,)) => {
            sqlx::query("UPDATE space_member SET role = 'admin' WHERE id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await
        }
        None => {
            sqlx::query(
                "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES (?, ?, ?, ?, 'admin', 'manual', ?)",
            )
            .bind(&now)
            .bind(&now)
            .bind(space_id)
            .bind(target_id)
            .bind(&now)
            .execute(&mut *tx)
            .await
        }
    };
    if result.is_err() {
        return envelope::error("transfer failed");
    }
    match tx.commit().await {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.ClearSpace`: items and groups, in one transaction.
async fn clear_space(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
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
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    if sqlx::query("DELETE FROM item_icon WHERE space_id = ?")
        .bind(space_id)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        return envelope::error("clear failed");
    }
    if sqlx::query("DELETE FROM item_icon_group WHERE space_id = ?")
        .bind(space_id)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        return envelope::error("clear failed");
    }
    match tx.commit().await {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
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
        Err(_) => return envelope::error(" [invalid name]"),
    };
    if req.name.is_empty() {
        return envelope::error(" [ Name为必填字段]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let now = now_like_go();
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let created = sqlx::query(
        r#"INSERT INTO "space" (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES (?, ?, 'shared', ?, ?, 0, 'yin', 0, 'direct')"#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&req.name)
    .bind(user.id)
    .execute(&mut *tx)
    .await;
    let yin_id = match created {
        Ok(result) => result.last_insert_rowid(),
        Err(error) => return envelope::error(&error.to_string()),
    };
    if sqlx::query(r#"UPDATE "space" SET pair_id = ? WHERE id = ?"#)
        .bind(yin_id)
        .bind(yin_id)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        return envelope::error("create failed");
    }
    let yang = sqlx::query(
        r#"INSERT INTO "space" (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES (?, ?, 'shared', ?, ?, ?, 'yang', 0, 'direct')"#,
    )
    .bind(&now)
    .bind(&now)
    .bind(format!("{}-B", req.name))
    .bind(user.id)
    .bind(yin_id)
    .execute(&mut *tx)
    .await;
    let yang_id = match yang {
        Ok(result) => result.last_insert_rowid(),
        Err(error) => return envelope::error(&error.to_string()),
    };
    for sid in [yin_id, yang_id] {
        if sqlx::query(
            "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES (?, ?, ?, ?, 'admin', 'manual', ?)",
        )
        .bind(&now)
        .bind(&now)
        .bind(sid)
        .bind(user.id)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return envelope::error("create failed");
        }
        if sqlx::query(
            "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id) VALUES (?, ?, 'material-symbols:apps', 'APP', '', 0, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .bind(user.id)
        .bind(sid)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return envelope::error("create failed");
        }
    }
    match tx.commit().await {
        Ok(_) => success(),
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
async fn sort_items(
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
    let req: ItemSortRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid sort request]"),
    };
    if req.group_id == 0 {
        return envelope::error(" [invalid sort request]");
    }
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    for entry in &req.sort_items {
        if sqlx::query(
            "UPDATE item_icon SET sort = ?, updated_at = ? WHERE id = ? AND space_id = ? AND item_icon_group_id = ?",
        )
        .bind(entry.sort)
        .bind(now_like_go())
        .bind(entry.id)
        .bind(space_id)
        .bind(req.group_id)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return envelope::error("sort failed");
        }
    }
    match tx.commit().await {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
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
async fn sort_groups(
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
    let req: GroupSortRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid group sort request]"),
    };
    if req.sort_groups.is_empty() {
        return envelope::error(" [invalid group sort request]");
    }
    if let Some(parent) = req.parent_id {
        if !group_belongs(&pool, parent, space_id).await {
            return envelope::error(" [invalid parent group]");
        }
    }
    let siblings: Vec<i64> = match req.parent_id {
        None => sqlx::query_as(
            "SELECT id FROM item_icon_group WHERE space_id = ? AND parent_id IS NULL",
        )
        .bind(space_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
        .into_iter(),
        Some(parent) => sqlx::query_as(
            "SELECT id FROM item_icon_group WHERE space_id = ? AND parent_id = ?",
        )
        .bind(space_id)
        .bind(parent)
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
        .into_iter(),
    }
    .map(|(id,)| id)
    .collect();
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
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    for entry in &req.sort_groups {
        // The parent scope is enforced by the sibling check above.
        if sqlx::query(
            "UPDATE item_icon_group SET sort = ?, updated_at = ? WHERE id = ? AND space_id = ?",
        )
        .bind(entry.sort)
        .bind(now_like_go())
        .bind(entry.id)
        .bind(space_id)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return envelope::error("sort failed");
        }
    }
    match tx.commit().await {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
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
        let exists: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM item_icon WHERE id = ? AND space_id = ?",
        )
        .bind(item.id)
        .bind(space_id)
        .fetch_optional(&mut *tx)
        .await
        .unwrap_or(None);
        if exists.is_none() {
            return envelope::error(&format!(" [item {} is not in this space]", item.id));
        }
        let group: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM item_icon_group WHERE id = ? AND space_id = ?",
        )
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
        let exists: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM item_icon_group WHERE id = ? AND space_id = ?",
        )
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let space: Option<(i64, String)> =
        sqlx::query_as(r#"SELECT owner_user_id, type FROM "space" WHERE id = ?"#)
            .bind(space_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    let Some((owner, kind)) = space else {
        return error_code(404);
    };
    if owner != user.id {
        return error_code(1005);
    }
    if kind == "personal" {
        return envelope::error("a personal space cannot be deleted");
    }
    let ids = paired_ids(&pool, space_id).await;
    let placeholders = vec!["?"; ids.len()].join(",");

    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };

    // Files named by this Space's item icons, kept only if nothing else uses them.
    let item_sql = format!(
        "SELECT icon_json FROM item_icon WHERE space_id IN ({placeholders})"
    );
    let mut query = sqlx::query_as::<_, (String,)>(&item_sql);
    for id in &ids {
        query = query.bind(id);
    }
    let icons = query.fetch_all(&mut *tx).await.unwrap_or_default();
    let mut referenced: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (icon_json,) in icons {
        if let Ok(info) = serde_json::from_str::<IconInfo>(&icon_json) {
            if !info.file_name.is_empty() {
                referenced.insert(info.file_name);
            }
        }
    }
    let mut removed: Vec<String> = Vec::new();
    for name in &referenced {
        let used: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM item_icon WHERE icon_json LIKE ?")
            .bind(format!("%\"{name}\"%"))
            .fetch_one(&mut *tx)
            .await
            .unwrap_or((0,));
        if used.0 > 0 {
            continue;
        }
        let deleted = sqlx::query("DELETE FROM file WHERE file_name = ?")
            .bind(name)
            .execute(&mut *tx)
            .await;
        if deleted.is_ok() {
            removed.push(name.clone());
        }
    }

    for table in [
        "item_icon",
        "item_icon_group",
        "space_member",
        "space_o_id_c_group",
    ] {
        let sql = format!("DELETE FROM {table} WHERE space_id IN ({placeholders})");
        let mut statement = sqlx::query(&sql);
        for id in &ids {
            statement = statement.bind(id);
        }
        if statement.execute(&mut *tx).await.is_err() {
            return envelope::error("delete failed");
        }
    }
    let sql = format!(r#"DELETE FROM "space" WHERE id IN ({placeholders})"#);
    let mut statement = sqlx::query(&sql);
    for id in &ids {
        statement = statement.bind(id);
    }
    if statement.execute(&mut *tx).await.is_err() {
        return envelope::error("delete failed");
    }
    if let Err(error) = tx.commit().await {
        return envelope::error(&error.to_string());
    }

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
async fn upload_image(headers: HeaderMap, mut multipart: Multipart) -> Response {
    if let Err(response) = require_user(&headers).await {
        return response;
    }
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(_) => return error_code(1300),
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
    let group: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM item_icon_group WHERE id = ? AND space_id = ?",
    )
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
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM file WHERE user_id = ? AND file_name = ?",
        )
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
        if let Err(error) =
            import_node(&mut tx, &icons, space_id, user.id, None, node, index as i64, &mut summary).await
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
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    let mail = input.mail.trim().to_lowercase();
    if mail.len() < 5 {
        return envelope::error(" [The account must be no less than 5 characters long]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let exists: Option<(i64,)> =
        sqlx::query_as(r#"SELECT id FROM "user" WHERE mail = ?"#)
            .bind(&mail)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    if exists.is_some() {
        return error_code(1008);
    }
    let now = now_like_go();
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let inserted = sqlx::query(
        r#"INSERT INTO "user" (created_at, updated_at, password, name, head_image, status, role, mail, token_version) VALUES (?, ?, ?, ?, ?, 1, ?, ?, 0)"#,
    )
    .bind(&now)
    .bind(&now)
    .bind(password_encryption(&input.password))
    .bind(&input.name)
    .bind(&input.head_image)
    .bind(input.role)
    .bind(&mail)
    .execute(&mut *tx)
    .await;
    let id = match inserted {
        Ok(result) => result.last_insert_rowid(),
        Err(error) => return envelope::error(&error.to_string()),
    };
    let yin = sqlx::query(
        r#"INSERT INTO "space" (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES (?, ?, 'personal', ?, ?, 0, 'yin', 0, 'direct')"#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&input.name)
    .bind(id)
    .execute(&mut *tx)
    .await;
    let yin_id = match yin {
        Ok(result) => result.last_insert_rowid(),
        Err(error) => return envelope::error(&error.to_string()),
    };
    let _ = sqlx::query(r#"UPDATE "space" SET pair_id = ? WHERE id = ?"#)
        .bind(yin_id)
        .bind(yin_id)
        .execute(&mut *tx)
        .await;
    let yang = sqlx::query(
        r#"INSERT INTO "space" (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES (?, ?, 'personal', ?, ?, ?, 'yang', 0, 'direct')"#,
    )
    .bind(&now)
    .bind(&now)
    .bind(format!("{}-B", input.name))
    .bind(id)
    .bind(yin_id)
    .execute(&mut *tx)
    .await;
    let yang_id = match yang {
        Ok(result) => result.last_insert_rowid(),
        Err(error) => return envelope::error(&error.to_string()),
    };
    for sid in [yin_id, yang_id] {
        if sqlx::query(
            "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES (?, ?, ?, ?, 'admin', 'manual', ?)",
        )
        .bind(&now)
        .bind(&now)
        .bind(sid)
        .bind(id)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return envelope::error("create user");
        }
        if sqlx::query(
            "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id) VALUES (?, ?, 'material-symbols:ad-group-outline', 'APP', '', 0, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .bind(id)
        .bind(sid)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return envelope::error("create user");
        }
    }
    if let Err(error) = tx.commit().await {
        return envelope::error(&error.to_string());
    }
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
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let existing: Option<(String, String)> =
        sqlx::query_as(r#"SELECT password, name FROM "user" WHERE id = ?"#)
            .bind(input.id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    let Some((existing_password, existing_name)) = existing else {
        return envelope::error(" [record not found]");
    };
    let mut password = input.password.clone();
    if password.is_empty() {
        password = existing_password.clone();
    }
    let mail = input.mail.trim().to_lowercase();
    if mail.len() < 3 {
        return envelope::error(" [The account must be no less than 3 characters long]");
    }
    let clash: Option<(i64,)> =
        sqlx::query_as(r#"SELECT id FROM "user" WHERE mail = ? AND id <> ?"#)
            .bind(&mail)
            .bind(input.id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    if clash.is_some() {
        return error_code(1008);
    }
    if password != existing_password {
        password = password_encryption(&password);
    }
    // GORM's `Updates` with a struct writes only non-zero fields.
    let mut set = vec!["mail = ?".to_string()];
    if !input.name.is_empty() {
        set.push("name = ?".to_string());
    }
    if !input.head_image.is_empty() {
        set.push("head_image = ?".to_string());
    }
    if input.status != 0 {
        set.push("status = ?".to_string());
    }
    if input.role != 0 {
        set.push("role = ?".to_string());
    }
    if password != existing_password {
        set.push("password = ?".to_string());
    }
    set.push("updated_at = ?".to_string());
    let sql = format!(r#"UPDATE "user" SET {} WHERE id = ?"#, set.join(", "));
    let mut statement = sqlx::query(&sql).bind(&mail);
    if !input.name.is_empty() {
        statement = statement.bind(&input.name);
    }
    if !input.head_image.is_empty() {
        statement = statement.bind(&input.head_image);
    }
    if input.status != 0 {
        statement = statement.bind(input.status);
    }
    if input.role != 0 {
        statement = statement.bind(input.role);
    }
    if password != existing_password {
        statement = statement.bind(&password);
    }
    statement = statement.bind(now_like_go()).bind(input.id);
    if statement.execute(&pool).await.is_err() {
        return envelope::error("update failed");
    }
    if !input.name.is_empty() && input.name != existing_name {
        let now = now_like_go();
        let _ = sqlx::query(
            r#"UPDATE "space" SET name = ?, updated_at = ? WHERE owner_user_id = ? AND side = 'yin' AND type = 'personal'"#,
        )
        .bind(&input.name)
        .bind(&now)
        .bind(input.id)
        .execute(&pool)
        .await;
        let _ = sqlx::query(
            r#"UPDATE "space" SET name = ?, updated_at = ? WHERE owner_user_id = ? AND side = 'yang' AND type = 'personal'"#,
        )
        .bind(format!("{}-B", input.name))
        .bind(&now)
        .bind(input.id)
        .execute(&pool)
        .await;
    }
    let stored: Option<(String, String, String, String, String, i64, i64)> = sqlx::query_as(
        r#"SELECT created_at, updated_at, password, name, head_image, status, role FROM "user" WHERE id = ?"#,
    )
    .bind(input.id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);
    let Some((created_at, updated_at, password, name, head_image, status, role)) = stored else {
        return envelope::database_error();
    };
    let _ = existing_name;
    envelope::success_struct(AdminUser {
        id: input.id,
        create_time: rfc3339(&created_at),
        update_time: rfc3339(&updated_at),
        password,
        name,
        head_image,
        status,
        role,
        mail: sqlx::query_as::<_, (String,)>(r#"SELECT mail FROM "user" WHERE id = ?"#)
            .bind(input.id)
            .fetch_one(&pool)
            .await
            .map(|value| value.0)
            .unwrap_or_default(),
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
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let owned: Vec<(i64, String)> =
        sqlx::query_as(r#"SELECT id, type FROM "space" WHERE owner_user_id = ?"#)
            .bind(req.user_id)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();
    if !req.force && owned.iter().any(|(_, kind)| kind == "shared") {
        return envelope::error("user owns shared spaces; transfer them before deletion");
    }
    let mut ids: Vec<i64> = owned.iter().map(|(id, _)| *id).collect();
    if !ids.is_empty() {
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!(r#"SELECT id FROM "space" WHERE pair_id IN ({placeholders})"#);
        let mut query = sqlx::query_as::<_, (i64,)>(&sql);
        for id in &ids {
            query = query.bind(id);
        }
        for (paired,) in query.fetch_all(&pool).await.unwrap_or_default() {
            ids.push(paired);
        }
    }
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    if !ids.is_empty() {
        let placeholders = vec!["?"; ids.len()].join(",");
        for table in ["item_icon", "item_icon_group", "space_member", "space_o_id_c_group"] {
            let sql = format!("DELETE FROM {table} WHERE space_id IN ({placeholders})");
            let mut statement = sqlx::query(&sql);
            for id in &ids {
                statement = statement.bind(id);
            }
            if statement.execute(&mut *tx).await.is_err() {
                return envelope::error("delete failed");
            }
        }
        let sql = format!(r#"DELETE FROM "space" WHERE id IN ({placeholders})"#);
        let mut statement = sqlx::query(&sql);
        for id in &ids {
            statement = statement.bind(id);
        }
        if statement.execute(&mut *tx).await.is_err() {
            return envelope::error("delete failed");
        }
    }
    for table in ["o_auth_identity", "item_icon", "item_icon_group", "module_config", "file"] {
        let sql = format!("DELETE FROM {table} WHERE user_id = ?");
        if sqlx::query(&sql)
            .bind(req.user_id)
            .execute(&mut *tx)
            .await
            .is_err()
        {
            return envelope::error("delete failed");
        }
    }
    if sqlx::query(r#"DELETE FROM "user" WHERE id = ?"#)
        .bind(req.user_id)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        return envelope::error("delete failed");
    }
    if let Err(error) = tx.commit().await {
        return envelope::error(&error.to_string());
    }
    success()
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
        return envelope::error(" [Key: 'RequestDeleteId.Id' Error:Field validation for 'Id' failed on the 'required' tag]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row: Option<(String,)> = sqlx::query_as("SELECT file_name FROM file WHERE id = ?")
        .bind(req.id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    let Some((file_name,)) = row else {
        return envelope::database_error();
    };
    let _ = std::fs::remove_file(storage_dir().join(&file_name));
    match sqlx::query("DELETE FROM file WHERE id = ?")
        .bind(req.id)
        .execute(pool)
        .await
    {
        Ok(_) => success(),
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
    let package: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM package_record_v2 WHERE id = ? AND removed = 0",
    )
    .bind(package_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    if package.is_none() {
        return Err("record not found".to_string());
    }
    let revision: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM revision_record_v2 WHERE id = ? AND package_id = ?",
    )
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !req.revision.is_empty() || !req.package_id.is_empty() || !req.mode.is_empty() {
        let mode = if req.mode.is_empty() { "auto".to_string() } else { req.mode.clone() };
        if !matches!(mode.as_str(), "light" | "dark" | "auto") {
            return envelope::error(" [mode must be light, dark, or auto]");
        }
        let revision_id = if !req.revision.is_empty() {
            req.revision.clone()
        } else if !req.package_id.is_empty() {
            match latest_revision_of(&pool, &req.package_id).await {
                Ok(id) => id,
                Err(_) => return error_code(404),
            }
        } else {
            let instance = match instance_default_revision(&pool).await {
                Ok(id) => id,
                Err(error) => return envelope::error(&error),
            };
            activation_revision(&pool, "instance").await.unwrap_or(instance)
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
        if let Err(error) =
            select_user_theme(&pool, user.id, &package_id, &revision_id, &mode).await
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
        let now = now_like_go();
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT mode FROM user_theme_preference_v2 WHERE user_id = ?")
                .bind(user.id)
                .fetch_optional(&pool)
                .await
                .unwrap_or(None);
        let mode = existing.map(|value| value.0).unwrap_or_else(|| "auto".to_string());
        if let Err(error) = sqlx::query(
            "INSERT INTO user_theme_preference_v2 (user_id, mode, theme_mode, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET theme_mode = excluded.theme_mode, updated_at = excluded.updated_at",
        )
        .bind(user.id)
        .bind(&mode)
        .bind(&req.theme_mode)
        .bind(&now)
        .execute(&pool)
        .await
        {
            return envelope::error(&error.to_string());
        }
    }
    success()
}

/// `ThemeRouter.RevisionsV2`.
async fn admin_revisions() -> Response {
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
    let mut package_list = Vec::with_capacity(packages.len());
    for (id, name, revision_id) in packages {
        let revision: Option<(String, i64, String)> = sqlx::query_as(
            "SELECT version, verified, created_at FROM revision_record_v2 WHERE id = ?",
        )
        .bind(&revision_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
        let (version, verified, created_at) = revision.unwrap_or_default();
        package_list.push(json!({
            "id": id,
            "name": name,
            "version": version,
            "revision": revision_id,
            "verified": verified != 0,
            "createdAt": rfc3339(&created_at),
        }));
    }
    let revisions: Vec<(String, String, String, i64, String)> = sqlx::query_as(
        "SELECT id, package_id, version, verified, created_at FROM revision_record_v2 ORDER BY package_id, created_at DESC",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let revision_list: Vec<serde_json::Value> = revisions
        .into_iter()
        .map(|(id, package_id, version, verified, created_at)| {
            // `gin.H`, so the keys come out sorted.
            json!({
                "createdAt": rfc3339(&created_at),
                "id": id,
                "packageId": package_id,
                "verified": verified != 0,
                "version": version,
            })
        })
        .collect();
    let instance = instance_default_revision(pool).await.unwrap_or_default();
    let default_revision = activation_revision(pool, "instance").await.unwrap_or(instance);
    let default_package: (String,) = sqlx::query_as(
        "SELECT package_id FROM revision_record_v2 WHERE id = ?",
    )
    .bind(&default_revision)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)
    .unwrap_or_default();
    let activation: Option<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at FROM activation_record_v2 WHERE scope = 'instance'",
    )
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    let (active, last_good, pending, trial) = activation.unwrap_or_default();
    success_data(json!({
        "activeRevision": active,
        "defaultPackage": default_package.0,
        "lastGoodRevision": last_good,
        "packages": package_list,
        "pendingRevision": pending,
        "revisions": revision_list,
        "trialStartedAt": trial.map(|value| rfc3339(&value)),
    }))
}

/// `ThemeRouter.Audit`: the newest 200 theme actions.
async fn admin_audit() -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let rows: Vec<(i64, i64, String, String, String)> = match sqlx::query_as(
        "SELECT id, actor_id, action, package_id, created_at FROM audit_record ORDER BY created_at DESC LIMIT 200",
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, actor, action, package, created_at)| {
            json!({
                "id": id,
                "actorId": actor,
                "action": action,
                "packageId": package,
                "createdAt": rfc3339(&created_at),
            })
        })
        .collect();
    success_data(serde_json::Value::Array(list))
}

/// `ThemeRouter.TrustedRuntimePolicyV2`.
async fn admin_trusted(Path(revision): Path<String>) -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let exists: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM revision_record_v2 WHERE id = ?")
            .bind(&revision)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    if exists.is_none() {
        return error_code(404);
    }
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT enabled FROM trusted_runtime_policy_v2 WHERE revision_id = ?",
    )
    .bind(&revision)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    success_data(json!({
        "enabled": row.map(|value| value.0 != 0).unwrap_or(false),
        "revision": revision,
    }))
}

/// `ThemeRouter.WebWallpaperAsset`: `index.html` or the poster file.
async fn web_wallpaper_asset(
    Path((id, name)): Path<(String, String)>,
) -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row: Option<(Vec<u8>, Vec<u8>, String)> = sqlx::query_as(
        "SELECT html, poster, poster_type FROM web_wallpaper_record WHERE id = ?",
    )
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
    serve_bytes(&media_type, &content)
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let manifest_row: Option<(String,)> =
        sqlx::query_as("SELECT manifest_json FROM revision_record_v2 WHERE id = ?")
            .bind(&revision)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    let Some((manifest_json,)) = manifest_row else {
        return error_code(404);
    };
    if req.enabled {
        let manifest: serde_json::Value =
            serde_json::from_str(&manifest_json).unwrap_or_default();
        if !supported_modes(&manifest).iter().any(|mode| mode == "trusted") {
            return envelope::error(" [theme does not declare support for the trusted runtime]");
        }
    }
    let now = now_like_go();
    match sqlx::query(
        "INSERT INTO trusted_runtime_policy_v2 (revision_id, enabled, updated_by, updated_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(revision_id) DO UPDATE SET enabled = excluded.enabled, updated_by = excluded.updated_by, updated_at = excluded.updated_at",
    )
    .bind(&revision)
    .bind(req.enabled as i64)
    .bind(user.id)
    .bind(&now)
    .execute(pool)
    .await
    {
        Ok(_) => {
            if !req.enabled {
                // Disabling the runtime also withdraws every trusted grant.
                let _ = sqlx::query(
                    "DELETE FROM grant_record_v2 WHERE revision_id = ? AND execution_mode = 'trusted'",
                )
                .bind(&revision)
                .execute(pool)
                .await;
            }
            success()
        }
        Err(error) => envelope::error(&error.to_string()),
    }
}

fn manifest_field<'a>(manifest: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
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
        if let Some(list) = manifest_field(manifest, &["permissions", key]).and_then(serde_json::Value::as_array) {
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
        || !supported_modes(manifest).iter().any(|mode| mode == execution_mode)
    {
        return Err(format!("theme does not support the {execution_mode} runtime"));
    }
    let allowed = declared_permissions(manifest);
    let mut granted = std::collections::HashSet::new();
    for permission in permissions {
        if !allowed.iter().any(|name| name == permission) || !granted.insert(permission.clone()) {
            return Err(format!("theme permission {permission:?} is not declared or is duplicated"));
        }
    }
    for required in required_permissions(manifest) {
        if !granted.contains(&required) {
            return Err(format!("required theme permission {required:?} must be granted"));
        }
    }
    Ok(())
}

fn is_builtin_theme(id: &str) -> bool {
    matches!(id, "org.yin.default" | "org.yin.glass" | "org.yin.directory")
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
    if !is_builtin_theme(manifest.get("id").and_then(serde_json::Value::as_str).unwrap_or("")) {
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
    let available = supported_modes(&manifest).iter().any(|mode| mode == &execution_mode);
    let trusted_enabled: Option<(i64,)> = sqlx::query_as(
        "SELECT enabled FROM trusted_runtime_policy_v2 WHERE revision_id = ?",
    )
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
        let permissions = if execution_mode == "sandbox" { permissions } else { Vec::new() };
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
    if execution_mode == "sandbox" && implicit_builtin_permissions(pool, &revision).await.is_some() {
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
async fn theme_preview_upload(headers: HeaderMap, mut multipart: Multipart) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
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
        "token": token,
        "package": package.preview_public(&token),
    }))
}

/// `POST /api/theme/v2/admin/install`.
async fn theme_install(headers: HeaderMap, mut multipart: Multipart) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    if user.role != 1 {
        return error_code(1005);
    }
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if install_package(&pool, user.id, &package).await.is_err() {
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

/// `InstallPackageV2`.
async fn install_package(
    pool: &sqlx::SqlitePool,
    actor_id: i64,
    package: &crate::packages::Package,
) -> Result<(), String> {
    let manifest = &package.manifest;
    let manifest_json = serde_json::to_string(manifest).map_err(|error| error.to_string())?;
    let tokens_json = serde_json::to_string(&package.tokens).map_err(|error| error.to_string())?;
    let now = now_like_go();
    let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM revision_record_v2 WHERE package_id = ? AND version = ?",
    )
    .bind(&manifest.id)
    .bind(&manifest.version)
    .fetch_optional(&mut *tx)
    .await
    .unwrap_or(None);
    if let Some((existing_id,)) = existing {
        if existing_id != package.revision {
            return Err("same theme package version has different content".to_string());
        }
        if sqlx::query(
            "UPDATE package_record_v2 SET name = ?, latest_revision = ?, removed = 0 WHERE id = ?",
        )
        .bind(&manifest.name)
        .bind(&package.revision)
        .bind(&manifest.id)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return Err("install failed".to_string());
        }
    } else {
        let record: Option<(i64,)> =
            sqlx::query_as("SELECT removed FROM package_record_v2 WHERE id = ?")
                .bind(&manifest.id)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or(None);
        match record {
            None => {
                sqlx::query(
                    "INSERT INTO package_record_v2 (id, name, latest_revision, removed, created_at, updated_at) VALUES (?, ?, '', 0, ?, ?)",
                )
                .bind(&manifest.id)
                .bind(&manifest.name)
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await
                .map_err(|error| error.to_string())?;
            }
            Some((removed,)) if removed != 0 => {
                return Err(
                    "removed theme packages must be explicitly restored before installation"
                        .to_string(),
                )
            }
            Some(_) => {}
        }
        sqlx::query(
            "INSERT INTO revision_record_v2 (id, package_id, version, manifest_json, tokens_json, verified, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&package.revision)
        .bind(&manifest.id)
        .bind(&manifest.version)
        .bind(&manifest_json)
        .bind(&tokens_json)
        .bind(package.verified as i64)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|error| error.to_string())?;
        for (name, (media_type, content)) in &package.files {
            sqlx::query(
                "INSERT INTO asset_record_v2 (revision_id, path, media_type, content) VALUES (?, ?, ?, ?)",
            )
            .bind(&package.revision)
            .bind(name)
            .bind(media_type)
            .bind(content)
            .execute(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
        }
        if sqlx::query(
            "UPDATE package_record_v2 SET name = ?, latest_revision = ?, removed = 0 WHERE id = ?",
        )
        .bind(&manifest.name)
        .bind(&package.revision)
        .bind(&manifest.id)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return Err("install failed".to_string());
        }
        // Only a new revision is an auditable install; re-installing the same
        // version is a no-op that Go does not record.
        sqlx::query(
            "INSERT INTO audit_record (actor_id, action, package_id, created_at) VALUES (?, 'install-v2', ?, ?)",
        )
        .bind(actor_id)
        .bind(&manifest.id)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|error| error.to_string())?;
    }
    tx.commit().await.map_err(|error| error.to_string())
}

/// `GET /api/theme/v2/preview/:token`.
async fn theme_preview_get(Path(token): Path<String>) -> Response {
    match crate::packages::get_preview(&token) {
        Some(package) => success_data(package.preview_public(&token)),
        None => error_code(404),
    }
}

/// `GET /api/theme/v2/preview/:token/assets/*name`.
async fn theme_preview_asset(Path((token, raw_name)): Path<(String, String)>) -> Response {
    let name = raw_name.trim_start_matches('/');
    let Some(package) = crate::packages::get_preview(&token) else {
        return envelope::empty_not_found();
    };
    let Some((media_type, content)) = package.files.get(name) else {
        return envelope::empty_not_found();
    };
    serve_bytes(media_type, content)
}

/// `serveThemeAsset` for an in-memory asset: nosniff, the immutable policy, an
/// ETag on HTML and the document sandbox policy.
fn serve_bytes(media_type: &str, content: &[u8]) -> Response {
    let is_html = media_type == "text/html";
    let mut response = Response::new(axum::body::Body::from(content.to_vec()));
    {
        let out = response.headers_mut();
        for (key, value) in envelope::headers() {
            out.insert(key, value);
        }
        out.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(media_type).unwrap_or(HeaderValue::from_static("application/octet-stream")),
        );
        out.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
        let cache = if is_html {
            "public, max-age=0, must-revalidate, no-transform"
        } else {
            "public, max-age=31536000, immutable, no-transform"
        };
        out.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
        if is_html {
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(content);
            let etag = format!("\"{}\"", digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>());
            out.insert(header::ETAG, HeaderValue::from_str(&etag).unwrap_or(HeaderValue::from_static("")));
            out.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline' https://static.cloudflareinsights.com; style-src 'unsafe-inline'; img-src data:; connect-src https://cloudflareinsights.com; font-src 'none'; media-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'"));
            out.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
        } else if media_type == "image/svg+xml" {
            out.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox; default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; object-src 'none'; form-action 'none'; base-uri 'none'"));
            out.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
        }
    }
    response
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
async fn remove_package(
    headers: HeaderMap,
    Path(package_id): Path<String>,
) -> Response {
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let record: Option<(i64,)> = sqlx::query_as(
        "SELECT removed FROM package_record_v2 WHERE id = ? AND removed = 0",
    )
    .bind(&package_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);
    if record.is_none() {
        return error_code(404);
    }
    let yin: Option<(String,)> = sqlx::query_as(
        "SELECT latest_revision FROM package_record_v2 WHERE id = 'org.yin.default' AND removed = 0",
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);
    let Some((yin_revision,)) = yin else {
        return envelope::database_error();
    };
    let now = now_like_go();
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let steps = [
        sqlx::query("UPDATE package_record_v2 SET removed = 1, updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&package_id)
            .execute(&mut *tx)
            .await,
        sqlx::query(
            "UPDATE activation_record_v2 SET package_id = 'org.yin.default', active_revision_id = ?, last_good_revision_id = ?, pending_revision_id = '', trial_started_at = NULL, updated_at = ? WHERE package_id = ?",
        )
        .bind(&yin_revision)
        .bind(&yin_revision)
        .bind(&now)
        .bind(&package_id)
        .execute(&mut *tx)
        .await,
        sqlx::query(
            "INSERT INTO audit_record (actor_id, action, package_id, created_at) VALUES (?, 'remove-v2', ?, ?)",
        )
        .bind(user.id)
        .bind(&package_id)
        .bind(&now)
        .execute(&mut *tx)
        .await,
    ];
    for step in steps {
        if step.is_err() {
            return envelope::error("remove failed");
        }
    }
    match tx.commit().await {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
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
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    if req.revision.is_empty() && req.package_id.is_empty() {
        return envelope::error(" [revision or packageId is required]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let revision_id = if !req.revision.is_empty() {
        req.revision.clone()
    } else {
        match latest_revision_of(&pool, &req.package_id).await {
            Ok(id) => id,
            Err(_) => return error_code(404),
        }
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
    let now = now_like_go();
    match sqlx::query(
        "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at)
         VALUES ('instance', ?, ?, ?, '', NULL, ?)
         ON CONFLICT(scope) DO UPDATE SET package_id = excluded.package_id, active_revision_id = excluded.active_revision_id, last_good_revision_id = excluded.last_good_revision_id, pending_revision_id = '', trial_started_at = NULL, updated_at = excluded.updated_at",
    )
    .bind(&package_id)
    .bind(&revision_id)
    .bind(&revision_id)
    .bind(&now)
    .execute(&pool)
    .await
    {
        Ok(_) => success(),
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
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    if req.revision.is_empty() {
        return envelope::error(" [revision is required]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let revision: Option<(String,)> =
        sqlx::query_as("SELECT package_id FROM revision_record_v2 WHERE id = ?")
            .bind(&req.revision)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    let Some((package_id,)) = revision else {
        return error_code(404);
    };
    let removed: Option<(i64,)> = sqlx::query_as(
        "SELECT removed FROM package_record_v2 WHERE id = ?",
    )
    .bind(&package_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);
    if removed.map(|value| value.0).unwrap_or(1) != 0 {
        return error_code(404);
    }
    let now = now_like_go();
    match sqlx::query(
        "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at)
         VALUES ('instance', ?, '', '', ?, ?, ?)
         ON CONFLICT(scope) DO UPDATE SET pending_revision_id = excluded.pending_revision_id, trial_started_at = excluded.trial_started_at, updated_at = excluded.updated_at",
    )
    .bind(&package_id)
    .bind(&req.revision)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    {
        Ok(_) => success_data(json!({
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
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    if req.revision.is_empty() {
        return envelope::error(" [revision is required]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let activation: Option<(String,)> = sqlx::query_as(
        "SELECT pending_revision_id FROM activation_record_v2 WHERE scope = 'instance'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);
    let Some((pending,)) = activation else {
        return envelope::error(" [record not found]");
    };
    if pending.is_empty() || pending != req.revision {
        return envelope::error(" [no matching theme activation trial]");
    }
    let revision: Option<(String,)> =
        sqlx::query_as("SELECT package_id FROM revision_record_v2 WHERE id = ?")
            .bind(&req.revision)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    let Some((package_id,)) = revision else {
        return envelope::error(" [record not found]");
    };
    let now = now_like_go();
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    if sqlx::query(
        "UPDATE activation_record_v2 SET package_id = ?, active_revision_id = ?, pending_revision_id = '', trial_started_at = NULL, updated_at = ? WHERE scope = 'instance'",
    )
    .bind(&package_id)
    .bind(&req.revision)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .is_err()
    {
        return envelope::error("confirm failed");
    }
    if sqlx::query(
        "UPDATE activation_record_v2 SET last_good_revision_id = ?, updated_at = ? WHERE scope = 'instance' AND active_revision_id = ? AND pending_revision_id = ''",
    )
    .bind(&req.revision)
    .bind(&now)
    .bind(&req.revision)
    .execute(&mut *tx)
    .await
    .is_err()
    {
        return envelope::database_error();
    }
    match tx.commit().await {
        Ok(_) => success(),
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let fallback = match instance_default_revision(&pool).await {
        Ok(revision) => revision,
        Err(_) => return envelope::database_error(),
    };
    let activation: Option<(String,)> = sqlx::query_as(
        "SELECT last_good_revision_id FROM activation_record_v2 WHERE scope = 'instance'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);
    let Some((last_good,)) = activation else {
        return envelope::database_error();
    };
    let revision_id = if last_good.is_empty() { fallback } else { last_good };
    let revision: Option<(String,)> =
        sqlx::query_as("SELECT package_id FROM revision_record_v2 WHERE id = ?")
            .bind(&revision_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);
    let Some((package_id,)) = revision else {
        return envelope::database_error();
    };
    match sqlx::query(
        "UPDATE activation_record_v2 SET package_id = ?, active_revision_id = ?, pending_revision_id = '', trial_started_at = NULL, updated_at = ? WHERE scope = 'instance'",
    )
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
            while index < tag.len() && !matches!(bytes[index] as char, '=' | ' ' | '\t' | '\n' | '\r' | '/') {
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
        .map_err(|error| error.to_string())?;
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
        Err(error) => return envelope::error(&format!(" [{}]", error)),
    };
    if req.url.trim().is_empty() {
        return success_data(json!({"iconUrl": "", "fileName": ""}));
    }
    let icon_url = match discover_icon(req.url.trim()).await {
        Ok(url) => url,
        Err(error) => {
            return envelope::error(&format!("acquisition failed: get ico error:{error}"))
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
            return envelope::error(&format!("acquisition failed: parsed ico URL :{error}"))
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
    let extension = if extension.is_empty() { ".ico".to_string() } else { extension };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| format!("{}", value.as_nanos()))
        .unwrap_or_default();
    let file_name = format!("{}{}", hex_digest(format!("{url_file_name}{now}").as_bytes(), false), extension);
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
        Some((content, media_type)) => serve_bytes(&media_type, &content),
        None => envelope::empty_not_found(),
    }
}

// ------------------------------------------------------- Go router fallback

fn path_segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|segment| !segment.is_empty()).collect()
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
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        let trimmed = &path[..path.len() - 1];
        let candidate = format!("{trimmed}/x");
        let redirects = crate::go_routes::GO_ROUTES.iter().any(|(route_method, pattern)| {
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
                    (header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8")),
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let existing: Option<(String,)> = sqlx::query_as(r#"SELECT name FROM "user" WHERE id = ?"#)
        .bind(user.id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
    let Some((existing_name,)) = existing else {
        return envelope::database_error();
    };
    let now = now_like_go();
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    if sqlx::query(r#"UPDATE "user" SET name = ?, head_image = ?, updated_at = ? WHERE id = ?"#)
        .bind(&req.name)
        .bind(&req.head_image)
        .bind(&now)
        .bind(user.id)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        return envelope::database_error();
    }
    if req.name != existing_name {
        let _ = sqlx::query(
            r#"UPDATE "space" SET name = ?, updated_at = ? WHERE type = 'personal' AND owner_user_id = ? AND (side = 'yin' OR side = '' OR side IS NULL)"#,
        )
        .bind(&req.name)
        .bind(&now)
        .bind(user.id)
        .execute(&mut *tx)
        .await;
        let _ = sqlx::query(
            r#"UPDATE "space" SET name = ?, updated_at = ? WHERE type = 'personal' AND owner_user_id = ? AND side = 'yang'"#,
        )
        .bind(format!("{}-B", req.name))
        .bind(&now)
        .bind(user.id)
        .execute(&mut *tx)
        .await;
    }
    match tx.commit().await {
        Ok(_) => success(),
        Err(error) => envelope::error(&error.to_string()),
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
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let stored: Option<(String,)> = sqlx::query_as(r#"SELECT password FROM "user" WHERE id = ?"#)
        .bind(user.id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    let Some((stored,)) = stored else {
        return envelope::error(" [record not found]");
    };
    if stored != password_encryption(&req.old_password) {
        return error_code(1007);
    }
    let now = now_like_go();
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    if sqlx::query(
        r#"UPDATE "user" SET password = ?, token_version = token_version + 1, updated_at = ? WHERE id = ?"#,
    )
    .bind(password_encryption(&req.new_password))
    .bind(&now)
    .bind(user.id)
    .execute(&mut *tx)
    .await
    .is_err()
    {
        return envelope::database_error();
    }
    if tx.commit().await.is_err() {
        return envelope::database_error();
    }
    success()
}

/// `FileRouter.GetS3File`: an uploaded file from local storage, with the cache
/// policy its content-hash name allows.
async fn serve_stored_file(Path(filepath): Path<String>) -> Response {
    serve_stored(filepath, false).await
}

/// The `/api` registration of the same handler; failures carry `no-store`.
async fn serve_stored_file_api(Path(filepath): Path<String>) -> Response {
    serve_stored(filepath, true).await
}

async fn serve_stored(filepath: String, api_group: bool) -> Response {
    let name = filepath.trim_start_matches('/').to_string();
    if name.is_empty() {
        return static_error(StatusCode::BAD_REQUEST, "{\"error\":\"file path is required\"}", api_group);
    }
    let path = storage_dir().join(&name);
    let content = match std::fs::read(&path) {
        Ok(content) => content,
        Err(_) => {
            return static_error(
                StatusCode::NOT_FOUND,
                "{\"error\":\"failed to get file\"}",
                api_group,
            )
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
            HeaderValue::from_str(content_type).unwrap_or(HeaderValue::from_static("application/octet-stream")),
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
    }
    response
}

/// A JSON error from the static file routes, with the middleware headers the
/// matched route would add.
fn static_error(status: StatusCode, body: &'static str, api_group: bool) -> Response {
    let mut response = (status, [(header::CONTENT_TYPE, HeaderValue::from_static("application/json; charset=utf-8"))], body).into_response();
    {
        let headers = response.headers_mut();
        headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("strict-origin-when-cross-origin"));
        headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
        headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("SAMEORIGIN"));
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
        Err(error) => return envelope::error(&format!(" [{}]", error)),
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

/// `ClearUserSpaceThemeV2`.
async fn clear_user_space_theme(
    headers: HeaderMap,
    Path(space_id): Path<String>,
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
async fn copy_space(
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
    if !can_access(&pool, user.id, space_id).await {
        return error_code(1005);
    }
    let source: Option<(String,)> = sqlx::query_as(r#"SELECT name FROM "space" WHERE id = ?"#)
        .bind(space_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
    let Some((source_name,)) = source else {
        return error_code(404);
    };
    let body = axum::body::to_bytes(request.into_body(), 1 << 20)
        .await
        .unwrap_or_default();
    let req: RenameRequest = serde_json::from_slice(&body).unwrap_or(RenameRequest { name: String::new() });
    let name = if req.name.is_empty() {
        format!("{source_name} Copy")
    } else {
        req.name
    };
    let now = now_like_go();
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let created = sqlx::query(
        r#"INSERT INTO "space" (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES (?, ?, 'shared', ?, ?, 0, 'yin', 0, 'direct')"#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&name)
    .bind(user.id)
    .execute(&mut *tx)
    .await;
    let dst = match created {
        Ok(result) => result.last_insert_rowid(),
        Err(_) => return envelope::database_error(),
    };
    if sqlx::query(
        "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES (?, ?, ?, ?, 'admin', 'manual', ?)",
    )
    .bind(&now)
    .bind(&now)
    .bind(dst)
    .bind(user.id)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .is_err()
    {
        return envelope::database_error();
    }
    let groups: Vec<(i64, String, String, String, i64, Option<i64>)> = sqlx::query_as(
        "SELECT id, icon, title, description, sort, parent_id FROM item_icon_group WHERE space_id = ?",
    )
    .bind(space_id)
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();
    for (old_id, icon, title, description, sort, parent_id) in groups {
        let inserted = sqlx::query(
            "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .bind(&icon)
        .bind(&title)
        .bind(&description)
        .bind(sort)
        .bind(user.id)
        .bind(dst)
        .bind(parent_id)
        .execute(&mut *tx)
        .await;
        let new_group = match inserted {
            Ok(result) => result.last_insert_rowid(),
            Err(_) => return envelope::database_error(),
        };
        if sqlx::query(
            "INSERT INTO item_icon (created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id)
             SELECT ?, ?, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, ?, ?, ? FROM item_icon WHERE space_id = ? AND item_icon_group_id = ?",
        )
        .bind(&now)
        .bind(&now)
        .bind(new_group)
        .bind(user.id)
        .bind(dst)
        .bind(space_id)
        .bind(old_id)
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return envelope::database_error();
        }
    }
    match tx.commit().await {
        Ok(_) => success(),
        Err(_) => envelope::database_error(),
    }
}

/// `validWallpaperBytes`: the media type must match the bytes.
fn valid_wallpaper_bytes(media_type: &str, data: &[u8]) -> bool {
    match media_type {
        "image/png" => data.len() >= 8 && data[..8] == [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'],
        "image/jpeg" => data.len() >= 3 && data[..3] == [0xff, 0xd8, 0xff],
        "image/gif" => data.len() >= 6 && (&data[..6] == b"GIF87a" || &data[..6] == b"GIF89a"),
        "image/webp" => data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP",
        "video/mp4" => data.len() >= 12 && data.len() <= 20 << 20 && &data[4..8] == b"ftyp",
        "video/webm" => data.len() >= 4 && data.len() <= 20 << 20 && data[..4] == [0x1a, 0x45, 0xdf, 0xa3],
        "text/html" => !data.is_empty() && data.len() <= 5 << 20 && std::str::from_utf8(data).is_ok(),
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
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))
        .map_err(|error| error.to_string())?;
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
    let html = html.ok_or_else(|| "web wallpaper must contain index.html and one poster".to_string())?;
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
async fn upload_web_wallpaper(headers: HeaderMap, mut multipart: Multipart) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
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
    let id: String = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
    let ctx = appcfg::ctx();
    let pool = match ctx.pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let exists: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM web_wallpaper_record WHERE id = ?")
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
            "/"
                | "/index.html"
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
