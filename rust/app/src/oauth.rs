//! OAuth/OIDC login: the Go build registers `/api/oauth/:provider` and
//! `/api/oauth/:provider/callback` when `oauth.enable` is true, redirects the
//! browser to the identity provider, and returns through the callback with a
//! session cookie. Both routes used to be listed only in `CONFIG.md`; this
//! module is the missing implementation.
//!
//! The provider state (nonce and PKCE verifier) stays on the server and is
//! single use, exactly like the Go service's `oauthStates` cache.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode_header};
use rand::RngCore;
use serde::Deserialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::appcfg::{Oauth, OauthProvider};
use crate::repository::UserRecord;

const STATE_TTL: Duration = Duration::from_secs(300);
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
struct PendingLogin {
    provider: String,
    nonce: String,
    pkce_verifier: String,
}

static PENDING: OnceLock<Mutex<HashMap<String, (PendingLogin, Instant)>>> = OnceLock::new();

fn pending() -> &'static Mutex<HashMap<String, (PendingLogin, Instant)>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn remember_state(state: &str, login: PendingLogin) {
    let mut states = pending().lock().expect("oauth state lock");
    let now = Instant::now();
    states.retain(|_, (_, created)| now.duration_since(*created) < STATE_TTL);
    states.insert(state.to_string(), (login, now));
}

fn take_state(state: &str) -> Option<PendingLogin> {
    let mut states = pending().lock().expect("oauth state lock");
    let now = Instant::now();
    states.retain(|_, (_, created)| now.duration_since(*created) < STATE_TTL);
    states.remove(state).map(|(login, _)| login)
}

fn random_url_safe(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buffer);
    URL_SAFE_NO_PAD.encode(buffer)
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// The Go router names the per-provider state cookie from a SHA-256 of the
/// provider name, truncated to eight bytes.
pub fn state_cookie_name(provider: &str) -> String {
    let digest = Sha256::digest(provider.as_bytes());
    format!("sun_panel_oauth_state_{}", hex_lower(&digest[..8]))
}

pub fn find_provider<'a>(oauth: &'a Oauth, name: &str) -> Option<&'a OauthProvider> {
    oauth
        .providers
        .iter()
        .find(|provider| provider.name.eq_ignore_ascii_case(name))
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for index in 0..left.len() {
        difference |= left[index] ^ right[index];
    }
    difference == 0
}

fn append_query(base: &str, params: &[(&str, &str)]) -> String {
    let mut url = base.to_string();
    let mut separator = if base.contains('?') { '&' } else { '?' };
    for (key, value) in params {
        url.push(separator);
        url.push_str(&urlencoding::encode(key));
        url.push('=');
        url.push_str(&urlencoding::encode(value));
        separator = '&';
    }
    url
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn oidc_scopes(scopes: &str) -> String {
    let fields: Vec<&str> = scopes.split_whitespace().collect();
    if fields.is_empty() {
        return "openid profile email".to_string();
    }
    if fields.iter().any(|scope| *scope == "openid") {
        fields.join(" ")
    } else {
        format!("openid {}", fields.join(" "))
    }
}

pub struct AuthStart {
    pub location: String,
    pub state_cookie: String,
}

pub fn disabled_404() -> Response {
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"))],
        "404 page not found",
    )
        .into_response()
}

fn state_cookie(provider: &str, state: &str, max_age: i64) -> String {
    let secure = crate::appcfg::ctx().secure_cookies();
    let secure = if secure { "; Secure" } else { "" };
    format!(
        "{}={state}; Path=/api/oauth/{provider}/callback; Max-Age={max_age}; HttpOnly{secure}; SameSite=Lax",
        state_cookie_name(provider)
    )
}

pub fn clear_state_cookie(provider: &str) -> String {
    state_cookie(provider, "", 0)
}

#[derive(Debug, Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(CALLBACK_TIMEOUT)
        .build()
        .map_err(|error| format!("build OAuth HTTP client: {error}"))
}

async fn discover(provider: &OauthProvider) -> Result<Discovery, String> {
    let issuer = provider.issuer_url.trim();
    if issuer.is_empty() {
        return Err("OIDC issuer_url is required".to_string());
    }
    if !issuer.starts_with("http://") && !issuer.starts_with("https://") {
        return Err("OIDC issuer_url must be an absolute URL".to_string());
    }
    let discovery_url = if provider.discovery_url.trim().is_empty() {
        format!("{}/.well-known/openid-configuration", issuer.trim_end_matches('/'))
    } else {
        provider.discovery_url.trim().to_string()
    };
    let response = http_client()?
        .get(&discovery_url)
        .send()
        .await
        .map_err(|error| format!("fetch OIDC discovery document: {error}"))?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(format!("OIDC discovery returned HTTP {}", response.status()));
    }
    let body = response
        .text()
        .await
        .map_err(|error| format!("read OIDC discovery document: {error}"))?;
    let metadata: Discovery = serde_json::from_str(&body)
        .map_err(|error| format!("decode OIDC discovery document: {error}"))?;
    if metadata.issuer != issuer {
        return Err("OIDC discovery issuer does not match issuer_url".to_string());
    }
    if metadata.authorization_endpoint.is_empty()
        || metadata.token_endpoint.is_empty()
        || metadata.jwks_uri.is_empty()
    {
        return Err("OIDC discovery document is missing a required endpoint".to_string());
    }
    Ok(metadata)
}

fn redirect_uri(provider: &str) -> String {
    format!("{}/api/oauth/{provider}/callback", crate::appcfg::ctx().root_url())
}

/// `OAuthLogin`: validate the provider and build the authorization redirect.
pub async fn begin(provider_name: &str) -> Result<AuthStart, String> {
    let ctx = crate::appcfg::ctx();
    let provider = find_provider(&ctx.conf.oauth, provider_name)
        .ok_or_else(|| "不支持的OAuth提供商".to_string())?;
    let redirect = redirect_uri(&provider.name);
    let state = random_url_safe(32);

    let location = if provider.issuer_url.trim().is_empty() {
        let auth_url = provider.auth_url.trim();
        if auth_url.is_empty() {
            return Err("获取OAuth授权URL失败".to_string());
        }
        remember_state(
            &state,
            PendingLogin {
                provider: provider.name.clone(),
                nonce: String::new(),
                pkce_verifier: String::new(),
            },
        );
        append_query(
            auth_url,
            &[
                ("client_id", provider.client_id.as_str()),
                ("redirect_uri", redirect.as_str()),
                ("scope", provider.scopes.as_str()),
                ("response_type", "code"),
                ("state", state.as_str()),
            ],
        )
    } else {
        let metadata = discover(provider)
            .await
            .map_err(|_| "获取OAuth授权URL失败".to_string())?;
        let nonce = random_url_safe(32);
        let verifier = random_url_safe(32);
        let challenge = pkce_challenge(&verifier);
        remember_state(
            &state,
            PendingLogin {
                provider: provider.name.clone(),
                nonce: nonce.clone(),
                pkce_verifier: verifier.clone(),
            },
        );
        append_query(
            metadata.authorization_endpoint.trim(),
            &[
                ("client_id", provider.client_id.as_str()),
                ("redirect_uri", redirect.as_str()),
                ("scope", oidc_scopes(&provider.scopes).as_str()),
                ("response_type", "code"),
                ("state", state.as_str()),
                ("code_challenge", challenge.as_str()),
                ("code_challenge_method", "S256"),
                ("nonce", nonce.as_str()),
            ],
        )
    };
    Ok(AuthStart {
        location,
        state_cookie: state_cookie(&provider.name, &state, STATE_TTL.as_secs() as i64),
    })
}

async fn exchange_code(
    provider: &OauthProvider,
    token_url: &str,
    redirect: &str,
    code: &str,
    verifier: &str,
) -> Result<Value, String> {
    if token_url.is_empty() {
        return Err("OAuth provider has no token endpoint".to_string());
    }
    let mut form: Vec<(&str, &str)> = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", provider.client_id.as_str()),
        ("client_secret", provider.client_secret.as_str()),
        ("redirect_uri", redirect),
    ];
    if !verifier.is_empty() {
        form.push(("code_verifier", verifier));
    }
    let response = http_client()?
        .post(token_url)
        .form(&form)
        .header(header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|error| format!("exchange OAuth authorization code: {error}"))?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(format!(
            "OAuth token endpoint returned HTTP {}",
            response.status()
        ));
    }
    let body = response
        .text()
        .await
        .map_err(|error| format!("read OAuth token response: {error}"))?;
    serde_json::from_str(&body)
        .map_err(|error| format!("decode OAuth token response: {error}"))
}

fn allowed_algorithm(algorithm: Algorithm) -> bool {
    matches!(
        algorithm,
        Algorithm::RS256
            | Algorithm::RS384
            | Algorithm::RS512
            | Algorithm::PS256
            | Algorithm::PS384
            | Algorithm::PS512
            | Algorithm::ES256
            | Algorithm::ES384
    )
}

async fn fetch_jwks(url: &str) -> Result<JwkSet, String> {
    let response = http_client()?
        .get(url)
        .send()
        .await
        .map_err(|error| format!("fetch OIDC JWKS: {error}"))?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(format!("OIDC JWKS returned HTTP {}", response.status()));
    }
    let body = response
        .text()
        .await
        .map_err(|error| format!("read OIDC JWKS: {error}"))?;
    let keys: JwkSet =
        serde_json::from_str(&body).map_err(|error| format!("decode OIDC JWKS: {error}"))?;
    if keys.keys.is_empty() {
        return Err("OIDC JWKS contains no keys".to_string());
    }
    Ok(keys)
}

async fn verify_id_token(
    raw: &str,
    provider: &OauthProvider,
    metadata: &Discovery,
    nonce: &str,
) -> Result<Map<String, Value>, String> {
    let header = decode_header(raw).map_err(|error| format!("decode OIDC ID token: {error}"))?;
    if !allowed_algorithm(header.alg) {
        return Err("OIDC ID token signing algorithm is not allowed".to_string());
    }
    let keys = fetch_jwks(&metadata.jwks_uri).await?;
    let key_id = header.kid.unwrap_or_default();
    let jwk = keys
        .keys
        .iter()
        .find(|key| key.common.key_id.as_deref() == Some(key_id.as_str()))
        .ok_or_else(|| "OIDC ID token key ID was not found in JWKS".to_string())?;
    let decoding = DecodingKey::from_jwk(jwk)
        .map_err(|error| format!("build OIDC decoding key: {error}"))?;

    let mut validation = Validation::new(header.alg);
    validation.set_issuer(&[provider.issuer_url.trim()]);
    validation.set_audience(&[provider.client_id.as_str()]);
    let decoded = jsonwebtoken::decode::<Map<String, Value>>(raw, &decoding, &validation)
        .map_err(|error| format!("verify OIDC ID token: {error}"))?;
    let claims = decoded.claims;
    let token_nonce = claims.get("nonce").and_then(Value::as_str).unwrap_or("");
    if token_nonce.is_empty() || token_nonce != nonce {
        return Err("OIDC ID token nonce does not match authorization request".to_string());
    }
    Ok(claims)
}

async fn fetch_user_info(provider: &OauthProvider, access_token: &str) -> Result<Value, String> {
    let user_info_url = provider.user_info_url.trim();
    if user_info_url.is_empty() {
        return Err("OAuth provider has no user_info_url".to_string());
    }
    let response = http_client()?
        .get(user_info_url)
        .bearer_auth(access_token)
        .header(header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|error| format!("fetch OAuth user info: {error}"))?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(format!(
            "OAuth user info endpoint returned HTTP {}",
            response.status()
        ));
    }
    let body = response
        .text()
        .await
        .map_err(|error| format!("read OAuth user info: {error}"))?;
    serde_json::from_str(&body).map_err(|error| format!("decode OAuth user info: {error}"))
}

#[derive(Debug, Deserialize)]
struct ProviderEmail {
    email: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    verified: bool,
}

/// Providers such as GitHub hide the primary address on `/user`; the dedicated
/// email endpoint is the only trustworthy source for it.
async fn enrich_verified_email(
    provider: &OauthProvider,
    access_token: &str,
    user_info: &mut Map<String, Value>,
) -> Result<(), String> {
    let url = provider.user_info_email_url.trim();
    if url.is_empty() {
        return Ok(());
    }
    let response = http_client()?
        .get(url)
        .bearer_auth(access_token)
        .header(header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|error| format!("fetch verified OAuth email: {error}"))?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(format!(
            "fetch verified OAuth email returned HTTP {}",
            response.status()
        ));
    }
    let body = response
        .text()
        .await
        .map_err(|error| format!("read verified OAuth email: {error}"))?;
    let emails: Vec<ProviderEmail> = serde_json::from_str(&body)
        .map_err(|error| format!("decode verified OAuth emails: {error}"))?;
    if let Some(candidate) = emails
        .iter()
        .find(|candidate| candidate.primary && candidate.verified)
    {
        user_info.insert("email".to_string(), Value::String(candidate.email.clone()));
        user_info.insert("email_verified".to_string(), Value::Bool(true));
    }
    Ok(())
}

fn claims_object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn claim_string<'a>(claims: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    claims.get(key).and_then(Value::as_str)
}

fn claim_identifier(claims: &Map<String, Value>, field: &str) -> Option<String> {
    let value = claims.get(field)?;
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn oidc_group_names(claims: &Map<String, Value>) -> Vec<String> {
    match claims.get("groups") {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

async fn sync_groups(provider: &str, user_id: i64, claims: &Map<String, Value>) {
    let groups = oidc_group_names(claims);
    if groups.is_empty() {
        return;
    }
    let ctx = crate::appcfg::ctx();
    let Ok(pool) = ctx.shared_pool().await else {
        return;
    };
    let driver = ctx.sqlx_driver();
    let rules = match crate::repository::oidc_group_rules(pool, driver, provider).await {
        Ok(rules) => rules,
        Err(_) => return,
    };
    for rule in rules {
        if !groups.iter().any(|group| group == &rule.group_name) {
            continue;
        }
        let _ = crate::repository::grant_oidc_member(
            pool,
            driver,
            rule.space_id,
            user_id,
            &rule.role,
            &crate::repository::go_timestamp(),
        )
        .await;
    }
}

async fn find_or_create_user(
    provider_name: &str,
    provider: &OauthProvider,
    claims: &Map<String, Value>,
) -> Result<UserRecord, String> {
    let identifier = claim_string(claims, "sub")
        .map(str::to_string)
        .or_else(|| {
            if provider.field_mapping_identifier.trim().is_empty() {
                None
            } else {
                claim_identifier(claims, provider.field_mapping_identifier.trim())
            }
        })
        .ok_or_else(|| "failed to get user identifier from OAuth provider".to_string())?;

    let email_field = if provider.field_mapping_email.trim().is_empty() {
        "email"
    } else {
        provider.field_mapping_email.trim()
    };
    let email = claim_string(claims, email_field)
        .unwrap_or("")
        .trim()
        .to_lowercase();
    let verified = claims
        .get("email_verified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if email.is_empty() || !verified {
        return Err("OAuth provider did not return a verified email".to_string());
    }

    let ctx = crate::appcfg::ctx();
    let pool = ctx.shared_pool().await?;
    let driver = ctx.sqlx_driver();
    let now = crate::repository::go_timestamp();

    if let Ok(Some(identity)) =
        crate::repository::oauth_identity(pool, driver, provider_name, &identifier).await
    {
        let linked = match crate::repository::user_by_id(pool, driver, identity.user_id).await {
            Ok(Some(user)) => user,
            Ok(None) => crate::repository::user_by_mail(pool, driver, &email)
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "the linked account no longer exists".to_string())?,
            Err(error) => return Err(error.to_string()),
        };
        let _ = crate::repository::link_oauth_identity(
            pool, driver, linked.id, provider_name, &identifier, &email, &now,
        )
        .await;
        if linked.status != 1 {
            return Err("user account is disabled or inactive".to_string());
        }
        sync_groups(provider_name, linked.id, claims).await;
        return Ok(linked);
    }

    if let Some(user) = crate::repository::user_by_mail(pool, driver, &email)
        .await
        .map_err(|error| error.to_string())?
    {
        crate::repository::link_oauth_identity(
            pool, driver, user.id, provider_name, &identifier, &email, &now,
        )
        .await
        .map_err(|error| error.to_string())?;
        return Ok(user);
    }

    let display_field = if provider.field_mapping_display_name.trim().is_empty() {
        "name"
    } else {
        provider.field_mapping_display_name.trim()
    };
    let display_name = claim_string(claims, display_field).unwrap_or("");
    let user_id = crate::repository::create_user(
        pool,
        driver,
        &email,
        "",
        display_name,
        "",
        2,
        &now,
    )
    .await
    .map_err(|error| error.to_string())?;
    crate::repository::link_oauth_identity(
        pool, driver, user_id, provider_name, &identifier, &email, &now,
    )
    .await
    .map_err(|error| error.to_string())?;
    sync_groups(provider_name, user_id, claims).await;
    crate::repository::user_by_id(pool, driver, user_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "the created account could not be read back".to_string())
}

/// `OAuthCallback`: validate the one-time state, exchange the code and resolve
/// the account. The caller turns the result into the success or failure redirect
/// and owns the cookies.
pub async fn complete(
    provider_name: &str,
    code: &str,
    state: &str,
    state_cookie: Option<&str>,
    provider_error: Option<&str>,
) -> Result<UserRecord, String> {
    let ctx = crate::appcfg::ctx();
    let provider = find_provider(&ctx.conf.oauth, provider_name)
        .ok_or_else(|| "provider_unsupported".to_string())?;
    if provider_error.is_some_and(|value| !value.is_empty()) {
        return Err("provider_denied".to_string());
    }
    if code.is_empty() {
        return Err("missing_code".to_string());
    }
    let cookie = state_cookie.ok_or_else(|| "state_invalid".to_string())?;
    if !constant_time_eq(state, cookie) {
        return Err("state_invalid".to_string());
    }
    let login = take_state(state).ok_or_else(|| "state_invalid".to_string())?;
    if !login.provider.eq_ignore_ascii_case(provider.name.as_str()) {
        return Err("state_invalid".to_string());
    }

    let redirect = redirect_uri(&provider.name);
    let mut claims = if provider.issuer_url.trim().is_empty() {
        let token = exchange_code(provider, provider.token_url.trim(), &redirect, code, "")
            .await
            .map_err(|_| "callback_failed".to_string())?;
        let access_token = claim_string(&claims_object(token), "access_token")
            .ok_or_else(|| "no access_token in response".to_string())?
            .to_string();
        let mut user_info = claims_object(fetch_user_info(provider, &access_token).await?);
        enrich_verified_email(provider, &access_token, &mut user_info)
            .await
            .map_err(|_| "callback_failed".to_string())?;
        user_info
    } else {
        let metadata = discover(provider).await.map_err(|_| "callback_failed".to_string())?;
        let token = exchange_code(
            provider,
            metadata.token_endpoint.trim(),
            &redirect,
            code,
            &login.pkce_verifier,
        )
        .await
        .map_err(|_| "callback_failed".to_string())?;
        let token = claims_object(token);
        let id_token = claim_string(&token, "id_token")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "callback_failed".to_string())?;
        verify_id_token(id_token, provider, &metadata, &login.nonce)
            .await
            .map_err(|_| "callback_failed".to_string())?
    };
    claims.remove("nonce");

    find_or_create_user(&provider.name, provider, &claims)
        .await
        .map_err(|_| "callback_failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        Oauth, OauthProvider, clear_state_cookie, constant_time_eq, find_provider, oidc_scopes,
        remember_state, state_cookie_name, take_state,
    };

    fn provider(name: &str) -> OauthProvider {
        OauthProvider {
            name: name.to_string(),
            client_id: "client".to_string(),
            auth_url: "https://id.example/authorize".to_string(),
            ..OauthProvider::default()
        }
    }

    #[test]
    fn provider_lookup_is_case_insensitive() {
        let oauth = Oauth {
            enable: true,
            providers: vec![provider("Authentik")],
        };
        assert_eq!(
            find_provider(&oauth, "authentik").map(|provider| provider.name.as_str()),
            Some("Authentik")
        );
        assert!(find_provider(&oauth, "github").is_none());
    }

    #[test]
    fn state_cookie_name_is_deterministic_and_truncated() {
        let name = state_cookie_name("authentik");
        assert!(name.starts_with("sun_panel_oauth_state_"));
        assert_eq!(name.len(), "sun_panel_oauth_state_".len() + 16);
        assert_eq!(name, state_cookie_name("authentik"));
        assert_ne!(name, state_cookie_name("github"));
    }

    #[test]
    fn state_is_single_use() {
        remember_state(
            "state-one",
            super::PendingLogin {
                provider: "authentik".to_string(),
                nonce: "nonce".to_string(),
                pkce_verifier: "verifier".to_string(),
            },
        );
        assert!(take_state("state-one").is_some());
        assert!(take_state("state-one").is_none());
    }

    #[test]
    fn clears_the_state_cookie_with_the_provider_path() {
        let cookie = clear_state_cookie("github");
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("Path=/api/oauth/github/callback"));
    }

    #[test]
    fn oidc_scopes_always_include_openid() {
        assert_eq!(oidc_scopes(""), "openid profile email");
        assert_eq!(oidc_scopes("profile email"), "openid profile email");
        assert_eq!(oidc_scopes("openid email"), "openid email");
    }

    #[test]
    fn constant_time_compare_matches_only_equal_strings() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(!constant_time_eq("", "x"));
    }
}
