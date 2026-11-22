//! The Rust port of the repository methods the Go adapter exposes.
//!
//! `internal/tools/repositoryport` is a non-product package: it points the
//! repository's package-level `*gorm.DB` at one file for a single call, so the
//! bridge can record the real methods. This crate performs the same queries
//! with `sqlx` against that file.

#![forbid(unsafe_code)]

use std::path::Path;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

/// The adapter refuses a path it cannot find, with this message.
async fn open(db_path: &str) -> Result<SqlitePool, String> {
    if db_path.is_empty() {
        return Err("database path is required".to_string());
    }
    if !Path::new(db_path).is_file() {
        return Err(format!("database not found: {db_path}"));
    }
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(db_path))
        .await
        .map_err(|error| error.to_string())
}

fn not_found() -> String {
    "record not found".to_string()
}

/// The bridge calls these from a Go thread, so each one owns its runtime.
fn run<F, T>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime.block_on(future),
        Err(error) => Err(error.to_string()),
    }
}

/// `UserRepo.Count`.
pub fn count_users(db_path: String) -> Result<serde_json::Value, String> {
    run(count_users_inner(db_path))
}

/// `UserRepo.GetByMail`, reduced to the mail and the nickname.
pub fn user_mail_and_name(db_path: String, mail: String) -> Result<serde_json::Value, String> {
    run(user_mail_and_name_inner(db_path, mail))
}

/// `UserRepo.GetByMail`, reduced to the stored hash.
pub fn get_user_password(db_path: String, mail: String) -> Result<String, String> {
    run(get_user_password_inner(db_path, mail))
}

/// `UserRepo.GetByMailAndPassword`: a miss is `false`, not an error.
pub fn password_matches(db_path: String, mail: String, password: String) -> Result<bool, String> {
    run(password_matches_inner(db_path, mail, password))
}

/// `ItemIconGroupRepo.Count`.
pub fn count_groups(db_path: String, user_id: i64) -> Result<i64, String> {
    run(count_groups_inner(db_path, user_id))
}

/// `ItemIconGroupRepo.GetList`, reduced to the titles. The order is the source's:
/// `sort, created_at`.
pub fn group_titles(db_path: String, user_id: i64) -> Result<Vec<String>, String> {
    run(group_titles_inner(db_path, user_id))
}

/// `UserRepo.UpdateUserInfo` restricted to the nickname, in one transaction.
pub fn update_user_name(db_path: String, user_id: i64, name: String) -> Result<(), String> {
    run(update_user_name_inner(db_path, user_id, name))
}

/// `UserRepo.Count`.
async fn count_users_inner(db_path: String) -> Result<serde_json::Value, String> {
    let pool = open(&db_path).await?;
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM \"user\"")
        .fetch_one(&pool)
        .await
        .map_err(|error| error.to_string())?;
    pool.close().await;
    Ok(serde_json::json!(row.0))
}

/// `UserRepo.GetByMail`, reduced to the mail and the nickname.
async fn user_mail_and_name_inner(
    db_path: String,
    mail: String,
) -> Result<serde_json::Value, String> {
    let pool = open(&db_path).await?;
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT mail, name FROM \"user\" WHERE mail = ?")
            .bind(&mail)
            .fetch_optional(&pool)
            .await
            .map_err(|error| error.to_string())?;
    pool.close().await;
    match row {
        Some((mail, name)) => Ok(serde_json::json!([mail, name])),
        None => Err(not_found()),
    }
}

/// `UserRepo.GetByMail`, reduced to the stored hash.
async fn get_user_password_inner(db_path: String, mail: String) -> Result<String, String> {
    let pool = open(&db_path).await?;
    let row: Option<(String,)> = sqlx::query_as("SELECT password FROM \"user\" WHERE mail = ?")
        .bind(&mail)
        .fetch_optional(&pool)
        .await
        .map_err(|error| error.to_string())?;
    pool.close().await;
    row.map(|value| value.0).ok_or_else(not_found)
}

/// `UserRepo.GetByMailAndPassword`: a miss is `false`, not an error.
async fn password_matches_inner(
    db_path: String,
    mail: String,
    password: String,
) -> Result<bool, String> {
    let pool = open(&db_path).await?;
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM \"user\" WHERE mail = ? AND password = ? LIMIT 1")
            .bind(&mail)
            .bind(&password)
            .fetch_optional(&pool)
            .await
            .map_err(|error| error.to_string())?;
    pool.close().await;
    Ok(row.is_some())
}

/// `ItemIconGroupRepo.Count`.
async fn count_groups_inner(db_path: String, user_id: i64) -> Result<i64, String> {
    let pool = open(&db_path).await?;
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM item_icon_group WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|error| error.to_string())?;
    pool.close().await;
    Ok(row.0)
}

/// `ItemIconGroupRepo.GetList`, reduced to the titles. The order is the source's:
/// `sort, created_at`.
async fn group_titles_inner(db_path: String, user_id: i64) -> Result<Vec<String>, String> {
    let pool = open(&db_path).await?;
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT title FROM item_icon_group WHERE user_id = ? ORDER BY sort, created_at",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| error.to_string())?;
    pool.close().await;
    Ok(rows.into_iter().map(|row| row.0).collect())
}

/// `UserRepo.UpdateUserInfo` restricted to the nickname, in one transaction.
async fn update_user_name_inner(db_path: String, user_id: i64, name: String) -> Result<(), String> {
    let pool = open(&db_path).await?;
    let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
    let exists: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM \"user\" WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| error.to_string())?;
    if exists.is_none() {
        return Err(not_found());
    }
    sqlx::query("UPDATE \"user\" SET name = ? WHERE id = ?")
        .bind(&name)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| error.to_string())?;
    tx.commit().await.map_err(|error| error.to_string())?;
    pool.close().await;
    Ok(())
}
