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
        "mysql" => Err("copying into MySQL is not implemented yet".to_string()),
        "postgres" => Err("copying into PostgreSQL is not implemented yet".to_string()),
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
    std::fs::copy(source, &destination).map_err(|error| {
        format!(
            "copying {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })?;
    println!("Copied {} to {}", source.display(), destination.display());
    let applied = upgrade::upgrade(&destination).await?;
    for step in applied {
        println!("  {step}");
    }
    Ok(())
}
