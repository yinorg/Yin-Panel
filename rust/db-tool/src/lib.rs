//! Shared pieces of the `yin-db` tool: configuration, the schema upgrade and
//! the database copy.

use std::path::{Path, PathBuf};

use serde::Deserialize;

pub mod copy;
pub mod copy_sql;
pub mod schema;
pub mod upgrade;

#[derive(Debug, Deserialize, Default)]
pub struct Conf {
    #[serde(default)]
    pub base: Base,
    #[serde(default)]
    pub sqlite: Sqlite,
    #[serde(default)]
    pub mysql: Sql,
    #[serde(default)]
    pub postgres: Sql,
}

#[derive(Debug, Deserialize, Default)]
pub struct Base {
    #[serde(default)]
    pub database_drive: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct Sqlite {
    #[serde(default)]
    pub file_path: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct Sql {
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub db_name: String,
    #[serde(default)]
    pub ssl_mode: String,
}

pub struct ConfigFile {
    pub conf: Conf,
    pub dir: PathBuf,
}

impl ConfigFile {
    pub fn driver(&self) -> String {
        if self.conf.base.database_drive.is_empty() {
            "sqlite".to_string()
        } else {
            self.conf.base.database_drive.clone()
        }
    }

    pub fn sqlite_path(&self) -> PathBuf {
        let configured = if self.conf.sqlite.file_path.is_empty() {
            "./database/database.db"
        } else {
            self.conf.sqlite.file_path.as_str()
        };
        let path = Path::new(configured);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.dir.join(path)
        }
    }

    pub fn mysql_url(&self) -> String {
        format!(
            "mysql://{}:{}@{}:{}/{}",
            self.conf.mysql.username,
            self.conf.mysql.password,
            self.conf.mysql.host,
            if self.conf.mysql.port.is_empty() {
                "3306"
            } else {
                &self.conf.mysql.port
            },
            self.conf.mysql.db_name
        )
    }

    pub fn postgres_url(&self) -> String {
        let ssl = if self.conf.postgres.ssl_mode.is_empty() {
            "disable".to_string()
        } else {
            self.conf.postgres.ssl_mode.clone()
        };
        format!(
            "postgres://{}:{}@{}:{}/{}?sslmode={ssl}",
            self.conf.postgres.username,
            self.conf.postgres.password,
            self.conf.postgres.host,
            if self.conf.postgres.port.is_empty() {
                "5432"
            } else {
                &self.conf.postgres.port
            },
            self.conf.postgres.db_name
        )
    }
}

pub fn load_conf(path: &str) -> Result<ConfigFile, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let conf: Conf = serde_yaml::from_str(&text).map_err(|error| format!("{path}: {error}"))?;
    let dir = Path::new(path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(ConfigFile { conf, dir })
}

/// `database.MigrateDB`: copy a SQLite deployment into MySQL/PostgreSQL.
///
/// The Go command validates the source and target configurations before it
/// connects, and its error text is part of what the port must reproduce:
/// `read source config: 配置文件不存在: <path>` and the two driver messages.
pub fn migrate_db(source_path: &str, target_path: &str) -> Result<(), String> {
    #[derive(Default, serde::Deserialize)]
    struct Base {
        #[serde(default)]
        database_drive: String,
    }
    #[derive(Default, serde::Deserialize)]
    struct Conf {
        #[serde(default)]
        base: Base,
    }
    fn read(path: &str) -> Result<Conf, String> {
        if !Path::new(path).exists() {
            return Err(format!("配置文件不存在: {path}"));
        }
        let text = std::fs::read_to_string(path).map_err(|error| match error.raw_os_error() {
            Some(21) => format!("read {path}: is a directory"),
            Some(13) => format!("read {path}: permission denied"),
            Some(2) => format!("read {path}: no such file or directory"),
            _ => format!("read {path}: {error}"),
        })?;
        serde_yaml::from_str(&text).map_err(|error| error.to_string())
    }

    let source = read(source_path).map_err(|error| format!("read source config: {error}"))?;
    if source.base.database_drive != "sqlite" {
        return Err(format!(
            "source must be sqlite, got {:?}",
            source.base.database_drive
        ));
    }
    let target = read(target_path).map_err(|error| format!("read target config: {error}"))?;
    let driver = target.base.database_drive.clone();
    if driver != "mysql" && driver != "postgres" {
        return Err(format!("target must be mysql or postgres, got {driver:?}"));
    }

    let source_conf = load_conf(source_path)?;
    let target_conf = load_conf(target_path)?;
    // `MigrateDB` hands `SQLite.FilePath` straight to the driver, so a relative
    // path resolves against the working directory, not the config file.
    let file = PathBuf::from(&source_conf.conf.sqlite.file_path);
    let file = if file.is_absolute() {
        file
    } else {
        PathBuf::from(".").join(file)
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(async {
        copy_sql::run_with(&source_conf, &file, &target_conf, &driver, true).await?;
        // `CopyDatabase` creates only the tables the models declare, so the
        // migration bookkeeping table is not part of the target; the next start
        // creates it. Drop it to leave the same schema.
        match driver.as_str() {
            "postgres" => {
                let target = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&target_conf.postgres_url())
                    .await
                    .map_err(|error| format!("connecting to PostgreSQL: {error}"))?;
                sqlx::Executor::execute(&target, "DROP TABLE IF EXISTS schema_migration")
                    .await
                    .map_err(|error| error.to_string())?;
                target.close().await;
            }
            "mysql" => {
                let target = sqlx::mysql::MySqlPoolOptions::new()
                    .max_connections(1)
                    .connect(&target_conf.mysql_url())
                    .await
                    .map_err(|error| format!("connecting to MySQL: {error}"))?;
                sqlx::Executor::execute(&target, "DROP TABLE IF EXISTS schema_migration")
                    .await
                    .map_err(|error| error.to_string())?;
                target.close().await;
            }
            _ => {}
        }
        Ok(())
    })
}
