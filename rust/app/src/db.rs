//! Shared SQLx query execution for the supported database drivers.

use std::sync::OnceLock;

use sqlx::{
    AnyPool,
    any::{AnyPoolOptions, install_default_drivers},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    Sqlite,
    MySql,
    Postgres,
}

impl Driver {
    #[cfg(test)]
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "sqlite" | "" => Ok(Self::Sqlite),
            "mysql" => Ok(Self::MySql),
            "postgres" | "postgresql" => Ok(Self::Postgres),
            other => Err(format!("unsupported database driver: {other}")),
        }
    }

    #[cfg(test)]
    pub fn url_scheme(self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::MySql => "mysql",
            Self::Postgres => "postgres",
        }
    }
}

pub async fn connect(driver: Driver, url: &str) -> Result<AnyPool, sqlx::Error> {
    static DRIVERS: OnceLock<()> = OnceLock::new();
    DRIVERS.get_or_init(install_default_drivers);
    AnyPoolOptions::new()
        .max_connections(if driver == Driver::Sqlite { 4 } else { 10 })
        .connect(url)
        .await
}

/// Rewrites positional `?` binds to PostgreSQL's `$n` syntax. Quoted text,
/// identifiers, and SQL comments are left untouched.
pub fn query(driver: Driver, sql: &str) -> String {
    if driver != Driver::Postgres {
        return sql.to_string();
    }
    let bytes = sql.as_bytes();
    let mut output = Vec::with_capacity(sql.len() + 8);
    let mut index = 0;
    let mut bind = 0;
    let mut quote = None;
    let mut line_comment = false;
    let mut block_comment = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if line_comment {
            output.push(byte);
            line_comment = byte != b'\n';
            index += 1;
        } else if block_comment {
            output.push(byte);
            if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                output.push(b'/');
                index += 2;
                block_comment = false;
            } else {
                index += 1;
            }
        } else if let Some(delimiter) = quote {
            output.push(byte);
            if byte == b'\\' && delimiter == b'\'' && index + 1 < bytes.len() {
                output.push(bytes[index + 1]);
                index += 2;
            } else if byte == delimiter {
                if bytes.get(index + 1) == Some(&delimiter) {
                    output.push(delimiter);
                    index += 2;
                } else {
                    quote = None;
                    index += 1;
                }
            } else {
                index += 1;
            }
        } else if byte == b'-' && bytes.get(index + 1) == Some(&b'-') {
            output.extend_from_slice(b"--");
            index += 2;
            line_comment = true;
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            output.extend_from_slice(b"/*");
            index += 2;
            block_comment = true;
        } else if matches!(byte, b'\'' | b'"' | b'`') {
            output.push(byte);
            quote = Some(byte);
            index += 1;
        } else if byte == b'?' {
            bind += 1;
            output.extend_from_slice(format!("${bind}").as_bytes());
            index += 1;
        } else {
            output.push(byte);
            index += 1;
        }
    }
    String::from_utf8(output).expect("SQL text remains valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::{Driver, connect, query};
    use sqlx::Row;

    async fn verify_query_contract(driver: Driver, url: &str) {
        let pool = connect(driver, url)
            .await
            .unwrap_or_else(|error| panic!("connect {driver:?} test database: {error}"));
        let table = format!("yin_db_contract_{}", std::process::id());
        sqlx::query(&query(
            driver,
            &format!("CREATE TABLE {table} (value BIGINT NOT NULL)"),
        ))
        .execute(&pool)
        .await
        .expect("create contract table");

        let mut transaction = pool.begin().await.expect("begin transaction");
        sqlx::query(&query(
            driver,
            &format!("INSERT INTO {table} (value) VALUES (?)"),
        ))
        .bind(123_i64)
        .execute(&mut *transaction)
        .await
        .expect("insert committed value");
        transaction.commit().await.expect("commit transaction");

        let mut transaction = pool.begin().await.expect("begin rollback transaction");
        sqlx::query(&query(
            driver,
            &format!("INSERT INTO {table} (value) VALUES (?)"),
        ))
        .bind(456_i64)
        .execute(&mut *transaction)
        .await
        .expect("insert rollback value");
        transaction.rollback().await.expect("rollback transaction");

        let row = sqlx::query(&query(
            driver,
            &format!("SELECT value FROM {table} WHERE value = ?"),
        ))
        .bind(123_i64)
        .fetch_one(&pool)
        .await
        .expect("read committed row");
        assert_eq!(row.try_get::<i64, _>("value").expect("value"), 123);
        let row = sqlx::query(&query(
            driver,
            &format!("SELECT COUNT(*) AS count FROM {table}"),
        ))
        .fetch_one(&pool)
        .await
        .expect("count rows");
        assert_eq!(row.try_get::<i64, _>("count").expect("count"), 1);

        sqlx::query(&query(driver, &format!("DROP TABLE {table}")))
            .execute(&pool)
            .await
            .expect("drop contract table");
        pool.close().await;
    }

    #[test]
    fn parses_supported_driver_names_and_rejects_unknown_names() {
        assert_eq!(Driver::parse("sqlite").unwrap(), Driver::Sqlite);
        assert_eq!(Driver::parse("MySQL").unwrap(), Driver::MySql);
        assert_eq!(Driver::parse("postgresql").unwrap(), Driver::Postgres);
        assert!(Driver::parse("oracle").is_err());
    }

    #[test]
    fn exposes_each_sqlx_url_scheme() {
        assert_eq!(Driver::Sqlite.url_scheme(), "sqlite");
        assert_eq!(Driver::MySql.url_scheme(), "mysql");
        assert_eq!(Driver::Postgres.url_scheme(), "postgres");
    }

    #[test]
    fn postgres_bind_rewrite_skips_quoted_text_and_comments() {
        assert_eq!(
            query(
                Driver::Postgres,
                "SELECT '?', \"?\", `?`, ? -- ?\n/* ? */ WHERE id = ?",
            ),
            "SELECT '?', \"?\", `?`, $1 -- ?\n/* ? */ WHERE id = $2",
        );
        assert_eq!(query(Driver::MySql, "SELECT ?"), "SELECT ?");
    }

    #[tokio::test]
    async fn any_pool_executes_bound_queries_and_transactions_on_sqlite() {
        let path = std::env::temp_dir().join(format!("yin-any-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        verify_query_contract(
            Driver::Sqlite,
            &format!("sqlite://{}?mode=rwc", path.display()),
        )
        .await;
        std::fs::remove_file(path).expect("remove test database");
    }

    #[tokio::test]
    async fn any_pool_executes_contract_on_configured_mysql_and_postgres() {
        if let Ok(url) = std::env::var("YIN_MYSQL_TEST_URL") {
            verify_query_contract(Driver::MySql, &url).await;
        } else {
            eprintln!("skipping MySQL integration: YIN_MYSQL_TEST_URL is unset");
        }
        if let Ok(url) = std::env::var("YIN_POSTGRES_TEST_URL") {
            verify_query_contract(Driver::Postgres, &url).await;
        } else {
            eprintln!("skipping PostgreSQL integration: YIN_POSTGRES_TEST_URL is unset");
        }
    }
}
