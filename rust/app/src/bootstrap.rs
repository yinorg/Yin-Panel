//! First-run bootstrap: create the database, upgrade its schema and seed it.
//!
//! The Go backend does this at startup: `initDatabase` migrates, installs the
//! three built-in themes and creates the default administrator. A deployment of
//! this binary must reach the same state from an empty directory, so the seed
//! below was generated from a freshly initialised Go database and is inserted
//! only when the database has no account yet.

use std::path::Path;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

#[path = "seed/assets.rs"]
mod seed_assets;
#[path = "seed/rows.rs"]
mod seed_rows;

/// Runs the schema upgrade and, on a database without accounts, the seed.
pub async fn prepare(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if !path.is_file() {
        std::fs::File::create(path).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    let applied = yin_db_tool::upgrade::upgrade(path).await?;
    if !applied.is_empty() {
        println!("schema: {}", applied.join("; "));
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(path))
        .await
        .map_err(|error| format!("opening {}: {error}", path.display()))?;
    let ready = seeded(&pool).await?;
    if !ready {
        seed(&pool).await?;
        println!("seeded the initial installation");
    }
    pool.close().await;
    Ok(())
}

async fn seeded(pool: &SqlitePool) -> Result<bool, String> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM \"user\"")
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())?;
    Ok(row.0 > 0)
}

async fn seed(pool: &SqlitePool) -> Result<(), String> {
    let now = now();
    let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
    for statement in seed_rows::ROWS {
        let placeholders = statement.matches("{}").count();
        let sql = statement.replace("{}", "?");
        let mut query = sqlx::query(&sql);
        for _ in 0..placeholders {
            query = query.bind(&now);
        }
        query
            .execute(&mut *tx)
            .await
            .map_err(|error| format!("seed: {error}"))?;
    }
    for (revision, path, media_type, content) in seed_assets::ASSETS {
        sqlx::query(
            "INSERT INTO asset_record_v2 (revision_id, path, media_type, content) VALUES (?, ?, ?, ?)",
        )
        .bind(revision)
        .bind(path)
        .bind(media_type)
        .bind(*content)
        .execute(&mut *tx)
        .await
        .map_err(|error| format!("seed asset: {error}"))?;
    }
    tx.commit().await.map_err(|error| error.to_string())?;
    Ok(())
}

/// The timestamp form SQLite stores: `YYYY-MM-DD HH:MM:SS(.frac)+offset`.
fn now() -> String {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let now = time::OffsetDateTime::now_utc().to_offset(offset);
    let format = time::macros::format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:9][offset_hour sign:mandatory]:[offset_minute]"
    );
    now.format(&format).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::prepare;

    #[tokio::test]
    async fn prepares_a_temporary_sqlite_database_and_is_idempotent() {
        let path = std::env::temp_dir().join(format!("yin-bootstrap-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        prepare(&path).await.expect("initial bootstrap");
        prepare(&path).await.expect("repeat bootstrap");

        let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", path.display()))
            .await
            .expect("open bootstrapped db");
        let (users,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM \"user\"")
            .fetch_one(&pool)
            .await
            .expect("count seeded users");
        let (themes,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM package_record_v2")
            .fetch_one(&pool)
            .await
            .expect("count seeded themes");
        assert_eq!(users, 1);
        assert!(themes >= 3);
        pool.close().await;
        std::fs::remove_file(path).expect("remove temporary database");
    }
}
