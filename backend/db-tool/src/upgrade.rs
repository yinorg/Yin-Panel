//! Schema upgrade for an existing SQLite database.
//!
//! The Go backend upgrades a deployment in place at startup: it deduplicates
//! the rows that would block the new unique indexes, adds what the current
//! models declare, normalises the login identity, merges accounts that share an
//! address, retires the legacy columns and finally creates the unique mail
//! index. This module performs the same steps in the same order. Every step is
//! idempotent, so running it on an up-to-date database changes nothing.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Executor, SqlitePool};

use crate::schema::{INDEXES, TABLES};

const ORPHAN_MAIL_DOMAIN: &str = "local.invalid";
const USER_MAIL_INDEX: &str = "uk_user_mail";

pub async fn upgrade(path: &Path) -> Result<Vec<String>, String> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|error| format!("opening {}: {error}", path.display()))?;

    let mut applied = Vec::new();
    pool.execute("PRAGMA foreign_keys = OFF")
        .await
        .map_err(|error| error.to_string())?;

    // 1. The tables must exist before the cleanup can look at them.
    for statement in TABLES {
        pool.execute(*statement)
            .await
            .map_err(|error| format!("create tables: {error}"))?;
    }

    // 2. Columns added since an old deployment was created. `AutoMigrate` adds
    //    them before the indexes; without this an index on a new column fails.
    for step in ensure_columns(&pool).await? {
        applied.push(step);
    }

    // 3. Remove the rows that would make the unique indexes fail.
    let removed = deduplicate(&pool).await?;
    if removed > 0 {
        applied.push(format!("deduplicated {removed} rows"));
    }

    // 4. Indexes, including the unique ones the cleanup prepared for.
    for statement in INDEXES {
        // The unique mail index is created last, once the addresses are unique;
        // creating it here would fail on the rows this upgrade is about to clean.
        if statement.contains(USER_MAIL_INDEX) {
            continue;
        }
        pool.execute(*statement)
            .await
            .map_err(|error| format!("create indexes: {error}"))?;
    }

    // 5. One canonical login address per account.
    if normalize_mails(&pool).await? > 0 {
        applied.push("normalized login mails".to_string());
    }

    // 6. Legacy provider columns, when the database still has them.
    if backfill_oauth_identities(&pool).await? > 0 {
        applied.push("backfilled oauth identities".to_string());
    }

    // 7. Accounts that share an address become one.
    if merge_duplicate_accounts(&pool).await? > 0 {
        applied.push("merged duplicate accounts".to_string());
    }

    // 8. Retired columns and the `team` table.
    for dropped in drop_legacy_schema(&pool).await? {
        applied.push(dropped);
    }

    // 9. The unique mail index, once the data is clean.
    let unique = format!("CREATE UNIQUE INDEX IF NOT EXISTS {USER_MAIL_INDEX} ON \"user\"(mail)");
    pool.execute(unique.as_str())
        .await
        .map_err(|error| format!("create {USER_MAIL_INDEX}: {error}"))?;

    pool.execute("PRAGMA foreign_keys = ON")
        .await
        .map_err(|error| error.to_string())?;
    pool.close().await;
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::upgrade;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::{Executor, SqlitePool};

    async fn pool(path: &std::path::Path) -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(false),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn upgrades_a_legacy_database_and_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy.db");
        std::fs::File::create(&path).unwrap();
        let db = pool(&path).await;
        db.execute(
            r#"
            CREATE TABLE "user" (
                id INTEGER PRIMARY KEY,
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT '',
                password TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL DEFAULT '',
                head_image TEXT NOT NULL DEFAULT '',
                status INTEGER NOT NULL DEFAULT 1,
                role TEXT NOT NULL DEFAULT 'user',
                mail TEXT NOT NULL DEFAULT '',
                oauth_provider TEXT,
                oauth_id TEXT
            );
            INSERT INTO "user" (id, mail, oauth_provider, oauth_id)
                VALUES (1, ' ADMIN@example.com ', 'github', 'subject-1');
            INSERT INTO "user" (id, mail)
                VALUES (2, '' );
            CREATE TABLE user_config (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT '',
                user_id INTEGER NOT NULL,
                config_name TEXT NOT NULL DEFAULT '',
                config_value TEXT NOT NULL DEFAULT ''
            );
            INSERT INTO user_config (user_id, config_name, config_value)
                VALUES (1, 'theme', 'old'), (1, 'theme', 'new');
            CREATE TABLE space (
                id INTEGER PRIMARY KEY,
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT '',
                type TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL DEFAULT '',
                owner_user_id INTEGER NOT NULL DEFAULT 0,
                pair_id INTEGER NOT NULL DEFAULT 0,
                side TEXT NOT NULL DEFAULT 'yin',
                team_id INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX idx_space_team_id ON space(team_id);
            INSERT INTO space (id, name, owner_user_id, team_id)
                VALUES (1, 'legacy home', 1, 9);
            CREATE TABLE team (id INTEGER PRIMARY KEY, name TEXT);
            INSERT INTO team VALUES (9, 'retired team');
            "#,
        )
        .await
        .unwrap();
        db.close().await;

        let applied = upgrade(&path).await.unwrap();
        assert!(applied.iter().any(|step| step == "normalized login mails"));
        assert!(
            applied
                .iter()
                .any(|step| step == "backfilled oauth identities")
        );
        assert!(
            applied
                .iter()
                .any(|step| step.contains("dropped space.team_id"))
        );

        let db = pool(&path).await;
        let mail: (String,) = sqlx::query_as("SELECT mail FROM \"user\" WHERE id = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(mail.0, "admin@example.com");
        let orphan: (String,) = sqlx::query_as("SELECT mail FROM \"user\" WHERE id = 2")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(orphan.0, "orphan-2@local.invalid");
        let identities: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM o_auth_identity WHERE user_id = 1 AND provider = 'github' AND subject = 'subject-1'")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(identities.0, 1);
        let configs: Vec<(String,)> = sqlx::query_as(
            "SELECT config_value FROM user_config WHERE user_id = 1 AND config_name = 'theme'",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(configs, vec![("new".to_string(),)]);
        let space: (String, i64) =
            sqlx::query_as("SELECT name, public_enabled FROM space WHERE id = 1")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(space.0, "legacy home");
        assert_eq!(space.1, 0);
        let team: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'team'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(team.0, 0);
        let index: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_space_team_id'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(index.0, 0);
        let unique_mail: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'uk_user_mail'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(unique_mail.0, 1);
        db.close().await;

        // Re-running an upgraded schema must do no work and preserve contents.
        assert!(upgrade(&path).await.unwrap().is_empty());
    }
}

/// Columns the current models declare that an older database may not have yet.
const ADDED_COLUMNS: &[(&str, &str, &str)] = &[
    ("user", "token_version", "integer NOT NULL DEFAULT 0"),
    ("user_config", "language", "text"),
    ("space", "public_enabled", "numeric NOT NULL DEFAULT false"),
    ("space", "public_id", "varchar(30)"),
    (
        "space",
        "public_mode",
        "varchar(20) NOT NULL DEFAULT 'direct'",
    ),
    ("space", "public_code_hash", "varchar(128)"),
    ("space", "search_config_json", "text"),
    ("space", "wallpaper_json", "text"),
    ("item_icon", "mobile_url", "text"),
    ("item_icon", "lan_url", "text"),
    ("item_icon_group", "description", "text"),
    ("item_icon_group", "parent_id", "integer"),
    ("file", "updated_at", "datetime"),
];

async fn ensure_columns(pool: &SqlitePool) -> Result<Vec<String>, String> {
    let mut applied = Vec::new();
    for (table, column, definition) in ADDED_COLUMNS {
        if !table_exists(pool, table).await || column_exists(pool, table, column).await {
            continue;
        }
        let sql = format!("ALTER TABLE \"{table}\" ADD COLUMN \"{column}\" {definition}");
        pool.execute(sql.as_str())
            .await
            .map_err(|error| format!("add {table}.{column}: {error}"))?;
        applied.push(format!("added {table}.{column}"));
    }
    Ok(applied)
}

async fn table_exists(pool: &SqlitePool, table: &str) -> bool {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1")
            .bind(table)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
    row.is_some()
}

async fn column_exists(pool: &SqlitePool, table: &str, column: &str) -> bool {
    let sql = format!("SELECT name FROM pragma_table_info('{table}')");
    let rows: Vec<(String,)> = sqlx::query_as(&sql)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.iter().any(|(name,)| name == column)
}

/// `deduplicateUserConfigs`, `deduplicateModuleConfigs` and
/// `deduplicateSystemSettings`: the newest row per key wins.
async fn deduplicate(pool: &SqlitePool) -> Result<usize, String> {
    let mut removed = 0usize;
    let steps: [(&str, &str, &str); 3] = [
        ("user_config", "user_config", "user_id"),
        ("module_config", "module_config", "user_id, name"),
        ("system_setting", "system_setting", "config_name"),
    ];
    for (table, _, keys) in steps {
        if !table_exists(pool, table).await {
            continue;
        }
        let sql = format!(
            "DELETE FROM {table} WHERE rowid NOT IN (SELECT MAX(rowid) FROM {table} GROUP BY {keys})"
        );
        let result = pool
            .execute(sql.as_str())
            .await
            .map_err(|error| error.to_string())?;
        removed += result.rows_affected() as usize;
    }
    Ok(removed)
}

/// `normalizeUserMails`: trim and lower-case, and give an account without an
/// address a unique placeholder so the unique index can be built.
async fn normalize_mails(pool: &SqlitePool) -> Result<usize, String> {
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, mail FROM \"user\"")
        .fetch_all(pool)
        .await
        .map_err(|error| error.to_string())?;
    let mut changed = 0usize;
    for (id, mail) in rows {
        let mut normalized = mail.trim().to_lowercase();
        if normalized.is_empty() {
            normalized = format!("orphan-{id}@{ORPHAN_MAIL_DOMAIN}");
        }
        if normalized == mail {
            continue;
        }
        sqlx::query("UPDATE \"user\" SET mail = ? WHERE id = ?")
            .bind(&normalized)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|error| error.to_string())?;
        changed += 1;
    }
    Ok(changed)
}

/// `backfillOAuthIdentities`: copy the retired provider columns into the
/// identity table before those columns are dropped.
async fn backfill_oauth_identities(pool: &SqlitePool) -> Result<usize, String> {
    if !column_exists(pool, "user", "oauth_provider").await
        || !column_exists(pool, "user", "oauth_id").await
    {
        return Ok(0);
    }
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, oauth_provider, oauth_id, mail FROM \"user\"
         WHERE oauth_provider IS NOT NULL AND oauth_provider <> ''
           AND oauth_provider <> 'buildin'
           AND oauth_id IS NOT NULL AND oauth_id <> ''",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| error.to_string())?;
    let mut created = 0usize;
    for (id, provider, subject, mail) in rows {
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM o_auth_identity WHERE provider = ? AND subject = ? LIMIT 1",
        )
        .bind(&provider)
        .bind(&subject)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
        if existing.is_some() {
            continue;
        }
        sqlx::query(
            "INSERT INTO o_auth_identity (created_at, updated_at, user_id, provider, subject, email)
             VALUES (datetime('now'), datetime('now'), ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(&provider)
        .bind(&subject)
        .bind(&mail)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;
        created += 1;
    }
    Ok(created)
}

/// `mergeDuplicateUserAccounts`: the oldest account with an address wins, and
/// every reference of the others moves to it.
async fn merge_duplicate_accounts(pool: &SqlitePool) -> Result<usize, String> {
    let mails: Vec<(String,)> =
        sqlx::query_as("SELECT mail FROM \"user\" GROUP BY mail HAVING COUNT(*) > 1")
            .fetch_all(pool)
            .await
            .map_err(|error| error.to_string())?;
    let mut merged = 0usize;
    for (mail,) in mails {
        let ids: Vec<(i64,)> = sqlx::query_as("SELECT id FROM \"user\" WHERE mail = ? ORDER BY id")
            .bind(&mail)
            .fetch_all(pool)
            .await
            .map_err(|error| error.to_string())?;
        let Some((canonical,)) = ids.first().copied() else {
            continue;
        };
        for (duplicate,) in ids.iter().skip(1) {
            merge_account(pool, canonical, *duplicate).await?;
            merged += 1;
        }
    }
    Ok(merged)
}

async fn merge_account(pool: &SqlitePool, canonical: i64, duplicate: i64) -> Result<(), String> {
    let moves: [(&str, &str); 6] = [
        ("space", "owner_user_id"),
        ("item_icon", "user_id"),
        ("item_icon_group", "user_id"),
        ("file", "user_id"),
        ("o_auth_identity", "user_id"),
        ("audit_record", "actor_id"),
    ];
    for (table, column) in moves {
        if !table_exists(pool, table).await {
            continue;
        }
        let sql = format!("UPDATE {table} SET {column} = ? WHERE {column} = ?");
        sqlx::query(&sql)
            .bind(canonical)
            .bind(duplicate)
            .execute(pool)
            .await
            .map_err(|error| error.to_string())?;
    }
    // Membership rows must not collide on (space_id, user_id).
    sqlx::query(
        "DELETE FROM space_member WHERE user_id = ? AND space_id IN (SELECT space_id FROM space_member WHERE user_id = ?)",
    )
    .bind(duplicate)
    .bind(canonical)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;
    sqlx::query("UPDATE space_member SET user_id = ? WHERE user_id = ?")
        .bind(canonical)
        .bind(duplicate)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;
    // Single-row configs: the canonical row wins.
    for (table, key) in [
        ("user_config", "user_id"),
        ("user_theme_preference_v2", "user_id"),
    ] {
        if !table_exists(pool, table).await {
            continue;
        }
        let exists: Option<(i64,)> =
            sqlx::query_as(&format!("SELECT 1 FROM {table} WHERE {key} = ? LIMIT 1"))
                .bind(canonical)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);
        if exists.is_some() {
            sqlx::query(&format!("DELETE FROM {table} WHERE {key} = ?"))
                .bind(duplicate)
                .execute(pool)
                .await
                .map_err(|error| error.to_string())?;
        } else {
            sqlx::query(&format!("UPDATE {table} SET {key} = ? WHERE {key} = ?"))
                .bind(canonical)
                .bind(duplicate)
                .execute(pool)
                .await
                .map_err(|error| error.to_string())?;
        }
    }
    if table_exists(pool, "module_config").await {
        sqlx::query(
            "DELETE FROM module_config WHERE user_id = ? AND name IN (SELECT name FROM module_config WHERE user_id = ?)",
        )
        .bind(duplicate)
        .bind(canonical)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;
        sqlx::query("UPDATE module_config SET user_id = ? WHERE user_id = ?")
            .bind(canonical)
            .bind(duplicate)
            .execute(pool)
            .await
            .map_err(|error| error.to_string())?;
    }
    sqlx::query("DELETE FROM \"user\" WHERE id = ?")
        .bind(duplicate)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// `dropLegacySchema`: the retired columns are inert, so a failure to drop one
/// is reported and the upgrade continues.
async fn drop_legacy_schema(pool: &SqlitePool) -> Result<Vec<String>, String> {
    let mut applied = Vec::new();
    for (table, column) in [
        ("user", "publiccode"),
        ("user", "oauth_provider"),
        ("user", "oauth_id"),
        ("space", "team_id"),
    ] {
        if !column_exists(pool, table, column).await {
            continue;
        }
        // SQLite refuses to drop a column an index still names, so the indexes
        // that reference it go first; `gorm.Migrator().DropColumn` rebuilds the
        // table and does the same.
        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = ? AND sql IS NOT NULL",
        )
        .bind(table)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        for (index,) in indexes {
            let definition: Option<(String,)> =
                sqlx::query_as("SELECT sql FROM sqlite_master WHERE name = ?")
                    .bind(&index)
                    .fetch_optional(pool)
                    .await
                    .unwrap_or(None);
            let names_column = definition
                .map(|(sql,)| {
                    sql.contains(&format!("\"{column}\""))
                        || sql.contains(&format!("`{column}`"))
                        || sql.contains(&format!("({column})"))
                        || sql.contains(&format!(",{column}"))
                })
                .unwrap_or(false);
            if names_column {
                let drop = format!("DROP INDEX IF EXISTS \"{index}\"");
                if pool.execute(drop.as_str()).await.is_ok() {
                    applied.push(format!("dropped index {index}"));
                }
            }
        }
        let sql = format!("ALTER TABLE \"{table}\" DROP COLUMN \"{column}\"");
        match pool.execute(sql.as_str()).await {
            Ok(_) => applied.push(format!("dropped {table}.{column}")),
            Err(error) => applied.push(format!("could not drop {table}.{column}: {error}")),
        }
    }
    if table_exists(pool, "team").await {
        pool.execute("DROP TABLE team")
            .await
            .map_err(|error| format!("drop table team: {error}"))?;
        applied.push("dropped table team".to_string());
    }
    Ok(applied)
}
