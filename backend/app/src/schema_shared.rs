//! Translates the recorded SQLite schema into MySQL and PostgreSQL DDL.
//!
//! `yin_db_tool::schema` stays the single source of truth: those statements are
//! generated from a fully migrated Go database. The Rust binary must create the
//! same tables from an empty MySQL/PostgreSQL database, so the differences are
//! applied here instead of keeping three schema lists in sync by hand.

use crate::db::Driver;

pub fn quote(driver: Driver, identifier: &str) -> String {
    match driver {
        Driver::MySql => format!("`{identifier}`"),
        _ => format!("\"{identifier}\""),
    }
}

/// Columns listed in a table-level `PRIMARY KEY (…)` clause.
fn primary_key_columns(statement: &str) -> Vec<String> {
    let Some(start) = statement.find("PRIMARY KEY (") else {
        return Vec::new();
    };
    let rest = &statement[start + "PRIMARY KEY (".len()..];
    let Some(end) = rest.find(')') else {
        return Vec::new();
    };
    rest[..end]
        .split(',')
        .map(|column| column.trim().trim_matches(['`', '"']).to_string())
        .filter(|column| !column.is_empty())
        .collect()
}

/// A `text` column cannot participate in a MySQL primary key, so those columns
/// become a bounded `varchar`; Go's `text` primary keys are identifiers.
fn mysql_text_primary_keys(statement: &str, columns: &[String]) -> String {
    let mut out = statement.to_string();
    for column in columns {
        let candidate = format!("`{column}` text");
        let replacement = format!("`{column}` varchar(191)");
        out = out.replace(&candidate, &replacement);
    }
    out
}

pub fn table_statement(driver: Driver, statement: &str) -> String {
    // A table definition goes through the same parser and renderer that
    // `migrate-db` uses, so startup schema creation and the copy tool cannot
    // drift apart. Non-CREATE statements (migrations) keep the text rules.
    if statement.starts_with("CREATE TABLE")
        && let Some(table) = yin_db_tool::copy_sql::parse_table(statement)
    {
        return match driver {
            Driver::Sqlite => statement.to_string(),
            Driver::MySql => {
                yin_db_tool::copy_sql::render(&table, yin_db_tool::copy_sql::Dialect::MySql)
            }
            Driver::Postgres => {
                yin_db_tool::copy_sql::render(&table, yin_db_tool::copy_sql::Dialect::Postgres)
            }
        };
    }
    let mut out = statement.to_string();
    if driver == Driver::Postgres {
        out = out.replace('`', "\"");
        out = out.replace("integer PRIMARY KEY AUTOINCREMENT", "BIGSERIAL PRIMARY KEY");
        out = out.replace(" integer", " BIGINT");
        out = out.replace("DATETIME", "TIMESTAMPTZ");
        out = out.replace("datetime", "TIMESTAMPTZ");
        out = out.replace("blob", "BYTEA");
        out = out.replace("numeric NOT NULL DEFAULT false", "BOOLEAN NOT NULL DEFAULT false");
        for default in ["yin", "manual", "direct", "custom"] {
            out = out.replace(&format!("DEFAULT \"{default}\""), &format!("DEFAULT '{default}'"));
        }
    } else {
        out = out.replace(
            "integer PRIMARY KEY AUTOINCREMENT",
            "BIGINT AUTO_INCREMENT PRIMARY KEY",
        );
        out = out.replace(" integer", " BIGINT");
        out = out.replace("DATETIME", "DATETIME(6)");
        out = out.replace("datetime", "DATETIME(6)");
        out = out.replace("blob", "LONGBLOB");
        out = out.replace("numeric NOT NULL DEFAULT false", "TINYINT(1) NOT NULL DEFAULT 0");
        // MySQL refuses a default on TEXT, so a defaulted text column becomes a
        // bounded varchar; only short enum-like columns carry defaults.
        out = out.replace("text NOT NULL DEFAULT", "varchar(191) NOT NULL DEFAULT");
        for default in ["yin", "manual", "direct", "custom"] {
            out = out.replace(&format!("DEFAULT \"{default}\""), &format!("DEFAULT '{default}'"));
        }
        let columns = primary_key_columns(statement);
        out = mysql_text_primary_keys(&out, &columns);
    }
    out
}

pub fn index_statement(driver: Driver, statement: &str) -> String {
    match driver {
        Driver::Postgres => {
            yin_db_tool::copy_sql::translate_index(statement, yin_db_tool::copy_sql::Dialect::Postgres)
        }
        Driver::Sqlite => statement.to_string(),
        Driver::MySql => {
            // MySQL 8 rejects `CREATE INDEX IF NOT EXISTS`; existence is checked
            // before the statement instead.
            yin_db_tool::copy_sql::translate_index(statement, yin_db_tool::copy_sql::Dialect::MySql)
        }
    }
}

pub fn tables(driver: Driver) -> Vec<String> {
    yin_db_tool::schema::TABLES
        .iter()
        .map(|statement| table_statement(driver, statement))
        .collect()
}

pub fn indexes(driver: Driver) -> Vec<String> {
    yin_db_tool::schema::INDEXES
        .iter()
        .map(|statement| index_statement(driver, statement))
        .collect()
}

/// The seed rows are recorded Go inserts: they quote `"user"` for SQLite and
/// inline `0`/`1` for columns PostgreSQL stores as booleans.
pub fn seed_statement(driver: Driver, statement: &str) -> String {
    match driver {
        Driver::Sqlite => statement.to_string(),
        Driver::MySql => statement.replace("\"user\"", "`user`"),
        Driver::Postgres => statement
            .replace("\"user\"", "\"user\"")
            .replace(", 0, {}, {})", ", FALSE, {}, {})")
            .replace(", 1, {})", ", TRUE, {})")
            .replace(", 0, NULL, 'direct'", ", FALSE, NULL, 'direct'"),
    }
}

/// Tables that carry an auto-increment `id` column on PostgreSQL.
pub fn serial_tables() -> Vec<String> {
    tables(Driver::Postgres)
        .iter()
        .filter(|statement| statement.contains("BIGSERIAL PRIMARY KEY"))
        .filter_map(|statement| {
            let rest = statement.split_once("EXISTS \"")?.1;
            rest.split('"').next().map(str::to_string)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{index_statement, seed_statement, table_statement};
    use crate::db::Driver;

    fn space_table_statement() -> &'static str {
        yin_db_tool::schema::TABLES
            .iter()
            .find(|statement| statement.contains("`space`"))
            .expect("space table")
    }

    #[test]
    fn postgres_converts_autoincrement_datetime_blob_and_defaults() {
        let out = table_statement(Driver::Postgres, space_table_statement());
        assert!(out.contains("\"id\" BIGSERIAL PRIMARY KEY"), "{out}");
        assert!(out.contains("\"created_at\" TIMESTAMPTZ"), "{out}");
        assert!(
            out.contains("\"public_enabled\" BOOLEAN NOT NULL DEFAULT false"),
            "{out}"
        );
        assert!(out.contains("DEFAULT 'yin'"), "{out}");
        assert!(!out.contains('`'), "{out}");
    }

    #[test]
    fn mysql_uses_auto_increment_datetime_and_bounded_primary_keys() {
        let space = table_statement(Driver::MySql, space_table_statement());
        assert!(
            space.contains("`id` BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY"),
            "{space}"
        );
        assert!(space.contains("`created_at` DATETIME(6)"), "{space}");
        assert!(
            space.contains("`public_enabled` TINYINT(1) NOT NULL DEFAULT 0"),
            "{space}"
        );
        assert!(!space.contains("DATETIME(6)(6)"), "{space}");

        let asset = yin_db_tool::schema::TABLES
            .iter()
            .find(|statement| statement.contains("`asset_record_v2`"))
            .expect("asset table");
        let out = table_statement(Driver::MySql, asset);
        let upper = out.to_uppercase();
        assert!(upper.contains("`REVISION_ID` VARCHAR(191)"), "{out}");
        assert!(upper.contains("`PATH` VARCHAR(191)"), "{out}");
        assert!(out.contains("`content` LONGBLOB"), "{out}");
    }

    #[test]
    fn mysql_indexes_drop_if_not_exists_and_postgres_keep_it() {
        let mysql = index_statement(Driver::MySql, "CREATE UNIQUE INDEX IF NOT EXISTS `i` ON `t`(`c`)");
        assert_eq!(mysql, "CREATE UNIQUE INDEX `i` ON `t`(`c`)");
        let postgres = index_statement(Driver::Postgres, "CREATE INDEX IF NOT EXISTS \"i\" ON \"t\"(\"c\")");
        assert_eq!(postgres, "CREATE INDEX IF NOT EXISTS \"i\" ON \"t\"(\"c\")");
    }

    #[test]
    fn seed_statements_quote_tables_and_boolean_literals() {
        let package_row = yin_db_tool::schema::TABLES
            .iter()
            .map(|_| ())
            .count();
        assert!(package_row > 0);
        let mysql = seed_statement(Driver::MySql, "INSERT INTO \"user\" (id) VALUES (1)");
        assert_eq!(mysql, "INSERT INTO `user` (id) VALUES (1)");
        let postgres = seed_statement(
            Driver::Postgres,
            "INSERT INTO package_record_v2 (id, name) VALUES ('a', 'b', 0, {}, {})",
        );
        assert!(postgres.contains(", FALSE, {}, {})"), "{postgres}");
    }
}
