//! The Rust port of `internal/infra/database/`, narrowed to the one function the
//! binary's `migrate-db` command calls.
//!
//! Every other function in that package takes the package-level `*gorm.DB` or a
//! `*gorm.DB` argument, so the bridge keeps them in Go; the schema upgrade they
//! perform is ported in `rust/db-tool`, and the HTTP layer exercises it.

#![forbid(unsafe_code)]

/// Port of `MigrateDB` (`internal/infra/database/copydb.go:34`).
///
/// The copy itself lives in `rust/db-tool` (the same function the binary's
/// `migrate-db` command calls): table definitions, dialect translation, the
/// per-table row copy and a `VACUUM INTO` snapshot of the source.
pub fn migrate_db(source_config_path: String, target_config_path: String) -> Result<(), String> {
    yin_db_tool::migrate_db(&source_config_path, &target_config_path)
}
