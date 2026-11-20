//! Session handling: the same HS256 token the Go build issues, carried in the
//! `yin_token` httpOnly cookie or an `Authorization: Bearer` header.

use axum::http::{HeaderMap, header};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::appcfg;

pub const COOKIE_NAME: &str = "yin_token";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub user_id: i64,
    pub token_version: i64,
    #[serde(default)]
    pub sub: String,
    /// Unix seconds; `jsonwebtoken` validates `exp` and `nbf`.
    pub exp: i64,
    pub nbf: i64,
    pub iat: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub password: String,
    pub name: String,
    pub status: i64,
    pub role: i64,
    pub mail: String,
    pub token_version: i64,
}

/// Why authentication failed, in the shape the Core answers with.
pub enum AuthError {
    NotLoggedIn,
    Disabled,
}

impl AuthError {
    pub fn code(&self) -> i64 {
        match self {
            AuthError::NotLoggedIn => 1000,
            AuthError::Disabled => 1004,
        }
    }
}

fn bearer_or_cookie(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers.get(header::AUTHORIZATION) {
        if let Ok(text) = value.to_str() {
            let text = text.trim();
            if let Some(rest) = text.strip_prefix("Bearer ") {
                return Some(rest.to_string());
            }
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    for value in headers.get_all(header::COOKIE) {
        let Ok(text) = value.to_str() else { continue };
        for part in text.split(';') {
            let part = part.trim();
            if let Some((name, value)) = part.split_once('=') {
                if name == COOKIE_NAME && !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

pub fn claims(headers: &HeaderMap) -> Result<Claims, AuthError> {
    let token = bearer_or_cookie(headers).ok_or(AuthError::NotLoggedIn)?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 5;
    let decoded = jsonwebtoken::decode::<Claims>(
        &token,
        &DecodingKey::from_secret(appcfg::ctx().jwt_secret()),
        &validation,
    )
    .map_err(|_| AuthError::NotLoggedIn)?;
    Ok(decoded.claims)
}

/// `interceptor.Auth`: a valid token, a live account and a matching token
/// version. A token version bump is how logout revokes every session.
pub async fn user(headers: &HeaderMap) -> Result<UserRow, AuthError> {
    let claims = claims(headers)?;
    let pool = appcfg::ctx()
        .pool()
        .await
        .map_err(|_| AuthError::NotLoggedIn)?;
    let user: Option<UserRow> = sqlx::query_as(
        r#"SELECT id, password, name, status, role, mail, token_version FROM "user" WHERE id = ?"#,
    )
    .bind(claims.user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AuthError::NotLoggedIn)?;
    let user = user.ok_or(AuthError::NotLoggedIn)?;
    if user.status != 1 {
        return Err(AuthError::Disabled);
    }
    if claims.token_version != user.token_version {
        return Err(AuthError::NotLoggedIn);
    }
    Ok(user)
}

/// `interceptor.SetAuthCookie`.
pub fn session_cookie(token: &str, max_age_seconds: i64) -> String {
    format!(
        "{COOKIE_NAME}={token}; Path=/; Max-Age={max_age_seconds}; HttpOnly; SameSite=Lax"
    )
}

/// `interceptor.ClearAuthCookie`.
pub fn cleared_cookie() -> String {
    format!("{COOKIE_NAME}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax")
}

pub fn issue_token(user_id: i64, token_version: i64) -> Result<String, String> {
    let now = now_unix();
    let claims = Claims {
        user_id,
        token_version,
        sub: "user_access".to_string(),
        exp: now + appcfg::ctx().jwt_expire_hours() * 3600,
        nbf: now,
        iat: now,
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::new(Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(appcfg::ctx().jwt_secret()),
    )
    .map_err(|error| format!("signing the token: {error}"))
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}
