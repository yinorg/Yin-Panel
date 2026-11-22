//! Account, configuration and bookmark routes.

use axum::extract::{Path, Query, Request};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::appcfg;
use crate::auth::{self, AuthError};
use crate::envelope::{self, error_code, success, success_data};

use super::*;

/// `ModuleConfigRouter.GetByName`: a missing row answers `"data": null`, which
/// Go produces by handing a nil map to a non-nil interface.
pub(super) async fn module_config(
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
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/module_config", &error);
            None
        });
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

/// `SpaceRouter.BookmarkTree`: groups ordered `sort, id`, items ordered
/// `sort, id`, folded into a tree capped at `bookmarkTreeMaxDepth`.
pub(super) async fn bookmarks_tree(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
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

/// `UsersRouter.GetList`: `LIMIT 0` for an absent `limit`, so an unpaged call
/// answers an empty list (not null) with the real total.
pub(super) async fn users_list(
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
pub(super) async fn user_config(headers: HeaderMap) -> Response {
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
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/user_config", &error);
            None
        });
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

/// `ModuleConfigRouter.Save`: Go binds `Value` into `map[string]interface{}`, so
/// a non-object value is rejected with the decoder's own message.
pub(super) async fn module_config_save(headers: HeaderMap, request: Request) -> Response {
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
pub(super) async fn user_config_save(headers: HeaderMap, request: Request) -> Response {
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

/// `UserService.CreateUser`: the account, its personal pair, both memberships
/// and the default group in one transaction.
pub(super) async fn create_user(headers: HeaderMap, request: Request) -> Response {
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
pub(super) async fn update_user(headers: HeaderMap, request: Request) -> Response {
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

/// `UserRepo.DeleteWithSpaces`.
pub(super) async fn delete_user(headers: HeaderMap, request: Request) -> Response {
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

/// `UserRouter.UpdateInfo`: a 3–15 character nickname and an optional avatar.
pub(super) async fn update_user_info(headers: HeaderMap, request: Request) -> Response {
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

/// `UserRouter.UpdatePasssword`: check the old password, store the new hash and
/// revoke every issued token.
pub(super) async fn update_password(headers: HeaderMap, request: Request) -> Response {
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
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/update_password", &error);
            None
        })
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
