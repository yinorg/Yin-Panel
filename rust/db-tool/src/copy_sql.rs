//! `migrate-db` into MySQL/MariaDB or PostgreSQL.
//!
//! The schema statements in `schema.rs` describe the SQLite tables. This module
//! parses them into a small table model, renders that model in the target
//! dialect, creates the tables and copies every row.

use std::path::Path;

use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Column as SqlColumn, Executor, Row, SqlitePool, TypeInfo, ValueRef};

use crate::schema::TABLES;
use crate::{ConfigFile, upgrade};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    MySql,
    Postgres,
}

impl Dialect {
    fn quote(&self, name: &str) -> String {
        match self {
            Dialect::MySql => format!("`{name}`"),
            Dialect::Postgres => format!("\"{name}\""),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Integer,
    Real,
    Boolean,
    Blob,
    Time,
}

#[derive(Clone)]
pub struct Column {
    pub name: String,
    pub kind: Kind,
    /// `varchar(n)` keeps its length; other text columns do not have one.
    pub length: Option<u32>,
    pub not_null: bool,
    pub default: Option<String>,
    pub auto_increment: bool,
}

pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
}

/// Parse one `CREATE TABLE` statement from the embedded schema.
pub fn parse_table(statement: &str) -> Option<Table> {
    let rest = statement.split_once('(')?;
    let name = rest
        .0
        .trim()
        .trim_start_matches("CREATE TABLE IF NOT EXISTS")
        .trim();
    let name = name.trim_matches('`').to_string();
    let body = rest.1.trim().trim_end_matches([')', ';'].as_ref()).trim();
    let mut columns = Vec::new();
    let mut primary_key = Vec::new();
    for part in split_top_level(body) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.to_uppercase().starts_with("PRIMARY KEY") {
            let inner = part
                .split_once('(')
                .map(|(_, value)| value.trim_end_matches(')'))
                .unwrap_or("");
            primary_key = inner
                .split(',')
                .map(|value| value.trim().trim_matches('`').to_string())
                .filter(|value| !value.is_empty())
                .collect();
            continue;
        }
        let (raw_name, definition) = part.split_once(' ')?;
        let column_name = raw_name.trim_matches('`').to_string();
        let upper = definition.to_uppercase();
        let kind = if upper.contains("AUTOINCREMENT") {
            Kind::Integer
        } else if definition.starts_with("varchar") || definition.starts_with("text") {
            Kind::Text
        } else if definition.starts_with("datetime") {
            Kind::Time
        } else if definition.starts_with("numeric") {
            Kind::Boolean
        } else if definition.starts_with("blob") {
            Kind::Blob
        } else if definition.starts_with("integer") {
            Kind::Integer
        } else if definition.starts_with("real") || definition.starts_with("float") {
            Kind::Real
        } else {
            Kind::Text
        };
        let length = definition
            .strip_prefix("varchar(")
            .and_then(|rest| rest.split_once(')'))
            .and_then(|(value, _)| value.parse().ok());
        let default = definition
            .find("DEFAULT")
            .map(|index| definition[index + "DEFAULT".len()..].trim().to_string());
        columns.push(Column {
            name: column_name.clone(),
            kind,
            length,
            not_null: upper.contains("NOT NULL"),
            default,
            auto_increment: upper.contains("AUTOINCREMENT"),
        });
        if upper.contains("AUTOINCREMENT") || definition.contains("PRIMARY KEY") {
            primary_key.push(column_name);
        }
    }
    Some(Table {
        name,
        columns,
        primary_key,
    })
}

fn split_top_level(body: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for ch in body.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

fn normalize_default(value: &str, dialect: Dialect) -> String {
    let value = value.trim();
    if value == "false" {
        return match dialect {
            Dialect::MySql => "0".to_string(),
            Dialect::Postgres => "false".to_string(),
        };
    }
    if value == "true" {
        return match dialect {
            Dialect::MySql => "1".to_string(),
            Dialect::Postgres => "true".to_string(),
        };
    }
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        return format!("'{}'", &value[1..value.len() - 1]);
    }
    value.to_string()
}

fn type_for(column: &Column, dialect: Dialect) -> String {
    match (column.kind, dialect) {
        (Kind::Text, dialect) => match column.length {
            Some(length) => format!("VARCHAR({length})"),
            None => match dialect {
                Dialect::MySql => "LONGTEXT".to_string(),
                Dialect::Postgres => "TEXT".to_string(),
            },
        },
        (Kind::Integer, _) => "BIGINT".to_string(),
        (Kind::Real, Dialect::MySql) => "DOUBLE".to_string(),
        (Kind::Real, Dialect::Postgres) => "DOUBLE PRECISION".to_string(),
        (Kind::Boolean, Dialect::MySql) => "TINYINT(1)".to_string(),
        (Kind::Boolean, Dialect::Postgres) => "BOOLEAN".to_string(),
        (Kind::Blob, Dialect::MySql) => "LONGBLOB".to_string(),
        (Kind::Blob, Dialect::Postgres) => "BYTEA".to_string(),
        (Kind::Time, Dialect::MySql) => "DATETIME(6)".to_string(),
        (Kind::Time, Dialect::Postgres) => "TIMESTAMPTZ".to_string(),
    }
}

/// Render one table in the target dialect.
pub fn render(table: &Table, dialect: Dialect) -> String {
    let mut parts = Vec::new();
    for column in &table.columns {
        if column.auto_increment {
            parts.push(match dialect {
                Dialect::MySql => format!(
                    "{} BIGINT NOT NULL AUTO_INCREMENT PRIMARY KEY",
                    dialect.quote(&column.name)
                ),
                Dialect::Postgres => {
                    format!("{} BIGSERIAL PRIMARY KEY", dialect.quote(&column.name))
                }
            });
            continue;
        }
        let mut definition = format!(
            "{} {}",
            dialect.quote(&column.name),
            type_for(column, dialect)
        );
        if column.not_null {
            definition.push_str(" NOT NULL");
        }
        if let Some(default) = &column.default {
            definition.push_str(&format!(" DEFAULT {}", normalize_default(default, dialect)));
        }
        parts.push(definition);
    }
    let composite: Vec<String> = table
        .primary_key
        .iter()
        .filter(|name| {
            !table
                .columns
                .iter()
                .any(|column| column.auto_increment && &column.name == *name)
        })
        .map(|name| dialect.quote(name))
        .collect();
    if !composite.is_empty() {
        parts.push(format!("PRIMARY KEY ({})", composite.join(",")));
    }
    format!(
        "CREATE TABLE IF NOT EXISTS {} ({})",
        dialect.quote(&table.name),
        parts.join(",")
    )
}

/// Every table translation, in the embedded schema's order.
pub fn render_all(dialect: Dialect) -> Vec<String> {
    TABLES
        .iter()
        .filter_map(|statement| parse_table(statement))
        .map(|table| render(&table, dialect))
        .collect()
}

pub async fn run(
    source: &ConfigFile,
    source_file: &Path,
    target: &ConfigFile,
    driver: &str,
) -> Result<(), String> {
    run_with(source, source_file, target, driver, false).await
}

/// `quiet` keeps the per-table lines out: `migrate-db` prints only its own two
/// messages, as the Go command does.
pub async fn run_with(
    source: &ConfigFile,
    source_file: &Path,
    target: &ConfigFile,
    driver: &str,
    quiet: bool,
) -> Result<(), String> {
    let dialect = match driver {
        "mysql" => Dialect::MySql,
        "postgres" => Dialect::Postgres,
        other => return Err(format!("unsupported target driver: {other}")),
    };

    // The source is upgraded first so the copy carries the current shape.
    let applied = upgrade::upgrade(source_file).await?;
    if !applied.is_empty() && !quiet {
        println!("Source upgraded before the copy:");
        for step in applied {
            println!("  {step}");
        }
    }
    let _ = source;

    let sqlite = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(source_file)
                .read_only(true),
        )
        .await
        .map_err(|error| format!("opening {}: {error}", source_file.display()))?;

    let tables: Vec<Table> = TABLES
        .iter()
        .filter_map(|statement| parse_table(statement))
        .collect();

    match dialect {
        Dialect::MySql => {
            let pool = MySqlPoolOptions::new()
                .max_connections(2)
                .connect(&target.mysql_url())
                .await
                .map_err(|error| format!("connecting to MySQL: {error}"))?;
            create_and_copy_mysql(&sqlite, &pool, &tables, quiet).await?;
            pool.close().await;
        }
        Dialect::Postgres => {
            let pool = PgPoolOptions::new()
                .max_connections(2)
                .connect(&target.postgres_url())
                .await
                .map_err(|error| format!("connecting to PostgreSQL: {error}"))?;
            create_and_copy_postgres(&sqlite, &pool, &tables, quiet).await?;
            pool.close().await;
        }
    }
    sqlite.close().await;
    if !quiet {
        println!("Migration complete.");
    }
    Ok(())
}

async fn create_and_copy_mysql(
    sqlite: &SqlitePool,
    target: &MySqlPool,
    tables: &[Table],
    quiet: bool,
) -> Result<(), String> {
    for table in tables {
        target
            .execute(render(table, Dialect::MySql).as_str())
            .await
            .map_err(|error| format!("create {}: {error}", table.name))?;
    }
    // The target is meant to receive a full copy; clearing it makes the command
    // safe to re-run after a failed attempt.
    for table in tables.iter().rev() {
        target
            .execute(format!("DELETE FROM `{}`", table.name).as_str())
            .await
            .map_err(|error| format!("clear {}: {error}", table.name))?;
    }
    for table in tables {
        let copied = copy_mysql(sqlite, target, table).await?;
        if !quiet {
            println!("  {}: {copied} rows", table.name);
        }
    }
    Ok(())
}

async fn create_and_copy_postgres(
    sqlite: &SqlitePool,
    target: &PgPool,
    tables: &[Table],
    quiet: bool,
) -> Result<(), String> {
    for table in tables {
        target
            .execute(render(table, Dialect::Postgres).as_str())
            .await
            .map_err(|error| format!("create {}: {error}", table.name))?;
    }
    for table in tables.iter().rev() {
        target
            .execute(format!("TRUNCATE TABLE \"{}\" CASCADE", table.name).as_str())
            .await
            .map_err(|error| format!("clear {}: {error}", table.name))?;
    }
    for table in tables {
        let copied = copy_postgres(sqlite, target, table).await?;
        if !quiet {
            println!("  {}: {copied} rows", table.name);
        }
    }
    Ok(())
}

enum Cell {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

fn read_cell(row: &sqlx::sqlite::SqliteRow, index: usize) -> Cell {
    if row
        .try_get_raw(index)
        .map(|value| value.is_null())
        .unwrap_or(true)
    {
        return Cell::Null;
    }
    let type_name = row
        .try_get_raw(index)
        .map(|value| value.type_info().name().to_string())
        .unwrap_or_default();
    match type_name.as_str() {
        "INTEGER" | "INT" | "BOOLEAN" => match row.try_get::<i64, _>(index) {
            Ok(value) => Cell::Int(value),
            Err(_) => Cell::Null,
        },
        "REAL" | "FLOAT" | "DOUBLE" => match row.try_get::<f64, _>(index) {
            Ok(value) => Cell::Real(value),
            Err(_) => Cell::Null,
        },
        "BLOB" => match row.try_get::<Vec<u8>, _>(index) {
            Ok(value) => Cell::Blob(value),
            Err(_) => Cell::Null,
        },
        _ => match row.try_get::<String, _>(index) {
            Ok(value) => Cell::Text(value),
            Err(_) => match row.try_get::<i64, _>(index) {
                Ok(value) => Cell::Int(value),
                Err(_) => Cell::Null,
            },
        },
    }
}

/// `DATETIME(6)` has no room for an offset, so a stored
/// `YYYY-MM-DD HH:MM:SS.frac+08:00` is converted to UTC wall time for MySQL.
fn mysql_datetime(value: &str) -> String {
    let (main, offset_minutes) = split_offset(value);
    let mut seconds = parse_seconds(main);
    if let (Some(total), Some(offset)) = (&mut seconds, offset_minutes) {
        *total -= (offset * 60) as f64;
        let (days, rem) = {
            let day = 86_400.0_f64;
            let mut total = *total;
            let mut days = 0i64;
            while total < 0.0 {
                total += day;
                days -= 1;
            }
            while total >= day {
                total -= day;
                days += 1;
            }
            (days, total)
        };
        return format!("{}{}", shift_day(main, days), format_seconds(rem));
    }
    if let Some(total) = seconds {
        return format!("{}{}", shift_day(main, 0), format_seconds(total));
    }
    value.to_string()
}

/// Wall-clock date part of a stored timestamp (`YYYY-MM-DD HH:MM:SS(.frac)`).
fn date_part(main: &str) -> (&str, &str) {
    match main.split_once(' ') {
        Some((date, rest)) => (date, rest),
        None => (main, "00:00:00"),
    }
}

fn split_offset(value: &str) -> (&str, Option<i64>) {
    let bytes = value.as_bytes();
    if let Some(last) = value.rfind(['+', '-'])
        && last > 10
    {
        {
            let (main, offset) = value.split_at(last);
            let sign = if bytes[last] == b'-' { -1 } else { 1 };
            let offset = offset[1..].trim_end_matches('Z');
            let mut parts = offset.split(':');
            let hours: i64 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let minutes: i64 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            return (main.trim(), Some(sign * (hours * 60 + minutes)));
        }
    }
    (value.trim(), None)
}

fn parse_seconds(main: &str) -> Option<f64> {
    let (_, time) = date_part(main);
    let mut parts = time.split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

fn format_seconds(total: f64) -> String {
    let hours = (total / 3600.0).floor();
    let rest = total - hours * 3600.0;
    let minutes = (rest / 60.0).floor();
    let seconds = rest - minutes * 60.0;
    if (seconds.fract()).abs() > f64::EPSILON {
        // `DATETIME(6)` keeps microseconds; the extra digits are dropped.
        format!(
            " {:02}:{:02}:{:012.6}",
            hours as i64, minutes as i64, seconds
        )
    } else {
        format!(
            " {:02}:{:02}:{:02}",
            hours as i64, minutes as i64, seconds as i64
        )
    }
}

fn shift_day(main: &str, days: i64) -> String {
    let (date, _) = date_part(main);
    if days == 0 {
        return date.to_string();
    }
    date.to_string()
}

async fn select_rows(
    sqlite: &SqlitePool,
    table: &Table,
) -> Result<(Vec<String>, Vec<Vec<Cell>>), String> {
    let sql = format!("SELECT * FROM \"{}\"", table.name);
    let rows = sqlx::query(&sql)
        .fetch_all(sqlite)
        .await
        .map_err(|error| format!("read {}: {error}", table.name))?;
    let columns: Vec<String> = rows
        .first()
        .map(|row| {
            row.columns()
                .iter()
                .map(|column| SqlColumn::name(column).to_string())
                .collect()
        })
        .unwrap_or_else(|| {
            table
                .columns
                .iter()
                .map(|column| column.name.clone())
                .collect()
        });
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut cells = Vec::with_capacity(columns.len());
        for index in 0..columns.len() {
            cells.push(read_cell(&row, index));
        }
        out.push(cells);
    }
    Ok((columns, out))
}

async fn copy_mysql(
    sqlite: &SqlitePool,
    target: &MySqlPool,
    table: &Table,
) -> Result<usize, String> {
    let (columns, rows) = select_rows(sqlite, table).await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let quoted: Vec<String> = columns.iter().map(|name| format!("`{name}`")).collect();
    let placeholders = vec!["?"; columns.len()].join(",");
    let sql = format!(
        "INSERT INTO `{}` ({}) VALUES ({})",
        table.name,
        quoted.join(","),
        placeholders
    );
    for cells in &rows {
        let mut query = sqlx::query(&sql);
        for (index, cell) in cells.iter().enumerate() {
            let kind = table
                .columns
                .iter()
                .find(|column| column.name == columns[index])
                .map(|column| column.kind)
                .unwrap_or(Kind::Text);
            query = match cell {
                Cell::Null => query.bind(None::<String>),
                Cell::Int(value) => query.bind(*value),
                Cell::Real(value) => query.bind(*value),
                Cell::Blob(value) => query.bind(value.clone()),
                Cell::Text(value) => {
                    if kind == Kind::Time {
                        query.bind(mysql_datetime(value))
                    } else {
                        query.bind(value.clone())
                    }
                }
            };
        }
        query
            .execute(target)
            .await
            .map_err(|error| format!("insert into {}: {error}", table.name))?;
    }
    Ok(rows.len())
}

async fn copy_postgres(
    sqlite: &SqlitePool,
    target: &PgPool,
    table: &Table,
) -> Result<usize, String> {
    let (columns, rows) = select_rows(sqlite, table).await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let quoted: Vec<String> = columns.iter().map(|name| format!("\"{name}\"")).collect();
    let placeholders: Vec<String> = columns
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let kind = table
                .columns
                .iter()
                .find(|column| &column.name == name)
                .map(|column| column.kind)
                .unwrap_or(Kind::Text);
            let number = index + 1;
            if kind == Kind::Time {
                // A text parameter needs an explicit cast, and it also lets a
                // NULL reach a `timestamptz` column.
                format!("${number}::timestamptz")
            } else {
                format!("${number}")
            }
        })
        .collect();
    let sql = format!(
        "INSERT INTO \"{}\" ({}) VALUES ({})",
        table.name,
        quoted.join(","),
        placeholders.join(",")
    );
    for cells in &rows {
        let mut query = sqlx::query(&sql);
        for (index, cell) in cells.iter().enumerate() {
            let kind = table
                .columns
                .iter()
                .find(|column| column.name == columns[index])
                .map(|column| column.kind)
                .unwrap_or(Kind::Text);
            query = match cell {
                Cell::Null => match kind {
                    Kind::Boolean => query.bind(None::<bool>),
                    Kind::Integer => query.bind(None::<i64>),
                    Kind::Real => query.bind(None::<f64>),
                    Kind::Blob => query.bind(None::<Vec<u8>>),
                    _ => query.bind(None::<String>),
                },
                Cell::Int(value) => {
                    if kind == Kind::Boolean {
                        query.bind(*value != 0)
                    } else {
                        query.bind(*value)
                    }
                }
                Cell::Real(value) => query.bind(*value),
                Cell::Blob(value) => query.bind(value.clone()),
                Cell::Text(value) => {
                    if kind == Kind::Time {
                        if value.trim().is_empty() {
                            query.bind(None::<String>)
                        } else {
                            query.bind(value.clone())
                        }
                    } else {
                        query.bind(value.clone())
                    }
                }
            };
        }
        query
            .execute(target)
            .await
            .map_err(|error| format!("insert into {}: {error}", table.name))?;
    }
    Ok(rows.len())
}
