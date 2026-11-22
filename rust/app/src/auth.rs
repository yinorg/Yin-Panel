//! Session handling: the same HS256 token the Go build issues, carried in the
//! `yin_token` httpOnly cookie or an `Authorization: Bearer` header.

use axum::http::{HeaderMap, header};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::appcfg;
use crate::repository;

pub use crate::repository::UserRecord as UserRow;

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

/// Why authentication failed, in the shape the Core answers with.
#[derive(Debug, PartialEq, Eq)]
pub enum AuthError {
    NotLoggedIn,
    Disabled,
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
    let ctx = appcfg::ctx();
    let pool = ctx
        .shared_pool()
        .await
        .map_err(|_| AuthError::NotLoggedIn)?;
    user_from_pool(pool, ctx.sqlx_driver(), &claims).await
}

pub(crate) async fn user_from_pool(
    pool: &sqlx::AnyPool,
    driver: crate::db::Driver,
    claims: &Claims,
) -> Result<UserRow, AuthError> {
    let user = repository::user_by_id(pool, driver, claims.user_id)
        .await
        .map_err(|_| AuthError::NotLoggedIn)?;
    validate_user(user, claims.token_version)
}

fn validate_user(user: Option<UserRow>, token_version: i64) -> Result<UserRow, AuthError> {
    let user = user.ok_or(AuthError::NotLoggedIn)?;
    if user.status != 1 {
        return Err(AuthError::Disabled);
    }
    if token_version != user.token_version {
        return Err(AuthError::NotLoggedIn);
    }
    Ok(user)
}

/// `interceptor.SetAuthCookie`.
pub fn session_cookie(token: &str, max_age_seconds: i64, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!(
        "{COOKIE_NAME}={token}; Path=/; Max-Age={max_age_seconds}; HttpOnly{secure}; SameSite=Lax"
    )
}

/// `interceptor.ClearAuthCookie`.
pub fn cleared_cookie(secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{COOKIE_NAME}=; Path=/; Max-Age=0; HttpOnly{secure}; SameSite=Lax")
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

#[cfg(test)]
mod tests {
    use super::{AuthError, COOKIE_NAME, Claims, UserRow, bearer_or_cookie, validate_user};
    use axum::http::{HeaderMap, HeaderValue, header};
    use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};

    const SECRET: &[u8] = b"go-token-compatibility-fixture-secret";

    #[test]
    fn accepts_go_hs256_claims_with_unsigned_integer_fields() {
        let now = super::now_unix();
        let go_claims = serde_json::json!({
            "user_id": 42u64,
            "token_version": 7u64,
            "exp": now + 60,
            "nbf": now - 1,
            "iat": now - 1,
            "sub": "user_access"
        });
        let token = jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &go_claims,
            &EncodingKey::from_secret(SECRET),
        )
        .expect("sign Go-compatible claims");
        let claims = jsonwebtoken::decode::<Claims>(
            &token,
            &DecodingKey::from_secret(SECRET),
            &Validation::new(Algorithm::HS256),
        )
        .expect("decode Go-compatible token")
        .claims;
        assert_eq!(claims.user_id, 42);
        assert_eq!(claims.token_version, 7);
        assert_eq!(claims.sub, "user_access");
    }

    #[test]
    fn verifies_a_token_captured_from_the_go_recorder() {
        let inputs =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../record/inputs.jsonl");
        let inputs_text = std::fs::read_to_string(inputs).expect("read Go recorder inputs");
        let line = inputs_text
            .lines()
            .find(|line| line.contains("yin_token="))
            .expect("recorded Go auth cookie");
        let input: serde_json::Value = serde_json::from_str(line).expect("parse recorder line");
        let cookie = input["headers"]["Cookie"].as_str().expect("cookie header");
        let token = cookie
            .split(';')
            .find_map(|part| part.trim().strip_prefix("yin_token="))
            .expect("yin_token cookie");
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 5;
        let decoded = jsonwebtoken::decode::<Claims>(
            token,
            &DecodingKey::from_secret(b"recording-only-secret"),
            &validation,
        )
        .expect("verify token issued by the Go recorder");
        assert_eq!(decoded.claims.user_id, 1);
        assert_eq!(decoded.claims.token_version, 0);
        assert_eq!(decoded.claims.sub, "user_access");
    }

    #[test]
    fn bearer_header_takes_precedence_over_cookie_and_cookie_is_supported() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("other=x; yin_token=cookie-token"),
        );
        assert_eq!(bearer_or_cookie(&headers).as_deref(), Some("cookie-token"));
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer bearer-token"),
        );
        assert_eq!(bearer_or_cookie(&headers).as_deref(), Some("bearer-token"));
        assert_eq!(COOKIE_NAME, "yin_token");
    }

    #[test]
    fn session_account_validation_checks_status_and_token_version() {
        let active = UserRow {
            id: 42,
            password: "hash".to_string(),
            name: "Test User".to_string(),
            status: 1,
            role: 2,
            mail: "user@example.test".to_string(),
            token_version: 8,
        };
        assert_eq!(validate_user(Some(active.clone()), 8), Ok(active.clone()));
        assert_eq!(
            validate_user(Some(active.clone()), 7),
            Err(AuthError::NotLoggedIn)
        );
        assert_eq!(
            validate_user(
                Some(UserRow {
                    status: 0,
                    ..active
                }),
                8
            ),
            Err(AuthError::Disabled)
        );
        assert_eq!(validate_user(None, 8), Err(AuthError::NotLoggedIn));
    }
}
