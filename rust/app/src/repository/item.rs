//! Groups, items and ordering operations.

use sqlx::{AnyPool, Row};

use crate::db::{Driver, query};

use super::*;

pub async fn bookmark_tree(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<(Vec<BookmarkGroupRecord>, Vec<BookmarkItemRecord>), sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let group_sql = query(
        driver,
        &format!(
            "SELECT id, CAST(title AS {text_type}) AS title, CAST(icon AS {text_type}) AS icon, parent_id FROM item_icon_group WHERE space_id = ? ORDER BY sort, id"
        ),
    );
    let groups = sqlx::query(&group_sql)
        .bind(space_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(BookmarkGroupRecord {
                id: row.try_get("id")?,
                title: text(&row, "title")?,
                icon: text(&row, "icon")?,
                parent_id: row.try_get("parent_id")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    let item_sql = query(
        driver,
        &format!(
            "SELECT item_icon_group_id AS group_id, CAST(title AS {text_type}) AS title, CAST(url AS {text_type}) AS url FROM item_icon WHERE space_id = ? ORDER BY sort, id"
        ),
    );
    let items = sqlx::query(&item_sql)
        .bind(space_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(BookmarkItemRecord {
                group_id: row.try_get("group_id")?,
                title: text(&row, "title")?,
                url: text(&row, "url")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    Ok((groups, items))
}

pub async fn oidc_groups(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Vec<OidcGroupRecord>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST(provider AS {text_type}) AS provider, CAST(group_name AS {text_type}) AS group_name, CAST(role AS {text_type}) AS role FROM space_o_id_c_group WHERE space_id = ? ORDER BY id"
        ),
    );
    let rows = sqlx::query(&sql).bind(space_id).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(OidcGroupRecord {
                id: row.try_get("id")?,
                created_at: text(&row, "created_at")?,
                updated_at: text(&row, "updated_at")?,
                provider: text(&row, "provider")?,
                group_name: text(&row, "group_name")?,
                role: text(&row, "role")?,
            })
        })
        .collect()
}

pub async fn item_belongs(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    item_id: i64,
) -> Result<bool, sqlx::Error> {
    let sql = query(
        driver,
        "SELECT 1 FROM item_icon WHERE id = ? AND space_id = ?",
    );
    Ok(sqlx::query(&sql)
        .bind(item_id)
        .bind(space_id)
        .fetch_optional(pool)
        .await?
        .is_some())
}

pub async fn group_belongs(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    group_id: i64,
) -> Result<bool, sqlx::Error> {
    let sql = query(
        driver,
        "SELECT 1 FROM item_icon_group WHERE id = ? AND space_id = ?",
    );
    Ok(sqlx::query(&sql)
        .bind(group_id)
        .bind(space_id)
        .fetch_optional(pool)
        .await?
        .is_some())
}

/// `groupHasDescendant`: walking up from the candidate must not reach the group.
pub async fn group_has_descendant(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    group_id: i64,
    candidate: i64,
) -> Result<bool, sqlx::Error> {
    let sql = query(
        driver,
        "SELECT parent_id FROM item_icon_group WHERE id = ? AND space_id = ?",
    );
    let mut current = candidate;
    let mut hops = 0;
    while current != 0 && hops < 256 {
        if current == group_id {
            return Ok(true);
        }
        let row = sqlx::query(&sql)
            .bind(current)
            .bind(space_id)
            .fetch_optional(pool)
            .await?;
        match row.and_then(|row| row.try_get::<Option<i64>, _>("parent_id").ok().flatten()) {
            Some(next) => current = next,
            None => return Ok(false),
        }
        hops += 1;
    }
    Ok(false)
}

/// Insert a group and return its new id on every driver: PostgreSQL and SQLite
/// support `RETURNING`, while MySQL reports `last_insert_id`.
pub async fn create_group(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    user_id: i64,
    icon: &str,
    title: &str,
    parent_id: Option<i64>,
    now: &str,
) -> Result<i64, sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let use_returning = driver != Driver::MySql;
    let suffix = if use_returning { " RETURNING id" } else { "" };
    let sql = query(
        driver,
        &format!(
            "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES ({timestamp}, {timestamp}, ?, ?, '', 9999, ?, ?, ?){suffix}"
        ),
    );
    let statement = sqlx::query(&sql)
        .bind(now)
        .bind(now)
        .bind(icon)
        .bind(title)
        .bind(user_id)
        .bind(space_id)
        .bind(parent_id);
    if use_returning {
        let row = statement.fetch_one(pool).await?;
        return row.try_get("id");
    }
    let result = statement.execute(pool).await?;
    result
        .last_insert_id()
        .ok_or_else(|| sqlx::Error::Protocol("driver did not report an insert id".into()))
}

pub async fn group_by_id(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    id: i64,
) -> Result<Option<GroupListRecord>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST(icon AS {text_type}) AS icon, CAST(title AS {text_type}) AS title, CAST(description AS {text_type}) AS description, sort, user_id, space_id, parent_id FROM item_icon_group WHERE id = ? AND space_id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(id)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
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
    .transpose()
}

pub async fn update_group(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    group_id: i64,
    title: &str,
    icon: &str,
    parent_id: Option<i64>,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = query(
        driver,
        &format!(
            "UPDATE item_icon_group SET title = ?, icon = ?, parent_id = ?, updated_at = {timestamp} WHERE id = ? AND space_id = ?"
        ),
    );
    sqlx::query(&sql)
        .bind(title)
        .bind(icon)
        .bind(parent_id)
        .bind(now)
        .bind(group_id)
        .bind(space_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete a group, its descendants and their items in one transaction.
pub async fn delete_group_tree(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    group_id: i64,
) -> Result<(), sqlx::Error> {
    let child_sql = query(
        driver,
        "SELECT id FROM item_icon_group WHERE space_id = ? AND parent_id = ?",
    );
    let mut ids = vec![group_id];
    let mut index = 0;
    while index < ids.len() {
        let rows = sqlx::query(&child_sql)
            .bind(space_id)
            .bind(ids[index])
            .fetch_all(pool)
            .await?;
        for row in rows {
            ids.push(row.try_get("id")?);
        }
        index += 1;
    }

    let placeholders = vec!["?"; ids.len()].join(",");
    let delete_items = query(
        driver,
        &format!(
            "DELETE FROM item_icon WHERE space_id = ? AND item_icon_group_id IN ({placeholders})"
        ),
    );
    let delete_groups = query(
        driver,
        &format!("DELETE FROM item_icon_group WHERE space_id = ? AND id IN ({placeholders})"),
    );

    let mut tx = pool.begin().await?;
    let mut statement = sqlx::query(&delete_items).bind(space_id);
    for id in &ids {
        statement = statement.bind(id);
    }
    statement.execute(&mut *tx).await?;
    let mut statement = sqlx::query(&delete_groups).bind(space_id);
    for id in &ids {
        statement = statement.bind(id);
    }
    statement.execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn item_by_id(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    id: i64,
) -> Result<Option<ItemListRecord>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST(icon_json AS {text_type}) AS icon_json, CAST(title AS {text_type}) AS title, CAST(url AS {text_type}) AS url, CAST(lan_url AS {text_type}) AS lan_url, CAST(mobile_url AS {text_type}) AS mobile_url, CAST(description AS {text_type}) AS description, open_method, sort, item_icon_group_id, user_id, space_id FROM item_icon WHERE id = ? AND space_id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(id)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
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
    .transpose()
}

pub async fn create_item(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    user_id: i64,
    item: &ItemWriteRecord,
    now: &str,
) -> Result<i64, sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let use_returning = driver != Driver::MySql;
    let suffix = if use_returning { " RETURNING id" } else { "" };
    let sql = query(
        driver,
        &format!(
            "INSERT INTO item_icon (created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES ({timestamp}, {timestamp}, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?){suffix}"
        ),
    );
    let statement = sqlx::query(&sql)
        .bind(now)
        .bind(now)
        .bind(&item.icon_json)
        .bind(&item.title)
        .bind(&item.url)
        .bind(&item.lan_url)
        .bind(&item.mobile_url)
        .bind(&item.description)
        .bind(item.open_method)
        .bind(item.sort)
        .bind(item.group_id)
        .bind(user_id)
        .bind(space_id);
    if use_returning {
        let row = statement.fetch_one(pool).await?;
        return row.try_get("id");
    }
    let result = statement.execute(pool).await?;
    result
        .last_insert_id()
        .ok_or_else(|| sqlx::Error::Protocol("driver did not report an insert id".into()))
}

pub async fn update_item(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    item_id: i64,
    item: &ItemWriteRecord,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = query(
        driver,
        &format!(
            "UPDATE item_icon SET icon_json = ?, title = ?, url = ?, lan_url = ?, mobile_url = ?, description = ?, open_method = ?, sort = ?, item_icon_group_id = ?, updated_at = {timestamp} WHERE id = ? AND space_id = ?"
        ),
    );
    sqlx::query(&sql)
        .bind(&item.icon_json)
        .bind(&item.title)
        .bind(&item.url)
        .bind(&item.lan_url)
        .bind(&item.mobile_url)
        .bind(&item.description)
        .bind(item.open_method)
        .bind(item.sort)
        .bind(item.group_id)
        .bind(now)
        .bind(item_id)
        .bind(space_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_item(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    item_id: i64,
) -> Result<(), sqlx::Error> {
    let sql = query(
        driver,
        "DELETE FROM item_icon WHERE id = ? AND space_id = ?",
    );
    sqlx::query(&sql)
        .bind(item_id)
        .bind(space_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `SpaceRouter.SortItems`: every entry updates one row of the same group in
/// one transaction.
pub async fn sort_items(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    group_id: i64,
    entries: &[(i64, i64)],
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = query(
        driver,
        &format!(
            "UPDATE item_icon SET sort = ?, updated_at = {timestamp} WHERE id = ? AND space_id = ? AND item_icon_group_id = ?"
        ),
    );
    let mut tx = pool.begin().await?;
    for (id, sort) in entries {
        sqlx::query(&sql)
            .bind(*sort)
            .bind(now)
            .bind(*id)
            .bind(space_id)
            .bind(group_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// The siblings a group sort must cover: roots or the children of one parent.
pub async fn group_siblings(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    parent_id: Option<i64>,
) -> Result<Vec<i64>, sqlx::Error> {
    let (sql, binds_parent) = match parent_id {
        None => (
            "SELECT id FROM item_icon_group WHERE space_id = ? AND parent_id IS NULL".to_string(),
            false,
        ),
        Some(_) => (
            "SELECT id FROM item_icon_group WHERE space_id = ? AND parent_id = ?".to_string(),
            true,
        ),
    };
    let sql = query(driver, &sql);
    let mut statement = sqlx::query(&sql).bind(space_id);
    if binds_parent {
        statement = statement.bind(parent_id);
    }
    let rows = statement.fetch_all(pool).await?;
    rows.into_iter().map(|row| row.try_get("id")).collect()
}

pub async fn sort_groups(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    entries: &[(i64, i64)],
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = query(
        driver,
        &format!(
            "UPDATE item_icon_group SET sort = ?, updated_at = {timestamp} WHERE id = ? AND space_id = ?"
        ),
    );
    let mut tx = pool.begin().await?;
    for (id, sort) in entries {
        sqlx::query(&sql)
            .bind(*sort)
            .bind(now)
            .bind(*id)
            .bind(space_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn add_oidc_group(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    provider: &str,
    group_name: &str,
    role: &str,
    now: &str,
) -> Result<i64, sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = format!(
        "INSERT INTO space_o_id_c_group (created_at, updated_at, space_id, provider, group_name, role) VALUES ({timestamp}, {timestamp}, ?, ?, ?, ?)"
    );
    let mut tx = pool.begin().await?;
    let id = insert_returning_id!(driver, &sql, [now, now, space_id, provider, group_name, role], &mut *tx);
    tx.commit().await?;
    Ok(id)
}

pub async fn delete_oidc_group(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    rule_id: i64,
) -> Result<(), sqlx::Error> {
    let sql = query(
        driver,
        "DELETE FROM space_o_id_c_group WHERE id = ? AND space_id = ?",
    );
    sqlx::query(&sql)
        .bind(rule_id)
        .bind(space_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// `SpaceRouter.CreateItemWithIcon`: the optional icon file row first, then the
/// item, in one transaction.
#[allow(clippy::too_many_arguments)]
pub async fn create_item_with_icon(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    user_id: i64,
    icon_json: &str,
    title: &str,
    url: &str,
    lan_url: &str,
    mobile_url: &str,
    description: &str,
    open_method: i64,
    sort: i64,
    group_id: i64,
    file_name: Option<&str>,
    now: &str,
) -> Result<i64, sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let count_file = query(
        driver,
        "SELECT COUNT(*) AS total FROM file WHERE user_id = ? AND file_name = ?",
    );
    let insert_file = query(
        driver,
        &format!(
            "INSERT INTO file (created_at, updated_at, user_id, file_name) VALUES ({timestamp}, {timestamp}, ?, ?)"
        ),
    );
    let insert_item = format!(
        "INSERT INTO item_icon (created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES ({timestamp}, {timestamp}, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    );
    let mut tx = pool.begin().await?;
    if let Some(name) = file_name {
        use sqlx::Row;
        let row = sqlx::query(&count_file)
            .bind(user_id)
            .bind(name)
            .fetch_one(&mut *tx)
            .await?;
        let total: i64 = row.try_get("total")?;
        if total == 0 {
            sqlx::query(&insert_file)
                .bind(now)
                .bind(now)
                .bind(user_id)
                .bind(name)
                .execute(&mut *tx)
                .await?;
        }
    }
    let id = insert_returning_id!(
        driver,
        &insert_item,
        [
            now,
            now,
            icon_json,
            title,
            url,
            lan_url,
            mobile_url,
            description,
            open_method,
            sort,
            group_id,
            user_id,
            space_id
        ],
        &mut *tx
    );
    tx.commit().await?;
    Ok(id)
}

/// One group inserted inside a caller-owned transaction, returning its id.
#[allow(clippy::too_many_arguments)]
pub async fn insert_group_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    driver: Driver,
    space_id: i64,
    user_id: i64,
    icon: &str,
    title: &str,
    sort: i64,
    parent_id: Option<i64>,
    now: &str,
) -> Result<i64, sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = format!(
        "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES ({timestamp}, {timestamp}, ?, ?, '', ?, ?, ?, ?)"
    );
    let id = insert_returning_id!(
        driver,
        &sql,
        [now, now, icon, title, sort, user_id, space_id, parent_id],
        &mut **tx
    );
    Ok(id)
}

/// Bookmark import stores every link with `openMethod = 2` and empty URLs.
#[allow(clippy::too_many_arguments)]
pub async fn insert_imported_item_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    driver: Driver,
    icon_json: &str,
    title: &str,
    url: &str,
    sort: i64,
    group_id: i64,
    owner_id: i64,
    space_id: i64,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = query(
        driver,
        &format!(
            "INSERT INTO item_icon (created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES ({timestamp}, {timestamp}, ?, ?, ?, '', '', '', 2, ?, ?, ?, ?)"
        ),
    );
    sqlx::query(&sql)
        .bind(now)
        .bind(now)
        .bind(icon_json)
        .bind(title)
        .bind(url)
        .bind(sort)
        .bind(group_id)
        .bind(owner_id)
        .bind(space_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// `ArrangeLayout`: items may change group and sort, groups sort, one
/// transaction.
pub async fn arrange_layout(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    items: &[(i64, i64, i64)],
    groups: &[(i64, i64)],
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let update_item = query(
        driver,
        &format!(
            "UPDATE item_icon SET item_icon_group_id = ?, sort = ?, updated_at = {timestamp} WHERE id = ? AND space_id = ?"
        ),
    );
    let update_group = query(
        driver,
        &format!(
            "UPDATE item_icon_group SET sort = ?, updated_at = {timestamp} WHERE id = ? AND space_id = ?"
        ),
    );
    let mut tx = pool.begin().await?;
    for (id, group_id, sort) in items {
        sqlx::query(&update_item)
            .bind(*group_id)
            .bind(*sort)
            .bind(now)
            .bind(*id)
            .bind(space_id)
            .execute(&mut *tx)
            .await?;
    }
    for (id, sort) in groups {
        sqlx::query(&update_group)
            .bind(*sort)
            .bind(now)
            .bind(*id)
            .bind(space_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}
