//! `migrate-db`: copy a SQLite deployment into another database.
//!
//! SQLite targets are a file copy followed by an upgrade, which is what a local
//! move needs. MySQL/MariaDB and PostgreSQL targets create the schema from the
//! same table definitions, translated to the target dialect, and copy every row.

use std::path::Path;

use crate::{ConfigFile, upgrade};

pub async fn run(source: &ConfigFile, target: &ConfigFile) -> Result<(), String> {
    if source.driver() != "sqlite" {
        return Err("migrate-db reads a SQLite deployment as its source".to_string());
    }
    let source_file = source.sqlite_path();
    if !source_file.is_file() {
        return Err(format!("database not found: {}", source_file.display()));
    }
    match target.driver().as_str() {
        "sqlite" => copy_sqlite(&source_file, target).await,
        "mysql" => crate::copy_sql::run(source, &source_file, target, "mysql").await,
        "postgres" => crate::copy_sql::run(source, &source_file, target, "postgres").await,
        other => Err(format!("unsupported target driver: {other}")),
    }
}

async fn copy_sqlite(source: &Path, target: &ConfigFile) -> Result<(), String> {
    let destination = target.sqlite_path();
    if destination == source {
        return Err("source and target are the same file".to_string());
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if destination.exists() {
        std::fs::remove_file(&destination).map_err(|error| error.to_string())?;
    }
    // A plain file copy misses everything still in the write-ahead log, which
    // on a running deployment is most of the recent data. `VACUUM INTO` takes a
    // consistent snapshot through SQLite itself.
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(source)
        .read_only(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|error| format!("opening {}: {error}", source.display()))?;
    let statement = format!("VACUUM INTO '{}'", destination.display());
    sqlx::Executor::execute(&pool, statement.as_str())
        .await
        .map_err(|error| format!("snapshotting {}: {error}", source.display()))?;
    pool.close().await;
    println!("Copied {} to {}", source.display(), destination.display());
    let applied = upgrade::upgrade(&destination).await?;
    for step in applied {
        println!("  {step}");
    }
    Ok(())
}
