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

use crate::db::{Driver, query};
use crate::schema_shared;

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

/// Creates the schema and, on a database without accounts, the seed for a
/// MySQL or PostgreSQL deployment. SQLite keeps `prepare`: it must upgrade a
/// legacy file in place, which the other drivers never need.
pub async fn prepare_shared(pool: &sqlx::AnyPool, driver: Driver) -> Result<(), String> {
    if !table_exists(pool, driver, "user").await? {
        for statement in schema_shared::tables(driver) {
            sqlx::query(&statement)
                .execute(pool)
                .await
                .map_err(|error| format!("create tables: {error}"))?;
        }
        for statement in schema_shared::indexes(driver) {
            if driver == Driver::MySql {
                let statement = mysql_index_with_prefix(pool, &statement).await?;
                if index_exists(pool, &statement).await? {
                    continue;
                }
                if let Err(error) = sqlx::query(&statement).execute(pool).await {
                    // A prior start may have created the index already.
                    if !error.to_string().contains("Duplicate key name") {
                        return Err(format!("create indexes: {error}\n{statement}"));
                    }
                }
                continue;
            }
            if let Err(error) = sqlx::query(&statement).execute(pool).await {
                return Err(format!("create indexes: {error}"));
            }
        }
        println!("schema created");
    }

    let row = sqlx::query(&query(
        driver,
        &format!(
            "SELECT COUNT(*) AS total FROM {}",
            schema_shared::quote(driver, "user")
        ),
    ))
    .fetch_one(pool)
    .await
    .map_err(|error| error.to_string())?;
    use sqlx::Row;
    let users: i64 = row.try_get("total").map_err(|error| error.to_string())?;
    if users == 0 {
        seed_shared(pool, driver).await?;
        println!("seeded the initial installation");
    }
    // Seeded rows carry explicit ids, which PostgreSQL does not fold into the
    // sequence; without this the first auto-increment insert would collide.
    reset_sequences(pool, driver).await?;
    Ok(())
}

/// Moves every `BIGSERIAL` sequence past the largest existing id. MySQL adjusts
/// its counter when an explicit id is inserted, so only PostgreSQL needs it.
pub async fn reset_sequences(pool: &sqlx::AnyPool, driver: Driver) -> Result<(), String> {
    if driver != Driver::Postgres {
        return Ok(());
    }
    for table in schema_shared::serial_tables() {
        use sqlx::Row;
        let exists = sqlx::query(&query(
            Driver::Postgres,
            "SELECT CAST(to_regclass(?) AS TEXT) AS relation",
        ))
        .bind(format!("\"{table}\""))
        .fetch_one(pool)
        .await
        .map_err(|error| format!("inspect {table}: {error}"))?;
        let relation: Option<String> = exists
            .try_get("relation")
            .map_err(|error| format!("inspect {table}: {error}"))?;
        if relation.is_none() {
            continue;
        }
        let sql = format!(
            "SELECT setval(pg_get_serial_sequence('{table}', 'id'), GREATEST(COALESCE((SELECT MAX(id) FROM \"{table}\"), 0), 1))"
        );
        sqlx::query(&sql)
            .execute(pool)
            .await
            .map_err(|error| format!("reset {table} sequence: {error}"))?;
    }
    Ok(())
}

async fn table_exists(
    pool: &sqlx::AnyPool,
    driver: Driver,
    table: &str,
) -> Result<bool, String> {
    let sql = match driver {
        Driver::MySql => {
            "SELECT COUNT(*) AS total FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = ?"
        }
        Driver::Postgres => {
            "SELECT COUNT(*) AS total FROM information_schema.tables WHERE table_schema = current_schema() AND table_name = ?"
        }
        Driver::Sqlite => {
            "SELECT COUNT(*) AS total FROM sqlite_master WHERE type = 'table' AND name = ?"
        }
    };
    let row = sqlx::query(&query(driver, sql))
        .bind(table)
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())?;
    use sqlx::Row;
    let total: i64 = row.try_get("total").map_err(|error| error.to_string())?;
    Ok(total > 0)
}

/// MySQL needs a key length for TEXT columns; only short identifier-like
/// columns are indexed, so a 191-character prefix is always safe here.
async fn mysql_index_with_prefix(
    pool: &sqlx::AnyPool,
    statement: &str,
) -> Result<String, String> {
    use sqlx::Row;

    let Some(on) = statement.find(" ON ") else {
        return Err(format!("unexpected index statement: {statement}"));
    };
    let head = &statement[..on + 4];
    let rest = &statement[on + 4..];
    let Some(open) = rest.find('(') else {
        return Err(format!("unexpected index statement: {statement}"));
    };
    let table = rest[..open].trim().trim_matches('`');
    let after = &rest[open + 1..];
    let Some(close) = after.rfind(')') else {
        return Err(format!("unexpected index statement: {statement}"));
    };
    let column_list = &after[..close];
    let tail = &after[close + 1..];

    let text_columns: Vec<String> = sqlx::query(
        "SELECT column_name FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ? AND data_type IN ('text','mediumtext','longtext','tinytext')",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|error| format!("inspect {table}: {error}"))?
    .into_iter()
    .map(|row| row.try_get::<String, _>(0).unwrap_or_default())
    .collect();

    let columns: Vec<String> = column_list
        .split(',')
        .map(|token| {
            let token = token.trim();
            let (name, order) = match token.split_once(' ') {
                Some((name, order)) => (name.trim().trim_matches('`'), format!(" {}", order.trim())),
                None => (token.trim_matches('`'), String::new()),
            };
            if text_columns.iter().any(|column| column == name) {
                format!("`{name}`(191){order}")
            } else {
                token.to_string()
            }
        })
        .collect();

    Ok(format!(
        "{head}{}({}){tail}",
        rest[..open].trim_end(),
        columns.join(",")
    ))
}

async fn index_exists(pool: &sqlx::AnyPool, statement: &str) -> Result<bool, String> {
    let after_index = statement
        .split_once("INDEX ")
        .map(|(_, rest)| rest)
        .ok_or_else(|| format!("unexpected index statement: {statement}"))?;
    let name = after_index
        .split_once(" ON ")
        .map(|(name, _)| name.trim().trim_matches('`'))
        .ok_or_else(|| format!("unexpected index statement: {statement}"))?;
    let row = sqlx::query(
        "SELECT COUNT(*) AS total FROM information_schema.statistics WHERE table_schema = DATABASE() AND index_name = ?",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(|error| error.to_string())?;
    use sqlx::Row;
    let total: i64 = row.try_get("total").map_err(|error| error.to_string())?;
    Ok(total > 0)
}

async fn seed_shared(pool: &sqlx::AnyPool, driver: Driver) -> Result<(), String> {
    let now = now();
    let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
    for statement in seed_rows::ROWS {
        let mut sql = schema_shared::seed_statement(driver, statement).replace("{}", "?");
        if driver == Driver::Postgres {
            // Every `{}` in the generated seed rows is a timestamp; PostgreSQL
            // will not coerce a text bind into timestamptz on its own.
            sql = sql.replace('?', "CAST(? AS TIMESTAMPTZ)");
        }
        let placeholders = sql.matches('?').count();
        let rendered = query(driver, &sql);
        let mut query = sqlx::query(&rendered);
        for _ in 0..placeholders {
            query = query.bind(&now);
        }
        query
            .execute(&mut *tx)
            .await
            .map_err(|error| format!("seed: {error}"))?;
    }
    for (revision, path, media_type, content) in seed_assets::ASSETS {
        sqlx::query(&query(
            driver,
            "INSERT INTO asset_record_v2 (revision_id, path, media_type, content) VALUES (?, ?, ?, ?)",
        ))
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

    /// Creates and seeds the same installation on a real MySQL and PostgreSQL.
    /// The containers are throwaway, so the schema is not dropped afterwards;
    /// the second call proves the bootstrap is idempotent.
    #[tokio::test]
    async fn prepares_configured_mysql_and_postgres_from_empty() {
        if let Ok(url) = std::env::var("YIN_MYSQL_TEST_URL") {
            verify_shared(super::super::db::Driver::MySql, &url).await;
        } else {
            eprintln!("skipping MySQL bootstrap: YIN_MYSQL_TEST_URL is unset");
        }
        if let Ok(url) = std::env::var("YIN_POSTGRES_TEST_URL") {
            verify_shared(super::super::db::Driver::Postgres, &url).await;
        } else {
            eprintln!("skipping PostgreSQL bootstrap: YIN_POSTGRES_TEST_URL is unset");
        }
    }

    async fn verify_shared(driver: super::super::db::Driver, url: &str) {
        use sqlx::Row;

        let pool = super::super::db::connect(driver, url)
            .await
            .unwrap_or_else(|error| panic!("connect {driver:?}: {error}"));
        super::prepare_shared(&pool, driver)
            .await
            .unwrap_or_else(|error| panic!("{driver:?} bootstrap: {error}"));
        super::prepare_shared(&pool, driver)
            .await
            .unwrap_or_else(|error| panic!("{driver:?} second bootstrap: {error}"));

        let count = |sql: &str| {
            let pool = pool.clone();
            let sql = sql.to_string();
            async move {
                let rendered = super::super::db::query(driver, &sql);
                let row = sqlx::query(&rendered)
                    .fetch_one(&pool)
                    .await
                    .expect("count seeded rows");
                row.try_get::<i64, _>("total").expect("total")
            }
        };
        let user_table = super::super::schema_shared::quote(driver, "user");
        let space_table = super::super::schema_shared::quote(driver, "space");
        assert_eq!(
            count(&format!("SELECT COUNT(*) AS total FROM {user_table}")).await,
            1,
            "{driver:?} seeds one administrator"
        );
        assert!(
            count("SELECT COUNT(*) AS total FROM package_record_v2").await >= 3,
            "{driver:?} seeds the three built-in themes"
        );
        assert!(
            count("SELECT COUNT(*) AS total FROM asset_record_v2").await > 0,
            "{driver:?} seeds the theme assets"
        );
        assert!(
            count(&format!("SELECT COUNT(*) AS total FROM {space_table}")).await >= 2,
            "{driver:?} seeds the paired space"
        );
        pool.close().await;
    }
}
