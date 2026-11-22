//! Shared repository reads over the configured SQLx driver.

use sqlx::{AnyPool, Row};

use crate::db::{Driver, query};

/// SQLx Any reports MySQL `TEXT` columns as `Blob`, so string columns decode
/// through bytes when the direct `String` decode is not supported.
pub(crate) fn text(row: &sqlx::any::AnyRow, column: &str) -> Result<String, sqlx::Error> {
    match row.try_get::<String, _>(column) {
        Ok(value) => Ok(value),
        Err(primary) => match row.try_get::<Vec<u8>, _>(column) {
            Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Err(_) => Err(primary),
        },
    }
}


pub(crate) fn text_opt(
    row: &sqlx::any::AnyRow,
    column: &str,
) -> Result<Option<String>, sqlx::Error> {
    match row.try_get::<Option<String>, _>(column) {
        Ok(value) => Ok(value),
        Err(primary) => match row.try_get::<Option<Vec<u8>>, _>(column) {
            Ok(bytes) => Ok(bytes.map(|bytes| String::from_utf8_lossy(&bytes).into_owned())),
            Err(_) => Err(primary),
        },
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRecord {
    pub id: i64,
    pub password: String,
    pub name: String,
    pub status: i64,
    pub role: i64,
    pub mail: String,
    pub token_version: i64,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceListRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub kind: String,
    pub name: String,
    pub owner_user_id: i64,
    pub pair_id: i64,
    pub side: String,
    pub paired_space_id: Option<i64>,
    pub public_enabled: bool,
    pub public_id: Option<String>,
    pub public_mode: String,
    pub can_edit: bool,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceMemberRecord {
    pub id: i64,
    pub space_id: i64,
    pub user_id: i64,
    pub role: String,
    pub source: String,
    pub joined_at: String,
    pub email: String,
    pub name: String,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupListRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub icon: String,
    pub title: String,
    pub description: String,
    pub sort: i64,
    pub user_id: i64,
    pub space_id: i64,
    pub parent_id: Option<i64>,
    pub item_count: i64,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceSearchConfig {
    pub owner_user_id: i64,
    pub config_json: String,
    pub is_member: bool,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemListRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub icon_json: String,
    pub title: String,
    pub url: String,
    pub lan_url: String,
    pub mobile_url: String,
    pub description: String,
    pub open_method: i64,
    pub sort: i64,
    pub item_icon_group_id: i64,
    pub user_id: i64,
    pub space_id: i64,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpacePublicConfig {
    pub enabled: bool,
    pub public_id: Option<String>,
    pub mode: String,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileListRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub file_name: String,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedUserRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub name: String,
    pub head_image: String,
    pub status: i64,
    pub role: i64,
    pub mail: String,
    pub token_version: i64,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserConfigRecord {
    pub user_id: i64,
    pub panel_json: String,
    pub language: String,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkGroupRecord {
    pub id: i64,
    pub title: String,
    pub icon: String,
    pub parent_id: Option<i64>,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkItemRecord {
    pub group_id: i64,
    pub title: String,
    pub url: String,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcGroupRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub provider: String,
    pub group_name: String,
    pub role: String,
}


/// The bind expression a driver needs to accept a Go-style timestamp string.
/// The timestamp form GORM writes: local time, nanosecond fraction, offset.
pub(crate) fn go_timestamp() -> String {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let now = time::OffsetDateTime::now_utc().to_offset(offset);
    let format = time::macros::format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:9][offset_hour sign:mandatory]:[offset_minute]"
    );
    now.format(&format).unwrap_or_default()
}


pub(crate) fn timestamp_bind(driver: Driver) -> &'static str {
    match driver {
        Driver::MySql => "CAST(LEFT(?, 26) AS DATETIME(6))",
        Driver::Sqlite => "?",
        Driver::Postgres => "CAST(? AS TIMESTAMP WITH TIME ZONE)",
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemWriteRecord {
    pub icon_json: String,
    pub title: String,
    pub url: String,
    pub lan_url: String,
    pub mobile_url: String,
    pub description: String,
    pub open_method: i64,
    pub sort: i64,
    pub group_id: i64,
}


/// A row of `o_auth_identity`, the canonical provider/subject link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OauthIdentityRecord {
    pub id: i64,
    pub user_id: i64,
    pub email: String,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcGroupRule {
    pub space_id: i64,
    pub group_name: String,
    pub role: String,
}


fn role_rank(role: &str) -> u8 {
    match role {
        "admin" => 3,
        "editor" => 2,
        "viewer" => 1,
        _ => 0,
    }
}


pub async fn set_member_role(
    pool: &AnyPool,
    driver: Driver,
    space_ids: &[i64],
    target_user_id: i64,
    role: &str,
) -> Result<(), sqlx::Error> {
    let placeholders = vec!["?"; space_ids.len()].join(",");
    let sql = query(
        driver,
        &format!("UPDATE space_member SET role = ? WHERE space_id IN ({placeholders}) AND user_id = ?"),
    );
    let mut statement = sqlx::query(&sql).bind(role);
    for space_id in space_ids {
        statement = statement.bind(*space_id);
    }
    statement.bind(target_user_id).execute(pool).await?;
    Ok(())
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminUserRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub password: String,
    pub name: String,
    pub head_image: String,
    pub status: i64,
    pub role: i64,
    pub mail: String,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePackageRow {
    pub id: String,
    pub name: String,
    pub latest_revision: String,
    pub version: String,
    pub verified: i64,
    pub created_at: String,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeRevisionRow {
    pub id: String,
    pub package_id: String,
    pub version: String,
    pub verified: i64,
    pub created_at: String,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
    pub id: i64,
    pub actor_id: i64,
    pub action: String,
    pub package_id: String,
    pub created_at: String,
}


#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeAdminState {
    pub packages: Vec<ThemePackageRow>,
    pub revisions: Vec<ThemeRevisionRow>,
    pub active_revision: String,
    pub last_good_revision: String,
    pub pending_revision: String,
    pub trial_started_at: Option<String>,
    pub default_package: String,
}


fn bool_expression(driver: Driver, column: &str) -> String {
    match driver {
        Driver::MySql => format!("CAST({column} AS SIGNED)"),
        Driver::Sqlite => format!("CAST({column} AS INTEGER)"),
        Driver::Postgres => format!(
            "CASE WHEN {column} THEN 1 ELSE 0 END"
        ),
    }
}


fn removed_literal(driver: Driver) -> &'static str {
    match driver {
        Driver::Postgres => "false",
        _ => "0",
    }
}


async fn package_removed_tx(
    tx: &mut sqlx::AnyConnection,
    driver: Driver,
    package_id: &str,
) -> Result<Option<i64>, sqlx::Error> {
    use sqlx::Row;
    let sql = query(
        driver,
        &format!(
            "SELECT {} AS removed FROM package_record_v2 WHERE id = ?",
            bool_expression(driver, "removed")
        ),
    );
    let row = sqlx::query(&sql)
        .bind(package_id)
        .fetch_optional(&mut *tx)
        .await?;
    row.map(|row| row.try_get("removed")).transpose()
}


/// `SetDefaultV2` and `BeginTrialV2` share the activation upsert; MySQL spells
/// it `ON DUPLICATE KEY UPDATE`.
fn activation_upsert(driver: Driver, set: &str, values: &str, conflict: &str) -> String {
    match driver {
        Driver::MySql => format!(
            "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at) VALUES ({values}) ON DUPLICATE KEY UPDATE {set}"
        ),
        _ => format!(
            "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at) VALUES ({values}) ON CONFLICT(scope) DO UPDATE SET {conflict}"
        ),
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebWallpaper {
    pub html: Vec<u8>,
    pub poster: Vec<u8>,
    pub poster_type: String,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionParts {
    pub manifest_json: String,
    pub tokens_json: String,
    pub verified: i64,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantRow {
    pub revision_id: String,
    pub execution_mode: String,
    pub permissions_json: String,
}

/// Inserts one row and returns its id on every driver. MySQL reports
/// `last_insert_id`; SQLite and PostgreSQL use `RETURNING`.
macro_rules! insert_returning_id {
    ($driver:expr, $sql:expr, [$($bind:expr),*], $exec:expr) => {{
        let suffix = if $driver == Driver::MySql {
            ""
        } else {
            " RETURNING id"
        };
        let rendered = query($driver, &format!("{}{}", $sql, suffix));
        let mut statement = sqlx::query(&rendered);
        $( statement = statement.bind($bind); )*
        if $driver == Driver::MySql {
            let result = statement.execute($exec).await?;
            result
                .last_insert_id()
                .ok_or(sqlx::Error::Protocol("driver did not report an insert id".into()))?
        } else {
            let row = statement.fetch_one($exec).await?;
            sqlx::Row::try_get::<i64, _>(&row, "id")?
        }
    }};
}

mod space;
mod item;
mod theme;
mod user;
mod file;
mod config;

pub use space::*;
pub use item::*;
pub use theme::*;
pub use user::*;
pub use file::*;
pub use config::*;

#[cfg(test)]
mod tests;
