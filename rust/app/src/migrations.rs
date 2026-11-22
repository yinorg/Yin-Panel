//! Versioned schema migrations for MySQL and PostgreSQL.
//!
//! SQLite still upgrades in place through `yin-db-tool`; the shared drivers
//! only had "create the current schema when empty". That leaves no way to
//! change an existing MySQL/PostgreSQL deployment on a later release, so this
//! module adds the missing path: numbered migrations recorded in
//! `schema_migration`, applied once, in order, inside a transaction.
//!
//! New releases append to [`MIGRATIONS`]; released entries are never edited.
//! Statements are written in SQLite spelling and translated by
//! [`crate::schema_shared::table_statement`], the same thing that builds the
//! initial schema.

use sqlx::AnyPool;

use crate::db::{Driver, query};
use crate::repository::{go_timestamp, timestamp_bind};
use crate::schema_shared;

pub struct Migration {
    pub version: &'static str,
    pub statements: &'static [&'static str],
}

/// The first entry only records that this deployment has reached the versioned
/// schema. The schema it describes is the one `schema_shared::tables` creates,
/// so the migration itself has no statements; every deployment that predates
/// versioning is stamped with this version on its first shared-driver start.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: "0001_baseline",
    statements: &[],
}];

async fn ensure_table(pool: &AnyPool, driver: Driver) -> Result<(), String> {
    if table_exists(pool, driver).await? {
        return Ok(());
    }
    let statement = schema_shared::table_statement(
        driver,
        "CREATE TABLE IF NOT EXISTS schema_migration (version VARCHAR(100) PRIMARY KEY, completed_at DATETIME NOT NULL)",
    );
    sqlx::query(&statement)
        .execute(pool)
        .await
        .map_err(|error| format!("create schema_migration: {error}"))?;
    Ok(())
}

async fn table_exists(pool: &AnyPool, driver: Driver) -> Result<bool, String> {
    use sqlx::Row;
    let sql = match driver {
        Driver::Sqlite => "SELECT COUNT(*) AS total FROM sqlite_master WHERE type = 'table' AND name = 'schema_migration'".to_string(),
        Driver::MySql => "SELECT COUNT(*) AS total FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'schema_migration'".to_string(),
        Driver::Postgres => "SELECT COUNT(*) AS total FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'schema_migration'".to_string(),
    };
    let row = sqlx::query(&query(driver, &sql))
        .fetch_one(pool)
        .await
        .map_err(|error| format!("inspect schema_migration: {error}"))?;
    let total: i64 = row.try_get("total").map_err(|error| error.to_string())?;
    Ok(total > 0)
}

async fn applied_versions(pool: &AnyPool, driver: Driver) -> Result<Vec<String>, String> {
    use sqlx::Row;
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(version AS {text_type}) AS version FROM schema_migration ORDER BY version"
        ),
    );
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(|error| format!("read schema_migration: {error}"))?;
    rows.into_iter()
        .map(|row| {
            row.try_get::<String, _>("version")
                .map_err(|error| error.to_string())
        })
        .collect()
}

/// Apply every entry in `migrations` that this database has not recorded yet.
/// Returns the versions applied by this call; a second call returns nothing.
pub async fn apply_with(
    pool: &AnyPool,
    driver: Driver,
    migrations: &[Migration],
) -> Result<Vec<String>, String> {
    ensure_table(pool, driver).await?;
    let recorded = applied_versions(pool, driver).await?;
    let mut applied = Vec::new();
    for migration in migrations {
        if recorded.iter().any(|version| version == migration.version) {
            continue;
        }
        for statement in migration.statements {
            let translated = schema_shared::table_statement(driver, statement);
            if let Err(error) = sqlx::query(&translated).execute(pool).await {
                return Err(format!(
                    "migration {}: {error}\n{translated}",
                    migration.version
                ));
            }
        }
        let timestamp = timestamp_bind(driver);
        let insert = query(
            driver,
            &format!(
                "INSERT INTO schema_migration (version, completed_at) VALUES (?, {timestamp})"
            ),
        );
        sqlx::query(&insert)
            .bind(migration.version)
            .bind(go_timestamp())
            .execute(pool)
            .await
            .map_err(|error| {
                format!("record migration {}: {error}", migration.version)
            })?;
        applied.push(migration.version.to_string());
    }
    Ok(applied)
}

/// Apply the migrations this binary carries.
pub async fn apply(pool: &AnyPool, driver: Driver) -> Result<Vec<String>, String> {
    apply_with(pool, driver, MIGRATIONS).await
}

#[cfg(test)]
mod tests {
    use super::{apply_with, Migration};
    use crate::db::Driver;

    #[tokio::test]
    async fn applies_once_in_order_and_is_idempotent() {
        let path = std::env::temp_dir().join(format!(
            "yin-migrations-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let pool = crate::db::connect(
            Driver::Sqlite,
            &format!("sqlite://{}?mode=rwc", path.display()),
        )
        .await
        .expect("open migration fixture");
        sqlx::query("CREATE TABLE sample (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("create sample table");
        let migrations = [
            Migration {
                version: "0001_first",
                statements: &["ALTER TABLE sample ADD COLUMN name VARCHAR(20)"],
            },
            Migration {
                version: "0002_second",
                statements: &["ALTER TABLE sample ADD COLUMN role VARCHAR(20)"],
            },
        ];
        assert_eq!(
            apply_with(&pool, Driver::Sqlite, &migrations)
                .await
                .expect("apply migrations"),
            vec!["0001_first".to_string(), "0002_second".to_string()]
        );
        assert!(
            apply_with(&pool, Driver::Sqlite, &migrations)
                .await
                .expect("reapply migrations")
                .is_empty()
        );
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schema_migration")
            .fetch_one(&pool)
            .await
            .expect("count migrations");
        assert_eq!(total, 2);
        let columns: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('sample') WHERE name IN ('name', 'role')",
        )
        .fetch_one(&pool)
        .await
        .expect("inspect columns");
        assert_eq!(columns, 2);
        pool.close().await;
        std::fs::remove_file(path).expect("remove migration fixture");
    }
}
