//! Space, membership and paired-space operations.

use sqlx::{AnyPool, Row};

use crate::db::{Driver, query};

use super::*;

pub async fn space_search_config(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
) -> Result<Option<SpaceSearchConfig>, sqlx::Error> {
    let (space_table, text_type) = if driver == Driver::MySql {
        ("`space`", "CHAR")
    } else {
        ("\"space\"", "TEXT")
    };
    let sql = query(
        driver,
        &format!(
            "SELECT s.owner_user_id, CAST(s.search_config_json AS {text_type}) AS config_json, CASE WHEN EXISTS (SELECT 1 FROM space_member m WHERE m.space_id = s.id AND m.user_id = ?) THEN 1 ELSE 0 END AS is_member FROM {space_table} s WHERE s.id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(SpaceSearchConfig {
            owner_user_id: row.try_get("owner_user_id")?,
            config_json: text(&row, "config_json")?,
            is_member: row.try_get::<i64, _>("is_member")? != 0,
        })
    })
    .transpose()
}

pub async fn can_edit_space(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
) -> Result<bool, sqlx::Error> {
    let space_table = if driver == Driver::MySql {
        "`space`"
    } else {
        "\"space\""
    };
    let sql = query(
        driver,
        &format!(
            "SELECT 1 FROM {space_table} s WHERE s.id = ? AND (s.owner_user_id = ? OR EXISTS (SELECT 1 FROM space_member m WHERE m.space_id = s.id AND m.user_id = ? AND m.role IN ('admin', 'editor')))"
        ),
    );
    Ok(sqlx::query(&sql)
        .bind(space_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .is_some())
}

pub async fn update_space_search_config(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    config_json: &str,
    updated_at: &str,
) -> Result<(), sqlx::Error> {
    let (space_table, timestamp_expression) = match driver {
        Driver::MySql => ("`space`", "CAST(LEFT(?, 26) AS DATETIME(6))"),
        Driver::Sqlite => ("\"space\"", "?"),
        Driver::Postgres => ("\"space\"", "CAST(? AS TIMESTAMP WITH TIME ZONE)"),
    };
    let sql = query(
        driver,
        &format!(
            "UPDATE {space_table} SET search_config_json = ?, updated_at = {timestamp_expression} WHERE id = ?"
        ),
    );
    sqlx::query(&sql)
        .bind(config_json)
        .bind(updated_at)
        .bind(space_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `SpaceRouter.Items`: `sort, created_at, id` order, optional group filter and
/// the 200-row page cap the Go handler applies.
pub async fn items_for_space(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    group_id: Option<i64>,
    page: i64,
    page_size: i64,
) -> Result<Vec<ItemListRecord>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let mut sql = format!(
        "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST(icon_json AS {text_type}) AS icon_json, CAST(title AS {text_type}) AS title, CAST(url AS {text_type}) AS url, CAST(lan_url AS {text_type}) AS lan_url, CAST(mobile_url AS {text_type}) AS mobile_url, CAST(description AS {text_type}) AS description, open_method, sort, item_icon_group_id, user_id, space_id FROM item_icon WHERE space_id = ?"
    );
    if group_id.is_some() {
        sql.push_str(" AND item_icon_group_id = ?");
    }
    sql.push_str(" ORDER BY sort, created_at, id");
    let paged = page > 0;
    let size = if page_size < 1 {
        50
    } else {
        page_size.min(200)
    };
    if paged {
        sql.push_str(" LIMIT ? OFFSET ?");
    }
    let sql = query(driver, &sql);
    let mut statement = sqlx::query(&sql).bind(space_id);
    if let Some(group_id) = group_id {
        statement = statement.bind(group_id);
    }
    if paged {
        statement = statement.bind(size).bind((page - 1).saturating_mul(size));
    }
    let rows = statement.fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(ItemListRecord {
                id: row.try_get("id")?,
                created_at: text(&row, "created_at")?,
                updated_at: text(&row, "updated_at")?,
                icon_json: text(&row, "icon_json")?,
                title: text(&row, "title")?,
                url: text(&row, "url")?,
                lan_url: text(&row, "lan_url")?,
                mobile_url: text(&row, "mobile_url")?,
                description: text(&row, "description")?,
                open_method: row.try_get("open_method")?,
                sort: row.try_get("sort")?,
                item_icon_group_id: row.try_get("item_icon_group_id")?,
                user_id: row.try_get("user_id")?,
                space_id: row.try_get("space_id")?,
            })
        })
        .collect()
}

pub async fn can_manage_space(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
) -> Result<bool, sqlx::Error> {
    let space_table = if driver == Driver::MySql {
        "`space`"
    } else {
        "\"space\""
    };
    let sql = query(
        driver,
        &format!(
            "SELECT 1 FROM {space_table} s WHERE s.id = ? AND (s.owner_user_id = ? OR EXISTS (SELECT 1 FROM space_member m WHERE m.space_id = s.id AND m.user_id = ? AND m.role = 'admin'))"
        ),
    );
    Ok(sqlx::query(&sql)
        .bind(space_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .is_some())
}

pub async fn space_public_config(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Option<SpacePublicConfig>, sqlx::Error> {
    let (space_table, enabled_expression) = match driver {
        Driver::MySql => ("`space`", "CAST(public_enabled AS SIGNED)"),
        Driver::Sqlite => ("\"space\"", "CAST(public_enabled AS INTEGER)"),
        Driver::Postgres => ("\"space\"", "CASE WHEN public_enabled THEN 1 ELSE 0 END"),
    };
    let sql = query(
        driver,
        &format!(
            "SELECT {enabled_expression} AS enabled, public_id, public_mode FROM {space_table} WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(SpacePublicConfig {
            enabled: row.try_get::<i64, _>("enabled")? != 0,
            public_id: text_opt(&row, "public_id")?,
            mode: text(&row, "public_mode")?,
        })
    })
    .transpose()
}

pub async fn update_space_public_config(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    enabled: bool,
    public_id: Option<&str>,
    mode: &str,
    code_hash: Option<&str>,
) -> Result<(), sqlx::Error> {
    let space_table = if driver == Driver::MySql {
        "`space`"
    } else {
        "\"space\""
    };
    let (sql, binds_code) = if code_hash.is_some() {
        (
            format!(
                "UPDATE {space_table} SET public_enabled = ?, public_id = ?, public_mode = ?, public_code_hash = ? WHERE id = ?"
            ),
            true,
        )
    } else {
        (
            format!(
                "UPDATE {space_table} SET public_enabled = ?, public_id = ?, public_mode = ? WHERE id = ?"
            ),
            false,
        )
    };
    let sql = query(driver, &sql);
    let mut statement = if driver == Driver::Postgres {
        sqlx::query(&sql).bind(enabled)
    } else {
        sqlx::query(&sql).bind(i64::from(enabled))
    };
    statement = statement.bind(public_id).bind(mode);
    if binds_code {
        statement = statement.bind(code_hash);
    }
    statement.bind(space_id).execute(pool).await?;
    Ok(())
}

pub async fn space_wallpaper_json(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let (space_table, text_type) = if driver == Driver::MySql {
        ("`space`", "CHAR")
    } else {
        ("\"space\"", "TEXT")
    };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(wallpaper_json AS {text_type}) AS wallpaper_json FROM {space_table} WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| text(&row, "wallpaper_json")).transpose()
}

pub async fn update_space_wallpaper(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    wallpaper_json: &str,
    updated_at: &str,
) -> Result<(), sqlx::Error> {
    let (space_table, timestamp_expression) = match driver {
        Driver::MySql => ("`space`", "CAST(LEFT(?, 26) AS DATETIME(6))"),
        Driver::Sqlite => ("\"space\"", "?"),
        Driver::Postgres => ("\"space\"", "CAST(? AS TIMESTAMP WITH TIME ZONE)"),
    };
    let sql = query(
        driver,
        &format!(
            "UPDATE {space_table} SET wallpaper_json = ?, updated_at = {timestamp_expression} WHERE id = ?"
        ),
    );
    sqlx::query(&sql)
        .bind(wallpaper_json)
        .bind(updated_at)
        .bind(space_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `pairedIDs`: the Space itself plus its Yang twin, resolved through pair_id.
pub async fn paired_space_ids(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    let (space_table, text_type) = if driver == Driver::MySql {
        ("`space`", "CHAR")
    } else {
        ("\"space\"", "TEXT")
    };
    let _ = text_type;
    let pair_sql = query(
        driver,
        &format!("SELECT pair_id FROM {space_table} WHERE id = ?"),
    );
    let row = sqlx::query(&pair_sql)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    let pair_id = row
        .map(|row| row.try_get::<i64, _>("pair_id"))
        .transpose()?
        .unwrap_or(0);
    if pair_id == 0 {
        return Ok(vec![space_id]);
    }
    let yang_sql = query(
        driver,
        &format!("SELECT id FROM {space_table} WHERE pair_id = ? AND side = 'yang' LIMIT 1"),
    );
    let yang = sqlx::query(&yang_sql)
        .bind(pair_id)
        .fetch_optional(pool)
        .await?;
    match yang {
        Some(row) => Ok(vec![pair_id, row.try_get("id")?]),
        None => Ok(vec![pair_id]),
    }
}

pub async fn space_name(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let space_table = crate::schema_shared::quote(driver, "space");
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(name AS {text_type}) AS name FROM {space_table} WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| text(&row, "name")).transpose()
}

pub async fn space_owner(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    let space_table = crate::schema_shared::quote(driver, "space");
    let sql = query(
        driver,
        &format!("SELECT owner_user_id FROM {space_table} WHERE id = ?"),
    );
    let row = sqlx::query(&sql)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| row.try_get("owner_user_id")).transpose()
}

/// `AddMember`: the row is created in the Space and in its Yang twin, or its
/// role is updated when it already exists.
pub async fn upsert_space_member(
    pool: &AnyPool,
    driver: Driver,
    space_ids: &[i64],
    target_user_id: i64,
    role: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let select = query(
        driver,
        "SELECT id FROM space_member WHERE space_id = ? AND user_id = ? LIMIT 1",
    );
    let update = query(driver, "UPDATE space_member SET role = ? WHERE id = ?");
    let insert = query(
        driver,
        &format!(
            "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES ({timestamp}, {timestamp}, ?, ?, ?, 'manual', {timestamp})"
        ),
    );
    for space_id in space_ids {
        let found = sqlx::query(&select)
            .bind(*space_id)
            .bind(target_user_id)
            .fetch_optional(pool)
            .await?;
        match found {
            Some(row) => {
                let id: i64 = row.try_get("id")?;
                sqlx::query(&update)
                    .bind(role)
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
            None => {
                sqlx::query(&insert)
                    .bind(now)
                    .bind(now)
                    .bind(*space_id)
                    .bind(target_user_id)
                    .bind(role)
                    .bind(now)
                    .execute(pool)
                    .await?;
            }
        }
    }
    Ok(())
}

pub async fn remove_space_member(
    pool: &AnyPool,
    driver: Driver,
    space_ids: &[i64],
    target_user_id: i64,
) -> Result<(), sqlx::Error> {
    let placeholders = vec!["?"; space_ids.len()].join(",");
    let sql = query(
        driver,
        &format!("DELETE FROM space_member WHERE space_id IN ({placeholders}) AND user_id = ?"),
    );
    let mut statement = sqlx::query(&sql);
    for space_id in space_ids {
        statement = statement.bind(*space_id);
    }
    statement.bind(target_user_id).execute(pool).await?;
    Ok(())
}

/// `SpaceRouter.CreateSpace`: a shared Yin space and its Yang twin, each with
/// the creator as an admin member and one default group.
pub async fn create_paired_space(
    pool: &AnyPool,
    driver: Driver,
    owner_id: i64,
    name: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let enabled = if driver == Driver::Postgres { "FALSE" } else { "0" };
    let space_table = crate::schema_shared::quote(driver, "space");
    let space_insert = format!(
        "INSERT INTO {space_table} (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES ({}, {}, 'shared', ?, ?, 0, 'yin', {enabled}, 'direct')",
        timestamp_bind(driver),
        timestamp_bind(driver)
    );
    let yang_insert = format!(
        "INSERT INTO {space_table} (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES ({}, {}, 'shared', ?, ?, ?, 'yang', {enabled}, 'direct')",
        timestamp_bind(driver),
        timestamp_bind(driver)
    );
    let member_insert = format!(
        "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES ({}, {}, ?, ?, 'admin', 'manual', {})",
        timestamp_bind(driver),
        timestamp_bind(driver),
        timestamp_bind(driver)
    );
    let group_insert = format!(
        "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id) VALUES ({}, {}, 'material-symbols:apps', 'APP', '', 0, ?, ?)",
        timestamp_bind(driver),
        timestamp_bind(driver)
    );
    let pair_update = format!("UPDATE {space_table} SET pair_id = ? WHERE id = ?");

    let mut tx = pool.begin().await?;
    let yin_id = insert_returning_id!(
        driver,
        &space_insert,
        [now, now, name, owner_id],
        &mut *tx
    );
    sqlx::query(&query(driver, &pair_update))
        .bind(yin_id)
        .bind(yin_id)
        .execute(&mut *tx)
        .await?;
    let yang_name = format!("{name}-B");
    let yang_id = insert_returning_id!(
        driver,
        &yang_insert,
        [now, now, yang_name, owner_id, yin_id],
        &mut *tx
    );
    for space_id in [yin_id, yang_id] {
        sqlx::query(&query(driver, &member_insert))
            .bind(now)
            .bind(now)
            .bind(space_id)
            .bind(owner_id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        sqlx::query(&query(driver, &group_insert))
            .bind(now)
            .bind(now)
            .bind(owner_id)
            .bind(space_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// `SpaceRouter.Rename`: the Yang twin becomes `<name>-B`.
pub async fn rename_paired_space(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    name: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let space_table = crate::schema_shared::quote(driver, "space");
    let update = format!(
        "UPDATE {space_table} SET name = ?, updated_at = {} WHERE id = ?",
        timestamp_bind(driver)
    );
    let paired = paired_space_ids(pool, driver, space_id).await?;
    let mut tx = pool.begin().await?;
    for sid in &paired {
        let name = if *sid == space_id {
            name.to_string()
        } else {
            format!("{name}-B")
        };
        sqlx::query(&query(driver, &update))
            .bind(name)
            .bind(now)
            .bind(*sid)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// `SpaceRouter.Copy`: a new shared Space holding copies of every group and
/// item of the source, in one transaction.
pub async fn copy_space(
    pool: &AnyPool,
    driver: Driver,
    source_id: i64,
    owner_id: i64,
    name: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    use sqlx::Row;

    let enabled = if driver == Driver::Postgres { "FALSE" } else { "0" };
    let space_table = crate::schema_shared::quote(driver, "space");
    let insert_space = format!(
        "INSERT INTO {space_table} (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES ({}, {}, 'shared', ?, ?, 0, 'yin', {enabled}, 'direct')",
        timestamp_bind(driver),
        timestamp_bind(driver)
    );
    let insert_member = format!(
        "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES ({}, {}, ?, ?, 'admin', 'manual', {})",
        timestamp_bind(driver),
        timestamp_bind(driver),
        timestamp_bind(driver)
    );
    let insert_group = format!(
        "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES ({}, {}, ?, ?, ?, ?, ?, ?, ?)",
        timestamp_bind(driver),
        timestamp_bind(driver)
    );
    let copy_items = query(
        driver,
        &format!(
            "INSERT INTO item_icon (created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) SELECT {}, {}, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, ?, ?, ? FROM item_icon WHERE space_id = ? AND item_icon_group_id = ?",
            timestamp_bind(driver),
            timestamp_bind(driver)
        ),
    );

    let mut tx = pool.begin().await?;
    let destination = insert_returning_id!(
        driver,
        &insert_space,
        [now, now, name, owner_id],
        &mut *tx
    );
    sqlx::query(&query(driver, &insert_member))
        .bind(now)
        .bind(now)
        .bind(destination)
        .bind(owner_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;

    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let group_sql = query(
        driver,
        &format!(
            "SELECT id, CAST(icon AS {text_type}) AS icon, CAST(title AS {text_type}) AS title, CAST(description AS {text_type}) AS description, sort, parent_id FROM item_icon_group WHERE space_id = ?"
        ),
    );
    let groups = sqlx::query(&group_sql)
        .bind(source_id)
        .fetch_all(&mut *tx)
        .await?;
    for group in groups {
        let old_id: i64 = group.try_get("id")?;
        let icon: String = text(&group, "icon")?;
        let title: String = text(&group, "title")?;
        let description: String = text(&group, "description")?;
        let sort: i64 = group.try_get("sort")?;
        let parent_id: Option<i64> = group.try_get("parent_id")?;
        let new_group = if driver == Driver::MySql {
            let rendered = insert_group.clone();
            let result = sqlx::query(&rendered)
                .bind(now)
                .bind(now)
                .bind(&icon)
                .bind(&title)
                .bind(&description)
                .bind(sort)
                .bind(owner_id)
                .bind(destination)
                .bind(parent_id)
                .execute(&mut *tx)
                .await?;
            result
                .last_insert_id()
                .ok_or(sqlx::Error::Protocol("driver did not report an insert id".into()))?
        } else {
            let rendered = query(driver, &format!("{insert_group} RETURNING id"));
            let row = sqlx::query(&rendered)
                .bind(now)
                .bind(now)
                .bind(&icon)
                .bind(&title)
                .bind(&description)
                .bind(sort)
                .bind(owner_id)
                .bind(destination)
                .bind(parent_id)
                .fetch_one(&mut *tx)
                .await?;
            row.try_get("id")?
        };
        sqlx::query(&copy_items)
            .bind(now)
            .bind(now)
            .bind(new_group)
            .bind(owner_id)
            .bind(destination)
            .bind(source_id)
            .bind(old_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn space_owner_and_type(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Option<(i64, String)>, sqlx::Error> {
    let space_table = crate::schema_shared::quote(driver, "space");
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT owner_user_id, CAST(type AS {text_type}) AS kind FROM {space_table} WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| Ok((row.try_get("owner_user_id")?, text(&row, "kind")?)))
        .transpose()
}

/// `SpaceRouter.Transfer`: the target becomes the owner with an admin row, the
/// caller drops to editor, in one transaction.
pub async fn transfer_space(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    actor_id: i64,
    target_id: i64,
    now: &str,
) -> Result<(), sqlx::Error> {
    use sqlx::Row;

    let timestamp = timestamp_bind(driver);
    let space_table = crate::schema_shared::quote(driver, "space");
    let update_owner = query(
        driver,
        &format!("UPDATE {space_table} SET owner_user_id = ?, updated_at = {timestamp} WHERE id = ?"),
    );
    let demote_actor = query(
        driver,
        "UPDATE space_member SET role = 'editor' WHERE space_id = ? AND user_id = ?",
    );
    let select_member = query(
        driver,
        "SELECT id FROM space_member WHERE space_id = ? AND user_id = ? LIMIT 1",
    );
    let promote = query(driver, "UPDATE space_member SET role = 'admin' WHERE id = ?");
    let insert = query(
        driver,
        &format!(
            "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES ({timestamp}, {timestamp}, ?, ?, 'admin', 'manual', {timestamp})"
        ),
    );

    let mut tx = pool.begin().await?;
    sqlx::query(&update_owner)
        .bind(target_id)
        .bind(now)
        .bind(space_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(&demote_actor)
        .bind(space_id)
        .bind(actor_id)
        .execute(&mut *tx)
        .await?;
    let existing = sqlx::query(&select_member)
        .bind(space_id)
        .bind(target_id)
        .fetch_optional(&mut *tx)
        .await?;
    match existing {
        Some(row) => {
            let id: i64 = row.try_get("id")?;
            sqlx::query(&promote).bind(id).execute(&mut *tx).await?;
        }
        None => {
            sqlx::query(&insert)
                .bind(now)
                .bind(now)
                .bind(space_id)
                .bind(target_id)
                .bind(now)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// `SpaceRouter.ClearSpace`: every item and group of one Space, one transaction.
pub async fn clear_space(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<(), sqlx::Error> {
    let delete_items = query(driver, "DELETE FROM item_icon WHERE space_id = ?");
    let delete_groups = query(driver, "DELETE FROM item_icon_group WHERE space_id = ?");
    let mut tx = pool.begin().await?;
    sqlx::query(&delete_items)
        .bind(space_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(&delete_groups)
        .bind(space_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// `SpaceRouter.Delete`: two paired Spaces, their rows, and the icon files that
/// nothing else references. Returns the file names the caller must unlink.
pub async fn delete_paired_space(
    pool: &AnyPool,
    driver: Driver,
    space_ids: &[i64],
) -> Result<Vec<String>, sqlx::Error> {
    use sqlx::Row;

    let placeholders = vec!["?"; space_ids.len()].join(",");
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let icons_sql = query(
        driver,
        &format!(
            "SELECT CAST(icon_json AS {text_type}) AS icon_json FROM item_icon WHERE space_id IN ({placeholders})"
        ),
    );
    let used_sql = query(
        driver,
        "SELECT COUNT(*) AS total FROM item_icon WHERE icon_json LIKE ?",
    );
    let delete_file = query(driver, "DELETE FROM file WHERE file_name = ?");

    let mut tx = pool.begin().await?;
    let mut statement = sqlx::query(&icons_sql);
    for id in space_ids {
        statement = statement.bind(*id);
    }
    let icons = statement.fetch_all(&mut *tx).await?;
    let mut referenced: std::collections::HashSet<String> = std::collections::HashSet::new();
    for row in icons {
        let json = text(&row, "icon_json")?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
            if let Some(name) = value.get("fileName").and_then(serde_json::Value::as_str) {
                if !name.is_empty() {
                    referenced.insert(name.to_string());
                }
            }
        }
    }
    let mut removed = Vec::new();
    for name in referenced {
        let used = sqlx::query(&used_sql)
            .bind(format!("%\"{name}\"%"))
            .fetch_one(&mut *tx)
            .await?;
        let total: i64 = used.try_get("total")?;
        if total > 0 {
            continue;
        }
        sqlx::query(&delete_file)
            .bind(&name)
            .execute(&mut *tx)
            .await?;
        removed.push(name);
    }

    for table in [
        "item_icon",
        "item_icon_group",
        "space_member",
        "space_o_id_c_group",
    ] {
        let sql = query(
            driver,
            &format!("DELETE FROM {table} WHERE space_id IN ({placeholders})"),
        );
        let mut statement = sqlx::query(&sql);
        for id in space_ids {
            statement = statement.bind(*id);
        }
        statement.execute(&mut *tx).await?;
    }
    let space_table = crate::schema_shared::quote(driver, "space");
    let sql = query(
        driver,
        &format!("DELETE FROM {space_table} WHERE id IN ({placeholders})"),
    );
    let mut statement = sqlx::query(&sql);
    for id in space_ids {
        statement = statement.bind(*id);
    }
    statement.execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(removed)
}

pub async fn spaces_owned_by(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Vec<(i64, String)>, sqlx::Error> {
    let space_table = crate::schema_shared::quote(driver, "space");
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(type AS {text_type}) AS kind FROM {space_table} WHERE owner_user_id = ?"
        ),
    );
    let rows = sqlx::query(&sql).bind(user_id).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| Ok((row.try_get("id")?, text(&row, "kind")?)))
        .collect()
}

pub async fn paired_ids_of(
    pool: &AnyPool,
    driver: Driver,
    ids: &[i64],
) -> Result<Vec<i64>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let space_table = crate::schema_shared::quote(driver, "space");
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = query(
        driver,
        &format!("SELECT id FROM {space_table} WHERE pair_id IN ({placeholders})"),
    );
    let mut statement = sqlx::query(&sql);
    for id in ids {
        statement = statement.bind(*id);
    }
    let rows = statement.fetch_all(pool).await?;
    rows.into_iter().map(|row| row.try_get("id")).collect()
}

pub async fn groups_for_space(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Vec<GroupListRecord>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let group_sql = query(
        driver,
        &format!(
            "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST(icon AS {text_type}) AS icon, CAST(title AS {text_type}) AS title, CAST(description AS {text_type}) AS description, sort, user_id, space_id, parent_id FROM item_icon_group WHERE space_id = ? ORDER BY sort, created_at"
        ),
    );
    let rows = sqlx::query(&group_sql)
        .bind(space_id)
        .fetch_all(pool)
        .await?;
    let mut groups = rows
        .into_iter()
        .map(|row| {
            Ok(GroupListRecord {
                id: row.try_get("id")?,
                created_at: text(&row, "created_at")?,
                updated_at: text(&row, "updated_at")?,
                icon: text(&row, "icon")?,
                title: text(&row, "title")?,
                description: text(&row, "description")?,
                sort: row.try_get("sort")?,
                user_id: row.try_get("user_id")?,
                space_id: row.try_get("space_id")?,
                parent_id: row.try_get("parent_id")?,
                item_count: 0,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    if groups.is_empty() {
        return Ok(groups);
    }

    let count_sql = query(
        driver,
        "SELECT item_icon_group_id, COUNT(*) AS item_count FROM item_icon WHERE space_id = ? GROUP BY item_icon_group_id",
    );
    let counts = async {
        let rows = sqlx::query(&count_sql)
            .bind(space_id)
            .fetch_all(pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                Ok((
                    row.try_get::<i64, _>("item_icon_group_id")?,
                    row.try_get::<i64, _>("item_count")?,
                ))
            })
            .collect::<Result<std::collections::HashMap<_, _>, sqlx::Error>>()
    }
    .await;
    if let Ok(counts) = counts {
        for group in &mut groups {
            group.item_count = counts.get(&group.id).copied().unwrap_or(0);
        }
    }
    Ok(groups)
}

pub async fn can_access_space(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
) -> Result<bool, sqlx::Error> {
    let space_table = if driver == Driver::MySql {
        "`space`"
    } else {
        "\"space\""
    };
    let sql = query(
        driver,
        &format!(
            "SELECT 1 FROM {space_table} s WHERE s.id = ? AND (s.owner_user_id = ? OR EXISTS (SELECT 1 FROM space_member m WHERE m.space_id = s.id AND m.user_id = ?))"
        ),
    );
    Ok(sqlx::query(&sql)
        .bind(space_id)
        .bind(user_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .is_some())
}

pub async fn space_members(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Vec<SpaceMemberRecord>, sqlx::Error> {
    let (user_table, text_type) = if driver == Driver::MySql {
        ("`user`", "CHAR")
    } else {
        ("\"user\"", "TEXT")
    };
    let sql = query(
        driver,
        &format!(
            "SELECT m.id, m.space_id, m.user_id, CAST(m.role AS {text_type}) AS role, CAST(m.source AS {text_type}) AS source, CAST(m.joined_at AS {text_type}) AS joined_at, COALESCE(CAST(u.mail AS {text_type}), '') AS email, COALESCE(CAST(u.name AS {text_type}), '') AS name FROM space_member m LEFT JOIN {user_table} u ON u.id = m.user_id WHERE m.space_id = ? ORDER BY m.id"
        ),
    );
    let rows = sqlx::query(&sql).bind(space_id).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(SpaceMemberRecord {
                id: row.try_get("id")?,
                space_id: row.try_get("space_id")?,
                user_id: row.try_get("user_id")?,
                role: text(&row, "role")?,
                source: text(&row, "source")?,
                joined_at: text(&row, "joined_at")?,
                email: text(&row, "email")?,
                name: text(&row, "name")?,
            })
        })
        .collect()
}

pub async fn space_list(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Vec<SpaceListRecord>, sqlx::Error> {
    let (space_table, type_column, text_type, enabled_expression) = match driver {
        Driver::MySql => (
            "`space`",
            "`type`",
            "CHAR",
            "CAST(public_enabled AS SIGNED)",
        ),
        Driver::Sqlite => (
            "\"space\"",
            "\"type\"",
            "TEXT",
            "CAST(public_enabled AS INTEGER)",
        ),
        Driver::Postgres => (
            "\"space\"",
            "\"type\"",
            "TEXT",
            "CASE WHEN public_enabled THEN 1 ELSE 0 END",
        ),
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST({type_column} AS {text_type}) AS kind, CAST(name AS {text_type}) AS name, owner_user_id, pair_id, CAST(side AS {text_type}) AS side, {enabled_expression} AS public_enabled, CAST(public_id AS {text_type}) AS public_id, CAST(public_mode AS {text_type}) AS public_mode FROM {space_table} WHERE (owner_user_id = ? OR id IN (SELECT space_id FROM space_member WHERE user_id = ?)) AND side = 'yin' ORDER BY {type_column}, created_at"
        ),
    );
    let rows = sqlx::query(&sql)
        .bind(user_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

    let mut spaces = Vec::with_capacity(rows.len());
    for row in rows {
        spaces.push(SpaceListRecord {
            id: row.try_get("id")?,
            created_at: text(&row, "created_at")?,
            updated_at: text(&row, "updated_at")?,
            kind: text(&row, "kind")?,
            name: text(&row, "name")?,
            owner_user_id: row.try_get("owner_user_id")?,
            pair_id: row.try_get("pair_id")?,
            side: text(&row, "side")?,
            paired_space_id: None,
            public_enabled: row.try_get::<i64, _>("public_enabled")? != 0,
            public_id: text_opt(&row, "public_id")?,
            public_mode: text(&row, "public_mode")?,
            can_edit: false,
        });
    }

    if spaces.is_empty() {
        return Ok(spaces);
    }

    let placeholders = vec!["?"; spaces.len()].join(",");
    let paired_sql = query(
        driver,
        &format!(
            "SELECT id, pair_id FROM {space_table} WHERE pair_id IN ({placeholders}) AND side = 'yang'"
        ),
    );
    if let Ok(paired_rows) = {
        let mut statement = sqlx::query(&paired_sql);
        for space in &spaces {
            statement = statement.bind(space.id);
        }
        statement.fetch_all(pool).await
    } {
        for row in paired_rows {
            let paired_id: i64 = row.try_get("id")?;
            let pair_id: i64 = row.try_get("pair_id")?;
            if let Some(space) = spaces.iter_mut().find(|space| space.id == pair_id) {
                space.paired_space_id = Some(paired_id);
            }
        }
    }

    let member_sql = query(
        driver,
        &format!(
            "SELECT space_id, CAST(role AS {text_type}) AS role FROM space_member WHERE user_id = ? AND space_id IN ({placeholders})"
        ),
    );
    let member_rows = {
        let mut statement = sqlx::query(&member_sql).bind(user_id);
        for space in &spaces {
            statement = statement.bind(space.id);
        }
        statement.fetch_all(pool).await
    };
    let mut roles = std::collections::HashMap::new();
    if let Ok(member_rows) = member_rows {
        for row in member_rows {
            let space_id: i64 = row.try_get("space_id")?;
            let role: String = text(&row, "role")?;
            roles.insert(space_id, role);
        }
    }
    for space in &mut spaces {
        let role = roles.get(&space.id).map(String::as_str).unwrap_or_default();
        space.can_edit = space.owner_user_id == user_id || role == "admin" || role == "editor";
    }

    Ok(spaces)
}
