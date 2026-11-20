//! Runtime configuration, read from the same `conf.yaml` the Go backend uses.
//!
//! Only the fields the ported routes need are modelled; everything else is
//! ignored, so an existing configuration file keeps working unchanged.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Conf {
    #[serde(default)]
    pub base: Base,
    #[serde(default)]
    pub sqlite: Sqlite,
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
        // The recording template carries `__PORT__`; the harness passes the
        // real port in the environment.
        let port = std::env::var("PORT").unwrap_or_default();
        let conf: Conf = std::fs::read_to_string(&path)
            .ok()
            .map(|text| text.replace("__PORT__", &port))
            .and_then(|text| serde_yaml::from_str(&text).ok())
            .unwrap_or_default();
        let dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ctx {
            conf,
            dir,
            pool: tokio::sync::OnceCell::new(),
        }
    })
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

/// The version strings the Go build reports. CI injects them with `-ldflags`;
/// the Rust build reads the same values from the environment at compile time.
pub fn version_name() -> &'static str {
    option_env!("YIN_PANEL_VERSION").unwrap_or("v0.8.2-dev")
}

pub fn build_commit() -> &'static str {
    option_env!("YIN_PANEL_COMMIT").unwrap_or("unknown")
}

pub fn build_time() -> &'static str {
    option_env!("YIN_PANEL_BUILD_TIME").unwrap_or("unknown")
}
