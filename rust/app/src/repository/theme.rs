//! Theme packages, revisions, grants and activation state.

use sqlx::{AnyPool, Row};

use crate::db::{Driver, query};

use super::*;

/// `syncOIDCGroups`: provider groups grant access through the configured rules.
/// It only adds or upgrades `source = 'oidc'` rows, never removes manual access.
pub async fn grant_oidc_member(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    user_id: i64,
    role: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let timestamp = timestamp_bind(driver);
    let select = query(
        driver,
        &format!(
            "SELECT id, CAST(source AS {text_type}) AS source, CAST(role AS {text_type}) AS role FROM space_member WHERE space_id = ? AND user_id = ? LIMIT 1"
        ),
    );
    let insert = query(
        driver,
        &format!(
            "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES ({timestamp}, {timestamp}, ?, ?, ?, 'oidc', {timestamp})"
        ),
    );
    let update = query(driver, "UPDATE space_member SET role = ? WHERE id = ?");
    match sqlx::query(&select)
        .bind(space_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
    {
        None => {
            sqlx::query(&insert)
                .bind(now)
                .bind(now)
                .bind(space_id)
                .bind(user_id)
                .bind(role)
                .bind(now)
                .execute(pool)
                .await?;
        }
        Some(row) => {
            let current_source: String = text(&row, "source")?;
            let current_role: String = text(&row, "role")?;
            if current_source == "oidc" && role_rank(role) > role_rank(&current_role) {
                let id: i64 = row.try_get("id")?;
                sqlx::query(&update)
                    .bind(role)
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
        }
    }
    Ok(())
}

pub async fn user_theme_preference(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Option<(String, String)>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(mode AS {text_type}) AS mode, CAST(theme_mode AS {text_type}) AS theme_mode FROM user_theme_preference_v2 WHERE user_id = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(user_id).fetch_optional(pool).await?;
    row.map(|row| Ok((text(&row, "mode")?, text(&row, "theme_mode")?)))
        .transpose()
}

pub async fn package_id_for_revision(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(package_id AS {text_type}) AS package_id FROM revision_record_v2 WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .fetch_optional(pool)
        .await?;
    row.map(|row| text(&row, "package_id")).transpose()
}

/// The preference upsert: MySQL needs `ON DUPLICATE KEY`, the others use
/// `ON CONFLICT`.
pub async fn save_theme_preference(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    mode: &str,
    theme_mode: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let (sql, has_mode) = match driver {
        Driver::MySql => (
            "INSERT INTO user_theme_preference_v2 (user_id, mode, theme_mode, updated_at) VALUES (?, ?, ?, TS) ON DUPLICATE KEY UPDATE mode = VALUES(mode), theme_mode = VALUES(theme_mode), updated_at = VALUES(updated_at)"
                .replace("TS", timestamp),
            true,
        ),
        _ => (
            "INSERT INTO user_theme_preference_v2 (user_id, mode, theme_mode, updated_at) VALUES (?, ?, ?, TS) ON CONFLICT(user_id) DO UPDATE SET mode = excluded.mode, theme_mode = excluded.theme_mode, updated_at = excluded.updated_at"
                .replace("TS", timestamp),
            true,
        ),
    };
    let _ = has_mode;
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(user_id)
        .bind(mode)
        .bind(theme_mode)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn theme_admin_state(
    pool: &AnyPool,
    driver: Driver,
) -> Result<ThemeAdminState, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let removed = match driver {
        Driver::Postgres => "NOT removed",
        _ => "removed = 0",
    };
    let packages_sql = query(
        driver,
        &format!(
            "SELECT CAST(id AS {text_type}) AS id, CAST(name AS {text_type}) AS name, CAST(latest_revision AS {text_type}) AS latest_revision, (SELECT CAST(version AS {text_type}) FROM revision_record_v2 WHERE id = package_record_v2.latest_revision) AS version, (SELECT {} FROM revision_record_v2 WHERE id = package_record_v2.latest_revision) AS verified, (SELECT CAST(created_at AS {text_type}) FROM revision_record_v2 WHERE id = package_record_v2.latest_revision) AS created_at FROM package_record_v2 WHERE {removed} ORDER BY id",
            bool_expression(driver, "verified")
        ),
    );
    let packages = sqlx::query(&packages_sql)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(ThemePackageRow {
                id: text(&row, "id")?,
                name: text(&row, "name")?,
                latest_revision: text(&row, "latest_revision")?,
                version: text(&row, "version").unwrap_or_default(),
                verified: row.try_get::<i64, _>("verified").unwrap_or(0),
                created_at: text(&row, "created_at").unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

    let revisions_sql = query(
        driver,
        &format!(
            "SELECT CAST(id AS {text_type}) AS id, CAST(package_id AS {text_type}) AS package_id, CAST(version AS {text_type}) AS version, {} AS verified, CAST(created_at AS {text_type}) AS created_at FROM revision_record_v2 ORDER BY package_id, created_at DESC",
            bool_expression(driver, "verified")
        ),
    );
    let revisions = sqlx::query(&revisions_sql)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(ThemeRevisionRow {
                id: text(&row, "id")?,
                package_id: text(&row, "package_id")?,
                version: text(&row, "version")?,
                verified: row.try_get("verified")?,
                created_at: text(&row, "created_at")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

    let activation_sql = query(
        driver,
        &format!(
            "SELECT CAST(active_revision_id AS {text_type}) AS active_revision_id, CAST(last_good_revision_id AS {text_type}) AS last_good_revision_id, CAST(pending_revision_id AS {text_type}) AS pending_revision_id, CAST(trial_started_at AS {text_type}) AS trial_started_at FROM activation_record_v2 WHERE scope = 'instance'"
        ),
    );
    let activation = sqlx::query(&activation_sql).fetch_optional(pool).await?;
    let mut state = ThemeAdminState {
        packages,
        revisions,
        ..Default::default()
    };
    if let Some(row) = activation {
        state.active_revision = text(&row, "active_revision_id").unwrap_or_default();
        state.last_good_revision = text(&row, "last_good_revision_id").unwrap_or_default();
        state.pending_revision = text(&row, "pending_revision_id").unwrap_or_default();
        state.trial_started_at = text_opt(&row, "trial_started_at").unwrap_or_default();
    }
    if state.active_revision.is_empty() {
        let latest_sql = query(
            driver,
            &format!(
                "SELECT CAST(latest_revision AS {text_type}) AS latest_revision FROM package_record_v2 WHERE id = 'org.yin.default' AND {removed}"
            ),
        );
        let latest = sqlx::query(&latest_sql).fetch_optional(pool).await?;
        state.active_revision = latest
            .map(|row| text(&row, "latest_revision").unwrap_or_default())
            .unwrap_or_default();
    }
    let package_sql = query(
        driver,
        &format!(
            "SELECT CAST(package_id AS {text_type}) AS package_id FROM revision_record_v2 WHERE id = ?"
        ),
    );
    state.default_package = sqlx::query(&package_sql)
        .bind(&state.active_revision)
        .fetch_optional(pool)
        .await?
        .map(|row| text(&row, "package_id").unwrap_or_default())
        .unwrap_or_default();
    Ok(state)
}

pub async fn audit_entries(
    pool: &AnyPool,
    driver: Driver,
) -> Result<Vec<AuditRow>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    // The recorded contract expects the newest 200, and ties to come out in
    // reverse insertion order; `id DESC` makes that deterministic.
    let sql = query(
        driver,
        &format!(
            "SELECT id, actor_id, CAST(action AS {text_type}) AS action, CAST(package_id AS {text_type}) AS package_id, CAST(created_at AS {text_type}) AS created_at FROM audit_record ORDER BY created_at DESC, id DESC LIMIT 200"
        ),
    );
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(AuditRow {
                id: row.try_get("id")?,
                actor_id: row.try_get("actor_id")?,
                action: text(&row, "action")?,
                package_id: text(&row, "package_id")?,
                created_at: text(&row, "created_at")?,
            })
        })
        .collect()
}

pub async fn revision_manifest(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(manifest_json AS {text_type}) AS manifest_json FROM revision_record_v2 WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .fetch_optional(pool)
        .await?;
    row.map(|row| text(&row, "manifest_json")).transpose()
}

pub async fn trusted_enabled(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<bool>, sqlx::Error> {
    let sql = query(
        driver,
        &format!(
            "SELECT {} AS enabled FROM trusted_runtime_policy_v2 WHERE revision_id = ?",
            bool_expression(driver, "enabled")
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .fetch_optional(pool)
        .await?;
    row.map(|row| Ok(row.try_get::<i64, _>("enabled")? != 0))
        .transpose()
}

pub async fn save_trusted_policy(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
    enabled: bool,
    user_id: i64,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = match driver {
        Driver::MySql => format!(
            "INSERT INTO trusted_runtime_policy_v2 (revision_id, enabled, updated_by, updated_at) VALUES (?, ?, ?, {timestamp}) ON DUPLICATE KEY UPDATE enabled = VALUES(enabled), updated_by = VALUES(updated_by), updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO trusted_runtime_policy_v2 (revision_id, enabled, updated_by, updated_at) VALUES (?, ?, ?, {timestamp}) ON CONFLICT(revision_id) DO UPDATE SET enabled = excluded.enabled, updated_by = excluded.updated_by, updated_at = excluded.updated_at"
        ),
    };
    let sql = query(driver, &sql);
    let mut statement = sqlx::query(&sql).bind(revision);
    statement = if driver == Driver::Postgres {
        statement.bind(enabled)
    } else {
        statement.bind(i64::from(enabled))
    };
    statement.bind(user_id).bind(now).execute(pool).await?;
    Ok(())
}

pub async fn withdraw_trusted_grants(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<(), sqlx::Error> {
    let sql = query(
        driver,
        "DELETE FROM grant_record_v2 WHERE revision_id = ? AND execution_mode = 'trusted'",
    );
    sqlx::query(&sql).bind(revision).execute(pool).await?;
    Ok(())
}

pub async fn latest_revision(
    pool: &AnyPool,
    driver: Driver,
    package_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let removed = match driver {
        Driver::Postgres => "NOT removed",
        _ => "removed = 0",
    };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(latest_revision AS {text_type}) AS latest_revision FROM package_record_v2 WHERE id = ? AND {removed}"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(package_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| text(&row, "latest_revision")).transpose()
}

pub async fn activation_revision(
    pool: &AnyPool,
    driver: Driver,
    scope: &str,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(active_revision_id AS {text_type}) AS active_revision_id FROM activation_record_v2 WHERE scope = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(scope).fetch_optional(pool).await?;
    Ok(row
        .map(|row| text(&row, "active_revision_id").unwrap_or_default())
        .filter(|value| !value.is_empty()))
}

/// `ThemeRouter.SelectV2`: the user activation record, the preference and the
/// audit row, with a MySQL-compatible upsert on both keys.
pub async fn select_user_theme(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    package_id: &str,
    revision_id: &str,
    mode: &str,
    now: &str,
) -> Result<(), String> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let removed = match driver {
        Driver::Postgres => "NOT removed",
        _ => "removed = 0",
    };
    let package_sql = query(
        driver,
        &format!(
            "SELECT 1 AS found FROM package_record_v2 WHERE id = ? AND {removed}"
        ),
    );
    let package = sqlx::query(&package_sql)
        .bind(package_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| error.to_string())?;
    if package.is_none() {
        return Err("record not found".to_string());
    }
    let revision_sql = query(
        driver,
        "SELECT 1 AS found FROM revision_record_v2 WHERE id = ? AND package_id = ?",
    );
    let revision = sqlx::query(&revision_sql)
        .bind(revision_id)
        .bind(package_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| error.to_string())?;
    if revision.is_none() {
        return Err("record not found".to_string());
    }

    let timestamp = timestamp_bind(driver);
    let scope = format!("user:{user_id}");
    let activation = match driver {
        Driver::MySql => format!(
            "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at) VALUES (?, ?, ?, ?, '', NULL, {timestamp}) ON DUPLICATE KEY UPDATE package_id = VALUES(package_id), active_revision_id = VALUES(active_revision_id), last_good_revision_id = VALUES(last_good_revision_id), pending_revision_id = '', trial_started_at = NULL, updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at) VALUES (?, ?, ?, ?, '', NULL, {timestamp}) ON CONFLICT(scope) DO UPDATE SET package_id = excluded.package_id, active_revision_id = excluded.active_revision_id, last_good_revision_id = excluded.last_good_revision_id, pending_revision_id = '', trial_started_at = NULL, updated_at = excluded.updated_at"
        ),
    };
    let activation = query(driver, &activation);
    sqlx::query(&activation)
        .bind(&scope)
        .bind(package_id)
        .bind(revision_id)
        .bind(revision_id)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;

    let preference = match driver {
        Driver::MySql => format!(
            "INSERT INTO user_theme_preference_v2 (user_id, mode, theme_mode, updated_at) VALUES (?, ?, ?, {timestamp}) ON DUPLICATE KEY UPDATE mode = VALUES(mode), theme_mode = VALUES(theme_mode), updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO user_theme_preference_v2 (user_id, mode, theme_mode, updated_at) VALUES (?, ?, ?, {timestamp}) ON CONFLICT(user_id) DO UPDATE SET mode = excluded.mode, theme_mode = excluded.theme_mode, updated_at = excluded.updated_at"
        ),
    };
    let preference = query(driver, &preference);
    sqlx::query(&preference)
        .bind(user_id)
        .bind(mode)
        .bind("custom")
        .bind(now)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;
    let _ = text_type;

    let audit = query(
        driver,
        &format!(
            "INSERT INTO audit_record (actor_id, action, package_id, created_at) VALUES (?, 'select', ?, {timestamp})"
        ),
    );
    sqlx::query(&audit)
        .bind(user_id)
        .bind(package_id)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub async fn revision_package_id(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<String>, sqlx::Error> {
    package_id_for_revision(pool, driver, revision).await
}

pub async fn package_removed(
    pool: &AnyPool,
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
        .fetch_optional(pool)
        .await?;
    row.map(|row| row.try_get("removed")).transpose()
}

/// `InstallPackageV2`.
pub async fn install_package(
    pool: &AnyPool,
    driver: Driver,
    actor_id: i64,
    package: &crate::packages::Package,
) -> Result<(), String> {
    let manifest = &package.manifest;
    let manifest_json = serde_json::to_string(manifest).map_err(|error| error.to_string())?;
    let tokens_json = serde_json::to_string(&package.tokens).map_err(|error| error.to_string())?;
    let now = go_timestamp();
    let removed = removed_literal(driver);
    let timestamp = timestamp_bind(driver);
    let mut tx = pool.begin().await.map_err(|error| error.to_string())?;

    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let existing_sql = query(
        driver,
        &format!(
            "SELECT CAST(id AS {text_type}) AS id FROM revision_record_v2 WHERE package_id = ? AND version = ?"
        ),
    );
    let existing = sqlx::query(&existing_sql)
        .bind(&manifest.id)
        .bind(&manifest.version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| error.to_string())?;
    let existing_id: Option<String> = existing.map(|row| text(&row, "id").unwrap_or_default());

    if let Some(existing_id) = existing_id {
        if existing_id != package.revision {
            return Err("same theme package version has different content".to_string());
        }
        let update = query(
            driver,
            &format!(
                "UPDATE package_record_v2 SET name = ?, latest_revision = ?, removed = {removed} WHERE id = ?"
            ),
        );
        sqlx::query(&update)
            .bind(&manifest.name)
            .bind(&package.revision)
            .bind(&manifest.id)
            .execute(&mut *tx)
            .await
            .map_err(|_| "install failed".to_string())?;
    } else {
        let record = package_removed_tx(&mut tx, driver, &manifest.id)
            .await
            .map_err(|error| error.to_string())?;
        match record {
            None => {
                let insert = query(
                    driver,
                    &format!(
                        "INSERT INTO package_record_v2 (id, name, latest_revision, removed, created_at, updated_at) VALUES (?, ?, '', {removed}, {timestamp}, {timestamp})"
                    ),
                );
                sqlx::query(&insert)
                    .bind(&manifest.id)
                    .bind(&manifest.name)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            Some(removed_flag) if removed_flag != 0 => {
                return Err(
                    "removed theme packages must be explicitly restored before installation"
                        .to_string(),
                );
            }
            Some(_) => {}
        }
        let insert_revision = query(
            driver,
            &format!(
                "INSERT INTO revision_record_v2 (id, package_id, version, manifest_json, tokens_json, verified, created_at) VALUES (?, ?, ?, ?, ?, ?, {timestamp})"
            ),
        );
        let mut statement = sqlx::query(&insert_revision)
            .bind(&package.revision)
            .bind(&manifest.id)
            .bind(&manifest.version)
            .bind(&manifest_json)
            .bind(&tokens_json);
        statement = if driver == Driver::Postgres {
            statement.bind(package.verified)
        } else {
            statement.bind(i64::from(package.verified))
        };
        statement
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
        for (name, (media_type, content)) in &package.files {
            let insert_asset = query(
                driver,
                "INSERT INTO asset_record_v2 (revision_id, path, media_type, content) VALUES (?, ?, ?, ?)",
            );
            sqlx::query(&insert_asset)
                .bind(&package.revision)
                .bind(name)
                .bind(media_type)
                .bind(content)
                .execute(&mut *tx)
                .await
                .map_err(|error| error.to_string())?;
        }
        let update = query(
            driver,
            &format!(
                "UPDATE package_record_v2 SET name = ?, latest_revision = ?, removed = {removed} WHERE id = ?"
            ),
        );
        sqlx::query(&update)
            .bind(&manifest.name)
            .bind(&package.revision)
            .bind(&manifest.id)
            .execute(&mut *tx)
            .await
            .map_err(|_| "install failed".to_string())?;
        let audit = query(
            driver,
            &format!(
                "INSERT INTO audit_record (actor_id, action, package_id, created_at) VALUES (?, 'install-v2', ?, {timestamp})"
            ),
        );
        sqlx::query(&audit)
            .bind(actor_id)
            .bind(&manifest.id)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
    }
    tx.commit().await.map_err(|error| error.to_string())
}

pub async fn remove_theme_package(
    pool: &AnyPool,
    driver: Driver,
    actor_id: i64,
    package_id: &str,
) -> Result<(), String> {
    let now = go_timestamp();
    let removed = removed_literal(driver);
    let one = match driver {
        Driver::Postgres => "true",
        _ => "1",
    };
    let timestamp = timestamp_bind(driver);
    let mut tx = pool.begin().await.map_err(|error| error.to_string())?;
    let update_package = query(
        driver,
        &format!(
            "UPDATE package_record_v2 SET removed = {one}, updated_at = {timestamp} WHERE id = ?"
        ),
    );
    sqlx::query(&update_package)
        .bind(&now)
        .bind(package_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| "remove failed".to_string())?;
    let reset = query(
        driver,
        &format!(
            "UPDATE activation_record_v2 SET package_id = 'org.yin.default', active_revision_id = ?, last_good_revision_id = ?, pending_revision_id = '', trial_started_at = NULL, updated_at = {timestamp} WHERE package_id = ?"
        ),
    );
    let yin = latest_revision(pool, driver, "org.yin.default")
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "default package missing".to_string())?;
    sqlx::query(&reset)
        .bind(&yin)
        .bind(&yin)
        .bind(&now)
        .bind(package_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| "remove failed".to_string())?;
    let audit = query(
        driver,
        &format!(
            "INSERT INTO audit_record (actor_id, action, package_id, created_at) VALUES (?, 'remove-v2', ?, {timestamp})"
        ),
    );
    sqlx::query(&audit)
        .bind(actor_id)
        .bind(package_id)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|_| "remove failed".to_string())?;
    let _ = removed;
    tx.commit().await.map_err(|error| error.to_string())
}

pub async fn set_default_theme(
    pool: &AnyPool,
    driver: Driver,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = activation_upsert(
        driver,
        "package_id = VALUES(package_id), active_revision_id = VALUES(active_revision_id), last_good_revision_id = VALUES(last_good_revision_id), pending_revision_id = '', trial_started_at = NULL, updated_at = VALUES(updated_at)",
        &format!("'instance', ?, ?, ?, '', NULL, {timestamp}"),
        "package_id = excluded.package_id, active_revision_id = excluded.active_revision_id, last_good_revision_id = excluded.last_good_revision_id, pending_revision_id = '', trial_started_at = NULL, updated_at = excluded.updated_at",
    );
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(package_id)
        .bind(revision_id)
        .bind(revision_id)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn begin_theme_trial(
    pool: &AnyPool,
    driver: Driver,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = activation_upsert(
        driver,
        "pending_revision_id = VALUES(pending_revision_id), trial_started_at = VALUES(trial_started_at), updated_at = VALUES(updated_at)",
        &format!("'instance', ?, '', '', ?, ?, {timestamp}"),
        "pending_revision_id = excluded.pending_revision_id, trial_started_at = excluded.trial_started_at, updated_at = excluded.updated_at",
    );
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(package_id)
        .bind(revision_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn confirm_theme_trial(
    pool: &AnyPool,
    driver: Driver,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let activate = query(
        driver,
        &format!(
            "UPDATE activation_record_v2 SET package_id = ?, active_revision_id = ?, pending_revision_id = '', trial_started_at = NULL, updated_at = {timestamp} WHERE scope = 'instance'"
        ),
    );
    let healthy = query(
        driver,
        &format!(
            "UPDATE activation_record_v2 SET last_good_revision_id = ?, updated_at = {timestamp} WHERE scope = 'instance' AND active_revision_id = ? AND pending_revision_id = ''"
        ),
    );
    let mut tx = pool.begin().await?;
    sqlx::query(&activate)
        .bind(package_id)
        .bind(revision_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query(&healthy)
        .bind(revision_id)
        .bind(now)
        .bind(revision_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn rollback_theme(
    pool: &AnyPool,
    driver: Driver,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = query(
        driver,
        &format!(
            "UPDATE activation_record_v2 SET package_id = ?, active_revision_id = ?, pending_revision_id = '', trial_started_at = NULL, updated_at = {timestamp} WHERE scope = 'instance'"
        ),
    );
    sqlx::query(&sql)
        .bind(package_id)
        .bind(revision_id)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn instance_last_good(
    pool: &AnyPool,
    driver: Driver,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(last_good_revision_id AS {text_type}) AS last_good_revision_id FROM activation_record_v2 WHERE scope = 'instance'"
        ),
    );
    let row = sqlx::query(&sql).fetch_optional(pool).await?;
    row.map(|row| text(&row, "last_good_revision_id")).transpose()
}

pub async fn instance_pending(
    pool: &AnyPool,
    driver: Driver,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(pending_revision_id AS {text_type}) AS pending_revision_id FROM activation_record_v2 WHERE scope = 'instance'"
        ),
    );
    let row = sqlx::query(&sql).fetch_optional(pool).await?;
    row.map(|row| text(&row, "pending_revision_id")).transpose()
}

pub async fn revision_parts(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<RevisionParts>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(manifest_json AS {text_type}) AS manifest_json, CAST(tokens_json AS {text_type}) AS tokens_json, {} AS verified FROM revision_record_v2 WHERE id = ?",
            bool_expression(driver, "verified")
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(RevisionParts {
            manifest_json: text(&row, "manifest_json")?,
            tokens_json: text(&row, "tokens_json")?,
            verified: row.try_get("verified")?,
        })
    })
    .transpose()
}

pub async fn activation_package(
    pool: &AnyPool,
    driver: Driver,
    scope: &str,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(package_id AS {text_type}) AS package_id FROM activation_record_v2 WHERE scope = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(scope).fetch_optional(pool).await?;
    row.map(|row| text(&row, "package_id")).transpose()
}

pub async fn user_space_theme_revision(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(revision_id AS {text_type}) AS revision_id FROM user_space_theme_preference_v2 WHERE user_id = ? AND space_id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| text(&row, "revision_id")).transpose()
}

pub async fn space_theme_revision(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(revision_id AS {text_type}) AS revision_id FROM space_theme_preference_v2 WHERE space_id = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(space_id).fetch_optional(pool).await?;
    row.map(|row| text(&row, "revision_id")).transpose()
}

pub async fn space_theme_pair(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<Option<(String, String)>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(package_id AS {text_type}) AS package_id, CAST(revision_id AS {text_type}) AS revision_id FROM space_theme_preference_v2 WHERE space_id = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(space_id).fetch_optional(pool).await?;
    row.map(|row| Ok((text(&row, "package_id")?, text(&row, "revision_id")?)))
        .transpose()
}

pub async fn user_space_theme_pair(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
) -> Result<Option<(String, String)>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(package_id AS {text_type}) AS package_id, CAST(revision_id AS {text_type}) AS revision_id FROM user_space_theme_preference_v2 WHERE user_id = ? AND space_id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .bind(space_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| Ok((text(&row, "package_id")?, text(&row, "revision_id")?)))
        .transpose()
}

pub async fn theme_asset_content(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
    path: &str,
) -> Result<Option<(String, Vec<u8>)>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(media_type AS {text_type}) AS media_type, content FROM asset_record_v2 WHERE revision_id = ? AND path = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .bind(path)
        .fetch_optional(pool)
        .await?;
    row.map(|row| Ok((text(&row, "media_type")?, row.try_get("content")?)))
        .transpose()
}

pub async fn grant_row(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    revision: &str,
    execution_mode: &str,
) -> Result<Option<GrantRow>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(revision_id AS {text_type}) AS revision_id, CAST(execution_mode AS {text_type}) AS execution_mode, CAST(permissions_json AS {text_type}) AS permissions_json FROM grant_record_v2 WHERE user_id = ? AND revision_id = ? AND execution_mode = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .bind(revision)
        .bind(execution_mode)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(GrantRow {
            revision_id: text(&row, "revision_id")?,
            execution_mode: text(&row, "execution_mode")?,
            permissions_json: text(&row, "permissions_json")?,
        })
    })
    .transpose()
}

pub async fn save_grant(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    revision: &str,
    execution_mode: &str,
    permissions_json: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = match driver {
        Driver::MySql => format!(
            "INSERT INTO grant_record_v2 (user_id, revision_id, execution_mode, permissions_json, updated_at) VALUES (?, ?, ?, ?, {timestamp}) ON DUPLICATE KEY UPDATE permissions_json = VALUES(permissions_json), updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO grant_record_v2 (user_id, revision_id, execution_mode, permissions_json, updated_at) VALUES (?, ?, ?, ?, {timestamp}) ON CONFLICT(user_id, revision_id, execution_mode) DO UPDATE SET permissions_json = excluded.permissions_json, updated_at = excluded.updated_at"
        ),
    };
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(user_id)
        .bind(revision)
        .bind(execution_mode)
        .bind(permissions_json)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_grant(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    revision: &str,
    execution_mode: &str,
) -> Result<(), sqlx::Error> {
    let sql = query(
        driver,
        "DELETE FROM grant_record_v2 WHERE user_id = ? AND revision_id = ? AND execution_mode = ?",
    );
    sqlx::query(&sql)
        .bind(user_id)
        .bind(revision)
        .bind(execution_mode)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn revision_verified_manifest(
    pool: &AnyPool,
    driver: Driver,
    revision: &str,
) -> Result<Option<(String, i64)>, sqlx::Error> {
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(manifest_json AS {text_type}) AS manifest_json, {} AS verified FROM revision_record_v2 WHERE id = ?",
            bool_expression(driver, "verified")
        ),
    );
    let row = sqlx::query(&sql)
        .bind(revision)
        .fetch_optional(pool)
        .await?;
    row.map(|row| Ok((text(&row, "manifest_json")?, row.try_get("verified")?)))
        .transpose()
}

pub async fn save_space_theme(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = match driver {
        Driver::MySql => format!(
            "INSERT INTO space_theme_preference_v2 (space_id, package_id, revision_id, updated_by, updated_at) VALUES (?, ?, ?, 0, {timestamp}) ON DUPLICATE KEY UPDATE package_id = VALUES(package_id), revision_id = VALUES(revision_id), updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO space_theme_preference_v2 (space_id, package_id, revision_id, updated_by, updated_at) VALUES (?, ?, ?, 0, {timestamp}) ON CONFLICT(space_id) DO UPDATE SET package_id = excluded.package_id, revision_id = excluded.revision_id, updated_at = excluded.updated_at"
        ),
    };
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(space_id)
        .bind(package_id)
        .bind(revision_id)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn save_user_space_theme(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
    package_id: &str,
    revision_id: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let sql = match driver {
        Driver::MySql => format!(
            "INSERT INTO user_space_theme_preference_v2 (user_id, space_id, package_id, revision_id, updated_at) VALUES (?, ?, ?, ?, {timestamp}) ON DUPLICATE KEY UPDATE package_id = VALUES(package_id), revision_id = VALUES(revision_id), updated_at = VALUES(updated_at)"
        ),
        _ => format!(
            "INSERT INTO user_space_theme_preference_v2 (user_id, space_id, package_id, revision_id, updated_at) VALUES (?, ?, ?, ?, {timestamp}) ON CONFLICT(user_id, space_id) DO UPDATE SET package_id = excluded.package_id, revision_id = excluded.revision_id, updated_at = excluded.updated_at"
        ),
    };
    let sql = query(driver, &sql);
    sqlx::query(&sql)
        .bind(user_id)
        .bind(space_id)
        .bind(package_id)
        .bind(revision_id)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn clear_space_theme_row(
    pool: &AnyPool,
    driver: Driver,
    space_id: i64,
) -> Result<(), sqlx::Error> {
    let sql = query(
        driver,
        "DELETE FROM space_theme_preference_v2 WHERE space_id = ?",
    );
    sqlx::query(&sql).bind(space_id).execute(pool).await?;
    Ok(())
}

pub async fn clear_user_space_theme_row(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_id: i64,
) -> Result<(), sqlx::Error> {
    let sql = query(
        driver,
        "DELETE FROM user_space_theme_preference_v2 WHERE user_id = ? AND space_id = ?",
    );
    sqlx::query(&sql)
        .bind(user_id)
        .bind(space_id)
        .execute(pool)
        .await?;
    Ok(())
}
