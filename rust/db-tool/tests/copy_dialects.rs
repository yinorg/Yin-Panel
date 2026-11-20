//! The DDL the copy renders for each target dialect.

use yin_db_tool::copy_sql::{Dialect, parse_table, render, render_all};

fn table(name: &str) -> yin_db_tool::copy_sql::Table {
    yin_db_tool::schema::TABLES
        .iter()
        .filter_map(|statement| parse_table(statement))
        .find(|table| table.name == name)
        .unwrap_or_else(|| panic!("table {name} not in the schema"))
}

#[test]
fn mysql_renders_auto_increment_lengths_and_booleans() {
    let user = render(&table("user"), Dialect::MySql);
    assert!(
        user.contains("`id` BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY"),
        "{user}"
    );
    assert!(user.contains("`mail` VARCHAR(255)"), "{user}");
    assert!(
        user.contains("`token_version` BIGINT NOT NULL DEFAULT 0"),
        "{user}"
    );

    let space = render(&table("space"), Dialect::MySql);
    assert!(
        space.contains("`public_enabled` TINYINT(1) NOT NULL DEFAULT 0"),
        "{space}"
    );
    assert!(
        space.contains("`side` VARCHAR(10) NOT NULL DEFAULT 'yin'"),
        "{space}"
    );
    assert!(space.contains("`search_config_json` LONGTEXT"), "{space}");
}

#[test]
fn postgres_renders_serial_boolean_and_bytea() {
    let user = render(&table("user"), Dialect::Postgres);
    assert!(user.contains("\"id\" BIGSERIAL PRIMARY KEY"), "{user}");
    assert!(user.contains("\"mail\" VARCHAR(255)"), "{user}");

    let space = render(&table("space"), Dialect::Postgres);
    assert!(
        space.contains("\"public_enabled\" BOOLEAN NOT NULL DEFAULT false"),
        "{space}"
    );

    let asset = render(&table("asset_record_v2"), Dialect::Postgres);
    assert!(asset.contains("\"content\" BYTEA NOT NULL"), "{asset}");
    assert!(
        asset.contains("PRIMARY KEY (\"revision_id\",\"path\")"),
        "{asset}"
    );
}

#[test]
fn every_table_renders_for_both_dialects() {
    let mysql = render_all(Dialect::MySql);
    let postgres = render_all(Dialect::Postgres);
    assert_eq!(mysql.len(), yin_db_tool::schema::TABLES.len());
    assert_eq!(postgres.len(), yin_db_tool::schema::TABLES.len());
    for statement in mysql.iter().chain(postgres.iter()) {
        assert!(
            statement.starts_with("CREATE TABLE IF NOT EXISTS"),
            "{statement}"
        );
        assert!(statement.ends_with(')'), "{statement}");
        assert!(!statement.contains("AUTOINCREMENT"), "{statement}");
        assert!(!statement.contains("DEFAULT \""), "{statement}");
    }
}
