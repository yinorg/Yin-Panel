//! Shared repository reads over the configured SQLx driver.

use sqlx::{AnyPool, Row};

use crate::db::{Driver, query};

/// SQLx Any reports MySQL `TEXT` columns as `Blob`, so string columns decode
/// through bytes when the direct `String` decode is not supported.
pub(crate) fn text(row: &sqlx::any::AnyRow, column: &str) -> Result<String, sqlx::Error> {
    match row.try_get::<String, _>(column) {
        Ok(value) => Ok(value),
        Err(primary) => match row.try_get::<Vec<u8>, _>(column) {
            Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Err(_) => Err(primary),
        },
    }
}

pub(crate) fn text_opt(
    row: &sqlx::any::AnyRow,
    column: &str,
) -> Result<Option<String>, sqlx::Error> {
    match row.try_get::<Option<String>, _>(column) {
        Ok(value) => Ok(value),
        Err(primary) => match row.try_get::<Option<Vec<u8>>, _>(column) {
            Ok(bytes) => Ok(bytes.map(|bytes| String::from_utf8_lossy(&bytes).into_owned())),
            Err(_) => Err(primary),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRecord {
    pub id: i64,
    pub password: String,
    pub name: String,
    pub status: i64,
    pub role: i64,
    pub mail: String,
    pub token_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceListRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub kind: String,
    pub name: String,
    pub owner_user_id: i64,
    pub pair_id: i64,
    pub side: String,
    pub paired_space_id: Option<i64>,
    pub public_enabled: bool,
    pub public_id: Option<String>,
    pub public_mode: String,
    pub can_edit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceMemberRecord {
    pub id: i64,
    pub space_id: i64,
    pub user_id: i64,
    pub role: String,
    pub source: String,
    pub joined_at: String,
    pub email: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupListRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub icon: String,
    pub title: String,
    pub description: String,
    pub sort: i64,
    pub user_id: i64,
    pub space_id: i64,
    pub parent_id: Option<i64>,
    pub item_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceSearchConfig {
    pub owner_user_id: i64,
    pub config_json: String,
    pub is_member: bool,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemListRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub icon_json: String,
    pub title: String,
    pub url: String,
    pub lan_url: String,
    pub mobile_url: String,
    pub description: String,
    pub open_method: i64,
    pub sort: i64,
    pub item_icon_group_id: i64,
    pub user_id: i64,
    pub space_id: i64,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpacePublicConfig {
    pub enabled: bool,
    pub public_id: Option<String>,
    pub mode: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileListRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedUserRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub name: String,
    pub head_image: String,
    pub status: i64,
    pub role: i64,
    pub mail: String,
    pub token_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserConfigRecord {
    pub user_id: i64,
    pub panel_json: String,
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkGroupRecord {
    pub id: i64,
    pub title: String,
    pub icon: String,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkItemRecord {
    pub group_id: i64,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcGroupRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub provider: String,
    pub group_name: String,
    pub role: String,
}

pub async fn module_config_value(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    name: &str,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(value_json AS {text_type}) AS value_json FROM module_config WHERE user_id = ? AND name = ? LIMIT 1"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .bind(name)
        .fetch_optional(pool)
        .await?;
    row.map(|row| text(&row, "value_json")).transpose()
}

pub async fn file_list(
    pool: &AnyPool,
    driver: Driver,
    limit: i64,
    page: i64,
) -> Result<(i64, Vec<FileListRecord>), sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let count: i64 = sqlx::query(&query(driver, "SELECT COUNT(*) AS total FROM file"))
        .fetch_one(pool)
        .await?
        .try_get("total")?;
    let rows = if limit > 0 {
        let limit = limit.min(200);
        let sql = query(
            driver,
            &format!(
                "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST(file_name AS {text_type}) AS file_name FROM file ORDER BY created_at DESC LIMIT ? OFFSET ?"
            ),
        );
        sqlx::query(&sql)
            .bind(limit)
            .bind((page - 1).saturating_mul(limit))
            .fetch_all(pool)
            .await?
    } else {
        let sql = query(
            driver,
            &format!(
                "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST(file_name AS {text_type}) AS file_name FROM file ORDER BY created_at DESC"
            ),
        );
        sqlx::query(&sql).fetch_all(pool).await?
    };
    let rows = rows
        .into_iter()
        .map(|row| {
            Ok(FileListRecord {
                id: row.try_get("id")?,
                created_at: text(&row, "created_at")?,
                updated_at: text(&row, "updated_at")?,
                file_name: text(&row, "file_name")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    Ok((count, rows))
}

pub async fn user_list(
    pool: &AnyPool,
    driver: Driver,
    limit: i64,
    page: i64,
) -> Result<(i64, Vec<ManagedUserRecord>), sqlx::Error> {
    let (user_table, text_type) = if driver == Driver::MySql {
        ("`user`", "CHAR")
    } else {
        ("\"user\"", "TEXT")
    };
    let count: i64 = sqlx::query(&query(
        driver,
        &format!("SELECT COUNT(*) AS total FROM {user_table}"),
    ))
    .fetch_one(pool)
    .await?
    .try_get("total")?;
    let rows = if limit > 0 {
        let offset = limit.saturating_mul(page.max(1) - 1);
        let sql = query(
            driver,
            &format!(
                "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST(name AS {text_type}) AS name, CAST(head_image AS {text_type}) AS head_image, status, role, CAST(mail AS {text_type}) AS mail, token_version FROM {user_table} ORDER BY id LIMIT ? OFFSET ?"
            ),
        );
        sqlx::query(&sql)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
    } else {
        Vec::new()
    };
    let rows = rows
        .into_iter()
        .map(|row| {
            Ok(ManagedUserRecord {
                id: row.try_get("id")?,
                created_at: text(&row, "created_at")?,
                updated_at: text(&row, "updated_at")?,
                name: text(&row, "name")?,
                head_image: text(&row, "head_image")?,
                status: row.try_get("status")?,
                role: row.try_get("role")?,
                mail: text(&row, "mail")?,
                token_version: row.try_get("token_version")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    Ok((count, rows))
}

pub async fn user_config(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Option<UserConfigRecord>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT user_id, CAST(panel_json AS {text_type}) AS panel_json, CAST(language AS {text_type}) AS language FROM user_config WHERE user_id = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(user_id).fetch_optional(pool).await?;
    row.map(|row| {
        Ok(UserConfigRecord {
            user_id: row.try_get("user_id")?,
            panel_json: text(&row, "panel_json")?,
            language: text(&row, "language")?,
        })
    })
    .transpose()
}

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

/// The bind expression a driver needs to accept a Go-style timestamp string.
/// The timestamp form GORM writes: local time, nanosecond fraction, offset.
pub(crate) fn go_timestamp() -> String {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let now = time::OffsetDateTime::now_utc().to_offset(offset);
    let format = time::macros::format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:9][offset_hour sign:mandatory]:[offset_minute]"
    );
    now.format(&format).unwrap_or_default()
}

pub(crate) fn timestamp_bind(driver: Driver) -> &'static str {
    match driver {
        Driver::MySql => "CAST(LEFT(?, 26) AS DATETIME(6))",
        Driver::Sqlite => "?",
        Driver::Postgres => "CAST(? AS TIMESTAMP WITH TIME ZONE)",
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemWriteRecord {
    pub icon_json: String,
    pub title: String,
    pub url: String,
    pub lan_url: String,
    pub mobile_url: String,
    pub description: String,
    pub open_method: i64,
    pub sort: i64,
    pub group_id: i64,
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

pub async fn user_id_by_mail(
    pool: &AnyPool,
    driver: Driver,
    mail: &str,
) -> Result<Option<i64>, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let sql = query(driver, &format!("SELECT id FROM {user_table} WHERE mail = ?"));
    let row = sqlx::query(&sql)
        .bind(mail)
        .fetch_optional(pool)
        .await?;
    row.map(|row| row.try_get("id")).transpose()
}

/// A row of `o_auth_identity`, the canonical provider/subject link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OauthIdentityRecord {
    pub id: i64,
    pub user_id: i64,
    pub email: String,
}

pub async fn oauth_identity(
    pool: &AnyPool,
    driver: Driver,
    provider: &str,
    subject: &str,
) -> Result<Option<OauthIdentityRecord>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, user_id, COALESCE(CAST(email AS {text_type}), '') AS email FROM o_auth_identity WHERE provider = ? AND subject = ? LIMIT 1"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(provider)
        .bind(subject)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(OauthIdentityRecord {
            id: row.try_get("id")?,
            user_id: row.try_get("user_id")?,
            email: text(&row, "email")?,
        })
    })
    .transpose()
}

/// `FirstOrCreate` with `Assign`: an existing provider/subject row is pointed at
/// the resolved account, a new one is inserted.
pub async fn link_oauth_identity(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    provider: &str,
    subject: &str,
    email: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let update = query(
        driver,
        &format!("UPDATE o_auth_identity SET updated_at = {timestamp}, user_id = ?, email = ? WHERE id = ?"),
    );
    let insert = query(
        driver,
        &format!(
            "INSERT INTO o_auth_identity (created_at, updated_at, user_id, provider, subject, email) VALUES ({timestamp}, {timestamp}, ?, ?, ?, ?)"
        ),
    );
    let existing = oauth_identity(pool, driver, provider, subject).await?;
    match existing {
        Some(identity) => {
            sqlx::query(&update)
                .bind(now)
                .bind(user_id)
                .bind(email)
                .bind(identity.id)
                .execute(pool)
                .await?;
        }
        None => {
            sqlx::query(&insert)
                .bind(now)
                .bind(now)
                .bind(user_id)
                .bind(provider)
                .bind(subject)
                .bind(email)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

pub async fn user_by_mail(
    pool: &AnyPool,
    driver: Driver,
    mail: &str,
) -> Result<Option<UserRecord>, sqlx::Error> {
    let table = match driver {
        Driver::MySql => "`user`",
        Driver::Sqlite | Driver::Postgres => "\"user\"",
    };
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(password AS {text_type}) AS password, CAST(name AS {text_type}) AS name, status, role, CAST(mail AS {text_type}) AS mail, token_version FROM {table} WHERE mail = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(mail).fetch_optional(pool).await?;
    row.map(|row| {
        Ok(UserRecord {
            id: row.try_get("id")?,
            password: text(&row, "password")?,
            name: text(&row, "name")?,
            status: row.try_get("status")?,
            role: row.try_get("role")?,
            mail: text(&row, "mail")?,
            token_version: row.try_get("token_version")?,
        })
    })
    .transpose()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcGroupRule {
    pub space_id: i64,
    pub group_name: String,
    pub role: String,
}

pub async fn oidc_group_rules(
    pool: &AnyPool,
    driver: Driver,
    provider: &str,
) -> Result<Vec<OidcGroupRule>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT space_id, CAST(group_name AS {text_type}) AS group_name, CAST(role AS {text_type}) AS role FROM space_o_id_c_group WHERE provider = ? ORDER BY id"
        ),
    );
    let rows = sqlx::query(&sql).bind(provider).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(OidcGroupRule {
                space_id: row.try_get("space_id")?,
                group_name: text(&row, "group_name")?,
                role: text(&row, "role")?,
            })
        })
        .collect()
}

fn role_rank(role: &str) -> u8 {
    match role {
        "admin" => 3,
        "editor" => 2,
        "viewer" => 1,
        _ => 0,
    }
}

/// `syncOIDCGroups`: provider groups grant access through the configured rules.
/// It only adds or upgrades `source = 'oidc'` rows, never removes manual access.
pub async fn grant_oidc_member(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    user_id: i64,
    role: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let timestamp = timestamp_bind(driver);
    let select = query(
        driver,
        &format!(
            "SELECT id, CAST(source AS {text_type}) AS source, CAST(role AS {text_type}) AS role FROM space_member WHERE space_id = ? AND user_id = ? LIMIT 1"
        ),
    );
    let insert = query(
        driver,
        &format!(
            "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES ({timestamp}, {timestamp}, ?, ?, ?, 'oidc', {timestamp})"
        ),
    );
    let update = query(driver, "UPDATE space_member SET role = ? WHERE id = ?");
    match sqlx::query(&select)
        .bind(space_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
    {
        None => {
            sqlx::query(&insert)
                .bind(now)
                .bind(now)
                .bind(space_id)
                .bind(user_id)
                .bind(role)
                .bind(now)
                .execute(pool)
                .await?;
        }
        Some(row) => {
            let current_source: String = text(&row, "source")?;
            let current_role: String = text(&row, "role")?;
            if current_source == "oidc" && role_rank(role) > role_rank(&current_role) {
                let id: i64 = row.try_get("id")?;
                sqlx::query(&update)
                    .bind(role)
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
        }
    }
    Ok(())
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

pub async fn set_member_role(
    pool: &AnyPool,
    driver: Driver,
    space_ids: &[i64],
    target_user_id: i64,
    role: &str,
) -> Result<(), sqlx::Error> {
    let placeholders = vec!["?"; space_ids.len()].join(",");
    let sql = query(
        driver,
        &format!("UPDATE space_member SET role = ? WHERE space_id IN ({placeholders}) AND user_id = ?"),
    );
    let mut statement = sqlx::query(&sql).bind(role);
    for space_id in space_ids {
        statement = statement.bind(*space_id);
    }
    statement.bind(target_user_id).execute(pool).await?;
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

pub async fn save_module_config(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    name: &str,
    value_json: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    use sqlx::Row;
    let timestamp = timestamp_bind(driver);
    let select = query(
        driver,
        "SELECT id FROM module_config WHERE user_id = ? AND name = ? LIMIT 1",
    );
    let existing = sqlx::query(&select)
        .bind(user_id)
        .bind(name)
        .fetch_optional(pool)
        .await?;
    match existing {
        Some(row) => {
            let id: i64 = row.try_get("id")?;
            let update = query(
                driver,
                &format!("UPDATE module_config SET value_json = ?, updated_at = {timestamp} WHERE id = ?"),
            );
            sqlx::query(&update)
                .bind(value_json)
                .bind(now)
                .bind(id)
                .execute(pool)
                .await?;
        }
        None => {
            let insert = query(
                driver,
                &format!(
                    "INSERT INTO module_config (created_at, updated_at, user_id, name, value_json) VALUES ({timestamp}, {timestamp}, ?, ?, ?)"
                ),
            );
            sqlx::query(&insert)
                .bind(now)
                .bind(now)
                .bind(user_id)
                .bind(name)
                .bind(value_json)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

pub async fn save_user_config(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    panel_json: Option<&str>,
    language: &str,
) -> Result<(), sqlx::Error> {
    let select = query(
        driver,
        "SELECT 1 AS found FROM user_config WHERE user_id = ? LIMIT 1",
    );
    let existing = sqlx::query(&select)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    let found = existing.is_some();
    if found {
        match panel_json {
            Some(panel) => {
                let update = query(
                    driver,
                    "UPDATE user_config SET panel_json = ?, language = ? WHERE user_id = ?",
                );
                sqlx::query(&update)
                    .bind(panel)
                    .bind(language)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            }
            None => {
                let update = query(
                    driver,
                    "UPDATE user_config SET language = ? WHERE user_id = ?",
                );
                sqlx::query(&update)
                    .bind(language)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            }
        }
    } else {
        let insert = query(
            driver,
            "INSERT INTO user_config (user_id, panel_json, language) VALUES (?, ?, ?)",
        );
        sqlx::query(&insert)
            .bind(user_id)
            .bind(panel_json.unwrap_or(""))
            .bind(language)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Inserts one row and returns its id on every driver. MySQL reports
/// `last_insert_id`; SQLite and PostgreSQL use `RETURNING`.
macro_rules! insert_returning_id {
    ($driver:expr, $sql:expr, [$($bind:expr),*], $exec:expr) => {{
        let suffix = if $driver == Driver::MySql {
            ""
        } else {
            " RETURNING id"
        };
        let rendered = query($driver, &format!("{}{}", $sql, suffix));
        let mut statement = sqlx::query(&rendered);
        $( statement = statement.bind($bind); )*
        if $driver == Driver::MySql {
            let result = statement.execute($exec).await?;
            result
                .last_insert_id()
                .ok_or(sqlx::Error::Protocol("driver did not report an insert id".into()))?
        } else {
            let row = statement.fetch_one($exec).await?;
            sqlx::Row::try_get::<i64, _>(&row, "id")?
        }
    }};
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

pub async fn insert_file(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    file_name: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = query(
        driver,
        &format!(
            "INSERT INTO file (created_at, updated_at, user_id, file_name) VALUES ({timestamp}, {timestamp}, ?, ?)"
        ),
    );
    sqlx::query(&sql)
        .bind(now)
        .bind(now)
        .bind(user_id)
        .bind(file_name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn file_name_by_id(
    pool: &AnyPool,
    driver: Driver,
    id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!("SELECT CAST(file_name AS {text_type}) AS file_name FROM file WHERE id = ?"),
    );
    let row = sqlx::query(&sql).bind(id).fetch_optional(pool).await?;
    row.map(|row| text(&row, "file_name")).transpose()
}

pub async fn delete_file_row(
    pool: &AnyPool,
    driver: Driver,
    id: i64,
) -> Result<(), sqlx::Error> {
    let sql = query(driver, "DELETE FROM file WHERE id = ?");
    sqlx::query(&sql).bind(id).execute(pool).await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminUserRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub password: String,
    pub name: String,
    pub head_image: String,
    pub status: i64,
    pub role: i64,
    pub mail: String,
}

pub async fn user_mail_exists(
    pool: &AnyPool,
    driver: Driver,
    mail: &str,
    exclude_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let sql = match exclude_id {
        None => query(driver, &format!("SELECT 1 FROM {user_table} WHERE mail = ? LIMIT 1")),
        Some(_) => query(
            driver,
            &format!("SELECT 1 FROM {user_table} WHERE mail = ? AND id <> ? LIMIT 1"),
        ),
    };
    let mut statement = sqlx::query(&sql).bind(mail);
    if let Some(id) = exclude_id {
        statement = statement.bind(id);
    }
    Ok(statement.fetch_optional(pool).await?.is_some())
}

/// `UsersRouter.Create`: the account, its personal Yin/Yang pair, both admin
/// rows and both default groups in one transaction.
pub async fn create_user(
    pool: &AnyPool,
    driver: Driver,
    mail: &str,
    password_hash: &str,
    name: &str,
    head_image: &str,
    role: i64,
    now: &str,
) -> Result<i64, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let space_table = crate::schema_shared::quote(driver, "space");
    let timestamp = timestamp_bind(driver);
    let enabled = if driver == Driver::Postgres { "FALSE" } else { "0" };
    let pair_update = query(
        driver,
        &format!("UPDATE {space_table} SET pair_id = ? WHERE id = ?"),
    );
    let insert_member = query(
        driver,
        &format!(
            "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES ({timestamp}, {timestamp}, ?, ?, 'admin', 'manual', {timestamp})"
        ),
    );
    let insert_group = query(
        driver,
        &format!(
            "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id) VALUES ({timestamp}, {timestamp}, 'material-symbols:ad-group-outline', 'APP', '', 0, ?, ?)"
        ),
    );

    let mut tx = pool.begin().await?;
    let id = insert_returning_id!(
        driver,
        &format!(
            "INSERT INTO {user_table} (created_at, updated_at, password, name, head_image, status, role, mail, token_version) VALUES ({timestamp}, {timestamp}, ?, ?, ?, 1, ?, ?, 0)"
        ),
        [now, now, password_hash, name, head_image, role, mail],
        &mut *tx
    );
    let yin_id = insert_returning_id!(
        driver,
        &format!(
            "INSERT INTO {space_table} (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES ({timestamp}, {timestamp}, 'personal', ?, ?, 0, 'yin', {enabled}, 'direct')"
        ),
        [now, now, name, id],
        &mut *tx
    );
    sqlx::query(&pair_update)
        .bind(yin_id)
        .bind(yin_id)
        .execute(&mut *tx)
        .await?;
    let yang_name = format!("{name}-B");
    let yang_id = insert_returning_id!(
        driver,
        &format!(
            "INSERT INTO {space_table} (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES ({timestamp}, {timestamp}, 'personal', ?, ?, ?, 'yang', {enabled}, 'direct')"
        ),
        [now, now, yang_name, id, yin_id],
        &mut *tx
    );
    let _ = &insert_group;
    for space_id in [yin_id, yang_id] {
        sqlx::query(&insert_member)
            .bind(now)
            .bind(now)
            .bind(space_id)
            .bind(id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        sqlx::query(&insert_group)
            .bind(now)
            .bind(now)
            .bind(id)
            .bind(space_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(id)
}

pub async fn user_password_and_name(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Option<(String, String)>, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(password AS {text_type}) AS password, CAST(name AS {text_type}) AS name FROM {user_table} WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| Ok((text(&row, "password")?, text(&row, "name")?)))
        .transpose()
}

/// `UsersRouter.Update`: GORM writes only the non-zero fields, so the SET list
/// follows the submitted values.
#[allow(clippy::too_many_arguments)]
pub async fn update_user_admin(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    mail: &str,
    name: Option<&str>,
    head_image: Option<&str>,
    status: Option<i64>,
    role: Option<i64>,
    password_hash: Option<&str>,
    now: &str,
    rename_pair: Option<(&str, &str)>,
) -> Result<(), sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let timestamp = timestamp_bind(driver);
    let mut columns = vec!["mail = ?".to_string()];
    if name.is_some() {
        columns.push("name = ?".to_string());
    }
    if head_image.is_some() {
        columns.push("head_image = ?".to_string());
    }
    if status.is_some() {
        columns.push("status = ?".to_string());
    }
    if role.is_some() {
        columns.push("role = ?".to_string());
    }
    if password_hash.is_some() {
        columns.push("password = ?".to_string());
    }
    columns.push(format!("updated_at = {timestamp}"));
    let sql = query(
        driver,
        &format!(
            "UPDATE {user_table} SET {} WHERE id = ?",
            columns.join(", ")
        ),
    );
    let mut statement = sqlx::query(&sql).bind(mail);
    if let Some(value) = name {
        statement = statement.bind(value);
    }
    if let Some(value) = head_image {
        statement = statement.bind(value);
    }
    if let Some(value) = status {
        statement = statement.bind(value);
    }
    if let Some(value) = role {
        statement = statement.bind(value);
    }
    if let Some(value) = password_hash {
        statement = statement.bind(value);
    }
    statement.bind(now).bind(user_id).execute(pool).await?;

    if let Some((yin, yang)) = rename_pair {
        let space_table = crate::schema_shared::quote(driver, "space");
        let rename = query(
            driver,
            &format!(
                "UPDATE {space_table} SET name = ?, updated_at = {timestamp} WHERE owner_user_id = ? AND type = 'personal' AND side = ?"
            ),
        );
        sqlx::query(&rename)
            .bind(yin)
            .bind(now)
            .bind(user_id)
            .bind("yin")
            .execute(pool)
            .await?;
        sqlx::query(&rename)
            .bind(yang)
            .bind(now)
            .bind(user_id)
            .bind("yang")
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn admin_user_view(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Option<AdminUserRecord>, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST(password AS {text_type}) AS password, CAST(name AS {text_type}) AS name, CAST(head_image AS {text_type}) AS head_image, status, role, CAST(mail AS {text_type}) AS mail FROM {user_table} WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(AdminUserRecord {
            id: row.try_get("id")?,
            created_at: text(&row, "created_at")?,
            updated_at: text(&row, "updated_at")?,
            password: text(&row, "password")?,
            name: text(&row, "name")?,
            head_image: text(&row, "head_image")?,
            status: row.try_get("status")?,
            role: row.try_get("role")?,
            mail: text(&row, "mail")?,
        })
    })
    .transpose()
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

/// `UserRepo.DeleteWithSpaces`: the owned Spaces, the user rows that carry its
/// ownership, then the account, in one transaction.
pub async fn delete_user(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_ids: &[i64],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    if !space_ids.is_empty() {
        let placeholders = vec!["?"; space_ids.len()].join(",");
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
    }
    for table in [
        "o_auth_identity",
        "item_icon",
        "item_icon_group",
        "module_config",
        "file",
    ] {
        let sql = query(driver, &format!("DELETE FROM {table} WHERE user_id = ?"));
        sqlx::query(&sql)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }
    let user_table = crate::schema_shared::quote(driver, "user");
    let sql = query(driver, &format!("DELETE FROM {user_table} WHERE id = ?"));
    sqlx::query(&sql)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// `UserRouter.UpdateInfo`: the profile and, when the nickname changed, the
/// personal pair that carries it.
pub async fn update_user_profile(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    name: &str,
    head_image: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let timestamp = timestamp_bind(driver);
    let update = query(
        driver,
        &format!(
            "UPDATE {user_table} SET name = ?, head_image = ?, updated_at = {timestamp} WHERE id = ?"
        ),
    );
    let mut tx = pool.begin().await?;
    sqlx::query(&update)
        .bind(name)
        .bind(head_image)
        .bind(now)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let space_table = crate::schema_shared::quote(driver, "space");
    let rename = query(
        driver,
        &format!(
            "UPDATE {space_table} SET name = ?, updated_at = {timestamp} WHERE type = 'personal' AND owner_user_id = ? AND side = ?"
        ),
    );
    sqlx::query(&rename)
        .bind(name)
        .bind(now)
        .bind(user_id)
        .bind("yin")
        .execute(&mut *tx)
        .await?;
    let yang_name = format!("{name}-B");
    sqlx::query(&rename)
        .bind(yang_name)
        .bind(now)
        .bind(user_id)
        .bind("yang")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// `UserRouter.UpdatePasssword`: verify the old hash, store the new one and
/// bump `token_version` so every issued token is revoked.
pub async fn update_user_password(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    old_hash: &str,
    new_hash: &str,
    now: &str,
) -> Result<bool, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let timestamp = timestamp_bind(driver);
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "UPDATE {user_table} SET password = ?, token_version = token_version + 1, updated_at = {timestamp} WHERE id = ? AND CAST(password AS {text_type}) = ?"
        ),
    );
    let result = sqlx::query(&sql)
        .bind(new_hash)
        .bind(now)
        .bind(user_id)
        .bind(old_hash)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePackageRow {
    pub id: String,
    pub name: String,
    pub latest_revision: String,
    pub version: String,
    pub verified: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeRevisionRow {
    pub id: String,
    pub package_id: String,
    pub version: String,
    pub verified: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
    pub id: i64,
    pub actor_id: i64,
    pub action: String,
    pub package_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeAdminState {
    pub packages: Vec<ThemePackageRow>,
    pub revisions: Vec<ThemeRevisionRow>,
    pub active_revision: String,
    pub last_good_revision: String,
    pub pending_revision: String,
    pub trial_started_at: Option<String>,
    pub default_package: String,
}

fn bool_expression(driver: Driver, column: &str) -> String {
    match driver {
        Driver::MySql => format!("CAST({column} AS SIGNED)"),
        Driver::Sqlite => format!("CAST({column} AS INTEGER)"),
        Driver::Postgres => format!(
            "CASE WHEN {column} THEN 1 ELSE 0 END"
        ),
    }
}

pub async fn user_theme_preference(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Option<(String, String)>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(mode AS {text_type}) AS mode, CAST(theme_mode AS {text_type}) AS theme_mode FROM user_theme_preference_v2 WHERE user_id = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(user_id).fetch_optional(pool).await?;
    row.map(|row| Ok((text(&row, "mode")?, text(&row, "theme_mode")?)))
        .transpose()
}

pub async fn package_id_for_revision(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(package_id AS {text_type}) AS package_id FROM revision_record_v2 WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .fetch_optional(pool)
        .await?;
    row.map(|row| text(&row, "package_id")).transpose()
}

/// The preference upsert: MySQL needs `ON DUPLICATE KEY`, the others use
/// `ON CONFLICT`.
pub async fn save_theme_preference(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    mode: &str,
    theme_mode: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let (sql, has_mode) = match driver {
        Driver::MySql => (
            "INSERT INTO user_theme_preference_v2 (user_id, mode, theme_mode, updated_at) VALUES (?, ?, ?, TS) ON DUPLICATE KEY UPDATE mode = VALUES(mode), theme_mode = VALUES(theme_mode), updated_at = VALUES(updated_at)"
                .replace("TS", timestamp),
            true,
        ),
        _ => (
            "INSERT INTO user_theme_preference_v2 (user_id, mode, theme_mode, updated_at) VALUES (?, ?, ?, TS) ON CONFLICT(user_id) DO UPDATE SET mode = excluded.mode, theme_mode = excluded.theme_mode, updated_at = excluded.updated_at"
                .replace("TS", timestamp),
            true,
        ),
    };
    let _ = has_mode;
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(user_id)
        .bind(mode)
        .bind(theme_mode)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn theme_admin_state(
    pool: &AnyPool,
    driver: Driver,
) -> Result<ThemeAdminState, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let removed = match driver {
        Driver::Postgres => "NOT removed",
        _ => "removed = 0",
    };
    let packages_sql = query(
        driver,
        &format!(
            "SELECT CAST(id AS {text_type}) AS id, CAST(name AS {text_type}) AS name, CAST(latest_revision AS {text_type}) AS latest_revision, (SELECT CAST(version AS {text_type}) FROM revision_record_v2 WHERE id = package_record_v2.latest_revision) AS version, (SELECT {} FROM revision_record_v2 WHERE id = package_record_v2.latest_revision) AS verified, (SELECT CAST(created_at AS {text_type}) FROM revision_record_v2 WHERE id = package_record_v2.latest_revision) AS created_at FROM package_record_v2 WHERE {removed} ORDER BY id",
            bool_expression(driver, "verified")
        ),
    );
    let packages = sqlx::query(&packages_sql)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(ThemePackageRow {
                id: text(&row, "id")?,
                name: text(&row, "name")?,
                latest_revision: text(&row, "latest_revision")?,
                version: text(&row, "version").unwrap_or_default(),
                verified: row.try_get::<i64, _>("verified").unwrap_or(0),
                created_at: text(&row, "created_at").unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

    let revisions_sql = query(
        driver,
        &format!(
            "SELECT CAST(id AS {text_type}) AS id, CAST(package_id AS {text_type}) AS package_id, CAST(version AS {text_type}) AS version, {} AS verified, CAST(created_at AS {text_type}) AS created_at FROM revision_record_v2 ORDER BY package_id, created_at DESC",
            bool_expression(driver, "verified")
        ),
    );
    let revisions = sqlx::query(&revisions_sql)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(ThemeRevisionRow {
                id: text(&row, "id")?,
                package_id: text(&row, "package_id")?,
                version: text(&row, "version")?,
                verified: row.try_get("verified")?,
                created_at: text(&row, "created_at")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

    let activation_sql = query(
        driver,
        &format!(
            "SELECT CAST(active_revision_id AS {text_type}) AS active_revision_id, CAST(last_good_revision_id AS {text_type}) AS last_good_revision_id, CAST(pending_revision_id AS {text_type}) AS pending_revision_id, CAST(trial_started_at AS {text_type}) AS trial_started_at FROM activation_record_v2 WHERE scope = 'instance'"
        ),
    );
    let activation = sqlx::query(&activation_sql).fetch_optional(pool).await?;
    let mut state = ThemeAdminState {
        packages,
        revisions,
        ..Default::default()
    };
    if let Some(row) = activation {
        state.active_revision = text(&row, "active_revision_id").unwrap_or_default();
        state.last_good_revision = text(&row, "last_good_revision_id").unwrap_or_default();
        state.pending_revision = text(&row, "pending_revision_id").unwrap_or_default();
        state.trial_started_at = text_opt(&row, "trial_started_at").unwrap_or_default();
    }
    if state.active_revision.is_empty() {
        let latest_sql = query(
            driver,
            &format!(
                "SELECT CAST(latest_revision AS {text_type}) AS latest_revision FROM package_record_v2 WHERE id = 'org.yin.default' AND {removed}"
            ),
        );
        let latest = sqlx::query(&latest_sql).fetch_optional(pool).await?;
        state.active_revision = latest
            .map(|row| text(&row, "latest_revision").unwrap_or_default())
            .unwrap_or_default();
    }
    let package_sql = query(
        driver,
        &format!(
            "SELECT CAST(package_id AS {text_type}) AS package_id FROM revision_record_v2 WHERE id = ?"
        ),
    );
    state.default_package = sqlx::query(&package_sql)
        .bind(&state.active_revision)
        .fetch_optional(pool)
        .await?
        .map(|row| text(&row, "package_id").unwrap_or_default())
        .unwrap_or_default();
    Ok(state)
}

pub async fn audit_entries(
    pool: &AnyPool,
    driver: Driver,
) -> Result<Vec<AuditRow>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    // The recorded contract expects the newest 200, and ties to come out in
    // reverse insertion order; `id DESC` makes that deterministic.
    let sql = query(
        driver,
        &format!(
            "SELECT id, actor_id, CAST(action AS {text_type}) AS action, CAST(package_id AS {text_type}) AS package_id, CAST(created_at AS {text_type}) AS created_at FROM audit_record ORDER BY created_at DESC, id DESC LIMIT 200"
        ),
    );
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(AuditRow {
                id: row.try_get("id")?,
                actor_id: row.try_get("actor_id")?,
                action: text(&row, "action")?,
                package_id: text(&row, "package_id")?,
                created_at: text(&row, "created_at")?,
            })
        })
        .collect()
}

pub async fn revision_manifest(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(manifest_json AS {text_type}) AS manifest_json FROM revision_record_v2 WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .fetch_optional(pool)
        .await?;
    row.map(|row| text(&row, "manifest_json")).transpose()
}

pub async fn trusted_enabled(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<bool>, sqlx::Error> {
    let sql = query(
        driver,
        &format!(
            "SELECT {} AS enabled FROM trusted_runtime_policy_v2 WHERE revision_id = ?",
            bool_expression(driver, "enabled")
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .fetch_optional(pool)
        .await?;
    row.map(|row| Ok(row.try_get::<i64, _>("enabled")? != 0))
        .transpose()
}

pub async fn save_trusted_policy(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
    enabled: bool,
    user_id: i64,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = match driver {
        Driver::MySql => format!(
            "INSERT INTO trusted_runtime_policy_v2 (revision_id, enabled, updated_by, updated_at) VALUES (?, ?, ?, {timestamp}) ON DUPLICATE KEY UPDATE enabled = VALUES(enabled), updated_by = VALUES(updated_by), updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO trusted_runtime_policy_v2 (revision_id, enabled, updated_by, updated_at) VALUES (?, ?, ?, {timestamp}) ON CONFLICT(revision_id) DO UPDATE SET enabled = excluded.enabled, updated_by = excluded.updated_by, updated_at = excluded.updated_at"
        ),
    };
    let sql = query(driver, &sql);
    let mut statement = sqlx::query(&sql).bind(revision);
    statement = if driver == Driver::Postgres {
        statement.bind(enabled)
    } else {
        statement.bind(i64::from(enabled))
    };
    statement.bind(user_id).bind(now).execute(pool).await?;
    Ok(())
}

pub async fn withdraw_trusted_grants(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<(), sqlx::Error> {
    let sql = query(
        driver,
        "DELETE FROM grant_record_v2 WHERE revision_id = ? AND execution_mode = 'trusted'",
    );
    sqlx::query(&sql).bind(revision).execute(pool).await?;
    Ok(())
}

pub async fn latest_revision(
    pool: &AnyPool,
    driver: Driver,
    package_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let removed = match driver {
        Driver::Postgres => "NOT removed",
        _ => "removed = 0",
    };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(latest_revision AS {text_type}) AS latest_revision FROM package_record_v2 WHERE id = ? AND {removed}"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(package_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| text(&row, "latest_revision")).transpose()
}

pub async fn activation_revision(
    pool: &AnyPool,
    driver: Driver,
    scope: &str,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(active_revision_id AS {text_type}) AS active_revision_id FROM activation_record_v2 WHERE scope = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(scope).fetch_optional(pool).await?;
    Ok(row
        .map(|row| text(&row, "active_revision_id").unwrap_or_default())
        .filter(|value| !value.is_empty()))
}

/// `ThemeRouter.SelectV2`: the user activation record, the preference and the
/// audit row, with a MySQL-compatible upsert on both keys.
pub async fn select_user_theme(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    package_id: &str,
    revision_id: &str,
    mode: &str,
    now: &str,
) -> Result<(), String> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let removed = match driver {
        Driver::Postgres => "NOT removed",
        _ => "removed = 0",
    };
    let package_sql = query(
        driver,
        &format!(
            "SELECT 1 AS found FROM package_record_v2 WHERE id = ? AND {removed}"
        ),
    );
    let package = sqlx::query(&package_sql)
        .bind(package_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| error.to_string())?;
    if package.is_none() {
        return Err("record not found".to_string());
    }
    let revision_sql = query(
        driver,
        "SELECT 1 AS found FROM revision_record_v2 WHERE id = ? AND package_id = ?",
    );
    let revision = sqlx::query(&revision_sql)
        .bind(revision_id)
        .bind(package_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| error.to_string())?;
    if revision.is_none() {
        return Err("record not found".to_string());
    }

    let timestamp = timestamp_bind(driver);
    let scope = format!("user:{user_id}");
    let activation = match driver {
        Driver::MySql => format!(
            "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at) VALUES (?, ?, ?, ?, '', NULL, {timestamp}) ON DUPLICATE KEY UPDATE package_id = VALUES(package_id), active_revision_id = VALUES(active_revision_id), last_good_revision_id = VALUES(last_good_revision_id), pending_revision_id = '', trial_started_at = NULL, updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at) VALUES (?, ?, ?, ?, '', NULL, {timestamp}) ON CONFLICT(scope) DO UPDATE SET package_id = excluded.package_id, active_revision_id = excluded.active_revision_id, last_good_revision_id = excluded.last_good_revision_id, pending_revision_id = '', trial_started_at = NULL, updated_at = excluded.updated_at"
        ),
    };
    let activation = query(driver, &activation);
    sqlx::query(&activation)
        .bind(&scope)
        .bind(package_id)
        .bind(revision_id)
        .bind(revision_id)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;

    let preference = match driver {
        Driver::MySql => format!(
            "INSERT INTO user_theme_preference_v2 (user_id, mode, theme_mode, updated_at) VALUES (?, ?, ?, {timestamp}) ON DUPLICATE KEY UPDATE mode = VALUES(mode), theme_mode = VALUES(theme_mode), updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO user_theme_preference_v2 (user_id, mode, theme_mode, updated_at) VALUES (?, ?, ?, {timestamp}) ON CONFLICT(user_id) DO UPDATE SET mode = excluded.mode, theme_mode = excluded.theme_mode, updated_at = excluded.updated_at"
        ),
    };
    let preference = query(driver, &preference);
    sqlx::query(&preference)
        .bind(user_id)
        .bind(mode)
        .bind("custom")
        .bind(now)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;
    let _ = text_type;

    let audit = query(
        driver,
        &format!(
            "INSERT INTO audit_record (actor_id, action, package_id, created_at) VALUES (?, 'select', ?, {timestamp})"
        ),
    );
    sqlx::query(&audit)
        .bind(user_id)
        .bind(package_id)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn removed_literal(driver: Driver) -> &'static str {
    match driver {
        Driver::Postgres => "false",
        _ => "0",
    }
}

pub async fn revision_package_id(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<String>, sqlx::Error> {
    package_id_for_revision(pool, driver, revision).await
}

pub async fn package_removed(
    pool: &AnyPool,
    driver: Driver,
    package_id: &str,
) -> Result<Option<i64>, sqlx::Error> {
    use sqlx::Row;
    let sql = query(
        driver,
        &format!(
            "SELECT {} AS removed FROM package_record_v2 WHERE id = ?",
            bool_expression(driver, "removed")
        ),
    );
    let row = sqlx::query(&sql)
        .bind(package_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| row.try_get("removed")).transpose()
}

/// `InstallPackageV2`.
pub async fn install_package(
    pool: &AnyPool,
    driver: Driver,
    actor_id: i64,
    package: &crate::packages::Package,
) -> Result<(), String> {
    let manifest = &package.manifest;
    let manifest_json = serde_json::to_string(manifest).map_err(|error| error.to_string())?;
    let tokens_json = serde_json::to_string(&package.tokens).map_err(|error| error.to_string())?;
    let now = go_timestamp();
    let removed = removed_literal(driver);
    let timestamp = timestamp_bind(driver);
    let mut tx = pool.begin().await.map_err(|error| error.to_string())?;

    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let existing_sql = query(
        driver,
        &format!(
            "SELECT CAST(id AS {text_type}) AS id FROM revision_record_v2 WHERE package_id = ? AND version = ?"
        ),
    );
    let existing = sqlx::query(&existing_sql)
        .bind(&manifest.id)
        .bind(&manifest.version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| error.to_string())?;
    let existing_id: Option<String> = existing.map(|row| text(&row, "id").unwrap_or_default());

    if let Some(existing_id) = existing_id {
        if existing_id != package.revision {
            return Err("same theme package version has different content".to_string());
        }
        let update = query(
            driver,
            &format!(
                "UPDATE package_record_v2 SET name = ?, latest_revision = ?, removed = {removed} WHERE id = ?"
            ),
        );
        sqlx::query(&update)
            .bind(&manifest.name)
            .bind(&package.revision)
            .bind(&manifest.id)
            .execute(&mut *tx)
            .await
            .map_err(|_| "install failed".to_string())?;
    } else {
        let record = package_removed_tx(&mut tx, driver, &manifest.id)
            .await
            .map_err(|error| error.to_string())?;
        match record {
            None => {
                let insert = query(
                    driver,
                    &format!(
                        "INSERT INTO package_record_v2 (id, name, latest_revision, removed, created_at, updated_at) VALUES (?, ?, '', {removed}, {timestamp}, {timestamp})"
                    ),
                );
                sqlx::query(&insert)
                    .bind(&manifest.id)
                    .bind(&manifest.name)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            Some(removed_flag) if removed_flag != 0 => {
                return Err(
                    "removed theme packages must be explicitly restored before installation"
                        .to_string(),
                );
            }
            Some(_) => {}
        }
        let insert_revision = query(
            driver,
            &format!(
                "INSERT INTO revision_record_v2 (id, package_id, version, manifest_json, tokens_json, verified, created_at) VALUES (?, ?, ?, ?, ?, ?, {timestamp})"
            ),
        );
        let mut statement = sqlx::query(&insert_revision)
            .bind(&package.revision)
            .bind(&manifest.id)
            .bind(&manifest.version)
            .bind(&manifest_json)
            .bind(&tokens_json);
        statement = if driver == Driver::Postgres {
            statement.bind(package.verified)
        } else {
            statement.bind(i64::from(package.verified))
        };
        statement
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
        for (name, (media_type, content)) in &package.files {
            let insert_asset = query(
                driver,
                "INSERT INTO asset_record_v2 (revision_id, path, media_type, content) VALUES (?, ?, ?, ?)",
            );
            sqlx::query(&insert_asset)
                .bind(&package.revision)
                .bind(name)
                .bind(media_type)
                .bind(content)
                .execute(&mut *tx)
                .await
                .map_err(|error| error.to_string())?;
        }
        let update = query(
            driver,
            &format!(
                "UPDATE package_record_v2 SET name = ?, latest_revision = ?, removed = {removed} WHERE id = ?"
            ),
        );
        sqlx::query(&update)
            .bind(&manifest.name)
            .bind(&package.revision)
            .bind(&manifest.id)
            .execute(&mut *tx)
            .await
            .map_err(|_| "install failed".to_string())?;
        let audit = query(
            driver,
            &format!(
                "INSERT INTO audit_record (actor_id, action, package_id, created_at) VALUES (?, 'install-v2', ?, {timestamp})"
            ),
        );
        sqlx::query(&audit)
            .bind(actor_id)
            .bind(&manifest.id)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
    }
    tx.commit().await.map_err(|error| error.to_string())
}

async fn package_removed_tx(
    tx: &mut sqlx::AnyConnection,
    driver: Driver,
    package_id: &str,
) -> Result<Option<i64>, sqlx::Error> {
    use sqlx::Row;
    let sql = query(
        driver,
        &format!(
            "SELECT {} AS removed FROM package_record_v2 WHERE id = ?",
            bool_expression(driver, "removed")
        ),
    );
    let row = sqlx::query(&sql)
        .bind(package_id)
        .fetch_optional(&mut *tx)
        .await?;
    row.map(|row| row.try_get("removed")).transpose()
}

pub async fn remove_theme_package(
    pool: &AnyPool,
    driver: Driver,
    actor_id: i64,
    package_id: &str,
) -> Result<(), String> {
    let now = go_timestamp();
    let removed = removed_literal(driver);
    let one = match driver {
        Driver::Postgres => "true",
        _ => "1",
    };
    let timestamp = timestamp_bind(driver);
    let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
    let update_package = query(
        driver,
        &format!(
            "UPDATE package_record_v2 SET removed = {one}, updated_at = {timestamp} WHERE id = ?"
        ),
    );
    sqlx::query(&update_package)
        .bind(&now)
        .bind(package_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| "remove failed".to_string())?;
    let reset = query(
        driver,
        &format!(
            "UPDATE activation_record_v2 SET package_id = 'org.yin.default', active_revision_id = ?, last_good_revision_id = ?, pending_revision_id = '', trial_started_at = NULL, updated_at = {timestamp} WHERE package_id = ?"
        ),
    );
    let yin = latest_revision(pool, driver, "org.yin.default")
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "default package missing".to_string())?;
    sqlx::query(&reset)
        .bind(&yin)
        .bind(&yin)
        .bind(&now)
        .bind(package_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| "remove failed".to_string())?;
    let audit = query(
        driver,
        &format!(
            "INSERT INTO audit_record (actor_id, action, package_id, created_at) VALUES (?, 'remove-v2', ?, {timestamp})"
        ),
    );
    sqlx::query(&audit)
        .bind(actor_id)
        .bind(package_id)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|_| "remove failed".to_string())?;
    let _ = removed;
    tx.commit().await.map_err(|error| error.to_string())
}

/// `SetDefaultV2` and `BeginTrialV2` share the activation upsert; MySQL spells
/// it `ON DUPLICATE KEY UPDATE`.
fn activation_upsert(driver: Driver, set: &str, values: &str, conflict: &str) -> String {
    match driver {
        Driver::MySql => format!(
            "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at) VALUES ({values}) ON DUPLICATE KEY UPDATE {set}"
        ),
        _ => format!(
            "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at) VALUES ({values}) ON CONFLICT(scope) DO UPDATE SET {conflict}"
        ),
    }
}

pub async fn set_default_theme(
    pool: &AnyPool,
    driver: Driver,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = activation_upsert(
        driver,
        "package_id = VALUES(package_id), active_revision_id = VALUES(active_revision_id), last_good_revision_id = VALUES(last_good_revision_id), pending_revision_id = '', trial_started_at = NULL, updated_at = VALUES(updated_at)",
        &format!("'instance', ?, ?, ?, '', NULL, {timestamp}"),
        "package_id = excluded.package_id, active_revision_id = excluded.active_revision_id, last_good_revision_id = excluded.last_good_revision_id, pending_revision_id = '', trial_started_at = NULL, updated_at = excluded.updated_at",
    );
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(package_id)
        .bind(revision_id)
        .bind(revision_id)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn begin_theme_trial(
    pool: &AnyPool,
    driver: Driver,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = activation_upsert(
        driver,
        "pending_revision_id = VALUES(pending_revision_id), trial_started_at = VALUES(trial_started_at), updated_at = VALUES(updated_at)",
        &format!("'instance', ?, '', '', ?, ?, {timestamp}"),
        "pending_revision_id = excluded.pending_revision_id, trial_started_at = excluded.trial_started_at, updated_at = excluded.updated_at",
    );
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(package_id)
        .bind(revision_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn confirm_theme_trial(
    pool: &AnyPool,
    driver: Driver,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let activate = query(
        driver,
        &format!(
            "UPDATE activation_record_v2 SET package_id = ?, active_revision_id = ?, pending_revision_id = '', trial_started_at = NULL, updated_at = {timestamp} WHERE scope = 'instance'"
        ),
    );
    let healthy = query(
        driver,
        &format!(
            "UPDATE activation_record_v2 SET last_good_revision_id = ?, updated_at = {timestamp} WHERE scope = 'instance' AND active_revision_id = ? AND pending_revision_id = ''"
        ),
    );
    let mut tx = pool.begin().await?;
    sqlx::query(&activate)
        .bind(package_id)
        .bind(revision_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query(&healthy)
        .bind(revision_id)
        .bind(now)
        .bind(revision_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn rollback_theme(
    pool: &AnyPool,
    driver: Driver,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = query(
        driver,
        &format!(
            "UPDATE activation_record_v2 SET package_id = ?, active_revision_id = ?, pending_revision_id = '', trial_started_at = NULL, updated_at = {timestamp} WHERE scope = 'instance'"
        ),
    );
    sqlx::query(&sql)
        .bind(package_id)
        .bind(revision_id)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn instance_last_good(
    pool: &AnyPool,
    driver: Driver,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(last_good_revision_id AS {text_type}) AS last_good_revision_id FROM activation_record_v2 WHERE scope = 'instance'"
        ),
    );
    let row = sqlx::query(&sql).fetch_optional(pool).await?;
    row.map(|row| text(&row, "last_good_revision_id")).transpose()
}

pub async fn instance_pending(
    pool: &AnyPool,
    driver: Driver,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(pending_revision_id AS {text_type}) AS pending_revision_id FROM activation_record_v2 WHERE scope = 'instance'"
        ),
    );
    let row = sqlx::query(&sql).fetch_optional(pool).await?;
    row.map(|row| text(&row, "pending_revision_id")).transpose()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebWallpaper {
    pub html: Vec<u8>,
    pub poster: Vec<u8>,
    pub poster_type: String,
}

pub async fn web_wallpaper_exists(
    pool: &AnyPool,
    driver: Driver,
    id: &str,
) -> Result<bool, sqlx::Error> {
    let sql = query(
        driver,
        "SELECT 1 AS found FROM web_wallpaper_record WHERE id = ?",
    );
    Ok(sqlx::query(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .is_some())
}

pub async fn insert_web_wallpaper(
    pool: &AnyPool,
    driver: Driver,
    id: &str,
    owner_id: i64,
    html: &[u8],
    poster: &[u8],
    poster_type: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = query(
        driver,
        &format!(
            "INSERT INTO web_wallpaper_record (id, owner_id, html, poster, poster_type, created_at) VALUES (?, ?, ?, ?, ?, {timestamp})"
        ),
    );
    sqlx::query(&sql)
        .bind(id)
        .bind(owner_id)
        .bind(html)
        .bind(poster)
        .bind(poster_type)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn web_wallpaper(
    pool: &AnyPool,
    driver: Driver,
    id: &str,
) -> Result<Option<WebWallpaper>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT html, poster, CAST(poster_type AS {text_type}) AS poster_type FROM web_wallpaper_record WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(id).fetch_optional(pool).await?;
    row.map(|row| {
        Ok(WebWallpaper {
            html: row.try_get("html")?,
            poster: row.try_get("poster")?,
            poster_type: text(&row, "poster_type")?,
        })
    })
    .transpose()
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

/// Idempotent file row inside a caller-owned transaction.
pub async fn insert_file_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    driver: Driver,
    user_id: i64,
    file_name: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    use sqlx::Row;
    let count = query(
        driver,
        "SELECT COUNT(*) AS total FROM file WHERE user_id = ? AND file_name = ?",
    );
    let row = sqlx::query(&count)
        .bind(user_id)
        .bind(file_name)
        .fetch_one(&mut **tx)
        .await?;
    let total: i64 = row.try_get("total")?;
    if total > 0 {
        return Ok(());
    }
    let timestamp = timestamp_bind(driver);
    let insert = query(
        driver,
        &format!(
            "INSERT INTO file (created_at, updated_at, user_id, file_name) VALUES ({timestamp}, {timestamp}, ?, ?)"
        ),
    );
    sqlx::query(&insert)
        .bind(now)
        .bind(now)
        .bind(user_id)
        .bind(file_name)
        .execute(&mut **tx)
        .await?;
    Ok(())
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionParts {
    pub manifest_json: String,
    pub tokens_json: String,
    pub verified: i64,
}

pub async fn revision_parts(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<RevisionParts>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(manifest_json AS {text_type}) AS manifest_json, CAST(tokens_json AS {text_type}) AS tokens_json, {} AS verified FROM revision_record_v2 WHERE id = ?",
            bool_expression(driver, "verified")
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(RevisionParts {
            manifest_json: text(&row, "manifest_json")?,
            tokens_json: text(&row, "tokens_json")?,
            verified: row.try_get("verified")?,
        })
    })
    .transpose()
}

pub async fn activation_package(
    pool: &AnyPool,
    driver: Driver,
    scope: &str,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(package_id AS {text_type}) AS package_id FROM activation_record_v2 WHERE scope = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(scope).fetch_optional(pool).await?;
    row.map(|row| text(&row, "package_id")).transpose()
}

pub async fn user_space_theme_revision(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(revision_id AS {text_type}) AS revision_id FROM user_space_theme_preference_v2 WHERE user_id = ? AND space_id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| text(&row, "revision_id")).transpose()
}

pub async fn space_theme_revision(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(revision_id AS {text_type}) AS revision_id FROM space_theme_preference_v2 WHERE space_id = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(space_id).fetch_optional(pool).await?;
    row.map(|row| text(&row, "revision_id")).transpose()
}

pub async fn space_theme_pair(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Option<(String, String)>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(package_id AS {text_type}) AS package_id, CAST(revision_id AS {text_type}) AS revision_id FROM space_theme_preference_v2 WHERE space_id = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(space_id).fetch_optional(pool).await?;
    row.map(|row| Ok((text(&row, "package_id")?, text(&row, "revision_id")?)))
        .transpose()
}

pub async fn user_space_theme_pair(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
) -> Result<Option<(String, String)>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(package_id AS {text_type}) AS package_id, CAST(revision_id AS {text_type}) AS revision_id FROM user_space_theme_preference_v2 WHERE user_id = ? AND space_id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| Ok((text(&row, "package_id")?, text(&row, "revision_id")?)))
        .transpose()
}

pub async fn theme_asset_content(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
    path: &str,
) -> Result<Option<(String, Vec<u8>)>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(media_type AS {text_type}) AS media_type, content FROM asset_record_v2 WHERE revision_id = ? AND path = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .bind(path)
        .fetch_optional(pool)
        .await?;
    row.map(|row| Ok((text(&row, "media_type")?, row.try_get("content")?)))
        .transpose()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantRow {
    pub revision_id: String,
    pub execution_mode: String,
    pub permissions_json: String,
}

pub async fn grant_row(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    revision: &str,
    execution_mode: &str,
) -> Result<Option<GrantRow>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(revision_id AS {text_type}) AS revision_id, CAST(execution_mode AS {text_type}) AS execution_mode, CAST(permissions_json AS {text_type}) AS permissions_json FROM grant_record_v2 WHERE user_id = ? AND revision_id = ? AND execution_mode = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .bind(revision)
        .bind(execution_mode)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(GrantRow {
            revision_id: text(&row, "revision_id")?,
            execution_mode: text(&row, "execution_mode")?,
            permissions_json: text(&row, "permissions_json")?,
        })
    })
    .transpose()
}

pub async fn save_grant(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    revision: &str,
    execution_mode: &str,
    permissions_json: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = match driver {
        Driver::MySql => format!(
            "INSERT INTO grant_record_v2 (user_id, revision_id, execution_mode, permissions_json, updated_at) VALUES (?, ?, ?, ?, {timestamp}) ON DUPLICATE KEY UPDATE permissions_json = VALUES(permissions_json), updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO grant_record_v2 (user_id, revision_id, execution_mode, permissions_json, updated_at) VALUES (?, ?, ?, ?, {timestamp}) ON CONFLICT(user_id, revision_id, execution_mode) DO UPDATE SET permissions_json = excluded.permissions_json, updated_at = excluded.updated_at"
        ),
    };
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(user_id)
        .bind(revision)
        .bind(execution_mode)
        .bind(permissions_json)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_grant(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    revision: &str,
    execution_mode: &str,
) -> Result<(), sqlx::Error> {
    let sql = query(
        driver,
        "DELETE FROM grant_record_v2 WHERE user_id = ? AND revision_id = ? AND execution_mode = ?",
    );
    sqlx::query(&sql)
        .bind(user_id)
        .bind(revision)
        .bind(execution_mode)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn revision_verified_manifest(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<(String, i64)>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(manifest_json AS {text_type}) AS manifest_json, {} AS verified FROM revision_record_v2 WHERE id = ?",
            bool_expression(driver, "verified")
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .fetch_optional(pool)
        .await?;
    row.map(|row| Ok((text(&row, "manifest_json")?, row.try_get("verified")?)))
        .transpose()
}

pub async fn save_space_theme(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = match driver {
        Driver::MySql => format!(
            "INSERT INTO space_theme_preference_v2 (space_id, package_id, revision_id, updated_by, updated_at) VALUES (?, ?, ?, 0, {timestamp}) ON DUPLICATE KEY UPDATE package_id = VALUES(package_id), revision_id = VALUES(revision_id), updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO space_theme_preference_v2 (space_id, package_id, revision_id, updated_by, updated_at) VALUES (?, ?, ?, 0, {timestamp}) ON CONFLICT(space_id) DO UPDATE SET package_id = excluded.package_id, revision_id = excluded.revision_id, updated_at = excluded.updated_at"
        ),
    };
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(space_id)
        .bind(package_id)
        .bind(revision_id)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn save_user_space_theme(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = match driver {
        Driver::MySql => format!(
            "INSERT INTO user_space_theme_preference_v2 (user_id, space_id, package_id, revision_id, updated_at) VALUES (?, ?, ?, ?, {timestamp}) ON DUPLICATE KEY UPDATE package_id = VALUES(package_id), revision_id = VALUES(revision_id), updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO user_space_theme_preference_v2 (user_id, space_id, package_id, revision_id, updated_at) VALUES (?, ?, ?, ?, {timestamp}) ON CONFLICT(user_id, space_id) DO UPDATE SET package_id = excluded.package_id, revision_id = excluded.revision_id, updated_at = excluded.updated_at"
        ),
    };
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(user_id)
        .bind(space_id)
        .bind(package_id)
        .bind(revision_id)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn clear_space_theme_row(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<(), sqlx::Error> {
    let sql = query(
        driver,
        "DELETE FROM space_theme_preference_v2 WHERE space_id = ?",
    );
    sqlx::query(&sql).bind(space_id).execute(pool).await?;
    Ok(())
}

pub async fn clear_user_space_theme_row(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
) -> Result<(), sqlx::Error> {
    let sql = query(
        driver,
        "DELETE FROM user_space_theme_preference_v2 WHERE user_id = ? AND space_id = ?",
    );
    sqlx::query(&sql)
        .bind(user_id)
        .bind(space_id)
        .execute(pool)
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

pub async fn user_by_id(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Option<UserRecord>, sqlx::Error> {
    let table = match driver {
        Driver::MySql => "`user`",
        Driver::Sqlite | Driver::Postgres => "\"user\"",
    };
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(password AS {text_type}) AS password, CAST(name AS {text_type}) AS name, status, role, CAST(mail AS {text_type}) AS mail, token_version FROM {table} WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(user_id).fetch_optional(pool).await?;
    row.map(|row| {
        Ok(UserRecord {
            id: row.try_get("id")?,
            password: text(&row, "password")?,
            name: text(&row, "name")?,
            status: row.try_get("status")?,
            role: row.try_get("role")?,
            mail: text(&row, "mail")?,
            token_version: row.try_get("token_version")?,
        })
    })
    .transpose()
}

pub async fn user_by_credentials(
    pool: &AnyPool,
    driver: Driver,
    mail: &str,
    password: &str,
) -> Result<Option<UserRecord>, sqlx::Error> {
    let table = match driver {
        Driver::MySql => "`user`",
        Driver::Sqlite | Driver::Postgres => "\"user\"",
    };
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(password AS {text_type}) AS password, CAST(name AS {text_type}) AS name, status, role, CAST(mail AS {text_type}) AS mail, token_version FROM {table} WHERE mail = ? AND password = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(mail)
        .bind(password)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(UserRecord {
            id: row.try_get("id")?,
            password: text(&row, "password")?,
            name: text(&row, "name")?,
            status: row.try_get("status")?,
            role: row.try_get("role")?,
            mail: text(&row, "mail")?,
            token_version: row.try_get("token_version")?,
        })
    })
    .transpose()
}

pub async fn revoke_user_sessions(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<(), sqlx::Error> {
    let table = match driver {
        Driver::MySql => "`user`",
        Driver::Sqlite | Driver::Postgres => "\"user\"",
    };
    let sql = query(
        driver,
        &format!("UPDATE {table} SET token_version = token_version + 1 WHERE id = ?"),
    );
    sqlx::query(&sql).bind(user_id).execute(pool).await?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::{
        activation_revision, add_oidc_group, admin_user_view, audit_entries, begin_theme_trial,
        bookmark_tree,
        arrange_layout, can_access_space, can_edit_space, can_manage_space,
        clear_space_theme_row, clear_user_space_theme_row, confirm_theme_trial,
        create_item_with_icon, delete_oidc_group,
        copy_space, clear_space, create_group, create_item, create_paired_space, create_user,
        delete_grant,
        delete_group_tree,
        delete_file_row, delete_item, delete_paired_space, delete_user, file_list,
        file_name_by_id,
        group_belongs, group_by_id,
        grant_row, group_has_descendant, group_siblings, groups_for_space, insert_file,
        item_by_id, items_for_space,
        grant_oidc_member, link_oauth_identity, module_config_value, oauth_identity,
        oidc_group_rules, oidc_groups, paired_ids_of, paired_space_ids, remove_space_member,
        rename_paired_space, revoke_user_sessions, save_module_config, save_user_config,
        set_member_role, spaces_owned_by,
        save_space_theme, save_user_space_theme, sort_groups, sort_items, space_list,
        space_members, space_name, space_owner, space_theme_pair, space_theme_revision,
        space_owner_and_type, space_public_config, space_search_config, space_wallpaper_json,
        transfer_space,
        update_group, update_item, update_space_public_config, update_space_search_config,
        update_space_wallpaper, upsert_space_member, user_by_credentials, user_by_id,
        user_space_theme_pair, user_space_theme_revision,
        user_config, user_id_by_mail, user_list, user_mail_exists, user_password_and_name,
        install_package, instance_last_good, instance_pending, latest_revision,
        revision_verified_manifest, save_grant,
        package_id_for_revision, package_removed, remove_theme_package, revision_manifest,
        rollback_theme, save_theme_preference,
        insert_web_wallpaper, save_trusted_policy, select_user_theme, set_default_theme,
        theme_admin_state, trusted_enabled, web_wallpaper, web_wallpaper_exists,
        update_user_admin, update_user_password, update_user_profile, user_theme_preference,
        withdraw_trusted_grants,
    };
    use super::timestamp_bind;
    use crate::db::{Driver, connect, query};
    use sqlx::Row;

    async fn verify_user_repository(driver: Driver, url: &str) {
        let pool = connect(driver, url)
            .await
            .unwrap_or_else(|error| panic!("connect {driver:?} test database: {error}"));
        let table = match driver {
            Driver::MySql => "`user`",
            Driver::Sqlite | Driver::Postgres => "\"user\"",
        };
        sqlx::query(&format!(
            "CREATE TABLE {table} (id BIGINT PRIMARY KEY, password VARCHAR(255) NOT NULL, name VARCHAR(100) NOT NULL, status BIGINT NOT NULL, role BIGINT NOT NULL, mail VARCHAR(255) NOT NULL, token_version BIGINT NOT NULL)"
        ))
        .execute(&pool)
        .await
        .expect("create user fixture");
        let insert = query(
            driver,
            &format!(
                "INSERT INTO {table} (id, password, name, status, role, mail, token_version) VALUES (?, ?, ?, ?, ?, ?, ?)"
            ),
        );
        sqlx::query(&insert)
            .bind(17_i64)
            .bind("hash-value")
            .bind("Test User")
            .bind(1_i64)
            .bind(2_i64)
            .bind("user@example.test")
            .bind(9_i64)
            .execute(&pool)
            .await
            .expect("insert user fixture");

        assert_eq!(
            user_by_id(&pool, driver, 17).await.expect("read full user"),
            Some(super::UserRecord {
                id: 17,
                password: "hash-value".to_string(),
                name: "Test User".to_string(),
                status: 1,
                role: 2,
                mail: "user@example.test".to_string(),
                token_version: 9,
            })
        );
        assert_eq!(
            user_by_credentials(&pool, driver, "user@example.test", "hash-value")
                .await
                .expect("lookup login credentials")
                .expect("matching credentials")
                .id,
            17
        );
        assert!(
            user_by_credentials(&pool, driver, "user@example.test", "wrong-password")
                .await
                .expect("lookup wrong credentials")
                .is_none()
        );
        let claims = crate::auth::Claims {
            user_id: 17,
            token_version: 9,
            sub: "user_access".to_string(),
            exp: 1,
            nbf: 1,
            iat: 1,
        };
        assert_eq!(
            crate::auth::user_from_pool(&pool, driver, &claims)
                .await
                .expect("authenticate user from shared pool")
                .mail,
            "user@example.test"
        );
        let stale_claims = crate::auth::Claims {
            token_version: 8,
            ..claims
        };
        assert_eq!(
            crate::auth::user_from_pool(&pool, driver, &stale_claims).await,
            Err(crate::auth::AuthError::NotLoggedIn)
        );
        revoke_user_sessions(&pool, driver, 17)
            .await
            .expect("revoke user sessions");
        assert_eq!(
            user_by_id(&pool, driver, 17)
                .await
                .expect("read user after logout")
                .expect("user remains")
                .token_version,
            10
        );

        sqlx::query(&format!("DROP TABLE {table}"))
            .execute(&pool)
            .await
            .expect("drop user fixture");

        verify_space_list(&pool, driver).await;
        verify_space_members(&pool, driver).await;
        verify_groups(&pool, driver).await;
        verify_item_list(&pool, driver).await;
        verify_search_config(&pool, driver).await;
        verify_space_public(&pool, driver).await;
        verify_space_wallpaper(&pool, driver).await;
        verify_admin_and_config_reads(&pool, driver).await;
        verify_oauth_identity(&pool, driver).await;
        verify_group_writes(&pool, driver).await;
        verify_item_writes(&pool, driver).await;
        verify_sort_writes(&pool, driver).await;
        verify_member_writes(&pool, driver).await;
        verify_config_writes(&pool, driver).await;
        verify_space_writes(&pool, driver).await;
        verify_space_admin_writes(&pool, driver).await;
        verify_file_writes(&pool, driver).await;
        verify_user_admin_writes(&pool, driver).await;
        verify_theme_writes(&pool, driver).await;
        verify_theme_package_writes(&pool, driver).await;
        verify_wallpaper_writes(&pool, driver).await;
        verify_icon_writes(&pool, driver).await;
        verify_theme_selection_writes(&pool, driver).await;
        pool.close().await;
    }

    async fn verify_space_list(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, type_column, enabled_type) = match driver {
            Driver::MySql => ("`space`", "`type`", "BOOLEAN"),
            Driver::Sqlite => ("\"space\"", "\"type\"", "INTEGER"),
            Driver::Postgres => ("\"space\"", "\"type\"", "BOOLEAN"),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, {type_column} VARCHAR(32) NOT NULL, name VARCHAR(100) NOT NULL, owner_user_id BIGINT NOT NULL, pair_id BIGINT NOT NULL, side VARCHAR(16) NOT NULL, public_enabled {enabled_type} NOT NULL, public_id VARCHAR(80), public_mode VARCHAR(32) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create space fixture");
        sqlx::query("CREATE TABLE space_member (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL)")
            .execute(pool)
            .await
            .expect("create space member fixture");

        let insert_space = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, created_at, updated_at, {type_column}, name, owner_user_id, pair_id, side, public_enabled, public_id, public_mode) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            ),
        );
        for (id, name, owner, pair_id, side, enabled, public_id) in [
            (
                1_i64,
                "Owned",
                7_i64,
                700_i64,
                "yin",
                true,
                Some("pub-owned"),
            ),
            (2, "Editor", 8, 800, "yin", false, None),
            (3, "Viewer", 8, 900, "yin", false, None),
            (4, "Hidden", 99, 1000, "yin", false, None),
            (101, "Paired", 8, 1, "yang", false, None),
        ] {
            let mut statement = sqlx::query(&insert_space)
                .bind(id)
                .bind("2024-01-01 00:00:00")
                .bind("2024-01-02 00:00:00")
                .bind("shared")
                .bind(name)
                .bind(owner)
                .bind(pair_id)
                .bind(side);
            statement = if driver == Driver::Postgres {
                statement.bind(enabled)
            } else {
                statement.bind(i64::from(enabled))
            };
            statement
                .bind(public_id)
                .bind("direct")
                .execute(pool)
                .await
                .expect("insert space fixture");
        }
        let member_insert = query(
            driver,
            "INSERT INTO space_member (id, space_id, user_id, role) VALUES (?, ?, ?, ?)",
        );
        for (id, space_id, role) in [(1_i64, 2_i64, "editor"), (2, 3, "viewer")] {
            sqlx::query(&member_insert)
                .bind(id)
                .bind(space_id)
                .bind(7_i64)
                .bind(role)
                .execute(pool)
                .await
                .expect("insert member fixture");
        }

        let rows = space_list(pool, driver, 7)
            .await
            .expect("load caller's spaces");
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows.iter().map(|space| space.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(rows[0].can_edit);
        assert_eq!(rows[0].paired_space_id, Some(101));
        assert!(rows[0].public_enabled);
        assert_eq!(rows[0].public_id.as_deref(), Some("pub-owned"));
        assert!(rows[1].can_edit);
        assert_eq!(rows[1].paired_space_id, None);
        assert!(!rows[2].can_edit);
        assert_eq!(rows[0].created_at, "2024-01-01 00:00:00");

        sqlx::query("DROP TABLE space_member")
            .execute(pool)
            .await
            .expect("drop space member fixture");
        sqlx::query(&format!("DROP TABLE {space_table}"))
            .execute(pool)
            .await
            .expect("drop space fixture");
    }

    async fn verify_space_members(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, user_table) = match driver {
            Driver::MySql => ("`space`", "`user`"),
            Driver::Sqlite | Driver::Postgres => ("\"space\"", "\"user\""),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id BIGINT PRIMARY KEY, owner_user_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create access space fixture");
        sqlx::query(&format!(
            "CREATE TABLE {user_table} (id BIGINT PRIMARY KEY, mail VARCHAR(255) NOT NULL, name VARCHAR(100) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create member user fixture");
        sqlx::query("CREATE TABLE space_member (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL, source VARCHAR(32) NOT NULL, joined_at VARCHAR(40) NOT NULL)")
            .execute(pool)
            .await
            .expect("create space member fixture");

        let insert_space = query(
            driver,
            &format!("INSERT INTO {space_table} (id, owner_user_id) VALUES (?, ?)"),
        );
        for (id, owner) in [(77_i64, 7_i64), (78, 8)] {
            sqlx::query(&insert_space)
                .bind(id)
                .bind(owner)
                .execute(pool)
                .await
                .expect("insert access space");
        }
        let insert_user = query(
            driver,
            &format!("INSERT INTO {user_table} (id, mail, name) VALUES (?, ?, ?)"),
        );
        for (id, email, name) in [
            (7_i64, "owner@example.test", "Owner"),
            (8, "member@example.test", "Member"),
        ] {
            sqlx::query(&insert_user)
                .bind(id)
                .bind(email)
                .bind(name)
                .execute(pool)
                .await
                .expect("insert member account");
        }
        let insert_member = query(
            driver,
            "INSERT INTO space_member (id, space_id, user_id, role, source, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
        );
        for (id, user_id, role) in [(1_i64, 7_i64, "admin"), (2, 8, "editor"), (3, 99, "viewer")] {
            sqlx::query(&insert_member)
                .bind(id)
                .bind(78_i64)
                .bind(user_id)
                .bind(role)
                .bind("manual")
                .bind("2024-02-03 04:05:06")
                .execute(pool)
                .await
                .expect("insert member row");
        }

        assert!(
            can_access_space(pool, driver, 7, 77)
                .await
                .expect("owner can access")
        );
        assert!(
            can_access_space(pool, driver, 7, 78)
                .await
                .expect("member can access")
        );
        assert!(
            !can_access_space(pool, driver, 9, 78)
                .await
                .expect("outsider cannot access")
        );

        let members = space_members(pool, driver, 78)
            .await
            .expect("load space members");
        assert_eq!(members.len(), 3);
        assert_eq!(
            members.iter().map(|member| member.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(members[0].email, "owner@example.test");
        assert_eq!(members[0].name, "Owner");
        assert_eq!(members[1].role, "editor");
        assert_eq!(members[1].source, "manual");
        assert_eq!(members[1].joined_at, "2024-02-03 04:05:06");
        assert_eq!(members[2].email, "");
        assert_eq!(members[2].name, "");
        assert!(
            space_members(pool, driver, 999)
                .await
                .expect("empty space has no members")
                .is_empty()
        );

        sqlx::query("DROP TABLE space_member")
            .execute(pool)
            .await
            .expect("drop member fixture");
        sqlx::query(&format!("DROP TABLE {user_table}"))
            .execute(pool)
            .await
            .expect("drop member user fixture");
        sqlx::query(&format!("DROP TABLE {space_table}"))
            .execute(pool)
            .await
            .expect("drop access space fixture");
    }

    async fn verify_groups(pool: &sqlx::AnyPool, driver: Driver) {
        sqlx::query("CREATE TABLE item_icon_group (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)")
            .execute(pool)
            .await
            .expect("create group fixture");
        sqlx::query("CREATE TABLE item_icon (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, item_icon_group_id BIGINT)")
            .execute(pool)
            .await
            .expect("create item fixture");

        let insert_group = query(
            driver,
            "INSERT INTO item_icon_group (id, created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        );
        for (id, title, sort, parent_id) in [
            (11_i64, "First", 1_i64, None),
            (12, "Second", 2, Some(11)),
            (13, "Empty", 3, None),
        ] {
            sqlx::query(&insert_group)
                .bind(id)
                .bind("2024-03-01 00:00:00")
                .bind("2024-03-02 00:00:00")
                .bind("folder")
                .bind(title)
                .bind("description")
                .bind(sort)
                .bind(7_i64)
                .bind(78_i64)
                .bind(parent_id)
                .execute(pool)
                .await
                .expect("insert group fixture");
        }
        let insert_item = query(
            driver,
            "INSERT INTO item_icon (id, space_id, item_icon_group_id) VALUES (?, ?, ?)",
        );
        for (id, group_id) in [(1_i64, 11_i64), (2, 11), (3, 12)] {
            sqlx::query(&insert_item)
                .bind(id)
                .bind(78_i64)
                .bind(group_id)
                .execute(pool)
                .await
                .expect("insert grouped item");
        }

        let groups = groups_for_space(pool, driver, 78)
            .await
            .expect("load groups with counts");
        assert_eq!(
            groups.iter().map(|group| group.id).collect::<Vec<_>>(),
            vec![11, 12, 13]
        );
        assert_eq!(
            groups
                .iter()
                .map(|group| group.item_count)
                .collect::<Vec<_>>(),
            vec![2, 1, 0]
        );
        assert_eq!(groups[0].title, "First");
        assert_eq!(groups[0].icon, "folder");
        assert_eq!(groups[0].created_at, "2024-03-01 00:00:00");
        assert_eq!(groups[1].parent_id, Some(11));
        assert!(
            groups_for_space(pool, driver, 999)
                .await
                .expect("unknown space has no groups")
                .is_empty()
        );

        sqlx::query("DROP TABLE item_icon")
            .execute(pool)
            .await
            .expect("drop item fixture to test count fallback");
        let groups_without_counts = groups_for_space(pool, driver, 78)
            .await
            .expect("group rows survive count query failure");
        assert!(
            groups_without_counts
                .iter()
                .all(|group| group.item_count == 0)
        );
        sqlx::query("DROP TABLE item_icon_group")
            .execute(pool)
            .await
            .expect("drop group fixture");
    }

    async fn verify_item_list(pool: &sqlx::AnyPool, driver: Driver) {
        sqlx::query("CREATE TABLE item_icon (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)")
            .execute(pool)
            .await
            .expect("create item list fixture");
        let insert = query(
            driver,
            "INSERT INTO item_icon (id, created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        );
        for (id, created_at, title, sort, group_id) in [
            (21_i64, "2024-05-01 00:00:00", "Second", 2_i64, 31_i64),
            (20, "2024-05-03 00:00:00", "First", 1, 31),
            (22, "2024-05-02 00:00:00", "Third", 3, 32),
        ] {
            sqlx::query(&insert)
                .bind(id)
                .bind(created_at)
                .bind("2024-05-04 00:00:00")
                .bind(r#"{"itemType":2,"src":"favicon.ico","text":"T"}"#)
                .bind(title)
                .bind("https://example.test")
                .bind("http://lan.example.test")
                .bind("")
                .bind("note")
                .bind(2_i64)
                .bind(sort)
                .bind(group_id)
                .bind(7_i64)
                .bind(78_i64)
                .execute(pool)
                .await
                .expect("insert item list fixture");
        }

        let all = items_for_space(pool, driver, 78, None, 0, 0)
            .await
            .expect("load all items");
        assert_eq!(
            all.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![20, 21, 22]
        );
        assert_eq!(all[0].title, "First");
        assert_eq!(
            all[0].icon_json,
            r#"{"itemType":2,"src":"favicon.ico","text":"T"}"#
        );
        assert_eq!(all[0].lan_url, "http://lan.example.test");
        let filtered = items_for_space(pool, driver, 78, Some(31), 0, 0)
            .await
            .expect("load group items");
        assert_eq!(
            filtered.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![20, 21]
        );
        assert!(
            items_for_space(pool, driver, 78, Some(-1), 0, 0)
                .await
                .expect("unknown group items")
                .is_empty()
        );
        let first_page = items_for_space(pool, driver, 78, None, 1, 2)
            .await
            .expect("load first page");
        assert_eq!(
            first_page.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![20, 21]
        );
        let second_page = items_for_space(pool, driver, 78, None, 2, 2)
            .await
            .expect("load second page");
        assert_eq!(
            second_page.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![22]
        );
        assert_eq!(
            items_for_space(pool, driver, 78, None, 1, 0)
                .await
                .expect("default page size")
                .len(),
            3
        );

        sqlx::query("DROP TABLE item_icon")
            .execute(pool)
            .await
            .expect("drop item list fixture");
    }

    async fn verify_theme_selection_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        let bool_type = match driver {
            Driver::MySql => "TINYINT NOT NULL",
            Driver::Sqlite => "INTEGER NOT NULL",
            Driver::Postgres => "BOOLEAN NOT NULL",
        };
        sqlx::query(&format!(
            "CREATE TABLE revision_record_v2 (id VARCHAR(191) PRIMARY KEY, package_id VARCHAR(191) NOT NULL, version VARCHAR(191) NOT NULL, manifest_json TEXT NOT NULL, tokens_json TEXT NOT NULL, verified {bool_type}, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create selection revision fixture");
        sqlx::query(&format!(
            "CREATE TABLE trusted_runtime_policy_v2 (revision_id VARCHAR(191) PRIMARY KEY, enabled {bool_type}, updated_by BIGINT NOT NULL, updated_at VARCHAR(64))"
        ))
        .execute(pool)
        .await
        .expect("create selection trusted fixture");
        sqlx::query("CREATE TABLE grant_record_v2 (user_id BIGINT NOT NULL, revision_id VARCHAR(191) NOT NULL, execution_mode VARCHAR(32) NOT NULL, permissions_json TEXT NOT NULL, updated_at VARCHAR(64), PRIMARY KEY (user_id, revision_id, execution_mode))")
            .execute(pool)
            .await
            .expect("create grant fixture");
        sqlx::query("CREATE TABLE space_theme_preference_v2 (space_id BIGINT PRIMARY KEY, package_id VARCHAR(191) NOT NULL, revision_id VARCHAR(191) NOT NULL, updated_by BIGINT NOT NULL, updated_at VARCHAR(64))")
            .execute(pool)
            .await
            .expect("create space theme fixture");
        sqlx::query("CREATE TABLE user_space_theme_preference_v2 (user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, package_id VARCHAR(191) NOT NULL, revision_id VARCHAR(191) NOT NULL, updated_at VARCHAR(64), PRIMARY KEY (user_id, space_id))")
            .execute(pool)
            .await
            .expect("create user space theme fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create selection item fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create selection group fixture");

        let insert_revision = query(
            driver,
            &format!(
                "INSERT INTO revision_record_v2 (id, package_id, version, manifest_json, tokens_json, verified, created_at) VALUES (?, ?, ?, ?, '{{}}', ?, {})",
                timestamp_bind(driver)
            ),
        );
        let mut statement = sqlx::query(&insert_revision)
            .bind("rev-a")
            .bind("community.a")
            .bind("1.0.0")
            .bind(r#"{"runtime":{"supportedModes":["sandbox","trusted"]}}"#);
        statement = if driver == Driver::Postgres {
            statement.bind(true)
        } else {
            statement.bind(1_i64)
        };
        statement
            .bind("2025-09-01 00:00:00")
            .execute(pool)
            .await
            .expect("insert selection revision");

        save_grant(
            pool,
            driver,
            7,
            "rev-a",
            "sandbox",
            r#"["spaces.read"]"#,
            "2025-09-02 00:00:00",
        )
        .await
        .expect("save grant");
        let grant = grant_row(pool, driver, 7, "rev-a", "sandbox")
            .await
            .expect("read grant")
            .expect("grant exists");
        assert_eq!(grant.permissions_json, r#"["spaces.read"]"#);
        save_grant(
            pool,
            driver,
            7,
            "rev-a",
            "sandbox",
            r#"["items.read"]"#,
            "2025-09-03 00:00:00",
        )
        .await
        .expect("update grant");
        assert_eq!(
            grant_row(pool, driver, 7, "rev-a", "sandbox")
                .await
                .expect("updated grant")
                .expect("grant remains")
                .permissions_json,
            r#"["items.read"]"#
        );
        delete_grant(pool, driver, 7, "rev-a", "sandbox")
            .await
            .expect("delete grant");
        assert_eq!(
            grant_row(pool, driver, 7, "rev-a", "sandbox")
                .await
                .expect("missing grant"),
            None
        );
        assert_eq!(
            revision_verified_manifest(pool, driver, "rev-a")
                .await
                .expect("verified manifest")
                .expect("revision exists")
                .1,
            1
        );

        save_space_theme(pool, driver, 78, "community.a", "rev-a", "2025-09-04 00:00:00")
            .await
            .expect("save space theme");
        assert_eq!(
            space_theme_pair(pool, driver, 78)
                .await
                .expect("read space theme"),
            Some(("community.a".to_string(), "rev-a".to_string()))
        );
        save_user_space_theme(
            pool,
            driver,
            7,
            78,
            "community.a",
            "rev-a",
            "2025-09-05 00:00:00",
        )
        .await
        .expect("save user space theme");
        assert_eq!(
            user_space_theme_pair(pool, driver, 7, 78)
                .await
                .expect("read user space theme"),
            Some(("community.a".to_string(), "rev-a".to_string()))
        );
        clear_user_space_theme_row(pool, driver, 7, 78)
            .await
            .expect("clear user space theme");
        assert_eq!(
            user_space_theme_revision(pool, driver, 7, 78)
                .await
                .expect("cleared user space theme"),
            None
        );
        clear_space_theme_row(pool, driver, 78)
            .await
            .expect("clear space theme");
        assert_eq!(
            space_theme_revision(pool, driver, 78)
                .await
                .expect("cleared space theme"),
            None
        );

        let insert_group = query(
            driver,
            &format!(
                "INSERT INTO item_icon_group (id, created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES (?, {}, {}, 'folder', ?, '', 0, 7, 78, NULL)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        for (id, title) in [(41_i64, "One"), (42, "Two")] {
            sqlx::query(&insert_group)
                .bind(id)
                .bind("2025-09-06 00:00:00")
                .bind("2025-09-06 00:00:00")
                .bind(title)
                .execute(pool)
                .await
                .expect("insert selection group");
        }
        let insert_item = query(
            driver,
            &format!(
                "INSERT INTO item_icon (id, created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES (?, {}, {}, '{{}}', ?, 'https://example.test', '', '', '', 2, 0, 41, 7, 78)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        sqlx::query(&insert_item)
            .bind(51_i64)
            .bind("2025-09-06 00:00:00")
            .bind("2025-09-06 00:00:00")
            .bind("Link")
            .execute(pool)
            .await
            .expect("insert selection item");

        arrange_layout(
            pool,
            driver,
            78,
            &[(51, 42, 5)],
            &[(41, 2), (42, 1)],
            "2025-09-07 00:00:00",
        )
        .await
        .expect("arrange layout");
        let item = item_by_id(pool, driver, 78, 51)
            .await
            .expect("read arranged item")
            .expect("item exists");
        assert_eq!(item.item_icon_group_id, 42);
        assert_eq!(item.sort, 5);
        let groups = groups_for_space(pool, driver, 78)
            .await
            .expect("read arranged groups");
        assert_eq!(
            groups.iter().map(|group| group.id).collect::<Vec<_>>(),
            vec![42, 41]
        );

        for table in [
            "item_icon",
            "item_icon_group",
            "user_space_theme_preference_v2",
            "space_theme_preference_v2",
            "grant_record_v2",
            "trusted_runtime_policy_v2",
            "revision_record_v2",
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop selection fixture");
        }
    }

    async fn verify_icon_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        sqlx::query(&format!(
            "CREATE TABLE space_o_id_c_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, space_id BIGINT NOT NULL, provider VARCHAR(50) NOT NULL, group_name VARCHAR(100) NOT NULL, role VARCHAR(32) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create oidc write fixture");
        sqlx::query(&format!(
            "CREATE TABLE file (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, user_id BIGINT NOT NULL, file_name VARCHAR(191) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create icon file fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create icon item fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create icon group fixture");

        let rule = add_oidc_group(
            pool,
            driver,
            78,
            "github",
            "team-admins",
            "editor",
            "2025-08-01 00:00:00",
        )
        .await
        .expect("add oidc group");
        assert!(rule > 0);
        let rows = oidc_groups(pool, driver, 78)
            .await
            .expect("read oidc group");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, rule);
        delete_oidc_group(pool, driver, 78, rule)
            .await
            .expect("delete oidc group");
        assert!(oidc_groups(pool, driver, 78)
            .await
            .expect("empty oidc groups")
            .is_empty());

        let item = create_item_with_icon(
            pool,
            driver,
            78,
            7,
            r#"{"itemType":2,"fileName":"icon.png"}"#,
            "Link",
            "https://example.test",
            "",
            "",
            "note",
            2,
            3,
            31,
            Some("icon.png"),
            "2025-08-02 00:00:00",
        )
        .await
        .expect("create item with icon");
        assert!(item > 0);
        let read = item_by_id(pool, driver, 78, item)
            .await
            .expect("read item with icon")
            .expect("item exists");
        assert_eq!(read.title, "Link");
        assert_eq!(read.icon_json, r#"{"itemType":2,"fileName":"icon.png"}"#);
        let files: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM file WHERE user_id = ?",
        ))
        .bind(7_i64)
        .fetch_one(pool)
        .await
        .expect("count file rows")
        .try_get("total")
        .expect("total");
        assert_eq!(files, 1, "one row per content hash");
        // A second item reusing the icon must not duplicate the file row.
        let second = create_item_with_icon(
            pool,
            driver,
            78,
            7,
            "{}",
            "Second",
            "https://example.test/two",
            "",
            "",
            "",
            2,
            4,
            31,
            Some("icon.png"),
            "2025-08-03 00:00:00",
        )
        .await
        .expect("create second item with the same icon");
        assert!(second > item);
        let files: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM file WHERE user_id = ?",
        ))
        .bind(7_i64)
        .fetch_one(pool)
        .await
        .expect("count file rows after second item")
        .try_get("total")
        .expect("total");
        assert_eq!(files, 1);

        let mut tx = pool.begin().await.expect("begin import transaction");
        let group = crate::repository::insert_group_in_tx(
            &mut tx,
            driver,
            78,
            7,
            "folder",
            "Bookmarks",
            0,
            None,
            "2025-08-04 00:00:00",
        )
        .await
        .expect("insert imported group");
        assert!(group > 0);
        crate::repository::insert_imported_item_in_tx(
            &mut tx,
            driver,
            "{}",
            "Imported",
            "https://imported.test",
            0,
            group,
            7,
            78,
            "2025-08-04 00:00:00",
        )
        .await
        .expect("insert imported item");
        crate::repository::insert_file_in_tx(
            &mut tx,
            driver,
            7,
            "icon.png",
            "2025-08-04 00:00:00",
        )
        .await
        .expect("insert existing file row is a no-op");
        tx.commit().await.expect("commit import transaction");
        assert_eq!(
            group_by_id(pool, driver, 78, group)
                .await
                .expect("read imported group")
                .expect("group exists")
                .title,
            "Bookmarks"
        );
        let imported = items_for_space(pool, driver, 78, Some(group), 0, 0)
            .await
            .expect("read imported items");
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].title, "Imported");

        for table in ["item_icon_group", "item_icon", "file", "space_o_id_c_group"] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop icon fixture");
        }
    }

    async fn verify_wallpaper_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let blob_type = match driver {
            Driver::MySql => "LONGBLOB",
            Driver::Sqlite => "BLOB",
            Driver::Postgres => "BYTEA",
        };
        sqlx::query(&format!(
            "CREATE TABLE web_wallpaper_record (id VARCHAR(191) PRIMARY KEY, owner_id BIGINT NOT NULL, html {blob_type} NOT NULL, poster {blob_type} NOT NULL, poster_type VARCHAR(32) NOT NULL, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create web wallpaper fixture");

        let html = b"<!doctype html><html><body>wall</body></html>".to_vec();
        let poster = b"\x89PNG\r\n\x1a\n".to_vec();
        assert!(
            !web_wallpaper_exists(pool, driver, "wall-1")
                .await
                .expect("missing wallpaper")
        );
        insert_web_wallpaper(
            pool,
            driver,
            "wall-1",
            7,
            &html,
            &poster,
            "image/png",
            "2025-07-01 00:00:00",
        )
        .await
        .expect("insert web wallpaper");
        assert!(
            web_wallpaper_exists(pool, driver, "wall-1")
                .await
                .expect("wallpaper exists")
        );
        let record = web_wallpaper(pool, driver, "wall-1")
            .await
            .expect("read web wallpaper")
            .expect("wallpaper row");
        assert_eq!(record.html, html);
        assert_eq!(record.poster, poster);
        assert_eq!(record.poster_type, "image/png");
        assert_eq!(
            web_wallpaper(pool, driver, "missing")
                .await
                .expect("missing row"),
            None
        );

        // The id is the content hash, so a second upload is a no-op.
        insert_web_wallpaper(
            pool,
            driver,
            "wall-1",
            7,
            &html,
            &poster,
            "image/png",
            "2025-07-01 00:00:00",
        )
        .await
        .expect_err("duplicate id is rejected by the primary key");

        sqlx::query("DROP TABLE web_wallpaper_record")
            .execute(pool)
            .await
            .expect("drop web wallpaper fixture");
    }

    async fn verify_theme_package_writes(pool: &sqlx::AnyPool, driver: Driver) {
        use std::collections::BTreeMap;

        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        let bool_type = match driver {
            Driver::MySql => "TINYINT NOT NULL",
            Driver::Sqlite => "INTEGER NOT NULL",
            Driver::Postgres => "BOOLEAN NOT NULL",
        };
        sqlx::query(&format!(
            "CREATE TABLE package_record_v2 (id VARCHAR(191) PRIMARY KEY, name VARCHAR(191) NOT NULL, latest_revision VARCHAR(191) NOT NULL, removed {bool_type}, created_at {timestamp_type}, updated_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create package fixture");
        sqlx::query(&format!(
            "CREATE TABLE revision_record_v2 (id VARCHAR(191) PRIMARY KEY, package_id VARCHAR(191) NOT NULL, version VARCHAR(191) NOT NULL, manifest_json TEXT NOT NULL, tokens_json TEXT NOT NULL, verified {bool_type}, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create revision fixture");
        let blob_type = match driver {
            Driver::MySql => "LONGBLOB",
            Driver::Sqlite => "BLOB",
            Driver::Postgres => "BYTEA",
        };
        sqlx::query(&format!(
            "CREATE TABLE asset_record_v2 (revision_id VARCHAR(191) NOT NULL, path VARCHAR(191) NOT NULL, media_type VARCHAR(64) NOT NULL, content {blob_type} NOT NULL, PRIMARY KEY (revision_id, path))"
        ))
            .execute(pool)
            .await
            .expect("create asset fixture");
        sqlx::query(&format!(
            "CREATE TABLE audit_record (id {id_type}, actor_id BIGINT NOT NULL, action VARCHAR(64) NOT NULL, package_id VARCHAR(191) NOT NULL, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create audit fixture");
        sqlx::query("CREATE TABLE activation_record_v2 (scope VARCHAR(191) PRIMARY KEY, package_id VARCHAR(191) NOT NULL, active_revision_id VARCHAR(191) NOT NULL, last_good_revision_id VARCHAR(191), pending_revision_id VARCHAR(191), trial_started_at VARCHAR(64), updated_at VARCHAR(64))")
            .execute(pool)
            .await
            .expect("create activation fixture");

        let yin_manifest: crate::packages::Manifest = serde_json::from_value(serde_json::json!({
            "format": "yin-theme",
            "formatVersion": 2,
            "id": "org.yin.default",
            "name": "Yin",
            "version": "1.0.0",
            "themeApi": "^1.0.0",
            "core": ">=0.4.0",
            "author": "Yin",
            "license": "AGPL-3.0"
        }))
        .expect("default manifest");
        let yin_package = crate::packages::Package {
            manifest: yin_manifest,
            tokens: BTreeMap::new(),
            files: BTreeMap::new(),
            verified: true,
            revision: "rev-default".to_string(),
        };
        install_package(pool, driver, 7, &yin_package)
            .await
            .expect("install default package");

        let test_package = crate::packages::Package {
            manifest: serde_json::from_value(serde_json::json!({
                "format": "yin-theme",
                "formatVersion": 2,
                "id": "community.test",
                "name": "Test",
                "version": "1.0.0",
                "themeApi": "^1.0.0",
                "core": ">=0.4.0",
                "author": "Test",
                "license": "MIT"
            }))
            .expect("test manifest"),
            tokens: BTreeMap::new(),
            files: BTreeMap::from([(
                "views/home.mjs".to_string(),
                ("text/javascript".to_string(), b"export {}".to_vec()),
            )]),
            verified: true,
            revision: "rev-test".to_string(),
        };
        install_package(pool, driver, 7, &test_package)
            .await
            .expect("install test package");
        let state = theme_admin_state(pool, driver).await.expect("state");
        assert!(state.packages.iter().any(|package| package.id == "community.test"));
        assert!(state.revisions.iter().any(|revision| revision.id == "rev-test"));
        let assets: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM asset_record_v2 WHERE revision_id = ?",
        ))
        .bind("rev-test")
        .fetch_one(pool)
        .await
        .expect("count assets")
        .try_get("total")
        .expect("total");
        assert_eq!(assets, 1);
        // Re-installing the same revision is a no-op that Go does not audit.
        install_package(pool, driver, 7, &test_package)
            .await
            .expect("reinstall");
        let audits = audit_entries(pool, driver).await.expect("audit");
        assert_eq!(
            audits
                .iter()
                .filter(|row| row.action == "install-v2" && row.package_id == "community.test")
                .count(),
            1
        );

        set_default_theme(pool, driver, "community.test", "rev-test", "2025-06-01 00:00:00")
            .await
            .expect("set default");
        let state = theme_admin_state(pool, driver).await.expect("state after default");
        assert_eq!(state.active_revision, "rev-test");
        assert_eq!(state.last_good_revision, "rev-test");
        assert_eq!(state.default_package, "community.test");

        begin_theme_trial(pool, driver, "community.test", "rev-test", "2025-06-02 00:00:00")
            .await
            .expect("begin trial twice-safe");
        assert_eq!(
            instance_pending(pool, driver).await.expect("pending"),
            Some("rev-test".to_string())
        );
        confirm_theme_trial(pool, driver, "community.test", "rev-test", "2025-06-03 00:00:00")
            .await
            .expect("confirm trial");
        assert_eq!(
            instance_pending(pool, driver)
                .await
                .expect("pending cleared"),
            Some(String::new())
        );
        assert_eq!(
            instance_last_good(pool, driver).await.expect("last good"),
            Some("rev-test".to_string())
        );
        rollback_theme(pool, driver, "org.yin.default", "rev-default", "2025-06-04 00:00:00")
            .await
            .expect("rollback");
        let state = theme_admin_state(pool, driver).await.expect("state after rollback");
        assert_eq!(state.active_revision, "rev-default");
        assert_eq!(state.default_package, "org.yin.default");

        remove_theme_package(pool, driver, 7, "community.test")
            .await
            .expect("remove package");
        assert_eq!(
            package_removed(pool, driver, "community.test")
                .await
                .expect("removed flag"),
            Some(1)
        );
        let audits = audit_entries(pool, driver).await.expect("audit after remove");
        assert!(audits.iter().any(|row| row.action == "remove-v2"));

        for table in [
            "activation_record_v2",
            "audit_record",
            "asset_record_v2",
            "revision_record_v2",
            "package_record_v2",
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop theme package fixture");
        }
    }

    async fn verify_theme_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        let bool_type = match driver {
            Driver::MySql => "TINYINT NOT NULL",
            Driver::Sqlite => "INTEGER NOT NULL",
            Driver::Postgres => "BOOLEAN NOT NULL",
        };
        sqlx::query(&format!(
            "CREATE TABLE package_record_v2 (id VARCHAR(191) PRIMARY KEY, name VARCHAR(191) NOT NULL, latest_revision VARCHAR(191) NOT NULL, removed {bool_type}, created_at {timestamp_type}, updated_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create theme package fixture");
        sqlx::query(&format!(
            "CREATE TABLE revision_record_v2 (id VARCHAR(191) PRIMARY KEY, package_id VARCHAR(191) NOT NULL, version VARCHAR(191) NOT NULL, manifest_json TEXT NOT NULL, tokens_json TEXT NOT NULL, verified {bool_type}, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create theme revision fixture");
        sqlx::query("CREATE TABLE activation_record_v2 (scope VARCHAR(191) PRIMARY KEY, package_id VARCHAR(191) NOT NULL, active_revision_id VARCHAR(191) NOT NULL, last_good_revision_id VARCHAR(191), pending_revision_id VARCHAR(191), trial_started_at VARCHAR(64), updated_at VARCHAR(64))")
            .execute(pool)
            .await
            .expect("create activation fixture");
        sqlx::query("CREATE TABLE user_theme_preference_v2 (user_id BIGINT PRIMARY KEY, mode VARCHAR(32) NOT NULL, theme_mode VARCHAR(32) NOT NULL, updated_at VARCHAR(64))")
            .execute(pool)
            .await
            .expect("create preference fixture");
        sqlx::query(&format!(
            "CREATE TABLE trusted_runtime_policy_v2 (revision_id VARCHAR(191) PRIMARY KEY, enabled {bool_type}, updated_by BIGINT NOT NULL, updated_at VARCHAR(64))"
        ))
        .execute(pool)
        .await
        .expect("create trusted fixture");
        sqlx::query("CREATE TABLE grant_record_v2 (user_id BIGINT NOT NULL, revision_id VARCHAR(191) NOT NULL, execution_mode VARCHAR(32) NOT NULL, permissions_json TEXT NOT NULL, updated_at VARCHAR(64))")
            .execute(pool)
            .await
            .expect("create grant fixture");
        sqlx::query(&format!(
            "CREATE TABLE audit_record (id {id_type}, actor_id BIGINT NOT NULL, action VARCHAR(64) NOT NULL, package_id VARCHAR(191) NOT NULL, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create audit fixture");

        let insert_package = query(
            driver,
            &format!(
                "INSERT INTO package_record_v2 (id, name, latest_revision, removed, created_at, updated_at) VALUES (?, ?, ?, ?, {}, {})",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        let mut statement = sqlx::query(&insert_package)
            .bind("org.yin.default")
            .bind("Yin")
            .bind("rev-1");
        statement = if driver == Driver::Postgres {
            statement.bind(false)
        } else {
            statement.bind(0_i64)
        };
        statement
            .bind("2025-05-01 00:00:00")
            .bind("2025-05-01 00:00:00")
            .execute(pool)
            .await
            .expect("insert theme package");

        let insert_revision = format!(
            "INSERT INTO revision_record_v2 (id, package_id, version, manifest_json, tokens_json, verified, created_at) VALUES (?, ?, ?, ?, '{{}}', ?, {})",
            timestamp_bind(driver)
        );
        let insert_revision = query(driver, &insert_revision);
        let mut statement = sqlx::query(&insert_revision)
            .bind("rev-1")
            .bind("org.yin.default")
            .bind("1.0.0")
            .bind(r#"{"runtime":{"supportedModes":["trusted","sandbox"]}}"#);
        statement = if driver == Driver::Postgres {
            statement.bind(true)
        } else {
            statement.bind(1_i64)
        };
        statement
            .bind("2025-05-02 00:00:00")
            .execute(pool)
            .await
            .expect("insert theme revision");

        sqlx::query(&query(
            driver,
            &format!(
                "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at) VALUES ('instance', 'org.yin.default', 'rev-1', 'rev-1', '', '2025-05-03 00:00:00', '2025-05-03 00:00:00')"
            ),
        ))
        .execute(pool)
        .await
        .expect("insert activation");

        let state = theme_admin_state(pool, driver)
            .await
            .expect("read admin state");
        assert_eq!(state.active_revision, "rev-1");
        assert_eq!(state.default_package, "org.yin.default");
        assert_eq!(state.last_good_revision, "rev-1");
        assert_eq!(state.packages.len(), 1);
        assert_eq!(state.packages[0].version, "1.0.0");
        assert_eq!(state.packages[0].verified, 1);
        assert_eq!(state.revisions.len(), 1);
        assert_eq!(
            state.trial_started_at.as_deref(),
            Some("2025-05-03 00:00:00")
        );

        assert_eq!(
            user_theme_preference(pool, driver, 7).await.expect("empty"),
            None
        );
        save_theme_preference(
            pool,
            driver,
            7,
            "dark",
            "custom",
            "2025-05-04 00:00:00",
        )
        .await
        .expect("insert preference");
        assert_eq!(
            user_theme_preference(pool, driver, 7)
                .await
                .expect("read preference"),
            Some(("dark".to_string(), "custom".to_string()))
        );
        save_theme_preference(
            pool,
            driver,
            7,
            "light",
            "custom",
            "2025-05-05 00:00:00",
        )
        .await
        .expect("update mode only");
        assert_eq!(
            user_theme_preference(pool, driver, 7)
                .await
                .expect("read updated preference"),
            Some(("light".to_string(), "custom".to_string()))
        );
        assert_eq!(
            latest_revision(pool, driver, "org.yin.default")
                .await
                .expect("latest revision"),
            Some("rev-1".to_string())
        );
        assert_eq!(
            latest_revision(pool, driver, "missing")
                .await
                .expect("missing package"),
            None
        );
        assert_eq!(
            package_id_for_revision(pool, driver, "rev-1")
                .await
                .expect("package of revision"),
            Some("org.yin.default".to_string())
        );

        select_user_theme(
            pool,
            driver,
            7,
            "org.yin.default",
            "rev-1",
            "dark",
            "2025-05-06 00:00:00",
        )
        .await
        .expect("select theme twice-safe");
        select_user_theme(
            pool,
            driver,
            7,
            "org.yin.default",
            "rev-1",
            "light",
            "2025-05-07 00:00:00",
        )
        .await
        .expect("select theme again");
        assert_eq!(
            user_theme_preference(pool, driver, 7)
                .await
                .expect("preference after select"),
            Some(("light".to_string(), "custom".to_string()))
        );
        assert_eq!(
            activation_revision(pool, driver, "instance")
                .await
                .expect("instance activation"),
            Some("rev-1".to_string())
        );

        assert_eq!(
            revision_manifest(pool, driver, "rev-1")
                .await
                .expect("manifest"),
            Some(r#"{"runtime":{"supportedModes":["trusted","sandbox"]}}"#.to_string())
        );
        assert_eq!(
            trusted_enabled(pool, driver, "rev-1")
                .await
                .expect("no policy"),
            None
        );
        save_trusted_policy(pool, driver, "rev-1", true, 7, "2025-05-08 00:00:00")
            .await
            .expect("enable trusted runtime");
        assert_eq!(
            trusted_enabled(pool, driver, "rev-1")
                .await
                .expect("policy enabled"),
            Some(true)
        );
        sqlx::query(&query(
            driver,
            "INSERT INTO grant_record_v2 (user_id, revision_id, execution_mode, permissions_json, updated_at) VALUES (7, 'rev-1', 'trusted', '{}', '2025-05-08 00:00:00')",
        ))
        .execute(pool)
        .await
        .expect("insert trusted grant");
        save_trusted_policy(pool, driver, "rev-1", false, 7, "2025-05-09 00:00:00")
            .await
            .expect("disable trusted runtime");
        assert_eq!(
            trusted_enabled(pool, driver, "rev-1")
                .await
                .expect("policy disabled"),
            Some(false)
        );
        withdraw_trusted_grants(pool, driver, "rev-1")
            .await
            .expect("withdraw grants");
        let grants: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM grant_record_v2",
        ))
        .fetch_one(pool)
        .await
        .expect("count grants")
        .try_get("total")
        .expect("total");
        assert_eq!(grants, 0);

        let audit = audit_entries(pool, driver).await.expect("audit rows");
        assert_eq!(audit.len(), 2);
        assert_eq!(audit[0].action, "select");
        assert_eq!(audit[0].package_id, "org.yin.default");

        for table in [
            "audit_record",
            "grant_record_v2",
            "trusted_runtime_policy_v2",
            "user_theme_preference_v2",
            "activation_record_v2",
            "revision_record_v2",
            "package_record_v2",
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop theme fixture");
        }
    }

    async fn verify_user_admin_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (user_table, space_table, timestamp_type, id_type) = match driver {
            Driver::MySql => ("`user`", "`space`", "DATETIME(6)", "BIGINT AUTO_INCREMENT PRIMARY KEY"),
            Driver::Sqlite => ("\"user\"", "\"space\"", "TEXT", "INTEGER PRIMARY KEY AUTOINCREMENT"),
            Driver::Postgres => ("\"user\"", "\"space\"", "TIMESTAMPTZ", "BIGSERIAL PRIMARY KEY"),
        };
        let enabled_type = match driver {
            Driver::MySql => "TINYINT NOT NULL DEFAULT 0",
            Driver::Sqlite => "INTEGER NOT NULL DEFAULT 0",
            Driver::Postgres => "BOOLEAN NOT NULL DEFAULT false",
        };
        sqlx::query(&format!(
            "CREATE TABLE {user_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, password VARCHAR(255) NOT NULL, name VARCHAR(100) NOT NULL, head_image VARCHAR(255) NOT NULL, status BIGINT NOT NULL, role BIGINT NOT NULL, mail VARCHAR(255) NOT NULL, token_version BIGINT NOT NULL DEFAULT 0)"
        ))
        .execute(pool)
        .await
        .expect("create user admin fixture");
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, type VARCHAR(20) NOT NULL, name VARCHAR(100) NOT NULL, owner_user_id BIGINT NOT NULL, pair_id BIGINT NOT NULL DEFAULT 0, side VARCHAR(16) NOT NULL DEFAULT 'yin', public_enabled {enabled_type}, public_id VARCHAR(80), public_mode VARCHAR(32) NOT NULL DEFAULT 'direct', public_code_hash VARCHAR(128), search_config_json TEXT, wallpaper_json TEXT)"
        ))
        .execute(pool)
        .await
        .expect("create user space fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_member (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL, source VARCHAR(32) NOT NULL, joined_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create user member fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create user group fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create user item fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_o_id_c_group (id {id_type}, space_id BIGINT NOT NULL, provider VARCHAR(50) NOT NULL, group_name VARCHAR(100) NOT NULL, role VARCHAR(32) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create user oidc fixture");
        for table in ["o_auth_identity", "module_config", "file"] {
            sqlx::query(&format!(
                "CREATE TABLE {table} (id {id_type}, user_id BIGINT NOT NULL)"
            ))
            .execute(pool)
            .await
            .expect("create user child fixture");
        }

        let created = create_user(
            pool,
            driver,
            "new@example.test",
            "hash-one",
            "New User",
            "/uploads/head.png",
            2,
            "2025-04-01 00:00:00",
        )
        .await
        .expect("create user");
        assert!(created > 0);
        let view = admin_user_view(pool, driver, created)
            .await
            .expect("read created user")
            .expect("user exists");
        assert_eq!(view.mail, "new@example.test");
        assert_eq!(view.name, "New User");
        assert_eq!(view.status, 1);
        assert_eq!(view.role, 2);
        assert_eq!(view.password, "hash-one");
        assert_eq!(
            user_password_and_name(pool, driver, created)
                .await
                .expect("password and name"),
            Some(("hash-one".to_string(), "New User".to_string()))
        );
        assert!(user_mail_exists(pool, driver, "new@example.test", None)
            .await
            .expect("mail exists"));
        assert!(!user_mail_exists(pool, driver, "new@example.test", Some(created))
            .await
            .expect("own mail excluded"));
        let paired = paired_space_ids(pool, driver, 1)
            .await
            .expect("personal pair lookup");
        assert_eq!(paired.len(), 2, "personal space has a Yang twin");

        update_user_admin(
            pool,
            driver,
            created,
            "renamed@example.test",
            Some("Renamed"),
            Some("/uploads/new.png"),
            Some(2),
            Some(3),
            Some("hash-two"),
            "2025-04-02 00:00:00",
            Some(("Renamed", "Renamed-B")),
        )
        .await
        .expect("update user");
        let view = admin_user_view(pool, driver, created)
            .await
            .expect("read updated user")
            .expect("user remains");
        assert_eq!(view.mail, "renamed@example.test");
        assert_eq!(view.name, "Renamed");
        assert_eq!(view.status, 2);
        assert_eq!(view.role, 3);
        assert_eq!(view.password, "hash-two");
        assert_eq!(
            space_name(pool, driver, paired[0])
                .await
                .expect("renamed yin space"),
            Some("Renamed".to_string())
        );
        assert_eq!(
            space_name(pool, driver, paired[1])
                .await
                .expect("renamed yang space"),
            Some("Renamed-B".to_string())
        );

        update_user_profile(
            pool,
            driver,
            created,
            "Profile",
            "/uploads/profile.png",
            "2025-04-03 00:00:00",
        )
        .await
        .expect("update profile");
        assert_eq!(
            admin_user_view(pool, driver, created)
                .await
                .expect("read profile")
                .expect("user remains")
                .name,
            "Profile"
        );

        assert!(
            update_user_password(
                pool,
                driver,
                created,
                "hash-two",
                "hash-three",
                "2025-04-04 00:00:00",
            )
            .await
            .expect("change password")
        );
        let view = admin_user_view(pool, driver, created)
            .await
            .expect("read password change")
            .expect("user remains");
        assert_eq!(view.password, "hash-three");
        assert!(
            !update_user_password(
                pool,
                driver,
                created,
                "hash-two",
                "hash-four",
                "2025-04-05 00:00:00",
            )
            .await
            .expect("stale password is rejected")
        );

        let owned = spaces_owned_by(pool, driver, created)
            .await
            .expect("owned spaces");
        let required = !owned.iter().any(|(_, kind)| kind == "shared");
        assert!(required, "personal spaces only");
        let mut ids: Vec<i64> = owned.iter().map(|(id, _)| *id).collect();
        ids.extend(
            paired_ids_of(pool, driver, &ids)
                .await
                .expect("paired ids of owned"),
        );
        delete_user(pool, driver, created, &ids)
            .await
            .expect("delete user with spaces");
        assert_eq!(
            admin_user_view(pool, driver, created)
                .await
                .expect("deleted user"),
            None
        );
        let remaining_spaces: i64 = sqlx::query(&query(
            driver,
            &format!("SELECT COUNT(*) AS total FROM {space_table}"),
        ))
        .fetch_one(pool)
        .await
        .expect("count spaces")
        .try_get("total")
        .expect("total");
        assert_eq!(remaining_spaces, 0);

        for table in [
            "file",
            "module_config",
            "o_auth_identity",
            "space_o_id_c_group",
            "item_icon",
            "item_icon_group",
            "space_member",
            space_table,
            user_table,
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop user admin fixture");
        }
    }

    async fn verify_file_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        sqlx::query(&format!(
            "CREATE TABLE file (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, user_id BIGINT NOT NULL, file_name VARCHAR(191) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create file write fixture");

        insert_file(pool, driver, 7, "first.png", "2025-03-01 00:00:00")
            .await
            .expect("insert file");
        insert_file(pool, driver, 7, "second.png", "2025-03-02 00:00:00")
            .await
            .expect("insert second file");
        let (total, rows) = file_list(pool, driver, 0, 1)
            .await
            .expect("list files newest first");
        assert_eq!(total, 2);
        assert_eq!(rows[0].file_name, "second.png");
        let id = rows[0].id;
        assert_eq!(
            file_name_by_id(pool, driver, id)
                .await
                .expect("read file name"),
            Some("second.png".to_string())
        );
        delete_file_row(pool, driver, id)
            .await
            .expect("delete file row");
        assert_eq!(
            file_name_by_id(pool, driver, id).await.expect("deleted row"),
            None
        );
        let (remaining, _) = file_list(pool, driver, 0, 1)
            .await
            .expect("list after delete");
        assert_eq!(remaining, 1);

        sqlx::query("DROP TABLE file")
            .execute(pool)
            .await
            .expect("drop file fixture");
    }

    async fn verify_space_admin_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, user_table, timestamp_type, id_type) = match driver {
            Driver::MySql => ("`space`", "`user`", "DATETIME(6)", "BIGINT AUTO_INCREMENT PRIMARY KEY"),
            Driver::Sqlite => ("\"space\"", "\"user\"", "TEXT", "INTEGER PRIMARY KEY AUTOINCREMENT"),
            Driver::Postgres => ("\"space\"", "\"user\"", "TIMESTAMPTZ", "BIGSERIAL PRIMARY KEY"),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, type VARCHAR(20) NOT NULL, name VARCHAR(100) NOT NULL, owner_user_id BIGINT NOT NULL, pair_id BIGINT NOT NULL DEFAULT 0, side VARCHAR(16) NOT NULL DEFAULT 'yin', public_enabled BIGINT NOT NULL DEFAULT 0, public_id VARCHAR(80), public_mode VARCHAR(32) NOT NULL DEFAULT 'direct', public_code_hash VARCHAR(128), search_config_json TEXT, wallpaper_json TEXT)"
        ))
        .execute(pool)
        .await
        .expect("create admin space fixture");
        sqlx::query(&format!(
            "CREATE TABLE {user_table} (id {id_type}, mail VARCHAR(255) NOT NULL, name VARCHAR(100) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create admin user fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_member (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL, source VARCHAR(32) NOT NULL, joined_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create admin member fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create admin group fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create admin item fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_o_id_c_group (id {id_type}, space_id BIGINT NOT NULL, provider VARCHAR(50) NOT NULL, group_name VARCHAR(100) NOT NULL, role VARCHAR(32) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create admin oidc fixture");
        sqlx::query("CREATE TABLE file (id BIGINT PRIMARY KEY, file_name VARCHAR(191) NOT NULL)")
            .execute(pool)
            .await
            .expect("create admin file fixture");

        let insert_space = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, created_at, updated_at, type, name, owner_user_id, pair_id, side) VALUES (?, {}, {}, ?, ?, ?, ?, ?)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        for (id, name, kind, side) in [
            (11_i64, "Team", "team", "yin"),
            (12, "Team-B", "team", "yang"),
            (13, "Other", "team", "yin"),
        ] {
            sqlx::query(&insert_space)
                .bind(id)
                .bind("2025-02-01 00:00:00")
                .bind("2025-02-01 00:00:00")
                .bind(kind)
                .bind(name)
                .bind(7_i64)
                .bind(11_i64)
                .bind(side)
                .execute(pool)
                .await
                .expect("insert admin space");
        }
        let insert_user = query(
            driver,
            &format!("INSERT INTO {user_table} (id, mail, name) VALUES (?, ?, ?)"),
        );
        for (id, mail) in [(7_i64, "owner@example.test"), (8, "target@example.test")] {
            sqlx::query(&insert_user)
                .bind(id)
                .bind(mail)
                .bind("User")
                .execute(pool)
                .await
                .expect("insert admin user");
        }
        let insert_member = query(
            driver,
            &format!(
                "INSERT INTO space_member (id, created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES (?, {}, {}, ?, ?, ?, 'manual', {})",
                timestamp_bind(driver),
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        for (id, space_id, user_id, role) in [
            (1_i64, 11_i64, 7_i64, "admin"),
            (2, 12, 7, "admin"),
            (3, 13, 7, "admin"),
        ] {
            sqlx::query(&insert_member)
                .bind(id)
                .bind("2025-02-01 00:00:00")
                .bind("2025-02-01 00:00:00")
                .bind(space_id)
                .bind(user_id)
                .bind(role)
                .bind("2025-02-01 00:00:00")
                .execute(pool)
                .await
                .expect("insert admin member");
        }

        assert_eq!(
            space_owner_and_type(pool, driver, 11)
                .await
                .expect("read owner and type"),
            Some((7, "team".to_string()))
        );
        // The fixture inserts explicit ids; PostgreSQL needs the sequence moved
        // forward exactly like the bootstrap does.
        crate::bootstrap::reset_sequences(pool, driver)
            .await
            .expect("reset sequences");

        transfer_space(pool, driver, 11, 7, 8, "2025-02-02 00:00:00")
            .await
            .expect("transfer space");
        assert_eq!(
            space_owner(pool, driver, 11).await.expect("new owner"),
            Some(8)
        );
        let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
        let roles: Vec<String> = sqlx::query(
            &query(
                driver,
                &format!(
                    "SELECT CAST(role AS {text_type}) AS role FROM space_member WHERE space_id = 11 ORDER BY user_id"
                ),
            ),
        )
        .fetch_all(pool)
        .await
        .expect("read roles")
        .into_iter()
        .map(|row| row.try_get::<String, _>("role").expect("role"))
        .collect();
        assert_eq!(roles, vec!["editor".to_string(), "admin".to_string()]);

        let insert_group = query(
            driver,
            &format!(
                "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES ({}, {}, 'folder', 'Group', '', 0, ?, ?, NULL)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        sqlx::query(&insert_group)
            .bind("2025-02-03 00:00:00")
            .bind("2025-02-03 00:00:00")
            .bind(7_i64)
            .bind(13_i64)
            .execute(pool)
            .await
            .expect("insert cleared group");
        clear_space(pool, driver, 13).await.expect("clear space");
        assert!(
            groups_for_space(pool, driver, 13)
                .await
                .expect("cleared space has no groups")
                .is_empty()
        );

        sqlx::query("INSERT INTO file (id, file_name) VALUES (1, 'icon.png')")
            .execute(pool)
            .await
            .expect("insert icon file");
        let insert_item = query(
            driver,
            &format!(
                "INSERT INTO item_icon (created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES ({}, {}, ?, 'Item', 'https://example.test', '', '', '', 0, 0, 0, ?, ?)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        sqlx::query(&insert_item)
            .bind("2025-02-04 00:00:00")
            .bind("2025-02-04 00:00:00")
            .bind(r#"{"fileName":"icon.png"}"#)
            .bind(7_i64)
            .bind(11_i64)
            .execute(pool)
            .await
            .expect("insert icon item");

        let paired = paired_space_ids(pool, driver, 11)
            .await
            .expect("paired ids");
        let removed = delete_paired_space(pool, driver, &paired)
            .await
            .expect("delete paired space");
        // Go checks references before deleting the item rows, so a file named by
        // the Space itself is always still referenced and is never unlinked.
        assert!(removed.is_empty(), "kept for parity: {:?}", removed);
        assert_eq!(
            space_owner_and_type(pool, driver, 11)
                .await
                .expect("deleted space"),
            None
        );
        let files: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM file",
        ))
        .fetch_one(pool)
        .await
        .expect("count files")
        .try_get("total")
        .expect("total");
        assert_eq!(files, 1, "the referenced file row survives, as in Go");

        for table in [
            "file",
            "space_o_id_c_group",
            "item_icon",
            "item_icon_group",
            "space_member",
            user_table,
            space_table,
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop admin fixture");
        }
    }

    async fn verify_space_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, timestamp_type, id_type, enabled_type) = match driver {
            Driver::MySql => (
                "`space`",
                "DATETIME(6)",
                "BIGINT AUTO_INCREMENT PRIMARY KEY",
                "TINYINT NOT NULL DEFAULT 0",
            ),
            Driver::Sqlite => (
                "\"space\"",
                "TEXT",
                "INTEGER PRIMARY KEY AUTOINCREMENT",
                "INTEGER NOT NULL DEFAULT 0",
            ),
            Driver::Postgres => (
                "\"space\"",
                "TIMESTAMPTZ",
                "BIGSERIAL PRIMARY KEY",
                "BOOLEAN NOT NULL DEFAULT false",
            ),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, type VARCHAR(20) NOT NULL, name VARCHAR(100) NOT NULL, owner_user_id BIGINT NOT NULL, pair_id BIGINT NOT NULL DEFAULT 0, side VARCHAR(16) NOT NULL DEFAULT 'yin', public_enabled {enabled_type} NOT NULL, public_id VARCHAR(80), public_mode VARCHAR(32) NOT NULL DEFAULT 'direct', public_code_hash VARCHAR(128), search_config_json TEXT, wallpaper_json TEXT)"
        ))
        .execute(pool)
        .await
        .expect("create space write fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_member (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL, source VARCHAR(32) NOT NULL, joined_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create space member write fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create space group write fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create space item write fixture");

        create_paired_space(
            pool,
            driver,
            7,
            "Created",
            "2025-01-01 00:00:00",
        )
        .await
        .expect("create paired space");
        let created = space_list(pool, driver, 7)
            .await
            .expect("list created spaces");
        let yin = created
            .iter()
            .find(|space| space.kind == "shared" && space.side == "yin")
            .expect("created yin space");
        assert_eq!(yin.name, "Created");
        assert_eq!(yin.pair_id, yin.id);
        assert_eq!(yin.paired_space_id, Some(yin.pair_id + 1));
        assert!(yin.can_edit);
        let paired = paired_space_ids(pool, driver, yin.id)
            .await
            .expect("paired ids");
        assert_eq!(paired, vec![yin.id, yin.id + 1]);
        let yang_id = paired[1];
        assert_eq!(
            space_name(pool, driver, yang_id)
                .await
                .expect("read yang name"),
            Some("Created-B".to_string())
        );
        let member_rows: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM space_member WHERE space_id = ? AND role = 'admin'",
        ))
        .bind(yin.id)
        .fetch_one(pool)
        .await
        .expect("count created members")
        .try_get("total")
        .expect("total");
        assert_eq!(member_rows, 1);

        rename_paired_space(pool, driver, yin.id, "Renamed", "2025-01-02 00:00:00")
            .await
            .expect("rename paired space");
        assert_eq!(
            space_name(pool, driver, yin.id)
                .await
                .expect("read renamed yin"),
            Some("Renamed".to_string())
        );
        assert_eq!(
            space_name(pool, driver, yang_id)
                .await
                .expect("read renamed yang"),
            Some("Renamed-B".to_string())
        );

        let group_sql = query(
            driver,
            &format!(
                "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES ({}, {}, 'folder', 'Links', '', 0, ?, ?, NULL)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        sqlx::query(&group_sql)
            .bind("2025-01-03 00:00:00")
            .bind("2025-01-03 00:00:00")
            .bind(7_i64)
            .bind(yin.id)
            .execute(pool)
            .await
            .expect("insert source group");
        let group_id: i64 = {
            use sqlx::Row;
            sqlx::query(&query(
                driver,
                "SELECT id FROM item_icon_group WHERE space_id = ?",
            ))
            .bind(yin.id)
            .fetch_one(pool)
            .await
            .expect("read source group")
            .try_get("id")
            .expect("group id")
        };
        let item_sql = query(
            driver,
            &format!(
                "INSERT INTO item_icon (created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES ({}, {}, '{{}}', 'Link', 'https://example.test', '', '', '', 0, 1, ?, ?, ?)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        sqlx::query(&item_sql)
            .bind("2025-01-03 00:00:00")
            .bind("2025-01-03 00:00:00")
            .bind(group_id)
            .bind(7_i64)
            .bind(yin.id)
            .execute(pool)
            .await
            .expect("insert source item");

        copy_space(
            pool,
            driver,
            yin.id,
            7,
            "Renamed Copy",
            "2025-01-04 00:00:00",
        )
        .await
        .expect("copy space");
        let after_copy = space_list(pool, driver, 7)
            .await
            .expect("list after copy");
        let copy = after_copy
            .iter()
            .find(|space| space.name == "Renamed Copy")
            .expect("copied space");
        assert_eq!(copy.pair_id, 0, "copy keeps the Go behaviour: no pair");
        let copied_members: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM space_member WHERE space_id = ?",
        ))
        .bind(copy.id)
        .fetch_one(pool)
        .await
        .expect("count copied members")
        .try_get("total")
        .expect("total");
        assert_eq!(copied_members, 1);
        let copied_groups = groups_for_space(pool, driver, copy.id)
            .await
            .expect("copied groups");
        // The created Yin space already had its default APP group.
        assert_eq!(copied_groups.len(), 2);
        assert!(
            copied_groups.iter().any(|group| group.title == "Links"),
            "the source group must be copied"
        );
        let copied_items = items_for_space(pool, driver, copy.id, None, 0, 0)
            .await
            .expect("copied items");
        assert_eq!(copied_items.len(), 1);
        assert_eq!(copied_items[0].title, "Link");
        assert_eq!(copied_items[0].space_id, copy.id);

        for table in ["item_icon", "item_icon_group", "space_member", space_table] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop space write fixture");
        }
    }

    async fn verify_config_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        sqlx::query(&format!(
            "CREATE TABLE module_config (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, user_id BIGINT NOT NULL, name VARCHAR(255) NOT NULL, value_json TEXT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create module config write fixture");
        sqlx::query("CREATE TABLE user_config (user_id BIGINT PRIMARY KEY, panel_json TEXT NOT NULL, language VARCHAR(16) NOT NULL)")
            .execute(pool)
            .await
            .expect("create user config write fixture");

        save_module_config(
            pool,
            driver,
            91,
            "home",
            r#"{"layout":"grid"}"#,
            "2024-12-01 00:00:00",
        )
        .await
        .expect("insert module config");
        assert_eq!(
            module_config_value(pool, driver, 91, "home")
                .await
                .expect("read inserted module config"),
            Some(r#"{"layout":"grid"}"#.to_string())
        );
        save_module_config(
            pool,
            driver,
            91,
            "home",
            r#"{"layout":"list"}"#,
            "2024-12-02 00:00:00",
        )
        .await
        .expect("update module config");
        assert_eq!(
            module_config_value(pool, driver, 91, "home")
                .await
                .expect("read updated module config"),
            Some(r#"{"layout":"list"}"#.to_string())
        );
        let rows: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM module_config WHERE user_id = ?",
        ))
        .bind(91_i64)
        .fetch_one(pool)
        .await
        .expect("count module config rows")
        .try_get("total")
        .expect("total");
        assert_eq!(rows, 1, "the update must not insert a second row");

        save_user_config(
            pool,
            driver,
            91,
            Some(r#"{"theme":"dark"}"#),
            "zh-CN",
        )
        .await
        .expect("insert user config");
        let config = user_config(pool, driver, 91)
            .await
            .expect("read inserted user config")
            .expect("user config exists");
        assert_eq!(config.panel_json, r#"{"theme":"dark"}"#);
        assert_eq!(config.language, "zh-CN");
        save_user_config(pool, driver, 91, None, "en-US")
            .await
            .expect("update language only");
        let config = user_config(pool, driver, 91)
            .await
            .expect("read updated user config")
            .expect("user config remains");
        assert_eq!(config.panel_json, r#"{"theme":"dark"}"#);
        assert_eq!(config.language, "en-US");

        for table in ["user_config", "module_config"] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop config fixture");
        }
    }

    async fn verify_member_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, user_table, timestamp_type, id_type) = match driver {
            Driver::MySql => (
                "`space`",
                "`user`",
                "DATETIME(6)",
                "BIGINT AUTO_INCREMENT PRIMARY KEY",
            ),
            Driver::Sqlite => (
                "\"space\"",
                "\"user\"",
                "TEXT",
                "INTEGER PRIMARY KEY AUTOINCREMENT",
            ),
            Driver::Postgres => (
                "\"space\"",
                "\"user\"",
                "TIMESTAMPTZ",
                "BIGSERIAL PRIMARY KEY",
            ),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id {id_type}, owner_user_id BIGINT NOT NULL, pair_id BIGINT NOT NULL DEFAULT 0, side VARCHAR(16) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create member space fixture");
        sqlx::query(&format!(
            "CREATE TABLE {user_table} (id {id_type}, mail VARCHAR(255) NOT NULL, name VARCHAR(100) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create member user fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_member (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL, source VARCHAR(32) NOT NULL, joined_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create member row fixture");

        let insert_space = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, owner_user_id, pair_id, side) VALUES (?, ?, ?, ?)"
            ),
        );
        for (id, owner, pair_id, side) in [
            (1_i64, 1_i64, 1_i64, "yin"),
            (2, 1, 1, "yang"),
            (3, 9, 3, "yin"),
        ] {
            sqlx::query(&insert_space)
                .bind(id)
                .bind(owner)
                .bind(pair_id)
                .bind(side)
                .execute(pool)
                .await
                .expect("insert member space");
        }
        sqlx::query(&query(
            driver,
            &format!("INSERT INTO {user_table} (id, mail, name) VALUES (?, ?, ?)"),
        ))
        .bind(5_i64)
        .bind("member@example.test")
        .bind("Member")
        .execute(pool)
        .await
        .expect("insert member account");

        assert_eq!(
            paired_space_ids(pool, driver, 1).await.expect("pair"),
            vec![1, 2]
        );
        assert_eq!(
            paired_space_ids(pool, driver, 3).await.expect("unpaired"),
            vec![3]
        );
        assert_eq!(
            user_id_by_mail(pool, driver, "member@example.test")
                .await
                .expect("lookup mail"),
            Some(5)
        );
        assert_eq!(
            user_id_by_mail(pool, driver, "missing@example.test")
                .await
                .expect("missing mail"),
            None
        );
        assert_eq!(
            space_owner(pool, driver, 1).await.expect("owner"),
            Some(1)
        );

        let pair = paired_space_ids(pool, driver, 1).await.expect("pair again");
        upsert_space_member(
            pool,
            driver,
            &pair,
            5,
            "editor",
            "2024-11-01 00:00:00",
        )
        .await
        .expect("add member to both spaces");
        let members = space_members(pool, driver, 2)
            .await
            .expect("read twin members");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].role, "editor");
        assert_eq!(members[0].source, "manual");
        upsert_space_member(
            pool,
            driver,
            &pair,
            5,
            "admin",
            "2024-11-02 00:00:00",
        )
        .await
        .expect("existing member takes the new role");
        let members = space_members(pool, driver, 2)
            .await
            .expect("read updated twin members");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].role, "admin");

        set_member_role(pool, driver, &pair, 5, "viewer")
            .await
            .expect("set role across the pair");
        let members = space_members(pool, driver, 1)
            .await
            .expect("read original members");
        assert_eq!(members[0].role, "viewer");

        remove_space_member(pool, driver, &pair, 5)
            .await
            .expect("remove from both spaces");
        assert!(space_members(pool, driver, 1)
            .await
            .expect("empty original")
            .is_empty());
        assert!(space_members(pool, driver, 2)
            .await
            .expect("empty twin")
            .is_empty());

        for table in ["space_member", user_table, space_table] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop member fixture");
        }
    }

    async fn verify_sort_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (group_table, item_table, timestamp_type, id_type) = match driver {
            Driver::MySql => (
                "item_icon_group",
                "item_icon",
                "DATETIME(6)",
                "BIGINT AUTO_INCREMENT PRIMARY KEY",
            ),
            Driver::Sqlite => (
                "item_icon_group",
                "item_icon",
                "TEXT",
                "INTEGER PRIMARY KEY AUTOINCREMENT",
            ),
            Driver::Postgres => (
                "item_icon_group",
                "item_icon",
                "TIMESTAMPTZ",
                "BIGSERIAL PRIMARY KEY",
            ),
        };
        sqlx::query(&format!(
            "CREATE TABLE {group_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create sort group fixture");
        sqlx::query(&format!(
            "CREATE TABLE {item_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create sort item fixture");

        let first = create_group(pool, driver, 78, 7, "folder", "First", None, "2024-10-01 00:00:00")
            .await
            .expect("create first group");
        let second = create_group(pool, driver, 78, 7, "folder", "Second", None, "2024-10-01 00:00:00")
            .await
            .expect("create second group");
        let siblings = group_siblings(pool, driver, 78, None)
            .await
            .expect("load root siblings");
        assert_eq!(siblings, vec![first, second]);

        sort_groups(
            pool,
            driver,
            78,
            &[(first, 2), (second, 1)],
            "2024-10-02 00:00:00",
        )
        .await
        .expect("sort groups");
        let sorted = groups_for_space(pool, driver, 78)
            .await
            .expect("read sorted groups");
        assert_eq!(
            sorted.iter().map(|group| group.id).collect::<Vec<_>>(),
            vec![second, first]
        );

        let item = create_item(
            pool,
            driver,
            78,
            7,
            &super::ItemWriteRecord {
                icon_json: "{}".to_string(),
                title: "Item".to_string(),
                url: "https://example.test".to_string(),
                lan_url: String::new(),
                mobile_url: String::new(),
                description: String::new(),
                open_method: 0,
                sort: 5,
                group_id: first,
            },
            "2024-10-03 00:00:00",
        )
        .await
        .expect("create item for sort");
        let other = create_item(
            pool,
            driver,
            78,
            7,
            &super::ItemWriteRecord {
                icon_json: "{}".to_string(),
                title: "Other".to_string(),
                url: "https://example.test".to_string(),
                lan_url: String::new(),
                mobile_url: String::new(),
                description: String::new(),
                open_method: 0,
                sort: 1,
                group_id: first,
            },
            "2024-10-03 00:00:00",
        )
        .await
        .expect("create second item");
        sort_items(
            pool,
            driver,
            78,
            first,
            &[(item, 1), (other, 2)],
            "2024-10-04 00:00:00",
        )
        .await
        .expect("sort items");
        let items = items_for_space(pool, driver, 78, Some(first), 0, 0)
            .await
            .expect("read sorted items");
        assert_eq!(
            items.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![item, other]
        );

        for table in [item_table, group_table] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop sort fixture");
        }
    }

    async fn verify_item_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (group_table, item_table, timestamp_type, id_type) = match driver {
            Driver::MySql => (
                "item_icon_group",
                "item_icon",
                "DATETIME(6)",
                "BIGINT AUTO_INCREMENT PRIMARY KEY",
            ),
            Driver::Sqlite => (
                "item_icon_group",
                "item_icon",
                "TEXT",
                "INTEGER PRIMARY KEY AUTOINCREMENT",
            ),
            Driver::Postgres => (
                "item_icon_group",
                "item_icon",
                "TIMESTAMPTZ",
                "BIGSERIAL PRIMARY KEY",
            ),
        };
        sqlx::query(&format!(
            "CREATE TABLE {group_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create item write group fixture");
        sqlx::query(&format!(
            "CREATE TABLE {item_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create item write fixture");

        let group = create_group(
            pool,
            driver,
            78,
            7,
            "folder",
            "Links",
            None,
            "2024-09-01 00:00:00",
        )
        .await
        .expect("create item group");
        let record = super::ItemWriteRecord {
            icon_json: r#"{"itemType":2,"src":"favicon.ico"}"#.to_string(),
            title: "Link".to_string(),
            url: "https://example.test".to_string(),
            lan_url: "http://lan.example.test".to_string(),
            mobile_url: String::new(),
            description: "note".to_string(),
            open_method: 2,
            sort: 3,
            group_id: group,
        };
        let item = create_item(pool, driver, 78, 7, &record, "2024-09-02 00:00:00")
            .await
            .expect("create item");
        assert!(item > 0);
        let row = item_by_id(pool, driver, 78, item)
            .await
            .expect("read created item")
            .expect("item exists");
        assert_eq!(row.title, "Link");
        assert_eq!(row.icon_json, record.icon_json);
        assert_eq!(row.lan_url, "http://lan.example.test");
        assert_eq!(row.open_method, 2);
        assert_eq!(row.sort, 3);
        assert_eq!(row.user_id, 7);
        assert_eq!(
            item_by_id(pool, driver, 78, item + 1)
                .await
                .expect("missing item"),
            None
        );

        let updated = super::ItemWriteRecord {
            title: "Renamed".to_string(),
            sort: 9,
            ..record.clone()
        };
        update_item(pool, driver, 78, item, &updated, "2024-09-03 00:00:00")
            .await
            .expect("update item");
        let row = item_by_id(pool, driver, 78, item)
            .await
            .expect("read updated item")
            .expect("item remains");
        assert_eq!(row.title, "Renamed");
        assert_eq!(row.sort, 9);

        delete_item(pool, driver, 78, item)
            .await
            .expect("delete item");
        assert_eq!(
            item_by_id(pool, driver, 78, item).await.expect("item gone"),
            None
        );

        sqlx::query(&format!("DROP TABLE {item_table}"))
            .execute(pool)
            .await
            .expect("drop item write fixture");
        sqlx::query(&format!("DROP TABLE {group_table}"))
            .execute(pool)
            .await
            .expect("drop item group fixture");
    }

    async fn verify_group_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (group_table, item_table, timestamp_type, id_type) = match driver {
            Driver::MySql => (
                "item_icon_group",
                "item_icon",
                "DATETIME(6)",
                "BIGINT AUTO_INCREMENT PRIMARY KEY",
            ),
            Driver::Sqlite => (
                "item_icon_group",
                "item_icon",
                "TEXT",
                "INTEGER PRIMARY KEY AUTOINCREMENT",
            ),
            Driver::Postgres => (
                "item_icon_group",
                "item_icon",
                "TIMESTAMPTZ",
                "BIGSERIAL PRIMARY KEY",
            ),
        };
        sqlx::query(&format!(
            "CREATE TABLE {group_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create group write fixture");
        sqlx::query(&format!(
            "CREATE TABLE {item_table} (id {id_type}, space_id BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create group item fixture");

        let root = create_group(
            pool,
            driver,
            78,
            7,
            "folder",
            "Root",
            None,
            "2024-08-01 00:00:00",
        )
        .await
        .expect("create root group");
        assert!(root > 0);
        let root_row = group_by_id(pool, driver, 78, root)
            .await
            .expect("read root group")
            .expect("root group exists");
        assert_eq!(root_row.title, "Root");
        assert_eq!(root_row.icon, "folder");
        assert_eq!(root_row.sort, 9999);
        assert_eq!(root_row.item_count, 0);
        assert_eq!(
            group_by_id(pool, driver, 78, root + 1)
                .await
                .expect("missing group"),
            None
        );

        let child = create_group(
            pool,
            driver,
            78,
            7,
            "file",
            "Child",
            Some(root),
            "2024-08-02 00:00:00",
        )
        .await
        .expect("create child group");
        assert!(child > root);
        assert!(
            group_belongs(pool, driver, 78, root)
                .await
                .expect("belongs in space")
        );
        assert!(
            !group_belongs(pool, driver, 78, root + 100)
                .await
                .expect("unknown group")
        );
        assert!(
            group_has_descendant(pool, driver, 78, root, child)
                .await
                .expect("cycle detected")
        );
        assert!(
            !group_has_descendant(pool, driver, 78, child, root)
                .await
                .expect("no cycle upwards")
        );

        update_group(
            pool,
            driver,
            78,
            child,
            "Renamed",
            "icon",
            Some(root),
            "2024-08-03 00:00:00",
        )
        .await
        .expect("rename child group");
        assert_eq!(
            group_by_id(pool, driver, 78, child)
                .await
                .expect("read renamed child")
                .expect("child exists")
                .title,
            "Renamed"
        );

        for id in [1_i64, 2] {
            sqlx::query(&query(
                driver,
                &format!(
                    "INSERT INTO {item_table} (id, space_id, item_icon_group_id) VALUES (?, ?, ?)"
                ),
            ))
            .bind(id)
            .bind(78_i64)
            .bind(root)
            .execute(pool)
            .await
            .expect("insert grouped item");
        }
        delete_group_tree(pool, driver, 78, root)
            .await
            .expect("delete group tree");
        assert_eq!(
            group_by_id(pool, driver, 78, root)
                .await
                .expect("root gone"),
            None
        );
        assert_eq!(
            group_by_id(pool, driver, 78, child)
                .await
                .expect("child gone"),
            None
        );
        let remaining: i64 = sqlx::query(&query(
            driver,
            &format!("SELECT COUNT(*) AS total FROM {item_table} WHERE space_id = ?"),
        ))
        .bind(78_i64)
        .fetch_one(pool)
        .await
        .expect("count remaining items")
        .try_get("total")
        .expect("total");
        assert_eq!(remaining, 0);

        sqlx::query(&format!("DROP TABLE {item_table}"))
            .execute(pool)
            .await
            .expect("drop group item fixture");
        sqlx::query(&format!("DROP TABLE {group_table}"))
            .execute(pool)
            .await
            .expect("drop group write fixture");
    }

    async fn verify_admin_and_config_reads(pool: &sqlx::AnyPool, driver: Driver) {
        let (user_table, text_type) = if driver == Driver::MySql {
            ("`user`", "CHAR")
        } else {
            ("\"user\"", "TEXT")
        };
        let mysql_unused = text_type;
        let _ = mysql_unused;
        sqlx::query("CREATE TABLE module_config (id BIGINT PRIMARY KEY, user_id BIGINT NOT NULL, name VARCHAR(64) NOT NULL, value_json TEXT NOT NULL)")
            .execute(pool)
            .await
            .expect("create module config fixture");
        sqlx::query("CREATE TABLE file (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, file_name TEXT NOT NULL)")
            .execute(pool)
            .await
            .expect("create file fixture");
        sqlx::query(&format!(
            "CREATE TABLE {user_table} (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, name VARCHAR(100) NOT NULL, head_image TEXT NOT NULL, status BIGINT NOT NULL, role BIGINT NOT NULL, mail VARCHAR(255) NOT NULL, token_version BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create managed user fixture");
        sqlx::query("CREATE TABLE user_config (user_id BIGINT PRIMARY KEY, panel_json TEXT NOT NULL, language TEXT NOT NULL)")
            .execute(pool)
            .await
            .expect("create user config fixture");
        sqlx::query("CREATE TABLE item_icon_group (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, sort BIGINT NOT NULL, title VARCHAR(100) NOT NULL, icon VARCHAR(100) NOT NULL, parent_id BIGINT)")
            .execute(pool)
            .await
            .expect("create bookmark group fixture");
        sqlx::query("CREATE TABLE item_icon (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, title VARCHAR(100) NOT NULL, url TEXT NOT NULL)")
            .execute(pool)
            .await
            .expect("create bookmark item fixture");
        sqlx::query("CREATE TABLE space_o_id_c_group (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, space_id BIGINT NOT NULL, provider VARCHAR(64) NOT NULL, group_name VARCHAR(100) NOT NULL, role VARCHAR(32) NOT NULL)")
            .execute(pool)
            .await
            .expect("create oidc group fixture");

        let insert_module = query(
            driver,
            "INSERT INTO module_config (id, user_id, name, value_json) VALUES (?, ?, ?, ?)",
        );
        for (id, name, value) in [
            (1_i64, "home", r#"{"layout":"grid"}"#),
            (2, "other", r#"{"layout":"list"}"#),
        ] {
            sqlx::query(&insert_module)
                .bind(id)
                .bind(7_i64)
                .bind(name)
                .bind(value)
                .execute(pool)
                .await
                .expect("insert module config");
        }
        let insert_file = query(
            driver,
            "INSERT INTO file (id, created_at, updated_at, file_name) VALUES (?, ?, ?, ?)",
        );
        for (id, created_at, name) in [
            (1_i64, "2024-01-01 00:00:00", "older.png"),
            (2, "2024-01-03 00:00:00", "newer.png"),
        ] {
            sqlx::query(&insert_file)
                .bind(id)
                .bind(created_at)
                .bind("2024-01-04 00:00:00")
                .bind(name)
                .execute(pool)
                .await
                .expect("insert file");
        }
        let insert_user = query(
            driver,
            &format!(
                "INSERT INTO {user_table} (id, created_at, updated_at, name, head_image, status, role, mail, token_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
            ),
        );
        for (id, name, role) in [(7_i64, "Owner", 2_i64), (8, "Admin", 1)] {
            sqlx::query(&insert_user)
                .bind(id)
                .bind("2024-01-01 00:00:00")
                .bind("2024-01-02 00:00:00")
                .bind(name)
                .bind("/uploads/head.png")
                .bind(1_i64)
                .bind(role)
                .bind("user@example.test")
                .bind(3_i64)
                .execute(pool)
                .await
                .expect("insert managed user");
        }
        sqlx::query(&query(
            driver,
            "INSERT INTO user_config (user_id, panel_json, language) VALUES (?, ?, ?)",
        ))
        .bind(7_i64)
        .bind(r#"{"theme":"dark"}"#)
        .bind("zh-CN")
        .execute(pool)
        .await
        .expect("insert user config");
        let insert_group = query(
            driver,
            "INSERT INTO item_icon_group (id, space_id, sort, title, icon, parent_id) VALUES (?, ?, ?, ?, ?, ?)",
        );
        for (id, sort, title, parent) in
            [(1_i64, 1_i64, "Root", None), (2, 2, "Child", Some(1_i64))]
        {
            sqlx::query(&insert_group)
                .bind(id)
                .bind(78_i64)
                .bind(sort)
                .bind(title)
                .bind("folder")
                .bind(parent)
                .execute(pool)
                .await
                .expect("insert bookmark group");
        }
        for (id, group_id, title) in [(1_i64, 1_i64, "Root link"), (2, 2, "Child link")] {
            sqlx::query(&query(
                driver,
                "INSERT INTO item_icon (id, space_id, sort, item_icon_group_id, title, url) VALUES (?, ?, ?, ?, ?, ?)",
            ))
            .bind(id)
            .bind(78_i64)
            .bind(id)
            .bind(group_id)
            .bind(title)
            .bind("https://example.test")
            .execute(pool)
            .await
            .expect("insert bookmark item");
        }
        sqlx::query(&query(
            driver,
            "INSERT INTO space_o_id_c_group (id, created_at, updated_at, space_id, provider, group_name, role) VALUES (?, ?, ?, ?, ?, ?, ?)",
        ))
        .bind(1_i64)
        .bind("2024-05-01 00:00:00")
        .bind("2024-05-02 00:00:00")
        .bind(78_i64)
        .bind("github")
        .bind("team-admins")
        .bind("editor")
        .execute(pool)
        .await
        .expect("insert oidc group");

        assert_eq!(
            module_config_value(pool, driver, 7, "home")
                .await
                .expect("read module config"),
            Some(r#"{"layout":"grid"}"#.to_string())
        );
        assert_eq!(
            module_config_value(pool, driver, 7, "missing")
                .await
                .expect("missing module config"),
            None
        );
        let (file_total, files) = file_list(pool, driver, 1, 1)
            .await
            .expect("first file page");
        assert_eq!(file_total, 2);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name, "newer.png");
        let (_, all_files) = file_list(pool, driver, 0, 1).await.expect("unpaged files");
        assert_eq!(all_files.len(), 2);
        let (user_total, users) = user_list(pool, driver, 2, 1)
            .await
            .expect("first user page");
        assert_eq!(user_total, 2);
        assert_eq!(
            users.iter().map(|user| user.id).collect::<Vec<_>>(),
            vec![7, 8]
        );
        assert_eq!(users[1].mail, "user@example.test");
        assert_eq!(users[0].head_image, "/uploads/head.png");
        let (_, unpaged_users) = user_list(pool, driver, 0, 0)
            .await
            .expect("users with no limit");
        assert!(unpaged_users.is_empty());
        let config = user_config(pool, driver, 7)
            .await
            .expect("read user config")
            .expect("config exists");
        assert_eq!(config.language, "zh-CN");
        assert_eq!(config.panel_json, r#"{"theme":"dark"}"#);
        assert_eq!(
            user_config(pool, driver, 8)
                .await
                .expect("missing user config"),
            None
        );
        let (groups, items) = bookmark_tree(pool, driver, 78)
            .await
            .expect("read bookmark tree");
        assert_eq!(
            groups.iter().map(|group| group.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(groups[1].parent_id, Some(1));
        assert_eq!(items[1].title, "Child link");
        assert_eq!(items[0].url, "https://example.test");
        let oidc = oidc_groups(pool, driver, 78)
            .await
            .expect("read oidc groups");
        assert_eq!(oidc.len(), 1);
        assert_eq!(oidc[0].provider, "github");
        assert_eq!(oidc[0].group_name, "team-admins");
        assert_eq!(oidc[0].role, "editor");
        assert!(
            oidc_groups(pool, driver, 999)
                .await
                .expect("empty oidc groups")
                .is_empty()
        );

        for table in [
            "space_o_id_c_group",
            "item_icon",
            "item_icon_group",
            "user_config",
            user_table,
            "file",
            "module_config",
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop misc read fixture");
        }
    }

    async fn verify_space_wallpaper(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, timestamp_type) = match driver {
            Driver::MySql => ("`space`", "DATETIME(6)"),
            Driver::Sqlite => ("\"space\"", "TEXT"),
            Driver::Postgres => ("\"space\"", "TIMESTAMPTZ"),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id BIGINT PRIMARY KEY, wallpaper_json TEXT NOT NULL, updated_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create wallpaper space fixture");
        let insert = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, wallpaper_json, updated_at) VALUES (?, ?, {})",
                if driver == Driver::Postgres {
                    "CAST(? AS TIMESTAMPTZ)"
                } else {
                    "?"
                }
            ),
        );
        for (id, stored) in [
            (80_i64, ""),
            (
                81,
                r#"{"mode":"library","libraryEntry":"pack:theme/orbit"}"#,
            ),
        ] {
            sqlx::query(&insert)
                .bind(id)
                .bind(stored)
                .bind("2024-06-01 00:00:00")
                .execute(pool)
                .await
                .expect("insert wallpaper space");
        }

        assert_eq!(
            space_wallpaper_json(pool, driver, 80)
                .await
                .expect("read empty wallpaper"),
            Some(String::new())
        );
        assert_eq!(
            space_wallpaper_json(pool, driver, 81)
                .await
                .expect("read wallpaper"),
            Some(r#"{"mode":"library","libraryEntry":"pack:theme/orbit"}"#.to_string())
        );
        assert_eq!(
            space_wallpaper_json(pool, driver, 999)
                .await
                .expect("missing wallpaper space"),
            None
        );

        update_space_wallpaper(
            pool,
            driver,
            80,
            r#"{"mode":"custom","source":"/uploads/custom.jpg"}"#,
            "2024-07-02 03:04:05.123456789+00:00",
        )
        .await
        .expect("write wallpaper and timestamp");
        assert_eq!(
            space_wallpaper_json(pool, driver, 80)
                .await
                .expect("read written wallpaper")
                .expect("space remains"),
            r#"{"mode":"custom","source":"/uploads/custom.jpg"}"#
        );
        update_space_wallpaper(pool, driver, 80, "", "2024-07-03 00:00:00+00:00")
            .await
            .expect("clear wallpaper for mode none");
        assert_eq!(
            space_wallpaper_json(pool, driver, 80)
                .await
                .expect("read cleared wallpaper"),
            Some(String::new())
        );

        sqlx::query(&format!("DROP TABLE {space_table}"))
            .execute(pool)
            .await
            .expect("drop wallpaper space fixture");
    }

    async fn verify_space_public(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, enabled_type) = match driver {
            Driver::MySql => ("`space`", "BOOLEAN"),
            Driver::Sqlite => ("\"space\"", "INTEGER"),
            Driver::Postgres => ("\"space\"", "BOOLEAN"),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id BIGINT PRIMARY KEY, owner_user_id BIGINT NOT NULL, public_enabled {enabled_type} NOT NULL, public_id VARCHAR(64), public_mode VARCHAR(20) NOT NULL, public_code_hash VARCHAR(128))"
        ))
        .execute(pool)
        .await
        .expect("create public config space fixture");
        sqlx::query("CREATE TABLE space_member (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL)")
            .execute(pool)
            .await
            .expect("create public config member fixture");
        let insert_space = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, owner_user_id, public_enabled, public_id, public_mode, public_code_hash) VALUES (?, ?, ?, ?, ?, ?)"
            ),
        );
        for (id, owner, enabled, public_id, mode) in [
            (90_i64, 7_i64, true, Some("mine"), "code"),
            (91, 8, false, None, "direct"),
            (92, 8, false, None, "direct"),
            (93, 8, false, None, "direct"),
        ] {
            let mut statement = sqlx::query(&insert_space).bind(id).bind(owner);
            statement = if driver == Driver::Postgres {
                statement.bind(enabled)
            } else {
                statement.bind(i64::from(enabled))
            };
            statement
                .bind(public_id)
                .bind(mode)
                .bind(Some("hash"))
                .execute(pool)
                .await
                .expect("insert public config space");
        }
        let insert_member = query(
            driver,
            "INSERT INTO space_member (id, space_id, user_id, role) VALUES (?, ?, ?, ?)",
        );
        for (id, space_id, role) in [
            (1_i64, 91_i64, "admin"),
            (2, 92, "editor"),
            (3, 93, "viewer"),
        ] {
            sqlx::query(&insert_member)
                .bind(id)
                .bind(space_id)
                .bind(7_i64)
                .bind(role)
                .execute(pool)
                .await
                .expect("insert public config member");
        }

        assert!(
            can_manage_space(pool, driver, 7, 90)
                .await
                .expect("owner manages")
        );
        assert!(
            can_manage_space(pool, driver, 7, 91)
                .await
                .expect("admin manages")
        );
        assert!(
            !can_manage_space(pool, driver, 7, 92)
                .await
                .expect("editor cannot manage")
        );
        assert!(
            !can_manage_space(pool, driver, 7, 93)
                .await
                .expect("viewer cannot manage")
        );
        assert!(
            !can_manage_space(pool, driver, 9, 90)
                .await
                .expect("outsider cannot manage")
        );
        let config = space_public_config(pool, driver, 90)
            .await
            .expect("read public config")
            .expect("public config exists");
        assert!(config.enabled);
        assert_eq!(config.public_id.as_deref(), Some("mine"));
        assert_eq!(config.mode, "code");
        assert_eq!(
            space_public_config(pool, driver, 999)
                .await
                .expect("missing public config"),
            None
        );

        update_space_public_config(
            pool,
            driver,
            91,
            true,
            Some("published"),
            "code",
            Some("new-hash"),
        )
        .await
        .expect("enable public access");
        let enabled = space_public_config(pool, driver, 91)
            .await
            .expect("read enabled config")
            .expect("space remains");
        assert!(enabled.enabled);
        assert_eq!(enabled.public_id.as_deref(), Some("published"));
        update_space_public_config(pool, driver, 91, false, None, "direct", Some(""))
            .await
            .expect("disable public access");
        let disabled = space_public_config(pool, driver, 91)
            .await
            .expect("read disabled config")
            .expect("space remains");
        assert!(!disabled.enabled);
        assert_eq!(disabled.public_id, None);
        assert_eq!(disabled.mode, "direct");

        sqlx::query("DROP TABLE space_member")
            .execute(pool)
            .await
            .expect("drop public config member fixture");
        sqlx::query(&format!("DROP TABLE {space_table}"))
            .execute(pool)
            .await
            .expect("drop public config space fixture");
    }

    async fn verify_search_config(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, timestamp_type) = match driver {
            Driver::MySql => ("`space`", "DATETIME(6)"),
            Driver::Sqlite => ("\"space\"", "TEXT"),
            Driver::Postgres => ("\"space\"", "TIMESTAMPTZ"),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id BIGINT PRIMARY KEY, owner_user_id BIGINT NOT NULL, search_config_json TEXT NOT NULL, updated_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create search config space fixture");
        sqlx::query("CREATE TABLE space_member (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL)")
            .execute(pool)
            .await
            .expect("create search config membership fixture");
        let insert_space = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, owner_user_id, search_config_json, updated_at) VALUES (?, ?, ?, {})",
                if driver == Driver::Postgres {
                    "CAST(? AS TIMESTAMPTZ)"
                } else {
                    "?"
                }
            ),
        );
        for (id, owner, config) in [
            (10_i64, 7_i64, ""),
            (
                11,
                8,
                r#"{"currentSearchEngine":{"url":"https://old.test"}}"#,
            ),
            (12, 8, ""),
            (13, 8, ""),
        ] {
            sqlx::query(&insert_space)
                .bind(id)
                .bind(owner)
                .bind(config)
                .bind("2024-01-01 00:00:00")
                .execute(pool)
                .await
                .expect("insert search config space");
        }
        let member_insert = query(
            driver,
            "INSERT INTO space_member (id, space_id, user_id, role) VALUES (?, ?, ?, ?)",
        );
        for (id, space_id, role) in [(1_i64, 11_i64, "editor"), (2, 12, "viewer")] {
            sqlx::query(&member_insert)
                .bind(id)
                .bind(space_id)
                .bind(7_i64)
                .bind(role)
                .execute(pool)
                .await
                .expect("insert search config member");
        }

        let owned = space_search_config(pool, driver, 7, 10)
            .await
            .expect("read owned config")
            .expect("owned space exists");
        assert_eq!(owned.owner_user_id, 7);
        assert!(!owned.is_member);
        assert!(owned.config_json.is_empty());
        let member = space_search_config(pool, driver, 7, 11)
            .await
            .expect("read member config")
            .expect("member space exists");
        assert!(member.is_member);
        assert!(
            can_edit_space(pool, driver, 7, 10)
                .await
                .expect("owner can edit")
        );
        assert!(
            can_edit_space(pool, driver, 7, 11)
                .await
                .expect("editor can edit")
        );
        assert!(
            !can_edit_space(pool, driver, 7, 12)
                .await
                .expect("viewer cannot edit")
        );
        assert!(
            !can_edit_space(pool, driver, 7, 13)
                .await
                .expect("outsider cannot edit")
        );
        assert_eq!(
            space_search_config(pool, driver, 7, 999)
                .await
                .expect("missing space lookup"),
            None
        );

        let updated_config = r#"{"currentSearchEngine":{"url":"https://new.test"}}"#;
        update_space_search_config(
            pool,
            driver,
            11,
            updated_config,
            "2024-04-05 06:07:08.123456789+00:00",
        )
        .await
        .expect("write search config and timestamp");
        assert_eq!(
            space_search_config(pool, driver, 7, 11)
                .await
                .expect("read updated config")
                .expect("space remains")
                .config_json,
            updated_config
        );

        sqlx::query("DROP TABLE space_member")
            .execute(pool)
            .await
            .expect("drop search config member fixture");
        sqlx::query(&format!("DROP TABLE {space_table}"))
            .execute(pool)
            .await
            .expect("drop search config fixture");
    }

    #[tokio::test]
    async fn shared_repository_slices_work_on_sqlite() {
        let path = std::env::temp_dir().join(format!("yin-user-repo-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        verify_user_repository(
            Driver::Sqlite,
            &format!("sqlite://{}?mode=rwc", path.display()),
        )
        .await;
        std::fs::remove_file(path).expect("remove repository fixture database");
    }

    #[tokio::test]
    async fn shared_repository_slices_work_on_configured_mysql_and_postgres() {
        if let Ok(url) = std::env::var("YIN_MYSQL_TEST_URL") {
            verify_user_repository(Driver::MySql, &url).await;
        } else {
            eprintln!("skipping MySQL repository integration: YIN_MYSQL_TEST_URL is unset");
        }
        if let Ok(url) = std::env::var("YIN_POSTGRES_TEST_URL") {
            verify_user_repository(Driver::Postgres, &url).await;
        } else {
            eprintln!("skipping PostgreSQL repository integration: YIN_POSTGRES_TEST_URL is unset");
        }
    }

    async fn verify_oauth_identity(pool: &sqlx::AnyPool, driver: Driver) {
        let identity_id = match driver {
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::MySql => "BIGINT PRIMARY KEY AUTO_INCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        for statement in [
            format!("CREATE TABLE o_auth_identity (id {identity_id}, created_at VARCHAR(40), updated_at VARCHAR(40), user_id BIGINT NOT NULL, provider VARCHAR(50) NOT NULL, subject VARCHAR(255) NOT NULL, email VARCHAR(255))"),
            format!("CREATE TABLE space_member (id {identity_id}, created_at VARCHAR(40), updated_at VARCHAR(40), space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(20) NOT NULL, source VARCHAR(20) NOT NULL DEFAULT 'manual', joined_at VARCHAR(40))"),
            format!("CREATE TABLE space_o_id_c_group (id {identity_id}, created_at VARCHAR(40), updated_at VARCHAR(40), space_id BIGINT NOT NULL, provider VARCHAR(50) NOT NULL, group_name VARCHAR(255) NOT NULL, role VARCHAR(20) NOT NULL)"),
        ] {
            sqlx::query(&statement)
                .execute(pool)
                .await
                .expect("create oauth fixture");
        }
        let now = "2024-01-01 00:00:00.000000000+00:00";
        let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };

        link_oauth_identity(pool, driver, 17, "authentik", "subject-17", "user@example.test", now)
            .await
            .expect("link oauth identity");
        let identity = oauth_identity(pool, driver, "authentik", "subject-17")
            .await
            .expect("read oauth identity")
            .expect("identity exists");
        assert_eq!(identity.user_id, 17);
        assert_eq!(identity.email, "user@example.test");

        link_oauth_identity(pool, driver, 18, "authentik", "subject-17", "second@example.test", now)
            .await
            .expect("relink oauth identity");
        let identity = oauth_identity(pool, driver, "authentik", "subject-17")
            .await
            .expect("read relinked identity")
            .expect("identity still exists");
        assert_eq!(identity.user_id, 18);
        assert_eq!(identity.email, "second@example.test");

        sqlx::query("INSERT INTO space_o_id_c_group (id, space_id, provider, group_name, role) VALUES (1, 80, 'authentik', 'team', 'editor')")
            .execute(pool)
            .await
            .expect("insert oidc group rule");
        sqlx::query("INSERT INTO space_o_id_c_group (id, space_id, provider, group_name, role) VALUES (2, 81, 'authentik', 'staff', 'admin')")
            .execute(pool)
            .await
            .expect("insert second oidc group rule");
        let rules = oidc_group_rules(pool, driver, "authentik")
            .await
            .expect("read oidc group rules");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].group_name, "team");
        assert_eq!(rules[0].role, "editor");

        grant_oidc_member(pool, driver, 80, 18, "viewer", now)
            .await
            .expect("insert oidc member");
        grant_oidc_member(pool, driver, 80, 18, "editor", now)
            .await
            .expect("upgrade oidc member");
        grant_oidc_member(pool, driver, 80, 18, "viewer", now)
            .await
            .expect("ignore a lower oidc role");
        let role: String = sqlx::query_scalar(&format!("SELECT CAST(role AS {text_type}) FROM space_member WHERE space_id = 80 AND user_id = 18"))
            .fetch_one(pool)
            .await
            .expect("read oidc member role");
        assert_eq!(role, "editor");
        let source: String = sqlx::query_scalar(&format!("SELECT CAST(source AS {text_type}) FROM space_member WHERE space_id = 80 AND user_id = 18"))
            .fetch_one(pool)
            .await
            .expect("read oidc member source");
        assert_eq!(source, "oidc");

        sqlx::query("INSERT INTO space_member (id, space_id, user_id, role, source) VALUES (11, 81, 18, 'viewer', 'manual')")
            .execute(pool)
            .await
            .expect("insert manual member");
        grant_oidc_member(pool, driver, 81, 18, "admin", now)
            .await
            .expect("leave manual member alone");
        let manual_role: String = sqlx::query_scalar(&format!("SELECT CAST(role AS {text_type}) FROM space_member WHERE id = 11"))
            .fetch_one(pool)
            .await
            .expect("read manual role");
        assert_eq!(manual_role, "viewer");

        for statement in [
            "DROP TABLE space_o_id_c_group",
            "DROP TABLE space_member",
            "DROP TABLE o_auth_identity",
        ] {
            sqlx::query(statement)
                .execute(pool)
                .await
                .expect("drop oauth fixture");
        }
    }

}
