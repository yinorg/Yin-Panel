//! Shared pieces of the `yin-db` tool: configuration, the schema upgrade and
//! the database copy.

use std::path::{Path, PathBuf};

use serde::Deserialize;

pub mod copy;
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
