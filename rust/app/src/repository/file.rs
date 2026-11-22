//! Uploaded files and web wallpapers.

use sqlx::{AnyPool, Row};

use crate::db::{Driver, query};

use super::*;

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
