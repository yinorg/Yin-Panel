//! Theme package, activation and admin routes.

use axum::extract::{FromRequest, Multipart, Path, Query, Request};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use serde_json::json;

use crate::appcfg;
use crate::auth::{self};
use crate::envelope::{self, error_code, success, success_data};

use super::*;

/// `ThemeRouter.Preference`: the stored mode, or `auto` / `follow-space`.
pub(super) async fn theme_preference(headers: HeaderMap) -> Response {
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
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/theme_preference", &error);
            None
        });
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
pub(super) async fn theme_space(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    if let Err(response) = require_user(&headers).await {
        return response;
    }
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let space_theme =
        crate::repository::space_theme_pair(pool, ctx.sqlx_driver(), space_id)
            .await
            .unwrap_or_else(|error| {
                crate::observability::warn("routes/theme_space", &error);
                None
            });
    let override_theme = crate::repository::user_space_theme_pair(
        pool,
        ctx.sqlx_driver(),
        user.id,
        space_id,
    )
    .await
    .unwrap_or_else(|error| {
        crate::observability::warn("routes/theme_space", &error);
        None
    });
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

/// `ThemeRouter.CurrentV2`: the instance default package.
pub(super) async fn theme_current() -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    match instance_default_package(pool, ctx.sqlx_driver()).await {
        Ok(value) => success_data(value),
        Err(error) => envelope::error(&error),
    }
}

/// `ThemeRouter.PublicListV2`.
pub(super) async fn theme_packages() -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let state = match crate::repository::theme_admin_state(pool, ctx.sqlx_driver()).await {
        Ok(state) => state,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let out: Vec<serde_json::Value> = state
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
    success_data(serde_json::Value::Array(out))
}

/// `ThemeRouter.PackageByIDV2`.
pub(super) async fn theme_package(Path(package_id): Path<String>) -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let revision = match crate::repository::latest_revision(pool, ctx.sqlx_driver(), &package_id)
        .await
    {
        Ok(Some(revision)) => revision,
        _ => return envelope::empty_not_found(),
    };
    match revision_public(pool, ctx.sqlx_driver(), &revision).await {
        Ok(value) => success_data(value),
        Err(_) => envelope::empty_not_found(),
    }
}

/// `ThemeRouter.Mine`: the revision the caller runs and their stored choice.
pub(super) async fn theme_mine(headers: HeaderMap) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let instance = match crate::repository::latest_revision(
        pool,
        ctx.sqlx_driver(),
        "org.yin.default",
    )
    .await
    {
        Ok(Some(revision)) => revision,
        _ => return envelope::error("package not found"),
    };
    let system = crate::repository::activation_revision(pool, ctx.sqlx_driver(), "instance")
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/theme_mine", &error);
            Some(instance)
        })
        .unwrap_or_default();
    let scope = format!("user:{}", user.id);
    let selected = crate::repository::activation_revision(pool, ctx.sqlx_driver(), &scope)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/theme_mine", &error);
            None
        });
    let revision = selected.clone().unwrap_or(system);
    let package = match revision_public(pool, ctx.sqlx_driver(), &revision).await {
        Ok(value) => value,
        Err(error) => return envelope::error(&error),
    };
    let (mode, theme_mode) = user_theme_modes(pool, ctx.sqlx_driver(), user.id).await;
    let package_id = crate::repository::activation_package(pool, ctx.sqlx_driver(), &scope)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/theme_mine", &error);
            None
        })
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

/// `ResolveThemeV2` and `ThemeRouter.EffectiveV2`.
pub(super) async fn theme_effective(
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
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let driver = ctx.sqlx_driver();
    let instance = match crate::repository::latest_revision(pool, driver, "org.yin.default").await
    {
        Ok(Some(revision)) => revision,
        _ => return envelope::error("package not found"),
    };
    let system = crate::repository::activation_revision(pool, driver, "instance")
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/theme_effective", &error);
            Some(instance)
        })
        .unwrap_or_default();

    let mut resolution: Option<(String, &str)> = None;
    if let (Some(user), Some(space)) = (&user, space_id) {
        if let Ok(Some(revision)) =
            crate::repository::user_space_theme_revision(pool, driver, user.id, space).await
        {
            resolution = Some((revision, "user-space"));
        }
    }
    if resolution.is_none() {
        if let Some(user) = &user {
            let (_, choice) = user_theme_modes(pool, driver, user.id).await;
            if choice == "custom" {
                let scope = format!("user:{}", user.id);
                if let Ok(Some(revision)) =
                    crate::repository::activation_revision(pool, driver, &scope).await
                {
                    resolution = Some((revision, "user"));
                }
            }
        }
    }
    if resolution.is_none() {
        if let Some(space) = space_id {
            if let Ok(Some(revision)) = crate::repository::space_theme_revision(pool, driver, space).await
            {
                resolution = Some((revision, "space"));
            }
        }
    }
    let (revision, source) = resolution.unwrap_or((system, "system"));
    let package = match revision_public(pool, driver, &revision).await {
        Ok(value) => value,
        Err(error) => return envelope::error(&error),
    };
    let mode = match &user {
        Some(user) => user_theme_modes(pool, driver, user.id).await.0,
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
pub(super) async fn theme_asset(
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
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let row = crate::repository::theme_asset_content(pool, ctx.sqlx_driver(), &revision, name)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/theme_asset", &error);
            None
        });
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

/// `ThemeRouter.SetPreference`.
pub(super) async fn set_theme_preference(headers: HeaderMap, request: Request) -> Response {
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
pub(super) async fn admin_revisions(headers: HeaderMap) -> Response {
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
pub(super) async fn admin_audit(headers: HeaderMap) -> Response {
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
pub(super) async fn admin_trusted(headers: HeaderMap, Path(revision): Path<String>) -> Response {
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
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/admin_trusted", &error);
            None
        })
        .unwrap_or(false);
    success_data(json!({
        "enabled": enabled,
        "revision": revision,
    }))
}

/// `ThemeRouter.PackageV2`: one revision's public package.
pub(super) async fn theme_revision(Path(revision): Path<String>) -> Response {
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    match revision_public(pool, ctx.sqlx_driver(), &revision).await {
        Ok(value) => success_data(value),
        Err(_) => envelope::empty_not_found(),
    }
}

/// `ThemeRouter.SetTrustedRuntimePolicyV2`: an administrator switches the
/// trusted runtime on or off for one revision.
pub(super) async fn set_trusted(
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
            Ok(None) => return error_code(404),
            Err(error) => {
                crate::observability::warn("routes/set_trusted", &error);
                return error_code(404);
            }
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

/// `ThemeRouter.ThemeGrantV2`.
pub(super) async fn theme_grant(
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
    let manifest_json =
        match crate::repository::revision_manifest(pool, driver, &revision).await {
            Ok(Some(manifest)) => manifest,
            Ok(None) => return error_code(404),
            Err(error) => {
                crate::observability::warn("routes/theme_grant", &error);
                return error_code(404);
            }
        };
    let manifest: serde_json::Value = serde_json::from_str(&manifest_json).unwrap_or_default();
    let available = supported_modes(&manifest)
        .iter()
        .any(|mode| mode == &execution_mode);
    let trusted = crate::repository::trusted_enabled(pool, driver, &revision)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/theme_grant", &error);
            None
        })
        .unwrap_or(false);
    let available = if execution_mode == "trusted" {
        available && trusted
    } else {
        available
    };
    let grant = crate::repository::grant_row(pool, driver, user.id, &revision, &execution_mode)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/theme_grant", &error);
            None
        });
    if let Some(grant) = grant {
        let permissions: Vec<String> =
            serde_json::from_str(&grant.permissions_json).unwrap_or_default();
        return success_data(json!({
            "available": available,
            "executionMode": grant.execution_mode,
            "granted": available,
            "permissions": permissions,
            "revision": grant.revision_id,
        }));
    }
    if let Some(permissions) = implicit_builtin_permissions(pool, driver, &revision).await {
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

/// `ThemeRouter.SetThemeGrantV2`.
pub(super) async fn set_theme_grant(
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
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    let driver = ctx.sqlx_driver();
    let manifest_json = match crate::repository::revision_manifest(pool, driver, &revision).await {
        Ok(Some(manifest)) => manifest,
        Ok(None) => return error_code(404),
        Err(error) => {
            crate::observability::warn("routes/set_theme_grant", &error);
            return error_code(404);
        }
    };
    let manifest: serde_json::Value = match serde_json::from_str(&manifest_json) {
        Ok(value) => value,
        Err(_) => return envelope::database_error(),
    };
    if let Err(error) = validate_grant(&manifest, &execution_mode, &req.permissions) {
        return envelope::error(&format!(" [{error}]"));
    }
    if execution_mode == "trusted"
        && !crate::repository::trusted_enabled(pool, driver, &revision)
            .await
            .unwrap_or_else(|error| {
                crate::observability::warn("routes/set_theme_grant", &error);
                None
            })
            .unwrap_or(false)
    {
        return envelope::error(" [trusted runtime is not enabled for this revision]");
    }
    let permissions = serde_json::to_string(&req.permissions).unwrap_or_else(|_| "[]".to_string());
    match crate::repository::save_grant(
        pool,
        driver,
        user.id,
        &revision,
        &execution_mode,
        &permissions,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

pub(super) async fn theme_preview_upload(headers: HeaderMap, request: Request) -> Response {
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
pub(super) async fn theme_install(headers: HeaderMap, request: Request) -> Response {
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
pub(super) async fn theme_preview_get(Path(token): Path<String>) -> Response {
    match crate::packages::get_preview(&token) {
        Some(package) => success_data(package.preview_public(&token)),
        None => envelope::empty_not_found(),
    }
}

/// `GET /api/theme/v2/preview/:token/assets/*name`.
pub(super) async fn theme_preview_asset(
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

/// `RemovePackageV2`: built-in themes are protected; a removed package's
/// activations fall back to the seeded Yin package.
pub(super) async fn remove_package(headers: HeaderMap, Path(package_id): Path<String>) -> Response {
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
pub(super) async fn set_default_package(headers: HeaderMap, request: Request) -> Response {
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
            Ok(None) => return error_code(404),
            Err(error) => {
                crate::observability::warn("routes/set_default_package", &error);
                return error_code(404);
            }
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
pub(super) async fn begin_trial(headers: HeaderMap, request: Request) -> Response {
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
            Ok(None) => return error_code(404),
            Err(error) => {
                crate::observability::warn("routes/begin_trial", &error);
                return error_code(404);
            }
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
pub(super) async fn confirm_trial(headers: HeaderMap, request: Request) -> Response {
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
        Ok(None) => return envelope::error(" [record not found]"),
        Err(error) => {
            crate::observability::warn("routes/confirm_trial", &error);
            return envelope::error(" [record not found]");
        }
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
pub(super) async fn rollback_theme(headers: HeaderMap) -> Response {
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
        Ok(None) => return envelope::database_error(),
        Err(error) => {
            crate::observability::warn("routes/rollback_theme", &error);
            return envelope::database_error();
        }
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
