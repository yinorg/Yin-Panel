//! Accounts, sessions, OAuth identities and OIDC group rules.

use sqlx::{AnyPool, Row};

use crate::db::{Driver, query};

use super::*;

pub async fn user_list(
    pool: &AnyPool,
    driver: Driver,
    limit: i64,
    page: i64,
) -> Result<(i64, Vec<ManagedUserRecord>), sqlx::Error> {
    let (user_table, text_type) = if driver == Driver::MySql {
        ("`user`", "CHAR")
    } else {
        ("\"user\"", "TEXT")
    };
    let count: i64 = sqlx::query(&query(
        driver,
        &format!("SELECT COUNT(*) AS total FROM {user_table}"),
    ))
    .fetch_one(pool)
    .await?
    .try_get("total")?;
    let rows = if limit > 0 {
        let offset = limit.saturating_mul(page.max(1) - 1);
        let sql = query(
            driver,
            &format!(
                "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST(name AS {text_type}) AS name, CAST(head_image AS {text_type}) AS head_image, status, role, CAST(mail AS {text_type}) AS mail, token_version FROM {user_table} ORDER BY id LIMIT ? OFFSET ?"
            ),
        );
        sqlx::query(&sql)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
    } else {
        Vec::new()
    };
    let rows = rows
        .into_iter()
        .map(|row| {
            Ok(ManagedUserRecord {
                id: row.try_get("id")?,
                created_at: text(&row, "created_at")?,
                updated_at: text(&row, "updated_at")?,
                name: text(&row, "name")?,
                head_image: text(&row, "head_image")?,
                status: row.try_get("status")?,
                role: row.try_get("role")?,
                mail: text(&row, "mail")?,
                token_version: row.try_get("token_version")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    Ok((count, rows))
}

pub async fn user_config(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Option<UserConfigRecord>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT user_id, CAST(panel_json AS {text_type}) AS panel_json, CAST(language AS {text_type}) AS language FROM user_config WHERE user_id = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(user_id).fetch_optional(pool).await?;
    row.map(|row| {
        Ok(UserConfigRecord {
            user_id: row.try_get("user_id")?,
            panel_json: text(&row, "panel_json")?,
            language: text(&row, "language")?,
        })
    })
    .transpose()
}

pub async fn user_id_by_mail(
    pool: &AnyPool,
    driver: Driver,
    mail: &str,
) -> Result<Option<i64>, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let sql = query(driver, &format!("SELECT id FROM {user_table} WHERE mail = ?"));
    let row = sqlx::query(&sql)
        .bind(mail)
        .fetch_optional(pool)
        .await?;
    row.map(|row| row.try_get("id")).transpose()
}

pub async fn oauth_identity(
    pool: &AnyPool,
    driver: Driver,
    provider: &str,
    subject: &str,
) -> Result<Option<OauthIdentityRecord>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, user_id, COALESCE(CAST(email AS {text_type}), '') AS email FROM o_auth_identity WHERE provider = ? AND subject = ? LIMIT 1"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(provider)
        .bind(subject)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(OauthIdentityRecord {
            id: row.try_get("id")?,
            user_id: row.try_get("user_id")?,
            email: text(&row, "email")?,
        })
    })
    .transpose()
}

/// `FirstOrCreate` with `Assign`: an existing provider/subject row is pointed at
/// the resolved account, a new one is inserted.
pub async fn link_oauth_identity(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    provider: &str,
    subject: &str,
    email: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let timestamp = timestamp_bind(driver);
    let update = query(
        driver,
        &format!("UPDATE o_auth_identity SET updated_at = {timestamp}, user_id = ?, email = ? WHERE id = ?"),
    );
    let insert = query(
        driver,
        &format!(
            "INSERT INTO o_auth_identity (created_at, updated_at, user_id, provider, subject, email) VALUES ({timestamp}, {timestamp}, ?, ?, ?, ?)"
        ),
    );
    let existing = oauth_identity(pool, driver, provider, subject).await?;
    match existing {
        Some(identity) => {
            sqlx::query(&update)
                .bind(now)
                .bind(user_id)
                .bind(email)
                .bind(identity.id)
                .execute(pool)
                .await?;
        }
        None => {
            sqlx::query(&insert)
                .bind(now)
                .bind(now)
                .bind(user_id)
                .bind(provider)
                .bind(subject)
                .bind(email)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

pub async fn user_by_mail(
    pool: &AnyPool,
    driver: Driver,
    mail: &str,
) -> Result<Option<UserRecord>, sqlx::Error> {
    let table = match driver {
        Driver::MySql => "`user`",
        Driver::Sqlite | Driver::Postgres => "\"user\"",
    };
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(password AS {text_type}) AS password, CAST(name AS {text_type}) AS name, status, role, CAST(mail AS {text_type}) AS mail, token_version FROM {table} WHERE mail = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(mail).fetch_optional(pool).await?;
    row.map(|row| {
        Ok(UserRecord {
            id: row.try_get("id")?,
            password: text(&row, "password")?,
            name: text(&row, "name")?,
            status: row.try_get("status")?,
            role: row.try_get("role")?,
            mail: text(&row, "mail")?,
            token_version: row.try_get("token_version")?,
        })
    })
    .transpose()
}

pub async fn oidc_group_rules(
    pool: &AnyPool,
    driver: Driver,
    provider: &str,
) -> Result<Vec<OidcGroupRule>, sqlx::Error> {
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT space_id, CAST(group_name AS {text_type}) AS group_name, CAST(role AS {text_type}) AS role FROM space_o_id_c_group WHERE provider = ? ORDER BY id"
        ),
    );
    let rows = sqlx::query(&sql).bind(provider).fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(OidcGroupRule {
                space_id: row.try_get("space_id")?,
                group_name: text(&row, "group_name")?,
                role: text(&row, "role")?,
            })
        })
        .collect()
}

pub async fn save_user_config(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    panel_json: Option<&str>,
    language: &str,
) -> Result<(), sqlx::Error> {
    let select = query(
        driver,
        "SELECT 1 AS found FROM user_config WHERE user_id = ? LIMIT 1",
    );
    let existing = sqlx::query(&select)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    let found = existing.is_some();
    if found {
        match panel_json {
            Some(panel) => {
                let update = query(
                    driver,
                    "UPDATE user_config SET panel_json = ?, language = ? WHERE user_id = ?",
                );
                sqlx::query(&update)
                    .bind(panel)
                    .bind(language)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            }
            None => {
                let update = query(
                    driver,
                    "UPDATE user_config SET language = ? WHERE user_id = ?",
                );
                sqlx::query(&update)
                    .bind(language)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            }
        }
    } else {
        let insert = query(
            driver,
            "INSERT INTO user_config (user_id, panel_json, language) VALUES (?, ?, ?)",
        );
        sqlx::query(&insert)
            .bind(user_id)
            .bind(panel_json.unwrap_or(""))
            .bind(language)
            .execute(pool)
            .await?;
    }
    Ok(())
}


pub async fn user_mail_exists(
    pool: &AnyPool,
    driver: Driver,
    mail: &str,
    exclude_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let sql = match exclude_id {
        None => query(driver, &format!("SELECT 1 FROM {user_table} WHERE mail = ? LIMIT 1")),
        Some(_) => query(
            driver,
            &format!("SELECT 1 FROM {user_table} WHERE mail = ? AND id <> ? LIMIT 1"),
        ),
    };
    let mut statement = sqlx::query(&sql).bind(mail);
    if let Some(id) = exclude_id {
        statement = statement.bind(id);
    }
    Ok(statement.fetch_optional(pool).await?.is_some())
}

/// `UsersRouter.Create`: the account, its personal Yin/Yang pair, both admin
/// rows and both default groups in one transaction.
pub async fn create_user(
    pool: &AnyPool,
    driver: Driver,
    mail: &str,
    password_hash: &str,
    name: &str,
    head_image: &str,
    role: i64,
    now: &str,
) -> Result<i64, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let space_table = crate::schema_shared::quote(driver, "space");
    let timestamp = timestamp_bind(driver);
    let enabled = if driver == Driver::Postgres { "FALSE" } else { "0" };
    let pair_update = query(
        driver,
        &format!("UPDATE {space_table} SET pair_id = ? WHERE id = ?"),
    );
    let insert_member = query(
        driver,
        &format!(
            "INSERT INTO space_member (created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES ({timestamp}, {timestamp}, ?, ?, 'admin', 'manual', {timestamp})"
        ),
    );
    let insert_group = query(
        driver,
        &format!(
            "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id) VALUES ({timestamp}, {timestamp}, 'material-symbols:ad-group-outline', 'APP', '', 0, ?, ?)"
        ),
    );

    let mut tx = pool.begin().await?;
    let id = insert_returning_id!(
        driver,
        &format!(
            "INSERT INTO {user_table} (created_at, updated_at, password, name, head_image, status, role, mail, token_version) VALUES ({timestamp}, {timestamp}, ?, ?, ?, 1, ?, ?, 0)"
        ),
        [now, now, password_hash, name, head_image, role, mail],
        &mut *tx
    );
    let yin_id = insert_returning_id!(
        driver,
        &format!(
            "INSERT INTO {space_table} (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES ({timestamp}, {timestamp}, 'personal', ?, ?, 0, 'yin', {enabled}, 'direct')"
        ),
        [now, now, name, id],
        &mut *tx
    );
    sqlx::query(&pair_update)
        .bind(yin_id)
        .bind(yin_id)
        .execute(&mut *tx)
        .await?;
    let yang_name = format!("{name}-B");
    let yang_id = insert_returning_id!(
        driver,
        &format!(
            "INSERT INTO {space_table} (created_at, updated_at, type, name, owner_user_id, pair_id, side, public_enabled, public_mode) VALUES ({timestamp}, {timestamp}, 'personal', ?, ?, ?, 'yang', {enabled}, 'direct')"
        ),
        [now, now, yang_name, id, yin_id],
        &mut *tx
    );
    let _ = &insert_group;
    for space_id in [yin_id, yang_id] {
        sqlx::query(&insert_member)
            .bind(now)
            .bind(now)
            .bind(space_id)
            .bind(id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        sqlx::query(&insert_group)
            .bind(now)
            .bind(now)
            .bind(id)
            .bind(space_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(id)
}

pub async fn user_password_and_name(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Option<(String, String)>, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT CAST(password AS {text_type}) AS password, CAST(name AS {text_type}) AS name FROM {user_table} WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| Ok((text(&row, "password")?, text(&row, "name")?)))
        .transpose()
}

/// `UsersRouter.Update`: GORM writes only the non-zero fields, so the SET list
/// follows the submitted values.
#[allow(clippy::too_many_arguments)]
pub async fn update_user_admin(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    mail: &str,
    name: Option<&str>,
    head_image: Option<&str>,
    status: Option<i64>,
    role: Option<i64>,
    password_hash: Option<&str>,
    now: &str,
    rename_pair: Option<(&str, &str)>,
) -> Result<(), sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let timestamp = timestamp_bind(driver);
    let mut columns = vec!["mail = ?".to_string()];
    if name.is_some() {
        columns.push("name = ?".to_string());
    }
    if head_image.is_some() {
        columns.push("head_image = ?".to_string());
    }
    if status.is_some() {
        columns.push("status = ?".to_string());
    }
    if role.is_some() {
        columns.push("role = ?".to_string());
    }
    if password_hash.is_some() {
        columns.push("password = ?".to_string());
    }
    columns.push(format!("updated_at = {timestamp}"));
    let sql = query(
        driver,
        &format!(
            "UPDATE {user_table} SET {} WHERE id = ?",
            columns.join(", ")
        ),
    );
    let mut statement = sqlx::query(&sql).bind(mail);
    if let Some(value) = name {
        statement = statement.bind(value);
    }
    if let Some(value) = head_image {
        statement = statement.bind(value);
    }
    if let Some(value) = status {
        statement = statement.bind(value);
    }
    if let Some(value) = role {
        statement = statement.bind(value);
    }
    if let Some(value) = password_hash {
        statement = statement.bind(value);
    }
    statement.bind(now).bind(user_id).execute(pool).await?;

    if let Some((yin, yang)) = rename_pair {
        let space_table = crate::schema_shared::quote(driver, "space");
        let rename = query(
            driver,
            &format!(
                "UPDATE {space_table} SET name = ?, updated_at = {timestamp} WHERE owner_user_id = ? AND type = 'personal' AND side = ?"
            ),
        );
        sqlx::query(&rename)
            .bind(yin)
            .bind(now)
            .bind(user_id)
            .bind("yin")
            .execute(pool)
            .await?;
        sqlx::query(&rename)
            .bind(yang)
            .bind(now)
            .bind(user_id)
            .bind("yang")
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn admin_user_view(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Option<AdminUserRecord>, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(created_at AS {text_type}) AS created_at, CAST(updated_at AS {text_type}) AS updated_at, CAST(password AS {text_type}) AS password, CAST(name AS {text_type}) AS name, CAST(head_image AS {text_type}) AS head_image, status, role, CAST(mail AS {text_type}) AS mail FROM {user_table} WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(AdminUserRecord {
            id: row.try_get("id")?,
            created_at: text(&row, "created_at")?,
            updated_at: text(&row, "updated_at")?,
            password: text(&row, "password")?,
            name: text(&row, "name")?,
            head_image: text(&row, "head_image")?,
            status: row.try_get("status")?,
            role: row.try_get("role")?,
            mail: text(&row, "mail")?,
        })
    })
    .transpose()
}

/// `UserRepo.DeleteWithSpaces`: the owned Spaces, the user rows that carry its
/// ownership, then the account, in one transaction.
pub async fn delete_user(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    space_ids: &[i64],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    if !space_ids.is_empty() {
        let placeholders = vec!["?"; space_ids.len()].join(",");
        for table in [
            "item_icon",
            "item_icon_group",
            "space_member",
            "space_o_id_c_group",
        ] {
            let sql = query(
                driver,
                &format!("DELETE FROM {table} WHERE space_id IN ({placeholders})"),
            );
            let mut statement = sqlx::query(&sql);
            for id in space_ids {
                statement = statement.bind(*id);
            }
            statement.execute(&mut *tx).await?;
        }
        let space_table = crate::schema_shared::quote(driver, "space");
        let sql = query(
            driver,
            &format!("DELETE FROM {space_table} WHERE id IN ({placeholders})"),
        );
        let mut statement = sqlx::query(&sql);
        for id in space_ids {
            statement = statement.bind(*id);
        }
        statement.execute(&mut *tx).await?;
    }
    for table in [
        "o_auth_identity",
        "item_icon",
        "item_icon_group",
        "module_config",
        "file",
    ] {
        let sql = query(driver, &format!("DELETE FROM {table} WHERE user_id = ?"));
        sqlx::query(&sql)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }
    let user_table = crate::schema_shared::quote(driver, "user");
    let sql = query(driver, &format!("DELETE FROM {user_table} WHERE id = ?"));
    sqlx::query(&sql)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// `UserRouter.UpdateInfo`: the profile and, when the nickname changed, the
/// personal pair that carries it.
pub async fn update_user_profile(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    name: &str,
    head_image: &str,
    now: &str,
) -> Result<(), sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let timestamp = timestamp_bind(driver);
    let update = query(
        driver,
        &format!(
            "UPDATE {user_table} SET name = ?, head_image = ?, updated_at = {timestamp} WHERE id = ?"
        ),
    );
    let mut tx = pool.begin().await?;
    sqlx::query(&update)
        .bind(name)
        .bind(head_image)
        .bind(now)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let space_table = crate::schema_shared::quote(driver, "space");
    let rename = query(
        driver,
        &format!(
            "UPDATE {space_table} SET name = ?, updated_at = {timestamp} WHERE type = 'personal' AND owner_user_id = ? AND side = ?"
        ),
    );
    sqlx::query(&rename)
        .bind(name)
        .bind(now)
        .bind(user_id)
        .bind("yin")
        .execute(&mut *tx)
        .await?;
    let yang_name = format!("{name}-B");
    sqlx::query(&rename)
        .bind(yang_name)
        .bind(now)
        .bind(user_id)
        .bind("yang")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// `UserRouter.UpdatePasssword`: verify the old hash, store the new one and
/// bump `token_version` so every issued token is revoked.
pub async fn update_user_password(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
    old_hash: &str,
    new_hash: &str,
    now: &str,
) -> Result<bool, sqlx::Error> {
    let user_table = crate::schema_shared::quote(driver, "user");
    let timestamp = timestamp_bind(driver);
    let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
    let sql = query(
        driver,
        &format!(
            "UPDATE {user_table} SET password = ?, token_version = token_version + 1, updated_at = {timestamp} WHERE id = ? AND CAST(password AS {text_type}) = ?"
        ),
    );
    let result = sqlx::query(&sql)
        .bind(new_hash)
        .bind(now)
        .bind(user_id)
        .bind(old_hash)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn user_by_id(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<Option<UserRecord>, sqlx::Error> {
    let table = match driver {
        Driver::MySql => "`user`",
        Driver::Sqlite | Driver::Postgres => "\"user\"",
    };
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(password AS {text_type}) AS password, CAST(name AS {text_type}) AS name, status, role, CAST(mail AS {text_type}) AS mail, token_version FROM {table} WHERE id = ?"
        ),
    );
    let row = sqlx::query(&sql).bind(user_id).fetch_optional(pool).await?;
    row.map(|row| {
        Ok(UserRecord {
            id: row.try_get("id")?,
            password: text(&row, "password")?,
            name: text(&row, "name")?,
            status: row.try_get("status")?,
            role: row.try_get("role")?,
            mail: text(&row, "mail")?,
            token_version: row.try_get("token_version")?,
        })
    })
    .transpose()
}

pub async fn user_by_credentials(
    pool: &AnyPool,
    driver: Driver,
    mail: &str,
    password: &str,
) -> Result<Option<UserRecord>, sqlx::Error> {
    let table = match driver {
        Driver::MySql => "`user`",
        Driver::Sqlite | Driver::Postgres => "\"user\"",
    };
    let text_type = if driver == Driver::MySql {
        "CHAR"
    } else {
        "TEXT"
    };
    let sql = query(
        driver,
        &format!(
            "SELECT id, CAST(password AS {text_type}) AS password, CAST(name AS {text_type}) AS name, status, role, CAST(mail AS {text_type}) AS mail, token_version FROM {table} WHERE mail = ? AND password = ?"
        ),
    );
    let row = sqlx::query(&sql)
        .bind(mail)
        .bind(password)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(UserRecord {
            id: row.try_get("id")?,
            password: text(&row, "password")?,
            name: text(&row, "name")?,
            status: row.try_get("status")?,
            role: row.try_get("role")?,
            mail: text(&row, "mail")?,
            token_version: row.try_get("token_version")?,
        })
    })
    .transpose()
}

pub async fn revoke_user_sessions(
    pool: &AnyPool,
    driver: Driver,
    user_id: i64,
) -> Result<(), sqlx::Error> {
    let table = match driver {
        Driver::MySql => "`user`",
        Driver::Sqlite | Driver::Postgres => "\"user\"",
    };
    let sql = query(
        driver,
        &format!("UPDATE {table} SET token_version = token_version + 1 WHERE id = ?"),
    );
    sqlx::query(&sql).bind(user_id).execute(pool).await?;
    Ok(())
}
