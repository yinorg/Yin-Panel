//! `migrate-db` into another SQLite file: the copy is a real copy and the
//! destination is upgraded to the current schema.

use std::path::PathBuf;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection, Executor, Row};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("yin-db-copy-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn copies_a_sqlite_deployment_into_another_file() {
    let dir = temp_dir("sqlite");
    let source = dir.join("source.db");
    let mut conn = SqliteConnectOptions::new()
        .filename(&source)
        .create_if_missing(true)
        .connect()
        .await
        .unwrap();
    for statement in [
        r#"CREATE TABLE "user" (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at datetime, updated_at datetime, password text, name text, head_image text, status integer, role integer, mail text, publiccode text)"#,
        r#"CREATE TABLE space (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at datetime, updated_at datetime, type varchar(20), name varchar(100), owner_user_id integer, pair_id integer, side varchar(10), team_id integer)"#,
        r#"INSERT INTO "user" (password, name, status, role, mail) VALUES ('h', 'Admin', 1, 1, 'Admin@Example.COM')"#,
        r#"INSERT INTO space (type, name, owner_user_id, pair_id, side) VALUES ('personal', 'Admin', 1, 1, 'yin')"#,
    ] {
        conn.execute(statement).await.unwrap();
    }
    conn.close().await.unwrap();

    let source_conf = dir.join("conf.yaml");
    let target_conf = dir.join("conf.target.yaml");
    std::fs::write(
        &source_conf,
        "base:\n  database_drive: sqlite\nsqlite:\n  file_path: ./source.db\n",
    )
    .unwrap();
    std::fs::write(
        &target_conf,
        "base:\n  database_drive: sqlite\nsqlite:\n  file_path: ./target.db\n",
    )
    .unwrap();

    let source = yin_db_tool::load_conf(source_conf.to_str().unwrap()).unwrap();
    let target = yin_db_tool::load_conf(target_conf.to_str().unwrap()).unwrap();
    yin_db_tool::copy::run(&source, &target).await.unwrap();

    let destination = dir.join("target.db");
    assert!(destination.is_file(), "the copy was written");
    let mut conn = SqliteConnectOptions::new()
        .filename(&destination)
        .connect()
        .await
        .unwrap();
    let user: (String, String) = sqlx::query_as("SELECT mail, name FROM \"user\"")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(user.1, "Admin");
    assert_eq!(user.0, "admin@example.com", "the copy is upgraded");
    let table: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'item_icon'",
    )
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert_eq!(table.0, 1, "the destination received the full schema");
    let index: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'uk_user_mail'",
    )
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert_eq!(index.0, 1);
    conn.close().await.unwrap();

    // The source is untouched.
    let mut conn = SqliteConnectOptions::new()
        .filename(dir.join("source.db"))
        .connect()
        .await
        .unwrap();
    let rows = sqlx::query("SELECT mail FROM \"user\"")
        .fetch_all(&mut conn)
        .await
        .unwrap();
    assert_eq!(rows[0].get::<String, _>("mail"), "Admin@Example.COM");
    conn.close().await.unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn migration_indexes_are_rendered_for_mysql_and_postgres() {
    use yin_db_tool::copy_sql::{Dialect, indexes_for, parse_table};
    use yin_db_tool::schema::{INDEXES, TABLES};

    let tables: Vec<_> = TABLES.iter().filter_map(|statement| parse_table(statement)).collect();
    let mysql = indexes_for(Dialect::MySql, &tables);
    let postgres = indexes_for(Dialect::Postgres, &tables);
    assert_eq!(mysql.len(), INDEXES.len());
    assert_eq!(postgres.len(), INDEXES.len());
    assert!(mysql.iter().all(|statement| !statement.contains("IF NOT EXISTS")));
    let revision = mysql
        .iter()
        .find(|statement| statement.contains("revision_record_v2"))
        .expect("revision indexes");
    assert!(revision.contains("`package_id`(191)"), "{revision}");
    assert!(
        postgres
            .iter()
            .any(|statement| statement.contains("\"package_id\"")),
        "postgres quotes identifiers"
    );
}
