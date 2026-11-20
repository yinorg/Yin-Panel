//! The first ported Core routes: the ones that need only configuration, the
//! session and one user row. Each mirrors the Go handler in
//! `backend/internal/web/router/system`.

use axum::Router;
use axum::extract::Request;
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
