//! Space, group, item and membership routes.

use axum::extract::{FromRequest, Multipart, Path, Query, Request};
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::json;

use crate::appcfg;
use crate::auth::{self, AuthError};
use crate::envelope::{self, error_code, success, success_data};

use super::*;

/// `SpaceRouter.List`: the caller's own Yin spaces, each with its paired Yang
/// id and the caller's edit right.
pub(super) async fn list_spaces(headers: HeaderMap) -> Response {
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
    let rows = match crate::repository::space_list(pool, ctx.sqlx_driver(), user.id).await {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let items = rows
        .into_iter()
        .map(|row| SpaceItem {
            paired_space_id: row.paired_space_id,
            can_edit: row.can_edit,
            id: row.id,
            create_time: rfc3339(&row.created_at),
            update_time: rfc3339(&row.updated_at),
            kind: row.kind,
            name: row.name,
            owner_user_id: row.owner_user_id,
            pair_id: row.pair_id,
            side: row.side,
            public_enabled: row.public_enabled,
            public_id: row.public_id,
            public_mode: row.public_mode,
        })
        .collect::<Vec<_>>();
    envelope::success_struct(items)
}

/// `SpaceRouter.GetSearchConfig`: `null` data is omitted from the envelope, so
/// an unset configuration answers `{"code":0,"msg":"OK"}`.
pub(super) async fn search_config(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
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
    let found =
        match crate::repository::space_search_config(pool, ctx.sqlx_driver(), user.id, space_id)
            .await
        {
            Ok(Some(found)) => found,
            Ok(None) | Err(_) => return error_code(404),
        };
    if found.owner_user_id != user.id && !found.is_member {
        return error_code(1005);
    }
    if found.config_json.is_empty() {
        return success();
    }
    match serde_json::from_str::<serde_json::Value>(&found.config_json) {
        Ok(value) => success_data(value),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.Groups`: one grouped count for the whole Space.
pub(super) async fn groups(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard_shared(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let rows =
        match crate::repository::groups_for_space(&pool, appcfg::ctx().sqlx_driver(), space_id)
            .await
        {
            Ok(rows) => rows,
            Err(error) => return envelope::error(&error.to_string()),
        };
    let items = rows
        .into_iter()
        .map(|row| GroupItem {
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
            item_count: row.item_count,
        })
        .collect::<Vec<_>>();
    envelope::success_struct(items)
}

/// `SpaceRouter.Items`: `sort, created_at, id` order, optional group filter and
/// the 200-row page cap.
pub(super) async fn items(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let pool = match guard_shared(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let group = query.get("groupId").cloned().unwrap_or_default();
    let group_id = if group.is_empty() {
        None
    } else {
        Some(group.parse::<i64>().unwrap_or(-1))
    };
    let page: i64 = query
        .get("page")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let page_size: i64 = query
        .get("pageSize")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let rows = match crate::repository::items_for_space(
        &pool,
        appcfg::ctx().sqlx_driver(),
        space_id,
        group_id,
        page,
        page_size,
    )
    .await
    {
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
pub(super) async fn members(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard_shared(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let rows = match crate::repository::space_members(&pool, appcfg::ctx().sqlx_driver(), space_id)
        .await
    {
        Ok(rows) => rows,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(json!({
            "email": row.email,
            "id": row.id,
            "joinedAt": rfc3339(&row.joined_at),
            "name": row.name,
            "role": row.role,
            "source": row.source,
            "spaceId": row.space_id,
            "userId": row.user_id,
        }));
    }
    success_data(serde_json::Value::Array(out))
}

/// `SpaceRouter.GetPublicConfig`.
pub(super) async fn space_public(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let user = match auth::user(&headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return error_code(1000),
        Err(AuthError::Disabled) => return error_code(1004),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/space_public", &error);
            false
        })
    {
        return error_code(1005);
    }
    let row = match crate::repository::space_public_config(&pool, ctx.sqlx_driver(), space_id).await
    {
        Ok(Some(row)) => row,
        Ok(None) | Err(_) => return error_code(404),
    };
    success_data(json!({
        "enabled": row.enabled,
        "mode": row.mode,
        "publicId": row.public_id,
    }))
}

pub(super) async fn set_space_public(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match auth::user(&headers).await {
        Ok(user) => user,
        Err(AuthError::NotLoggedIn) => return error_code(1000),
        Err(AuthError::Disabled) => return error_code(1004),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/set_space_public", &error);
            false
        })
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error(" [invalid public config]"),
    };
    let req: PublicConfigRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return envelope::error(" [invalid public config]"),
    };
    let valid_public_id = !req.public_id.is_empty()
        && req.public_id.len() <= 64
        && req
            .public_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    if req.enabled && !valid_public_id {
        return envelope::error(" [invalid publicId]");
    }
    if req.mode != "direct" && req.mode != "code" {
        return envelope::error(" [invalid public mode]");
    }
    if req.mode == "code" && req.enabled && !(4..=12).contains(&req.access_code.len()) {
        return envelope::error(" [invalid access code]");
    }
    let public_id = if req.enabled {
        Some(req.public_id.as_str())
    } else {
        None
    };
    let code_hash = if !req.enabled {
        Some(String::new())
    } else if req.mode == "code" && !req.access_code.is_empty() {
        Some(hex_digest(req.access_code.as_bytes(), true))
    } else {
        None
    };
    match crate::repository::update_space_public_config(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        req.enabled,
        public_id,
        &req.mode,
        code_hash.as_deref(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&format!("save public config: {error}")),
    }
}

/// `SpaceRouter.CreateGroup`.
pub(super) async fn create_group(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/create_group", &error);
            false
        })
    {
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
        match crate::repository::group_belongs(&pool, ctx.sqlx_driver(), space_id, parent).await {
            Ok(true) => {}
            _ => return envelope::error(" [invalid parent group]"),
        }
    }
    let now = now_like_go();
    let id = match crate::repository::create_group(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        user.id,
        &req.icon,
        &req.title,
        req.parent_id,
        &now,
    )
    .await
    {
        Ok(id) => id,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let group = match crate::repository::group_by_id(&pool, ctx.sqlx_driver(), space_id, id).await {
        Ok(Some(row)) => group_item_json(row),
        _ => return envelope::error("group not found"),
    };
    success_data(group)
}

/// `SpaceRouter.UpdateGroup`.
pub(super) async fn update_group(
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
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/update_group", &error);
            false
        })
    {
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
        let belongs = crate::repository::group_belongs(&pool, ctx.sqlx_driver(), space_id, parent)
            .await
            .unwrap_or_else(|error| {
                crate::observability::warn("routes/update_group", &error);
                false
            });
        if parent == group_id || !belongs {
            return envelope::error(" [invalid parent group]");
        }
        if crate::repository::group_has_descendant(
            &pool,
            ctx.sqlx_driver(),
            space_id,
            group_id,
            parent,
        )
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/update_group", &error);
            false
        })
        {
            return envelope::error(" [group cycle detected]");
        }
    }
    let now = now_like_go();
    match crate::repository::update_group(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        group_id,
        &req.title,
        &req.icon,
        req.parent_id,
        &now,
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.DeleteGroup`: the group, its descendants and their items.
pub(super) async fn delete_group(
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
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/delete_group", &error);
            false
        })
    {
        return error_code(1005);
    }
    match crate::repository::delete_group_tree(&pool, ctx.sqlx_driver(), space_id, group_id).await {
        Ok(()) => success(),
        Err(_) => envelope::error("delete failed"),
    }
}

/// `SpaceRouter.CreateItem`.
pub(super) async fn create_item(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/create_item", &error);
            false
        })
    {
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
    let group_exists = crate::repository::group_belongs(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        input.item_icon_group_id,
    )
    .await
    .unwrap_or_else(|error| {
        crate::observability::warn("routes/create_item", &error);
        false
    });
    if !group_exists {
        return error_code(404);
    }
    let now = now_like_go();
    let icon_json = serde_json::to_string(&input.icon).unwrap_or_else(|_| "{}".to_string());
    let record = crate::repository::ItemWriteRecord {
        icon_json,
        title: input.title,
        url: input.url,
        lan_url: input.lan_url,
        mobile_url: input.mobile_url,
        description: input.description,
        open_method: input.open_method,
        sort: input.sort,
        group_id: input.item_icon_group_id,
    };
    let id = match crate::repository::create_item(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        user.id,
        &record,
        &now,
    )
    .await
    {
        Ok(id) => id,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let row = match crate::repository::item_by_id(&pool, ctx.sqlx_driver(), space_id, id).await {
        Ok(Some(row)) => row,
        _ => return error_code(404),
    };
    success_data(item_json_value(row))
}

/// `SpaceRouter.UpdateItem`.
pub(super) async fn update_item(
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
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/update_item", &error);
            false
        })
    {
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
    let group_exists = crate::repository::group_belongs(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        input.item_icon_group_id,
    )
    .await
    .unwrap_or_else(|error| {
        crate::observability::warn("routes/update_item", &error);
        false
    });
    if !group_exists {
        return error_code(404);
    }
    let icon_json = serde_json::to_string(&input.icon).unwrap_or_else(|_| "{}".to_string());
    let record = crate::repository::ItemWriteRecord {
        icon_json,
        title: input.title,
        url: input.url,
        lan_url: input.lan_url,
        mobile_url: input.mobile_url,
        description: input.description,
        open_method: input.open_method,
        sort: input.sort,
        group_id: input.item_icon_group_id,
    };
    match crate::repository::update_item(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        item_id,
        &record,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.DeleteItem`.
pub(super) async fn delete_item(
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
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/delete_item", &error);
            false
        })
    {
        return error_code(1005);
    }
    match crate::repository::delete_item(&pool, ctx.sqlx_driver(), space_id, item_id).await {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.AddMember`: the member row exists in both paired Spaces.
pub(super) async fn add_member(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/add_member", &error);
            false
        })
    {
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
    let target = match crate::repository::user_id_by_mail(&pool, ctx.sqlx_driver(), &email).await {
        Ok(Some(target_id)) => target_id,
        Ok(None) | Err(_) => return error_code(404),
    };
    if crate::repository::space_owner(&pool, ctx.sqlx_driver(), space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/add_member", &error);
            None
        })
        .is_none()
    {
        return error_code(404);
    }
    let space_ids = match crate::repository::paired_space_ids(&pool, ctx.sqlx_driver(), space_id)
        .await
    {
        Ok(ids) => ids,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let now = now_like_go();
    match crate::repository::upsert_space_member(
        &pool,
        ctx.sqlx_driver(),
        &space_ids,
        target,
        &req.role,
        &now,
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.UpdateMember`.
pub(super) async fn update_member(
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
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/update_member", &error);
            false
        })
    {
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
    if crate::repository::space_owner(&pool, ctx.sqlx_driver(), space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/update_member", &error);
            None
        })
        .is_none()
    {
        return error_code(404);
    }
    let space_ids = match crate::repository::paired_space_ids(&pool, ctx.sqlx_driver(), space_id)
        .await
    {
        Ok(ids) => ids,
        Err(error) => return envelope::error(&error.to_string()),
    };
    match crate::repository::set_member_role(
        &pool,
        ctx.sqlx_driver(),
        &space_ids,
        target,
        &req.role,
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.RemoveMember`: the owner cannot be removed.
pub(super) async fn remove_member(
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
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/remove_member", &error);
            false
        })
    {
        return error_code(1005);
    }
    let owner = match crate::repository::space_owner(&pool, ctx.sqlx_driver(), space_id).await {
        Ok(Some(owner)) => owner,
        Ok(None) | Err(_) => return error_code(404),
    };
    if target == owner {
        return envelope::error("the space owner cannot be removed; leave the space to delete it");
    }
    let space_ids = match crate::repository::paired_space_ids(&pool, ctx.sqlx_driver(), space_id)
        .await
    {
        Ok(ids) => ids,
        Err(error) => return envelope::error(&error.to_string()),
    };
    match crate::repository::remove_space_member(
        &pool,
        ctx.sqlx_driver(),
        &space_ids,
        target,
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.Rename`: the Yang twin becomes `<name>-B`.
pub(super) async fn rename_space(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/rename_space", &error);
            false
        })
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: RenameRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    if req.name.is_empty() {
        return envelope::error(" [ Name为必填字段]");
    }
    if req.name.chars().count() > 100 {
        return envelope::error(" [ Name长度不能超过100个字符]");
    }
    if crate::repository::space_owner(&pool, ctx.sqlx_driver(), space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/rename_space", &error);
            None
        })
        .is_none()
    {
        return error_code(404);
    }
    match crate::repository::rename_paired_space(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        &req.name,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.SetSearchConfig`.
pub(super) async fn set_search_config(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/set_search_config", &error);
            false
        })
    {
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
    match crate::repository::update_space_search_config(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        &value.to_string(),
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.GetWallpaperConfig`.
pub(super) async fn get_wallpaper(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard_shared(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let stored =
        match crate::repository::space_wallpaper_json(&pool, appcfg::ctx().sqlx_driver(), space_id)
            .await
        {
            Ok(Some(stored)) => stored,
            Ok(None) | Err(_) => return error_code(404),
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

/// `SpaceRouter.SetWallpaperConfig`: `none` stores an empty column.
pub(super) async fn set_wallpaper(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/set_wallpaper", &error);
            false
        })
    {
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
    match crate::repository::update_space_wallpaper(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        &stored,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.OIDCGroups` (the list the routes expose alongside add/delete).
pub(super) async fn list_oidc_groups(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let pool = match guard_shared(&headers, space_id).await {
        Ok(pool) => pool,
        Err(response) => return response,
    };
    let rows =
        match crate::repository::oidc_groups(&pool, appcfg::ctx().sqlx_driver(), space_id).await {
            Ok(rows) => rows,
            Err(error) => return envelope::error(&error.to_string()),
        };
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "createTime": rfc3339(&row.created_at),
                "updateTime": rfc3339(&row.updated_at),
                "spaceId": space_id,
                "provider": row.provider,
                "groupName": row.group_name,
                "role": row.role,
            })
        })
        .collect();
    success_data(serde_json::Value::Array(list))
}

/// `SpaceRouter.AddOIDCGroup`: a rule may grant editor or viewer, never admin.
pub(super) async fn add_oidc_group(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/add_oidc_group", &error);
            false
        })
    {
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
    let id = match crate::repository::add_oidc_group(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        &req.provider,
        &req.group_name,
        &req.role,
        &now,
    )
    .await
    {
        Ok(id) => id,
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

/// `SpaceRouter.DeleteOIDCGroup`.
pub(super) async fn delete_oidc_group(
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
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/delete_oidc_group", &error);
            false
        })
    {
        return error_code(1005);
    }
    match crate::repository::delete_oidc_group(&pool, ctx.sqlx_driver(), space_id, rule_id).await {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.Transfer`: the new owner is made admin, the old one editor.
pub(super) async fn transfer_space(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/transfer_space", &error);
            false
        })
    {
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
    let target_id = match crate::repository::user_id_by_mail(&pool, ctx.sqlx_driver(), &email).await
    {
        Ok(Some(id)) => id,
        Ok(None) | Err(_) => return error_code(404),
    };
    match crate::repository::transfer_space(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        user.id,
        target_id,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(_) => envelope::error("transfer failed"),
    }
}

/// `SpaceRouter.ClearSpace`: items and groups, in one transaction.
pub(super) async fn clear_space(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_manage_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/clear_space", &error);
            false
        })
    {
        return error_code(1005);
    }
    match crate::repository::clear_space(&pool, ctx.sqlx_driver(), space_id).await {
        Ok(()) => success(),
        Err(_) => envelope::error("clear failed"),
    }
}

/// `SpaceRouter.CreateSpace`: the Yin Space, its Yang twin, both admin rows and
/// the default `APP` group in each.
pub(super) async fn create_space(headers: HeaderMap, request: Request) -> Response {
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
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    if req.name.is_empty() {
        return envelope::error(" [ Name为必填字段]");
    }
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    match crate::repository::create_paired_space(
        &pool,
        ctx.sqlx_driver(),
        user.id,
        &req.name,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.SortItems`.
pub(super) async fn sort_items(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/sort_items", &error);
            false
        })
    {
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
    let entries: Vec<(i64, i64)> = req
        .sort_items
        .iter()
        .map(|entry| (entry.id, entry.sort))
        .collect();
    match crate::repository::sort_items(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        req.group_id,
        &entries,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(_) => envelope::error("sort failed"),
    }
}

/// `SpaceRouter.SortGroups`: every sibling must appear exactly once with a
/// distinct, positive sort value.
pub(super) async fn sort_groups(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/sort_groups", &error);
            false
        })
    {
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
        let belongs = crate::repository::group_belongs(&pool, ctx.sqlx_driver(), space_id, parent)
            .await
            .unwrap_or_else(|error| {
                crate::observability::warn("routes/sort_groups", &error);
                false
            });
        if !belongs {
            return envelope::error(" [invalid parent group]");
        }
    }
    let siblings =
        match crate::repository::group_siblings(&pool, ctx.sqlx_driver(), space_id, req.parent_id)
            .await
        {
            Ok(siblings) => siblings,
            Err(error) => return envelope::error(&error.to_string()),
        };
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
    let entries: Vec<(i64, i64)> = req
        .sort_groups
        .iter()
        .map(|entry| (entry.id, entry.sort))
        .collect();
    match crate::repository::sort_groups(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        &entries,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(_) => envelope::error("sort failed"),
    }
}

/// `SpaceRouter.ArrangeLayout`: items may change group and sort, groups sort.
pub(super) async fn arrange_layout(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/arrange_layout", &error);
            false
        })
    {
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
    // Existence checks run inside the repository transaction so a concurrent
    // move cannot land between the check and the update.
    let mut items = Vec::with_capacity(req.items.len());
    for item in &req.items {
        items.push((item.id, item.item_icon_group_id, item.sort));
    }
    let mut groups = Vec::with_capacity(req.groups.len());
    for group in &req.groups {
        groups.push((group.id, group.sort));
    }
    for item in &req.items {
        if !crate::repository::item_belongs(&pool, ctx.sqlx_driver(), space_id, item.id)
            .await
            .unwrap_or_else(|error| {
                crate::observability::warn("routes/arrange_layout", &error);
                false
            })
        {
            return envelope::error(&format!(" [item {} is not in this space]", item.id));
        }
        if !crate::repository::group_belongs(
            &pool,
            ctx.sqlx_driver(),
            space_id,
            item.item_icon_group_id,
        )
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/arrange_layout", &error);
            false
        })
        {
            return envelope::error(&format!(
                " [target group {} is not in this space]",
                item.item_icon_group_id
            ));
        }
    }
    for group in &req.groups {
        if !crate::repository::group_belongs(&pool, ctx.sqlx_driver(), space_id, group.id)
            .await
            .unwrap_or_else(|error| {
                crate::observability::warn("routes/arrange_layout", &error);
                false
            })
        {
            return envelope::error(&format!(" [group {} is not in this space]", group.id));
        }
    }
    match crate::repository::arrange_layout(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        &items,
        &groups,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `SpaceRouter.DeleteSpace` plus `deleteSpaceData`: the rows go in one
/// transaction, and a stored file is removed only when nothing else references
/// it. Storage failures after the commit are logged, never rolled back.
pub(super) async fn delete_space(headers: HeaderMap, Path(space_id): Path<i64>) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let (owner, kind) =
        match crate::repository::space_owner_and_type(&pool, ctx.sqlx_driver(), space_id).await {
            Ok(Some(space)) => space,
            Ok(None) | Err(_) => return error_code(404),
        };
    if owner != user.id {
        return error_code(1005);
    }
    if kind == "personal" {
        return envelope::error("a personal space cannot be deleted");
    }
    let ids = match crate::repository::paired_space_ids(&pool, ctx.sqlx_driver(), space_id).await {
        Ok(ids) => ids,
        Err(error) => return envelope::error(&error.to_string()),
    };
    let removed = match crate::repository::delete_paired_space(&pool, ctx.sqlx_driver(), &ids).await
    {
        Ok(removed) => removed,
        Err(_) => return envelope::error("delete failed"),
    };

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
/// `SpaceRouter.CreateItemWithIcon`: the item arrives as a JSON form field and
/// the icon as an optional SHA-256-named file beside it.
pub(super) async fn create_item_with_icon(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    mut multipart: Multipart,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/create_item_with_icon", &error);
            false
        })
    {
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
    if !crate::repository::group_belongs(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        input.item_icon_group_id,
    )
    .await
    .unwrap_or_else(|error| {
        crate::observability::warn("routes/create_item_with_icon", &error);
        false
    })
    {
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
    let id = match crate::repository::create_item_with_icon(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        user.id,
        &icon_json,
        &input.title,
        &input.url,
        &input.lan_url,
        &input.mobile_url,
        &input.description,
        input.open_method,
        input.sort,
        input.item_icon_group_id,
        stored.as_deref(),
        &now_like_go(),
    )
    .await
    {
        Ok(id) => id,
        Err(error) => return envelope::error(&error.to_string()),
    };
    match crate::repository::item_by_id(&pool, ctx.sqlx_driver(), space_id, id).await {
        Ok(Some(row)) => success_data(crate::routes::item_json_value(row)),
        _ => error_code(404),
    }
}

/// `SpaceRouter.ImportBookmarksBatch`.
pub(super) async fn import_bookmarks(
    headers: HeaderMap,
    Path(space_id): Path<i64>,
    request: Request,
) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_edit_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/import_bookmarks", &error);
            false
        })
    {
        return error_code(1005);
    }

    let mut multipart = match Multipart::from_request(request, &()).await {
        Ok(multipart) => multipart,
        Err(_) => return envelope::error(" [invalid bookmarks]"),
    };

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
        if crate::repository::insert_file_in_tx(
            &mut tx,
            ctx.sqlx_driver(),
            user.id,
            &icon.file_name,
            &now,
        )
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
        if let Err(error) = import_node(
            &mut tx,
            ctx.sqlx_driver(),
            &icons,
            space_id,
            user.id,
            None,
            node,
            index as i64,
            &mut summary,
        )
        .await
        {
            return envelope::error(&error);
        }
    }
    for (icon_json, title, group_id, sort, space, owner, url) in &summary.items {
        if crate::repository::insert_imported_item_in_tx(
            &mut tx,
            ctx.sqlx_driver(),
            icon_json,
            title,
            url,
            *sort,
            *group_id,
            *owner,
            *space,
            &now,
        )
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

/// `SetUserSpaceThemeV2`: a per-Space override for the caller.
pub(super) async fn set_user_space_theme(
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
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let driver = ctx.sqlx_driver();
    let revision_id = if !req.revision.is_empty() {
        req.revision.clone()
    } else if !req.package_id.is_empty() {
        match crate::repository::latest_revision(&pool, driver, &req.package_id).await {
            Ok(Some(id)) => id,
            _ => return error_code(404),
        }
    } else {
        return envelope::error(" [packageId or revision is required]");
    };
    let package_id =
        match crate::repository::package_id_for_revision(&pool, driver, &revision_id).await {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => return error_code(404),
        };
    match crate::repository::save_user_space_theme(
        &pool,
        driver,
        user.id,
        space_id,
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

pub(super) async fn set_space_theme(
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
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let driver = ctx.sqlx_driver();
    if crate::repository::space_owner(&pool, driver, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/set_space_theme", &error);
            None
        })
        .is_none()
    {
        return error_code(404);
    }
    if !crate::repository::can_manage_space(&pool, driver, user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/set_space_theme", &error);
            false
        })
    {
        return error_code(1005);
    }
    let body = match axum::body::to_bytes(request.into_body(), 1 << 20).await {
        Ok(body) => body,
        Err(_) => return envelope::error("invalid request body"),
    };
    let req: RevisionRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(error) => return envelope::error(&format!(" [{}]", go_json_error(&error))),
    };
    let revision_id = if !req.revision.is_empty() {
        req.revision.clone()
    } else if !req.package_id.is_empty() {
        match crate::repository::latest_revision(&pool, driver, &req.package_id).await {
            Ok(Some(id)) => id,
            _ => return error_code(404),
        }
    } else {
        return envelope::error(" [packageId or revision is required]");
    };
    let package_id =
        match crate::repository::package_id_for_revision(&pool, driver, &revision_id).await {
            Ok(Some(id)) => id,
            Ok(None) | Err(_) => return error_code(404),
        };
    match crate::repository::save_space_theme(
        &pool,
        driver,
        space_id,
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

pub(super) async fn clear_space_theme(headers: HeaderMap, Path(space_id): Path<String>) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let space_id: i64 = match space_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [spaceId must be a positive integer]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    let driver = ctx.sqlx_driver();
    if crate::repository::space_owner(&pool, driver, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/clear_space_theme", &error);
            None
        })
        .is_none()
    {
        return error_code(404);
    }
    if !crate::repository::can_manage_space(&pool, driver, user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/clear_space_theme", &error);
            false
        })
    {
        return error_code(1005);
    }
    match crate::repository::clear_space_theme_row(&pool, driver, space_id).await {
        Ok(()) => success(),
        Err(error) => envelope::error(&error.to_string()),
    }
}

/// `ClearUserSpaceThemeV2`.
pub(super) async fn clear_user_space_theme(headers: HeaderMap, Path(space_id): Path<String>) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let space_id: i64 = match space_id.parse() {
        Ok(value) => value,
        Err(_) => return envelope::error(" [spaceId must be a positive integer]"),
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool,
        Err(error) => return envelope::error(&error),
    };
    match crate::repository::clear_user_space_theme_row(
        pool,
        ctx.sqlx_driver(),
        user.id,
        space_id,
    )
    .await
    {
        Ok(()) => success(),
        Err(_) => envelope::database_error(),
    }
}

/// `SpaceRouter.Copy`: a new shared Space holding copies of every group and item.
pub(super) async fn copy_space(headers: HeaderMap, Path(space_id): Path<i64>, request: Request) -> Response {
    let user = match require_user(&headers).await {
        Ok(user) => user,
        Err(response) => return response,
    };
    let ctx = appcfg::ctx();
    let pool = match ctx.shared_pool().await {
        Ok(pool) => pool.clone(),
        Err(error) => return envelope::error(&error),
    };
    if !crate::repository::can_access_space(&pool, ctx.sqlx_driver(), user.id, space_id)
        .await
        .unwrap_or_else(|error| {
            crate::observability::warn("routes/copy_space", &error);
            false
        })
    {
        return error_code(1005);
    }
    let source_name = match crate::repository::space_name(&pool, ctx.sqlx_driver(), space_id).await
    {
        Ok(Some(name)) => name,
        Ok(None) | Err(_) => return error_code(404),
    };
    let body = axum::body::to_bytes(request.into_body(), 1 << 20)
        .await
        .unwrap_or_default();
    let req: RenameRequest = serde_json::from_slice(&body).unwrap_or(RenameRequest {
        name: String::new(),
    });
    let name = if req.name.is_empty() {
        format!("{source_name} Copy")
    } else {
        req.name
    };
    match crate::repository::copy_space(
        &pool,
        ctx.sqlx_driver(),
        space_id,
        user.id,
        &name,
        &now_like_go(),
    )
    .await
    {
        Ok(()) => success(),
        Err(_) => envelope::database_error(),
    }
}
