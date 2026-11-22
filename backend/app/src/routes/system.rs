//! Health, capabilities, OAuth and session routes.

use axum::extract::{Path, Query, Request};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use serde_json::json;

use crate::appcfg;
use crate::auth::{self, AuthError};
use crate::envelope::{self, error_code, success, success_data};

use super::*;

/// `AboutRouter.Get`.
pub(super) async fn about() -> Response {
    success_data(json!({
        "buildTime": appcfg::build_time(),
        "commit": appcfg::build_commit(),
        "versionName": appcfg::version_name(),
    }))
}

/// `CapabilitiesRouter.Get`: Core registers no extension modules (ADR-002).
pub(super) async fn capabilities() -> Response {
    success_data(json!({
        "capabilities": [],
        "edition": "core",
        "readOnly": false,
        "versionName": appcfg::version_name(),
    }))
}

/// `OAuthRouter.GetConfig`: `providers` is a nil slice in Go, which encodes as
/// `null`, not `[]`.
pub(super) async fn oauth_config() -> Response {
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

/// `OAuthRouter.OAuthLogin`. The Go router only registers the route when
/// `oauth.enable` is true; a disabled deployment answers Gin's plain-text 404.
pub(super) async fn oauth_login(Path(provider): Path<String>) -> Response {
    if !appcfg::ctx().conf.oauth.enable {
        return crate::oauth::disabled_404();
    }
    match crate::oauth::begin(&provider).await {
        Ok(start) => redirect_with_cookies(&start.location, &[start.state_cookie]),
        Err(message) => envelope::error(&message),
    }
}

/// `OAuthRouter.OAuthCallback` plus the service's code exchange and account
/// resolution. Every outcome clears the one-time state cookie.
pub(super) async fn oauth_callback(
    Path(provider): Path<String>,
    Query(query): Query<OauthCallbackQuery>,
    headers: HeaderMap,
) -> Response {
    if !appcfg::ctx().conf.oauth.enable {
        return crate::oauth::disabled_404();
    }
    let state_cookie = cookie_named(&headers, &crate::oauth::state_cookie_name(&provider));
    let result = crate::oauth::complete(
        &provider,
        &query.code,
        &query.state,
        state_cookie.as_deref(),
        query.error.as_deref(),
    )
    .await;
    let user = match result {
        Ok(user) => user,
        Err(reason) => return oauth_failure(&provider, &reason),
    };
    let ctx = appcfg::ctx();
    let token = match auth::issue_token(user.id, user.token_version) {
        Ok(token) => token,
        Err(_) => return oauth_failure(&provider, "token_failed"),
    };
    let max_age = ctx.jwt_expire_hours() * 3600;
    crate::observability::count_login();
    let session = auth::session_cookie(&token, max_age, ctx.secure_cookies());
    let location = format!("{}/oauth/callback", ctx.root_url());
    redirect_with_cookies(
        &location,
        &[crate::oauth::clear_state_cookie(&provider), session],
    )
}

/// `LoginRouter.Login`.
pub(super) async fn login(request: Request) -> Response {
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
    crate::observability::count_login();
    let cookie = auth::session_cookie(&token, max_age, ctx.secure_cookies());
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

/// `UserRouter.GetUser`: the caller's own row, with an empty token field.
pub(super) async fn get_user(headers: HeaderMap) -> Response {
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
pub(super) async fn logout(headers: HeaderMap) -> Response {
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
        HeaderValue::from_str(&auth::cleared_cookie(appcfg::ctx().secure_cookies()))
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    )
}

/// `MonitorRouter.GetEnableStatus`.
pub(super) async fn monitor_enable_status(headers: HeaderMap) -> Response {
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

/// `MonitorRouter.GetSnapshot`.
pub(super) async fn monitor_snapshot(headers: HeaderMap) -> Response {
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
pub(super) async fn monitor_disk_mountpoints(headers: HeaderMap) -> Response {
    if let Err(response) = monitor_guard(&headers).await {
        return response;
    }
    match crate::monitor::disk_mountpoints() {
        Ok(mountpoints) => success_data(json!(mountpoints)),
        Err(error) => envelope::error(&error),
    }
}

/// `MonitorRouter.GetDiskStateByPath`.
pub(super) async fn monitor_disk_state(request: Request) -> Response {
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

/// `ItemIconRouter.GetSiteFavicon`: discover the icon, download it, store it and
/// record the file row. The name is content-addressed by time in Go, so it is
/// reproduced the same way rather than being made deterministic.
pub(super) async fn get_site_favicon(headers: HeaderMap, request: Request) -> Response {
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
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
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
        return envelope::database_error();
    }
    // `ItemIconGetSiteFaviconResp` keeps `iconUrl` before `fileName`.
    success_data(json!({
        "iconUrl": format!("{}{}", upload_prefix(), file_name),
        "fileName": file_name,
    }))
}
