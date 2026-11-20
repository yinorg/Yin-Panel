//! The first ported Core routes: the ones that need only configuration, the
//! session and one user row. Each mirrors the Go handler in
//! `backend/internal/web/router/system`.

use axum::Router;
use axum::extract::{Path, Request};
use axum::http::{HeaderMap, HeaderValue, header};
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
