//! Upgrades a database that predates the current schema.
//!
//! The fixture is the shape an old deployment has: retired columns, a `team`
//! table, mixed-case and duplicate login addresses, an account without one,
//! duplicate configuration rows and no unique mail index.

use std::path::PathBuf;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection, Executor, Row, SqliteConnection};

fn fixture_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("yin-db-test-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("database.db")
}

async fn connect(path: &PathBuf) -> SqliteConnection {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .connect()
        .await
        .unwrap()
}

async fn seed_legacy(path: &PathBuf) {
    let mut conn = connect(path).await;
    for statement in [
        r#"CREATE TABLE "user" (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at datetime, updated_at datetime, password text, name text, head_image text, status integer, role integer, mail text, username text, publiccode text, oauth_provider text, oauth_id text)"#,
        r#"CREATE TABLE space (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at datetime, updated_at datetime, type varchar(20), name varchar(100), owner_user_id integer, pair_id integer, side varchar(10), team_id integer)"#,
        r#"CREATE TABLE space_member (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at datetime, updated_at datetime, space_id integer, user_id integer, role varchar(20), source varchar(20), joined_at datetime)"#,
        r#"CREATE TABLE item_icon (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at datetime, updated_at datetime, icon_json text, title varchar(50), url text, description text, open_method integer, sort integer, item_icon_group_id integer, user_id integer, space_id integer)"#,
        r#"CREATE TABLE item_icon_group (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at datetime, updated_at datetime, icon varchar(50), title varchar(50), sort integer, user_id integer, space_id integer)"#,
        r#"CREATE TABLE user_config (user_id integer, panel_json text)"#,
        r#"CREATE TABLE module_config (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at datetime, updated_at datetime, user_id integer, name varchar(255), value_json text)"#,
        r#"CREATE TABLE system_setting (id INTEGER PRIMARY KEY AUTOINCREMENT, config_name varchar(50), config_value text)"#,
        r#"CREATE TABLE file (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at datetime, user_id integer, file_name varchar(100))"#,
        r#"CREATE TABLE o_auth_identity (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at datetime, updated_at datetime, user_id integer, provider varchar(50), subject varchar(255), email varchar(255))"#,
        r#"CREATE TABLE team (id INTEGER PRIMARY KEY, name text)"#,
        r#"INSERT INTO team (id, name) VALUES (1, 'legacy')"#,
        r#"INSERT INTO "user" (id, password, name, status, role, mail, username, oauth_provider, oauth_id) VALUES
             (1, 'h1', 'Admin', 1, 1, 'Admin@Example.COM ', 'admin', 'github', 'gh-1'),
             (2, 'h2', 'Duplicate', 1, 2, 'admin@example.com', 'dup', '', ''),
             (3, 'h3', 'Orphan', 1, 2, '', 'orphan', '', '')"#,
        r#"INSERT INTO space (id, type, name, owner_user_id, pair_id, side, team_id) VALUES (1, 'shared', 'Team', 2, 1, 'yin', 1)"#,
        r#"INSERT INTO space_member (space_id, user_id, role, source) VALUES (1, 1, 'admin', 'manual'), (1, 2, 'viewer', 'manual')"#,
        r#"INSERT INTO item_icon (icon_json, title, url, open_method, sort, item_icon_group_id, user_id, space_id) VALUES ('{}', 'Link', 'https://x.test', 2, 0, 1, 2, 1)"#,
        r#"INSERT INTO user_config (user_id, panel_json) VALUES (1, '{"a":1}'), (1, '{"a":2}')"#,
        r#"INSERT INTO module_config (user_id, name, value_json) VALUES (1, 'home', '{"v":1}'), (1, 'home', '{"v":2}')"#,
        r#"INSERT INTO system_setting (config_name, config_value) VALUES ('site', 'one'), ('site', 'two')"#,
    ] {
        conn.execute(statement)
            .await
            .unwrap_or_else(|error| panic!("seed statement failed: {error}\n{statement}"));
    }
    conn.close().await.unwrap();
}

async fn column_names(path: &PathBuf, table: &str) -> Vec<String> {
    let mut conn = connect(path).await;
    let rows = sqlx::query(&format!("SELECT name FROM pragma_table_info('{table}')"))
        .fetch_all(&mut conn)
        .await
        .unwrap();
    conn.close().await.unwrap();
    rows.iter()
        .map(|row| row.get::<String, _>("name"))
        .collect()
}

#[tokio::test]
async fn upgrades_a_legacy_sqlite_database() {
    let path = fixture_path("upgrade");
    seed_legacy(&path).await;

    let steps = yin_db_tool::upgrade::upgrade(&path).await.unwrap();
    assert!(!steps.is_empty(), "the upgrade should report its steps");

    let mut conn = connect(&path).await;
    // One canonical account per address, with the address normalised.
    let users: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, mail, name FROM \"user\" ORDER BY id")
            .fetch_all(&mut conn)
            .await
            .unwrap();
    assert_eq!(users.len(), 2, "duplicate account was merged: {users:?}");
    assert_eq!(users[0].1, "admin@example.com");
    assert_eq!(users[1].1, "orphan-3@local.invalid");

    // Every reference of the merged account moved to the survivor.
    let owner: (i64,) = sqlx::query_as("SELECT owner_user_id FROM space WHERE id = 1")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(owner.0, 1);
    let item_owner: (i64,) = sqlx::query_as("SELECT user_id FROM item_icon WHERE id = 1")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(item_owner.0, 1);
    let members: Vec<(i64,)> =
        sqlx::query_as("SELECT user_id FROM space_member WHERE space_id = 1 ORDER BY user_id")
            .fetch_all(&mut conn)
            .await
            .unwrap();
    assert_eq!(members, vec![(1,)], "the duplicate membership collapsed");

    // Duplicate configuration rows kept exactly one survivor each.
    let configs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM user_config")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(configs.0, 1);
    let modules: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM module_config")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(modules.0, 1);
    let settings: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM system_setting")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(settings.0, 1);

    // The legacy provider columns became identities before they were dropped.
    let identity: (String, String) =
        sqlx::query_as("SELECT provider, subject FROM o_auth_identity")
            .fetch_one(&mut conn)
            .await
            .unwrap();
    assert_eq!(identity, ("github".to_string(), "gh-1".to_string()));

    // Retired schema is gone and the added columns exist.
    let user_columns = column_names(&path, "user").await;
    for retired in ["publiccode", "oauth_provider", "oauth_id"] {
        assert!(
            !user_columns.contains(&retired.to_string()),
            "{retired} survived"
        );
    }
    let space_columns = column_names(&path, "space").await;
    assert!(!space_columns.contains(&"team_id".to_string()));
    assert!(space_columns.contains(&"search_config_json".to_string()));
    assert!(user_columns.contains(&"token_version".to_string()));
    let team: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'team'")
            .fetch_one(&mut conn)
            .await
            .unwrap();
    assert_eq!(team.0, 0, "the legacy team table was dropped");

    // The unique mail index exists.
    let index: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'uk_user_mail'",
    )
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert_eq!(index.0, 1);
    conn.close().await.unwrap();

    // A second run changes nothing and does not fail.
    let again = yin_db_tool::upgrade::upgrade(&path).await.unwrap();
    assert!(
        again.is_empty(),
        "the upgrade must be idempotent, second run reported {again:?}"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
