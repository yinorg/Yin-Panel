//! The first ported Core routes: the ones that need only configuration, the
//! session and one user row. Each mirrors the Go handler in
//! `backend/internal/web/router/system`.

use axum::Router;
use axum::extract::{Path, Query, Request};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::{get, post};
use md5::{Digest, Md5};
use serde::Serialize;
use serde_json::json;

use crate::appcfg;
use crate::auth::{self, AuthError};
use crate::envelope::{self, error_code, success, success_data};
use crate::original::Original;

pub fn router() -> Router<Original> {
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
        .route("/api/spaces/{space_id}/bookmarks/tree", get(bookmarks_tree))
        .route("/api/panel/users/getList", get(users_list))
        .route("/api/panel/userConfig/getConfig", get(user_config))
        .route("/api/system/monitor/getEnableStatus", post(monitor_enable_status))
        .route("/api/system/monitor/getSnapshot", post(monitor_disabled))
        .route("/api/system/monitor/getDiskMountpoints", post(monitor_disabled))
        .route("/api/system/monitor/getDiskStateByPath", post(monitor_disabled))
        .route("/api/theme/v2/preference", get(theme_preference))
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

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let paired: Option<(i64,)> =
            sqlx::query_as(r#"SELECT id FROM "space" WHERE pair_id = ? AND side = 'yang' LIMIT 1"#)
                .bind(row.id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);
        let role: Option<(String,)> =
            sqlx::query_as(r#"SELECT role FROM space_member WHERE space_id = ? AND user_id = ? LIMIT 1"#)
                .bind(row.id)
                .bind(user.id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);
        let role = role.map(|value| value.0).unwrap_or_default();
        let can_edit = row.owner_user_id == user.id || role == "admin" || role == "editor";
        items.push(SpaceItem {
            id: row.id,
            create_time: rfc3339(&row.created_at),
            update_time: rfc3339(&row.updated_at),
            kind: row.kind,
            name: row.name,
            owner_user_id: row.owner_user_id,
            pair_id: row.pair_id,
            side: row.side,
            paired_space_id: paired.map(|value| value.0),
            public_enabled: row.public_enabled != 0,
            public_id: row.public_id,
            public_mode: row.public_mode,
            can_edit,
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
    // A live snapshot needs the monitor port (B8); until then the honest answer
    // is the same failure the Go service gives when the monitor is enabled but
    // the cache is empty.
    envelope::error("failed")
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
        return envelope::error(" [ spaceId must be a positive integer]");
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
