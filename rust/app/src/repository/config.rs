//! Module and user configuration plus list projections.

use sqlx::AnyPool;

use crate::db::{Driver, query};

use super::*;

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
