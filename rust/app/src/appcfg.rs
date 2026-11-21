//! Runtime configuration, read from the same `conf.yaml` the Go backend uses.
//!
//! Only the fields the ported routes need are modelled; everything else is
//! ignored, so an existing configuration file keeps working unchanged.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use sqlx::{AnyPool, SqlitePool};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Conf {
    #[serde(default)]
    pub base: Base,
    #[serde(default)]
    pub sqlite: Sqlite,
    #[serde(default)]
    pub mysql: Mysql,
    #[serde(default)]
    pub postgres: Postgres,
    #[serde(default)]
    pub jwt: Jwt,
    #[serde(default)]
    pub oauth: Oauth,
    #[serde(default)]
    pub rclone: Rclone,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Base {
    #[serde(default)]
    pub http_port: String,
    #[serde(default)]
    pub root_url: String,
    #[serde(default)]
    pub database_drive: String,
    #[serde(default)]
    pub enable_static_server: bool,
    #[serde(default)]
    pub enable_monitor: bool,
    #[serde(default)]
    pub monitor_refresh_interval: i64,
    #[serde(default)]
    pub url_prefix: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Sqlite {
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub read_pool_size: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Mysql {
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub db_name: String,
    #[serde(default)]
    pub wait_timeout: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Postgres {
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub db_name: String,
    #[serde(default)]
    pub ssl_mode: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Jwt {
    #[serde(default)]
    pub secret: String,
    #[serde(default)]
    pub expire: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Oauth {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub providers: Vec<OauthProvider>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OauthProvider {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Rclone {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub bucket: String,
}

/// Everything a handler needs, initialised once.
pub struct Ctx {
    pub conf: Conf,
    /// Directory the configuration was read from; relative paths resolve here,
    /// exactly as the Go binary resolves them from its working directory.
    pub dir: PathBuf,
    pool: tokio::sync::OnceCell<SqlitePool>,
    shared_pool: tokio::sync::OnceCell<AnyPool>,
}

static CTX: OnceLock<Ctx> = OnceLock::new();

/// The Go binary reads `conf.yaml` from its working directory. So does this one:
/// an explicit `YIN_PANEL_CONFIG` wins, then `./conf.yaml`. The recording
/// directory is the last resort, for a build started outside its work directory.
fn config_path() -> PathBuf {
    if let Ok(explicit) = std::env::var("YIN_PANEL_CONFIG") {
        return PathBuf::from(explicit);
    }
    let local = PathBuf::from("conf.yaml");
    if local.is_file() {
        return local;
    }
    let work = std::env::var("YIN_PANEL_RECORD_DIR").unwrap_or_else(|_| {
        let tmp = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/yin-panel-record", tmp.trim_end_matches('/'))
    });
    let candidate = Path::new(&work).join("conf.yaml");
    if candidate.is_file() {
        return candidate;
    }
    // Last resort, for a source checkout started from the repository root: the
    // recording configuration. A deployment always has `./conf.yaml`.
    let recorded = Path::new(env!("CARGO_MANIFEST_DIR")).join("../record/conf.yaml");
    if recorded.is_file() {
        return recorded;
    }
    local
}

pub fn ctx() -> &'static Ctx {
    CTX.get_or_init(|| {
        let path = config_path();
        let port = std::env::var("PORT").unwrap_or_default();
        let conf = load_conf(&path, &port)
            .unwrap_or_else(|error| panic!("configuration load failed: {error}"));
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ctx {
            conf,
            dir,
            pool: tokio::sync::OnceCell::new(),
            shared_pool: tokio::sync::OnceCell::new(),
        }
    })
}

fn load_conf(path: &Path, port: &str) -> Result<Conf, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("reading {}: {error}", path.display()))?;
    serde_yaml::from_str(&text.replace("__PORT__", port))
        .map_err(|error| format!("parsing {}: {error}", path.display()))
}

impl Ctx {
    /// The pool is created on first use: during a recording the Go child
    /// creates the database before the first request arrives.
    pub async fn pool(&self) -> Result<&SqlitePool, String> {
        self.pool
            .get_or_try_init(|| async {
                let file = self.database_file();
                let options = sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&file)
                    .create_if_missing(false)
                    .read_only(false);
                sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(4)
                    .connect_with(options)
                    .await
                    .map_err(|error| format!("opening {}: {error}", file.display()))
            })
            .await
    }

    pub async fn shared_pool(&self) -> Result<&AnyPool, String> {
        self.shared_pool
            .get_or_try_init(|| async {
                let (driver, url) = match self.driver() {
                    DatabaseDriver::Sqlite => {
                        let file = self.database_file();
                        if !file.is_file() {
                            return Err(format!(
                                "database file does not exist: {}",
                                file.display()
                            ));
                        }
                        (
                            crate::db::Driver::Sqlite,
                            format!("sqlite://{}", file.display()),
                        )
                    }
                    DatabaseDriver::MySql => (crate::db::Driver::MySql, self.mysql_url()?),
                    DatabaseDriver::Postgres => (crate::db::Driver::Postgres, self.postgres_url()?),
                };
                crate::db::connect(driver, &url)
                    .await
                    .map_err(|error| format!("opening {:?} database: {error}", driver))
            })
            .await
    }

    pub fn driver(&self) -> DatabaseDriver {
        match self
            .conf
            .base
            .database_drive
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "mysql" => DatabaseDriver::MySql,
            "postgres" | "postgresql" => DatabaseDriver::Postgres,
            _ => DatabaseDriver::Sqlite,
        }
    }

    pub fn sqlx_driver(&self) -> crate::db::Driver {
        match self.driver() {
            DatabaseDriver::Sqlite => crate::db::Driver::Sqlite,
            DatabaseDriver::MySql => crate::db::Driver::MySql,
            DatabaseDriver::Postgres => crate::db::Driver::Postgres,
        }
    }

    fn mysql_url(&self) -> Result<String, String> {
        let config = &self.conf.mysql;
        if config.host.is_empty() || config.db_name.is_empty() {
            return Err("mysql.host and mysql.db_name are required".to_string());
        }
        let user = urlencoding::encode(&config.username);
        let password = urlencoding::encode(&config.password);
        Ok(format!(
            "mysql://{user}:{password}@{}:{}/{}",
            config.host,
            if config.port == 0 { 3306 } else { config.port },
            urlencoding::encode(&config.db_name)
        ))
    }

    fn postgres_url(&self) -> Result<String, String> {
        let config = &self.conf.postgres;
        if config.host.is_empty() || config.db_name.is_empty() {
            return Err("postgres.host and postgres.db_name are required".to_string());
        }
        let user = urlencoding::encode(&config.username);
        let password = urlencoding::encode(&config.password);
        let mut url = format!(
            "postgres://{user}:{password}@{}:{}/{}",
            config.host,
            if config.port == 0 { 5432 } else { config.port },
            urlencoding::encode(&config.db_name)
        );
        if !config.ssl_mode.is_empty() {
            url.push_str("?sslmode=");
            url.push_str(&urlencoding::encode(&config.ssl_mode));
        }
        Ok(url)
    }

    pub fn database_file(&self) -> PathBuf {
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

    /// The port to listen on: `PORT` wins (the replay harness sets it), then the
    /// configuration's `base.http_port`, then 8080.
    pub fn listen_port(&self) -> u16 {
        if let Ok(value) = std::env::var("PORT")
            && let Ok(port) = value.parse()
        {
            return port;
        }
        self.conf.base.http_port.parse().unwrap_or(8080)
    }

    pub fn jwt_secret(&self) -> &[u8] {
        self.conf.jwt.secret.as_bytes()
    }

    pub fn jwt_expire_hours(&self) -> i64 {
        if self.conf.jwt.expire > 0 {
            self.conf.jwt.expire
        } else {
            90 * 24
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseDriver {
    Sqlite,
    MySql,
    Postgres,
}

#[cfg(test)]
mod tests {
    use super::{DatabaseDriver, load_conf};

    #[test]
    fn config_expands_recorder_port_and_reads_database_settings() {
        let path = std::env::temp_dir().join(format!("yin-conf-{}-4321.yaml", std::process::id()));
        std::fs::write(
            &path,
            "base:\n  http_port: __PORT__\n  root_url: http://127.0.0.1:__PORT__\nsqlite:\n  file_path: ./data.db\nmysql:\n  host: db.internal\n  port: 3307\n  username: user\n  db_name: yin\npostgres:\n  host: pg.internal\n  port: 5433\n  db_name: yin_pg\n",
        )
        .expect("write temporary config");
        let config = load_conf(&path, "4321").expect("parse config");
        assert_eq!(config.base.http_port, "4321");
        assert_eq!(config.base.root_url, "http://127.0.0.1:4321");
        assert_eq!(config.sqlite.file_path, "./data.db");
        assert_eq!(config.mysql.host, "db.internal");
        assert_eq!(config.mysql.port, 3307);
        assert_eq!(config.postgres.host, "pg.internal");
        assert_eq!(config.postgres.port, 5433);
        let ctx = super::Ctx {
            conf: config,
            dir: std::env::temp_dir(),
            pool: tokio::sync::OnceCell::new(),
            shared_pool: tokio::sync::OnceCell::new(),
        };
        assert_eq!(ctx.driver(), DatabaseDriver::Sqlite);
        assert!(
            ctx.mysql_url()
                .unwrap()
                .starts_with("mysql://user:@db.internal:3307/yin")
        );
        assert!(
            ctx.postgres_url()
                .unwrap()
                .starts_with("postgres://:@pg.internal:5433/yin_pg")
        );
        std::fs::remove_file(path).expect("remove temporary config");
    }

    #[test]
    fn missing_config_is_reported() {
        let path =
            std::env::temp_dir().join(format!("yin-missing-conf-{}.yaml", std::process::id()));
        let error = load_conf(&path, "").expect_err("missing config must fail");
        assert!(error.contains("reading"));
        assert!(error.contains("yin-missing-conf"));
    }

    #[test]
    fn invalid_yaml_is_reported_instead_of_defaulted() {
        let path =
            std::env::temp_dir().join(format!("yin-invalid-conf-{}-4321.yaml", std::process::id()));
        std::fs::write(&path, "base: [unterminated").expect("write invalid config");
        let error = load_conf(&path, "").expect_err("invalid YAML must fail");
        assert!(error.contains("parsing"));
        std::fs::remove_file(path).expect("remove temporary config");
    }

    #[test]
    fn selects_database_driver_case_insensitively() {
        for (value, expected) in [
            ("mysql", DatabaseDriver::MySql),
            ("MySQL", DatabaseDriver::MySql),
            ("postgres", DatabaseDriver::Postgres),
            ("postgresql", DatabaseDriver::Postgres),
            ("sqlite", DatabaseDriver::Sqlite),
        ] {
            let path = std::env::temp_dir().join("unused-config-path");
            let config = super::Conf {
                base: super::Base {
                    database_drive: value.to_string(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let ctx = super::Ctx {
                conf: config,
                dir: path,
                pool: tokio::sync::OnceCell::new(),
                shared_pool: tokio::sync::OnceCell::new(),
            };
            assert_eq!(ctx.driver(), expected);
        }
    }
}

/// The version strings the Go build reports. CI injects them with `-ldflags`;
/// the Rust build reads the same values from the environment at compile time.
/// Build metadata. Go bakes these in with `-ldflags` at build time; reading the
/// environment at start instead keeps one binary valid for both a deployment
/// (which exports them) and a replay (which does not, and must see the defaults
/// the recordings were made with).
fn build_value(key: &str, default: &str) -> &'static str {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    static COMMIT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    static TIME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let slot = match key {
        "YIN_PANEL_VERSION" => &VERSION,
        "YIN_PANEL_COMMIT" => &COMMIT,
        _ => &TIME,
    };
    slot.get_or_init(|| std::env::var(key).unwrap_or_else(|_| default.to_string()))
        .as_str()
}

pub fn version_name() -> &'static str {
    build_value("YIN_PANEL_VERSION", "v0.8.2-dev")
}

pub fn build_commit() -> &'static str {
    build_value("YIN_PANEL_COMMIT", "unknown")
}

pub fn build_time() -> &'static str {
    build_value("YIN_PANEL_BUILD_TIME", "unknown")
}
