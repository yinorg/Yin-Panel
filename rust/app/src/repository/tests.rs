    use super::{
        activation_revision, add_oidc_group, admin_user_view, audit_entries, begin_theme_trial,
        bookmark_tree,
        arrange_layout, can_access_space, can_edit_space, can_manage_space,
        clear_space_theme_row, clear_user_space_theme_row, confirm_theme_trial,
        create_item_with_icon, delete_oidc_group,
        copy_space, clear_space, create_group, create_item, create_paired_space, create_user,
        delete_grant,
        delete_group_tree,
        delete_file_row, delete_item, delete_paired_space, delete_user, file_list,
        file_name_by_id,
        group_belongs, group_by_id,
        grant_row, group_has_descendant, group_siblings, groups_for_space, insert_file,
        item_by_id, items_for_space,
        grant_oidc_member, link_oauth_identity, module_config_value, oauth_identity,
        oidc_group_rules, oidc_groups, paired_ids_of, paired_space_ids, remove_space_member,
        rename_paired_space, revoke_user_sessions, save_module_config, save_user_config,
        set_member_role, spaces_owned_by,
        save_space_theme, save_user_space_theme, sort_groups, sort_items, space_list,
        space_members, space_name, space_owner, space_theme_pair, space_theme_revision,
        space_owner_and_type, space_public_config, space_search_config, space_wallpaper_json,
        transfer_space,
        update_group, update_item, update_space_public_config, update_space_search_config,
        update_space_wallpaper, upsert_space_member, user_by_credentials, user_by_id,
        user_space_theme_pair, user_space_theme_revision,
        user_config, user_id_by_mail, user_list, user_mail_exists, user_password_and_name,
        install_package, instance_last_good, instance_pending, latest_revision,
        revision_verified_manifest, save_grant,
        package_id_for_revision, package_removed, remove_theme_package, revision_manifest,
        rollback_theme, save_theme_preference,
        insert_web_wallpaper, save_trusted_policy, select_user_theme, set_default_theme,
        theme_admin_state, trusted_enabled, web_wallpaper, web_wallpaper_exists,
        update_user_admin, update_user_password, update_user_profile, user_theme_preference,
        withdraw_trusted_grants,
    };
    use super::timestamp_bind;
    use crate::db::{Driver, connect, query};
    use sqlx::Row;

    async fn verify_user_repository(driver: Driver, url: &str) {
        let pool = connect(driver, url)
            .await
            .unwrap_or_else(|error| panic!("connect {driver:?} test database: {error}"));
        let table = match driver {
            Driver::MySql => "`user`",
            Driver::Sqlite | Driver::Postgres => "\"user\"",
        };
        sqlx::query(&format!(
            "CREATE TABLE {table} (id BIGINT PRIMARY KEY, password VARCHAR(255) NOT NULL, name VARCHAR(100) NOT NULL, status BIGINT NOT NULL, role BIGINT NOT NULL, mail VARCHAR(255) NOT NULL, token_version BIGINT NOT NULL)"
        ))
        .execute(&pool)
        .await
        .expect("create user fixture");
        let insert = query(
            driver,
            &format!(
                "INSERT INTO {table} (id, password, name, status, role, mail, token_version) VALUES (?, ?, ?, ?, ?, ?, ?)"
            ),
        );
        sqlx::query(&insert)
            .bind(17_i64)
            .bind("hash-value")
            .bind("Test User")
            .bind(1_i64)
            .bind(2_i64)
            .bind("user@example.test")
            .bind(9_i64)
            .execute(&pool)
            .await
            .expect("insert user fixture");

        assert_eq!(
            user_by_id(&pool, driver, 17).await.expect("read full user"),
            Some(super::UserRecord {
                id: 17,
                password: "hash-value".to_string(),
                name: "Test User".to_string(),
                status: 1,
                role: 2,
                mail: "user@example.test".to_string(),
                token_version: 9,
            })
        );
        assert_eq!(
            user_by_credentials(&pool, driver, "user@example.test", "hash-value")
                .await
                .expect("lookup login credentials")
                .expect("matching credentials")
                .id,
            17
        );
        assert!(
            user_by_credentials(&pool, driver, "user@example.test", "wrong-password")
                .await
                .expect("lookup wrong credentials")
                .is_none()
        );
        let claims = crate::auth::Claims {
            user_id: 17,
            token_version: 9,
            sub: "user_access".to_string(),
            exp: 1,
            nbf: 1,
            iat: 1,
        };
        assert_eq!(
            crate::auth::user_from_pool(&pool, driver, &claims)
                .await
                .expect("authenticate user from shared pool")
                .mail,
            "user@example.test"
        );
        let stale_claims = crate::auth::Claims {
            token_version: 8,
            ..claims
        };
        assert_eq!(
            crate::auth::user_from_pool(&pool, driver, &stale_claims).await,
            Err(crate::auth::AuthError::NotLoggedIn)
        );
        revoke_user_sessions(&pool, driver, 17)
            .await
            .expect("revoke user sessions");
        assert_eq!(
            user_by_id(&pool, driver, 17)
                .await
                .expect("read user after logout")
                .expect("user remains")
                .token_version,
            10
        );

        sqlx::query(&format!("DROP TABLE {table}"))
            .execute(&pool)
            .await
            .expect("drop user fixture");

        verify_space_list(&pool, driver).await;
        verify_space_members(&pool, driver).await;
        verify_groups(&pool, driver).await;
        verify_item_list(&pool, driver).await;
        verify_search_config(&pool, driver).await;
        verify_space_public(&pool, driver).await;
        verify_space_wallpaper(&pool, driver).await;
        verify_admin_and_config_reads(&pool, driver).await;
        verify_oauth_identity(&pool, driver).await;
        verify_group_writes(&pool, driver).await;
        verify_item_writes(&pool, driver).await;
        verify_sort_writes(&pool, driver).await;
        verify_member_writes(&pool, driver).await;
        verify_config_writes(&pool, driver).await;
        verify_space_writes(&pool, driver).await;
        verify_space_admin_writes(&pool, driver).await;
        verify_file_writes(&pool, driver).await;
        verify_user_admin_writes(&pool, driver).await;
        verify_theme_writes(&pool, driver).await;
        verify_theme_package_writes(&pool, driver).await;
        verify_wallpaper_writes(&pool, driver).await;
        verify_icon_writes(&pool, driver).await;
        verify_theme_selection_writes(&pool, driver).await;
        pool.close().await;
    }

    async fn verify_space_list(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, type_column, enabled_type) = match driver {
            Driver::MySql => ("`space`", "`type`", "BOOLEAN"),
            Driver::Sqlite => ("\"space\"", "\"type\"", "INTEGER"),
            Driver::Postgres => ("\"space\"", "\"type\"", "BOOLEAN"),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, {type_column} VARCHAR(32) NOT NULL, name VARCHAR(100) NOT NULL, owner_user_id BIGINT NOT NULL, pair_id BIGINT NOT NULL, side VARCHAR(16) NOT NULL, public_enabled {enabled_type} NOT NULL, public_id VARCHAR(80), public_mode VARCHAR(32) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create space fixture");
        sqlx::query("CREATE TABLE space_member (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL)")
            .execute(pool)
            .await
            .expect("create space member fixture");

        let insert_space = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, created_at, updated_at, {type_column}, name, owner_user_id, pair_id, side, public_enabled, public_id, public_mode) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            ),
        );
        for (id, name, owner, pair_id, side, enabled, public_id) in [
            (
                1_i64,
                "Owned",
                7_i64,
                700_i64,
                "yin",
                true,
                Some("pub-owned"),
            ),
            (2, "Editor", 8, 800, "yin", false, None),
            (3, "Viewer", 8, 900, "yin", false, None),
            (4, "Hidden", 99, 1000, "yin", false, None),
            (101, "Paired", 8, 1, "yang", false, None),
        ] {
            let mut statement = sqlx::query(&insert_space)
                .bind(id)
                .bind("2024-01-01 00:00:00")
                .bind("2024-01-02 00:00:00")
                .bind("shared")
                .bind(name)
                .bind(owner)
                .bind(pair_id)
                .bind(side);
            statement = if driver == Driver::Postgres {
                statement.bind(enabled)
            } else {
                statement.bind(i64::from(enabled))
            };
            statement
                .bind(public_id)
                .bind("direct")
                .execute(pool)
                .await
                .expect("insert space fixture");
        }
        let member_insert = query(
            driver,
            "INSERT INTO space_member (id, space_id, user_id, role) VALUES (?, ?, ?, ?)",
        );
        for (id, space_id, role) in [(1_i64, 2_i64, "editor"), (2, 3, "viewer")] {
            sqlx::query(&member_insert)
                .bind(id)
                .bind(space_id)
                .bind(7_i64)
                .bind(role)
                .execute(pool)
                .await
                .expect("insert member fixture");
        }

        let rows = space_list(pool, driver, 7)
            .await
            .expect("load caller's spaces");
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows.iter().map(|space| space.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(rows[0].can_edit);
        assert_eq!(rows[0].paired_space_id, Some(101));
        assert!(rows[0].public_enabled);
        assert_eq!(rows[0].public_id.as_deref(), Some("pub-owned"));
        assert!(rows[1].can_edit);
        assert_eq!(rows[1].paired_space_id, None);
        assert!(!rows[2].can_edit);
        assert_eq!(rows[0].created_at, "2024-01-01 00:00:00");

        sqlx::query("DROP TABLE space_member")
            .execute(pool)
            .await
            .expect("drop space member fixture");
        sqlx::query(&format!("DROP TABLE {space_table}"))
            .execute(pool)
            .await
            .expect("drop space fixture");
    }

    async fn verify_space_members(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, user_table) = match driver {
            Driver::MySql => ("`space`", "`user`"),
            Driver::Sqlite | Driver::Postgres => ("\"space\"", "\"user\""),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id BIGINT PRIMARY KEY, owner_user_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create access space fixture");
        sqlx::query(&format!(
            "CREATE TABLE {user_table} (id BIGINT PRIMARY KEY, mail VARCHAR(255) NOT NULL, name VARCHAR(100) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create member user fixture");
        sqlx::query("CREATE TABLE space_member (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL, source VARCHAR(32) NOT NULL, joined_at VARCHAR(40) NOT NULL)")
            .execute(pool)
            .await
            .expect("create space member fixture");

        let insert_space = query(
            driver,
            &format!("INSERT INTO {space_table} (id, owner_user_id) VALUES (?, ?)"),
        );
        for (id, owner) in [(77_i64, 7_i64), (78, 8)] {
            sqlx::query(&insert_space)
                .bind(id)
                .bind(owner)
                .execute(pool)
                .await
                .expect("insert access space");
        }
        let insert_user = query(
            driver,
            &format!("INSERT INTO {user_table} (id, mail, name) VALUES (?, ?, ?)"),
        );
        for (id, email, name) in [
            (7_i64, "owner@example.test", "Owner"),
            (8, "member@example.test", "Member"),
        ] {
            sqlx::query(&insert_user)
                .bind(id)
                .bind(email)
                .bind(name)
                .execute(pool)
                .await
                .expect("insert member account");
        }
        let insert_member = query(
            driver,
            "INSERT INTO space_member (id, space_id, user_id, role, source, joined_at) VALUES (?, ?, ?, ?, ?, ?)",
        );
        for (id, user_id, role) in [(1_i64, 7_i64, "admin"), (2, 8, "editor"), (3, 99, "viewer")] {
            sqlx::query(&insert_member)
                .bind(id)
                .bind(78_i64)
                .bind(user_id)
                .bind(role)
                .bind("manual")
                .bind("2024-02-03 04:05:06")
                .execute(pool)
                .await
                .expect("insert member row");
        }

        assert!(
            can_access_space(pool, driver, 7, 77)
                .await
                .expect("owner can access")
        );
        assert!(
            can_access_space(pool, driver, 7, 78)
                .await
                .expect("member can access")
        );
        assert!(
            !can_access_space(pool, driver, 9, 78)
                .await
                .expect("outsider cannot access")
        );

        let members = space_members(pool, driver, 78)
            .await
            .expect("load space members");
        assert_eq!(members.len(), 3);
        assert_eq!(
            members.iter().map(|member| member.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(members[0].email, "owner@example.test");
        assert_eq!(members[0].name, "Owner");
        assert_eq!(members[1].role, "editor");
        assert_eq!(members[1].source, "manual");
        assert_eq!(members[1].joined_at, "2024-02-03 04:05:06");
        assert_eq!(members[2].email, "");
        assert_eq!(members[2].name, "");
        assert!(
            space_members(pool, driver, 999)
                .await
                .expect("empty space has no members")
                .is_empty()
        );

        sqlx::query("DROP TABLE space_member")
            .execute(pool)
            .await
            .expect("drop member fixture");
        sqlx::query(&format!("DROP TABLE {user_table}"))
            .execute(pool)
            .await
            .expect("drop member user fixture");
        sqlx::query(&format!("DROP TABLE {space_table}"))
            .execute(pool)
            .await
            .expect("drop access space fixture");
    }

    async fn verify_groups(pool: &sqlx::AnyPool, driver: Driver) {
        sqlx::query("CREATE TABLE item_icon_group (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)")
            .execute(pool)
            .await
            .expect("create group fixture");
        sqlx::query("CREATE TABLE item_icon (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, item_icon_group_id BIGINT)")
            .execute(pool)
            .await
            .expect("create item fixture");

        let insert_group = query(
            driver,
            "INSERT INTO item_icon_group (id, created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        );
        for (id, title, sort, parent_id) in [
            (11_i64, "First", 1_i64, None),
            (12, "Second", 2, Some(11)),
            (13, "Empty", 3, None),
        ] {
            sqlx::query(&insert_group)
                .bind(id)
                .bind("2024-03-01 00:00:00")
                .bind("2024-03-02 00:00:00")
                .bind("folder")
                .bind(title)
                .bind("description")
                .bind(sort)
                .bind(7_i64)
                .bind(78_i64)
                .bind(parent_id)
                .execute(pool)
                .await
                .expect("insert group fixture");
        }
        let insert_item = query(
            driver,
            "INSERT INTO item_icon (id, space_id, item_icon_group_id) VALUES (?, ?, ?)",
        );
        for (id, group_id) in [(1_i64, 11_i64), (2, 11), (3, 12)] {
            sqlx::query(&insert_item)
                .bind(id)
                .bind(78_i64)
                .bind(group_id)
                .execute(pool)
                .await
                .expect("insert grouped item");
        }

        let groups = groups_for_space(pool, driver, 78)
            .await
            .expect("load groups with counts");
        assert_eq!(
            groups.iter().map(|group| group.id).collect::<Vec<_>>(),
            vec![11, 12, 13]
        );
        assert_eq!(
            groups
                .iter()
                .map(|group| group.item_count)
                .collect::<Vec<_>>(),
            vec![2, 1, 0]
        );
        assert_eq!(groups[0].title, "First");
        assert_eq!(groups[0].icon, "folder");
        assert_eq!(groups[0].created_at, "2024-03-01 00:00:00");
        assert_eq!(groups[1].parent_id, Some(11));
        assert!(
            groups_for_space(pool, driver, 999)
                .await
                .expect("unknown space has no groups")
                .is_empty()
        );

        sqlx::query("DROP TABLE item_icon")
            .execute(pool)
            .await
            .expect("drop item fixture to test count fallback");
        let groups_without_counts = groups_for_space(pool, driver, 78)
            .await
            .expect("group rows survive count query failure");
        assert!(
            groups_without_counts
                .iter()
                .all(|group| group.item_count == 0)
        );
        sqlx::query("DROP TABLE item_icon_group")
            .execute(pool)
            .await
            .expect("drop group fixture");
    }

    async fn verify_item_list(pool: &sqlx::AnyPool, driver: Driver) {
        sqlx::query("CREATE TABLE item_icon (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)")
            .execute(pool)
            .await
            .expect("create item list fixture");
        let insert = query(
            driver,
            "INSERT INTO item_icon (id, created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        );
        for (id, created_at, title, sort, group_id) in [
            (21_i64, "2024-05-01 00:00:00", "Second", 2_i64, 31_i64),
            (20, "2024-05-03 00:00:00", "First", 1, 31),
            (22, "2024-05-02 00:00:00", "Third", 3, 32),
        ] {
            sqlx::query(&insert)
                .bind(id)
                .bind(created_at)
                .bind("2024-05-04 00:00:00")
                .bind(r#"{"itemType":2,"src":"favicon.ico","text":"T"}"#)
                .bind(title)
                .bind("https://example.test")
                .bind("http://lan.example.test")
                .bind("")
                .bind("note")
                .bind(2_i64)
                .bind(sort)
                .bind(group_id)
                .bind(7_i64)
                .bind(78_i64)
                .execute(pool)
                .await
                .expect("insert item list fixture");
        }

        let all = items_for_space(pool, driver, 78, None, 0, 0)
            .await
            .expect("load all items");
        assert_eq!(
            all.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![20, 21, 22]
        );
        assert_eq!(all[0].title, "First");
        assert_eq!(
            all[0].icon_json,
            r#"{"itemType":2,"src":"favicon.ico","text":"T"}"#
        );
        assert_eq!(all[0].lan_url, "http://lan.example.test");
        let filtered = items_for_space(pool, driver, 78, Some(31), 0, 0)
            .await
            .expect("load group items");
        assert_eq!(
            filtered.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![20, 21]
        );
        assert!(
            items_for_space(pool, driver, 78, Some(-1), 0, 0)
                .await
                .expect("unknown group items")
                .is_empty()
        );
        let first_page = items_for_space(pool, driver, 78, None, 1, 2)
            .await
            .expect("load first page");
        assert_eq!(
            first_page.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![20, 21]
        );
        let second_page = items_for_space(pool, driver, 78, None, 2, 2)
            .await
            .expect("load second page");
        assert_eq!(
            second_page.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![22]
        );
        assert_eq!(
            items_for_space(pool, driver, 78, None, 1, 0)
                .await
                .expect("default page size")
                .len(),
            3
        );

        sqlx::query("DROP TABLE item_icon")
            .execute(pool)
            .await
            .expect("drop item list fixture");
    }

    async fn verify_theme_selection_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        let bool_type = match driver {
            Driver::MySql => "TINYINT NOT NULL",
            Driver::Sqlite => "INTEGER NOT NULL",
            Driver::Postgres => "BOOLEAN NOT NULL",
        };
        sqlx::query(&format!(
            "CREATE TABLE revision_record_v2 (id VARCHAR(191) PRIMARY KEY, package_id VARCHAR(191) NOT NULL, version VARCHAR(191) NOT NULL, manifest_json TEXT NOT NULL, tokens_json TEXT NOT NULL, verified {bool_type}, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create selection revision fixture");
        sqlx::query(&format!(
            "CREATE TABLE trusted_runtime_policy_v2 (revision_id VARCHAR(191) PRIMARY KEY, enabled {bool_type}, updated_by BIGINT NOT NULL, updated_at VARCHAR(64))"
        ))
        .execute(pool)
        .await
        .expect("create selection trusted fixture");
        sqlx::query("CREATE TABLE grant_record_v2 (user_id BIGINT NOT NULL, revision_id VARCHAR(191) NOT NULL, execution_mode VARCHAR(32) NOT NULL, permissions_json TEXT NOT NULL, updated_at VARCHAR(64), PRIMARY KEY (user_id, revision_id, execution_mode))")
            .execute(pool)
            .await
            .expect("create grant fixture");
        sqlx::query("CREATE TABLE space_theme_preference_v2 (space_id BIGINT PRIMARY KEY, package_id VARCHAR(191) NOT NULL, revision_id VARCHAR(191) NOT NULL, updated_by BIGINT NOT NULL, updated_at VARCHAR(64))")
            .execute(pool)
            .await
            .expect("create space theme fixture");
        sqlx::query("CREATE TABLE user_space_theme_preference_v2 (user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, package_id VARCHAR(191) NOT NULL, revision_id VARCHAR(191) NOT NULL, updated_at VARCHAR(64), PRIMARY KEY (user_id, space_id))")
            .execute(pool)
            .await
            .expect("create user space theme fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create selection item fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create selection group fixture");

        let insert_revision = query(
            driver,
            &format!(
                "INSERT INTO revision_record_v2 (id, package_id, version, manifest_json, tokens_json, verified, created_at) VALUES (?, ?, ?, ?, '{{}}', ?, {})",
                timestamp_bind(driver)
            ),
        );
        let mut statement = sqlx::query(&insert_revision)
            .bind("rev-a")
            .bind("community.a")
            .bind("1.0.0")
            .bind(r#"{"runtime":{"supportedModes":["sandbox","trusted"]}}"#);
        statement = if driver == Driver::Postgres {
            statement.bind(true)
        } else {
            statement.bind(1_i64)
        };
        statement
            .bind("2025-09-01 00:00:00")
            .execute(pool)
            .await
            .expect("insert selection revision");

        save_grant(
            pool,
            driver,
            7,
            "rev-a",
            "sandbox",
            r#"["spaces.read"]"#,
            "2025-09-02 00:00:00",
        )
        .await
        .expect("save grant");
        let grant = grant_row(pool, driver, 7, "rev-a", "sandbox")
            .await
            .expect("read grant")
            .expect("grant exists");
        assert_eq!(grant.permissions_json, r#"["spaces.read"]"#);
        save_grant(
            pool,
            driver,
            7,
            "rev-a",
            "sandbox",
            r#"["items.read"]"#,
            "2025-09-03 00:00:00",
        )
        .await
        .expect("update grant");
        assert_eq!(
            grant_row(pool, driver, 7, "rev-a", "sandbox")
                .await
                .expect("updated grant")
                .expect("grant remains")
                .permissions_json,
            r#"["items.read"]"#
        );
        delete_grant(pool, driver, 7, "rev-a", "sandbox")
            .await
            .expect("delete grant");
        assert_eq!(
            grant_row(pool, driver, 7, "rev-a", "sandbox")
                .await
                .expect("missing grant"),
            None
        );
        assert_eq!(
            revision_verified_manifest(pool, driver, "rev-a")
                .await
                .expect("verified manifest")
                .expect("revision exists")
                .1,
            1
        );

        save_space_theme(pool, driver, 78, "community.a", "rev-a", "2025-09-04 00:00:00")
            .await
            .expect("save space theme");
        assert_eq!(
            space_theme_pair(pool, driver, 78)
                .await
                .expect("read space theme"),
            Some(("community.a".to_string(), "rev-a".to_string()))
        );
        save_user_space_theme(
            pool,
            driver,
            7,
            78,
            "community.a",
            "rev-a",
            "2025-09-05 00:00:00",
        )
        .await
        .expect("save user space theme");
        assert_eq!(
            user_space_theme_pair(pool, driver, 7, 78)
                .await
                .expect("read user space theme"),
            Some(("community.a".to_string(), "rev-a".to_string()))
        );
        clear_user_space_theme_row(pool, driver, 7, 78)
            .await
            .expect("clear user space theme");
        assert_eq!(
            user_space_theme_revision(pool, driver, 7, 78)
                .await
                .expect("cleared user space theme"),
            None
        );
        clear_space_theme_row(pool, driver, 78)
            .await
            .expect("clear space theme");
        assert_eq!(
            space_theme_revision(pool, driver, 78)
                .await
                .expect("cleared space theme"),
            None
        );

        let insert_group = query(
            driver,
            &format!(
                "INSERT INTO item_icon_group (id, created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES (?, {}, {}, 'folder', ?, '', 0, 7, 78, NULL)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        for (id, title) in [(41_i64, "One"), (42, "Two")] {
            sqlx::query(&insert_group)
                .bind(id)
                .bind("2025-09-06 00:00:00")
                .bind("2025-09-06 00:00:00")
                .bind(title)
                .execute(pool)
                .await
                .expect("insert selection group");
        }
        let insert_item = query(
            driver,
            &format!(
                "INSERT INTO item_icon (id, created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES (?, {}, {}, '{{}}', ?, 'https://example.test', '', '', '', 2, 0, 41, 7, 78)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        sqlx::query(&insert_item)
            .bind(51_i64)
            .bind("2025-09-06 00:00:00")
            .bind("2025-09-06 00:00:00")
            .bind("Link")
            .execute(pool)
            .await
            .expect("insert selection item");

        arrange_layout(
            pool,
            driver,
            78,
            &[(51, 42, 5)],
            &[(41, 2), (42, 1)],
            "2025-09-07 00:00:00",
        )
        .await
        .expect("arrange layout");
        let item = item_by_id(pool, driver, 78, 51)
            .await
            .expect("read arranged item")
            .expect("item exists");
        assert_eq!(item.item_icon_group_id, 42);
        assert_eq!(item.sort, 5);
        let groups = groups_for_space(pool, driver, 78)
            .await
            .expect("read arranged groups");
        assert_eq!(
            groups.iter().map(|group| group.id).collect::<Vec<_>>(),
            vec![42, 41]
        );

        for table in [
            "item_icon",
            "item_icon_group",
            "user_space_theme_preference_v2",
            "space_theme_preference_v2",
            "grant_record_v2",
            "trusted_runtime_policy_v2",
            "revision_record_v2",
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop selection fixture");
        }
    }

    async fn verify_icon_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        sqlx::query(&format!(
            "CREATE TABLE space_o_id_c_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, space_id BIGINT NOT NULL, provider VARCHAR(50) NOT NULL, group_name VARCHAR(100) NOT NULL, role VARCHAR(32) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create oidc write fixture");
        sqlx::query(&format!(
            "CREATE TABLE file (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, user_id BIGINT NOT NULL, file_name VARCHAR(191) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create icon file fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create icon item fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create icon group fixture");

        let rule = add_oidc_group(
            pool,
            driver,
            78,
            "github",
            "team-admins",
            "editor",
            "2025-08-01 00:00:00",
        )
        .await
        .expect("add oidc group");
        assert!(rule > 0);
        let rows = oidc_groups(pool, driver, 78)
            .await
            .expect("read oidc group");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, rule);
        delete_oidc_group(pool, driver, 78, rule)
            .await
            .expect("delete oidc group");
        assert!(oidc_groups(pool, driver, 78)
            .await
            .expect("empty oidc groups")
            .is_empty());

        let item = create_item_with_icon(
            pool,
            driver,
            78,
            7,
            r#"{"itemType":2,"fileName":"icon.png"}"#,
            "Link",
            "https://example.test",
            "",
            "",
            "note",
            2,
            3,
            31,
            Some("icon.png"),
            "2025-08-02 00:00:00",
        )
        .await
        .expect("create item with icon");
        assert!(item > 0);
        let read = item_by_id(pool, driver, 78, item)
            .await
            .expect("read item with icon")
            .expect("item exists");
        assert_eq!(read.title, "Link");
        assert_eq!(read.icon_json, r#"{"itemType":2,"fileName":"icon.png"}"#);
        let files: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM file WHERE user_id = ?",
        ))
        .bind(7_i64)
        .fetch_one(pool)
        .await
        .expect("count file rows")
        .try_get("total")
        .expect("total");
        assert_eq!(files, 1, "one row per content hash");
        // A second item reusing the icon must not duplicate the file row.
        let second = create_item_with_icon(
            pool,
            driver,
            78,
            7,
            "{}",
            "Second",
            "https://example.test/two",
            "",
            "",
            "",
            2,
            4,
            31,
            Some("icon.png"),
            "2025-08-03 00:00:00",
        )
        .await
        .expect("create second item with the same icon");
        assert!(second > item);
        let files: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM file WHERE user_id = ?",
        ))
        .bind(7_i64)
        .fetch_one(pool)
        .await
        .expect("count file rows after second item")
        .try_get("total")
        .expect("total");
        assert_eq!(files, 1);

        let mut tx = pool.begin().await.expect("begin import transaction");
        let group = crate::repository::insert_group_in_tx(
            &mut tx,
            driver,
            78,
            7,
            "folder",
            "Bookmarks",
            0,
            None,
            "2025-08-04 00:00:00",
        )
        .await
        .expect("insert imported group");
        assert!(group > 0);
        crate::repository::insert_imported_item_in_tx(
            &mut tx,
            driver,
            "{}",
            "Imported",
            "https://imported.test",
            0,
            group,
            7,
            78,
            "2025-08-04 00:00:00",
        )
        .await
        .expect("insert imported item");
        crate::repository::insert_file_in_tx(
            &mut tx,
            driver,
            7,
            "icon.png",
            "2025-08-04 00:00:00",
        )
        .await
        .expect("insert existing file row is a no-op");
        tx.commit().await.expect("commit import transaction");
        assert_eq!(
            group_by_id(pool, driver, 78, group)
                .await
                .expect("read imported group")
                .expect("group exists")
                .title,
            "Bookmarks"
        );
        let imported = items_for_space(pool, driver, 78, Some(group), 0, 0)
            .await
            .expect("read imported items");
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].title, "Imported");

        for table in ["item_icon_group", "item_icon", "file", "space_o_id_c_group"] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop icon fixture");
        }
    }

    async fn verify_wallpaper_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let blob_type = match driver {
            Driver::MySql => "LONGBLOB",
            Driver::Sqlite => "BLOB",
            Driver::Postgres => "BYTEA",
        };
        sqlx::query(&format!(
            "CREATE TABLE web_wallpaper_record (id VARCHAR(191) PRIMARY KEY, owner_id BIGINT NOT NULL, html {blob_type} NOT NULL, poster {blob_type} NOT NULL, poster_type VARCHAR(32) NOT NULL, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create web wallpaper fixture");

        let html = b"<!doctype html><html><body>wall</body></html>".to_vec();
        let poster = b"\x89PNG\r\n\x1a\n".to_vec();
        assert!(
            !web_wallpaper_exists(pool, driver, "wall-1")
                .await
                .expect("missing wallpaper")
        );
        insert_web_wallpaper(
            pool,
            driver,
            "wall-1",
            7,
            &html,
            &poster,
            "image/png",
            "2025-07-01 00:00:00",
        )
        .await
        .expect("insert web wallpaper");
        assert!(
            web_wallpaper_exists(pool, driver, "wall-1")
                .await
                .expect("wallpaper exists")
        );
        let record = web_wallpaper(pool, driver, "wall-1")
            .await
            .expect("read web wallpaper")
            .expect("wallpaper row");
        assert_eq!(record.html, html);
        assert_eq!(record.poster, poster);
        assert_eq!(record.poster_type, "image/png");
        assert_eq!(
            web_wallpaper(pool, driver, "missing")
                .await
                .expect("missing row"),
            None
        );

        // The id is the content hash, so a second upload is a no-op.
        insert_web_wallpaper(
            pool,
            driver,
            "wall-1",
            7,
            &html,
            &poster,
            "image/png",
            "2025-07-01 00:00:00",
        )
        .await
        .expect_err("duplicate id is rejected by the primary key");

        sqlx::query("DROP TABLE web_wallpaper_record")
            .execute(pool)
            .await
            .expect("drop web wallpaper fixture");
    }

    async fn verify_theme_package_writes(pool: &sqlx::AnyPool, driver: Driver) {
        use std::collections::BTreeMap;

        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        let bool_type = match driver {
            Driver::MySql => "TINYINT NOT NULL",
            Driver::Sqlite => "INTEGER NOT NULL",
            Driver::Postgres => "BOOLEAN NOT NULL",
        };
        sqlx::query(&format!(
            "CREATE TABLE package_record_v2 (id VARCHAR(191) PRIMARY KEY, name VARCHAR(191) NOT NULL, latest_revision VARCHAR(191) NOT NULL, removed {bool_type}, created_at {timestamp_type}, updated_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create package fixture");
        sqlx::query(&format!(
            "CREATE TABLE revision_record_v2 (id VARCHAR(191) PRIMARY KEY, package_id VARCHAR(191) NOT NULL, version VARCHAR(191) NOT NULL, manifest_json TEXT NOT NULL, tokens_json TEXT NOT NULL, verified {bool_type}, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create revision fixture");
        let blob_type = match driver {
            Driver::MySql => "LONGBLOB",
            Driver::Sqlite => "BLOB",
            Driver::Postgres => "BYTEA",
        };
        sqlx::query(&format!(
            "CREATE TABLE asset_record_v2 (revision_id VARCHAR(191) NOT NULL, path VARCHAR(191) NOT NULL, media_type VARCHAR(64) NOT NULL, content {blob_type} NOT NULL, PRIMARY KEY (revision_id, path))"
        ))
            .execute(pool)
            .await
            .expect("create asset fixture");
        sqlx::query(&format!(
            "CREATE TABLE audit_record (id {id_type}, actor_id BIGINT NOT NULL, action VARCHAR(64) NOT NULL, package_id VARCHAR(191) NOT NULL, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create audit fixture");
        sqlx::query("CREATE TABLE activation_record_v2 (scope VARCHAR(191) PRIMARY KEY, package_id VARCHAR(191) NOT NULL, active_revision_id VARCHAR(191) NOT NULL, last_good_revision_id VARCHAR(191), pending_revision_id VARCHAR(191), trial_started_at VARCHAR(64), updated_at VARCHAR(64))")
            .execute(pool)
            .await
            .expect("create activation fixture");

        let yin_manifest: crate::packages::Manifest = serde_json::from_value(serde_json::json!({
            "format": "yin-theme",
            "formatVersion": 2,
            "id": "org.yin.default",
            "name": "Yin",
            "version": "1.0.0",
            "themeApi": "^1.0.0",
            "core": ">=0.4.0",
            "author": "Yin",
            "license": "AGPL-3.0"
        }))
        .expect("default manifest");
        let yin_package = crate::packages::Package {
            manifest: yin_manifest,
            tokens: BTreeMap::new(),
            files: BTreeMap::new(),
            verified: true,
            revision: "rev-default".to_string(),
        };
        install_package(pool, driver, 7, &yin_package)
            .await
            .expect("install default package");

        let test_package = crate::packages::Package {
            manifest: serde_json::from_value(serde_json::json!({
                "format": "yin-theme",
                "formatVersion": 2,
                "id": "community.test",
                "name": "Test",
                "version": "1.0.0",
                "themeApi": "^1.0.0",
                "core": ">=0.4.0",
                "author": "Test",
                "license": "MIT"
            }))
            .expect("test manifest"),
            tokens: BTreeMap::new(),
            files: BTreeMap::from([(
                "views/home.mjs".to_string(),
                ("text/javascript".to_string(), b"export {}".to_vec()),
            )]),
            verified: true,
            revision: "rev-test".to_string(),
        };
        install_package(pool, driver, 7, &test_package)
            .await
            .expect("install test package");
        let state = theme_admin_state(pool, driver).await.expect("state");
        assert!(state.packages.iter().any(|package| package.id == "community.test"));
        assert!(state.revisions.iter().any(|revision| revision.id == "rev-test"));
        let assets: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM asset_record_v2 WHERE revision_id = ?",
        ))
        .bind("rev-test")
        .fetch_one(pool)
        .await
        .expect("count assets")
        .try_get("total")
        .expect("total");
        assert_eq!(assets, 1);
        // Re-installing the same revision is a no-op that Go does not audit.
        install_package(pool, driver, 7, &test_package)
            .await
            .expect("reinstall");
        let audits = audit_entries(pool, driver).await.expect("audit");
        assert_eq!(
            audits
                .iter()
                .filter(|row| row.action == "install-v2" && row.package_id == "community.test")
                .count(),
            1
        );

        set_default_theme(pool, driver, "community.test", "rev-test", "2025-06-01 00:00:00")
            .await
            .expect("set default");
        let state = theme_admin_state(pool, driver).await.expect("state after default");
        assert_eq!(state.active_revision, "rev-test");
        assert_eq!(state.last_good_revision, "rev-test");
        assert_eq!(state.default_package, "community.test");

        begin_theme_trial(pool, driver, "community.test", "rev-test", "2025-06-02 00:00:00")
            .await
            .expect("begin trial twice-safe");
        assert_eq!(
            instance_pending(pool, driver).await.expect("pending"),
            Some("rev-test".to_string())
        );
        confirm_theme_trial(pool, driver, "community.test", "rev-test", "2025-06-03 00:00:00")
            .await
            .expect("confirm trial");
        assert_eq!(
            instance_pending(pool, driver)
                .await
                .expect("pending cleared"),
            Some(String::new())
        );
        assert_eq!(
            instance_last_good(pool, driver).await.expect("last good"),
            Some("rev-test".to_string())
        );
        rollback_theme(pool, driver, "org.yin.default", "rev-default", "2025-06-04 00:00:00")
            .await
            .expect("rollback");
        let state = theme_admin_state(pool, driver).await.expect("state after rollback");
        assert_eq!(state.active_revision, "rev-default");
        assert_eq!(state.default_package, "org.yin.default");

        remove_theme_package(pool, driver, 7, "community.test")
            .await
            .expect("remove package");
        assert_eq!(
            package_removed(pool, driver, "community.test")
                .await
                .expect("removed flag"),
            Some(1)
        );
        let audits = audit_entries(pool, driver).await.expect("audit after remove");
        assert!(audits.iter().any(|row| row.action == "remove-v2"));

        for table in [
            "activation_record_v2",
            "audit_record",
            "asset_record_v2",
            "revision_record_v2",
            "package_record_v2",
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop theme package fixture");
        }
    }

    async fn verify_theme_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        let bool_type = match driver {
            Driver::MySql => "TINYINT NOT NULL",
            Driver::Sqlite => "INTEGER NOT NULL",
            Driver::Postgres => "BOOLEAN NOT NULL",
        };
        sqlx::query(&format!(
            "CREATE TABLE package_record_v2 (id VARCHAR(191) PRIMARY KEY, name VARCHAR(191) NOT NULL, latest_revision VARCHAR(191) NOT NULL, removed {bool_type}, created_at {timestamp_type}, updated_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create theme package fixture");
        sqlx::query(&format!(
            "CREATE TABLE revision_record_v2 (id VARCHAR(191) PRIMARY KEY, package_id VARCHAR(191) NOT NULL, version VARCHAR(191) NOT NULL, manifest_json TEXT NOT NULL, tokens_json TEXT NOT NULL, verified {bool_type}, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create theme revision fixture");
        sqlx::query("CREATE TABLE activation_record_v2 (scope VARCHAR(191) PRIMARY KEY, package_id VARCHAR(191) NOT NULL, active_revision_id VARCHAR(191) NOT NULL, last_good_revision_id VARCHAR(191), pending_revision_id VARCHAR(191), trial_started_at VARCHAR(64), updated_at VARCHAR(64))")
            .execute(pool)
            .await
            .expect("create activation fixture");
        sqlx::query("CREATE TABLE user_theme_preference_v2 (user_id BIGINT PRIMARY KEY, mode VARCHAR(32) NOT NULL, theme_mode VARCHAR(32) NOT NULL, updated_at VARCHAR(64))")
            .execute(pool)
            .await
            .expect("create preference fixture");
        sqlx::query(&format!(
            "CREATE TABLE trusted_runtime_policy_v2 (revision_id VARCHAR(191) PRIMARY KEY, enabled {bool_type}, updated_by BIGINT NOT NULL, updated_at VARCHAR(64))"
        ))
        .execute(pool)
        .await
        .expect("create trusted fixture");
        sqlx::query("CREATE TABLE grant_record_v2 (user_id BIGINT NOT NULL, revision_id VARCHAR(191) NOT NULL, execution_mode VARCHAR(32) NOT NULL, permissions_json TEXT NOT NULL, updated_at VARCHAR(64))")
            .execute(pool)
            .await
            .expect("create grant fixture");
        sqlx::query(&format!(
            "CREATE TABLE audit_record (id {id_type}, actor_id BIGINT NOT NULL, action VARCHAR(64) NOT NULL, package_id VARCHAR(191) NOT NULL, created_at {timestamp_type})"
        ))
        .execute(pool)
        .await
        .expect("create audit fixture");

        let insert_package = query(
            driver,
            &format!(
                "INSERT INTO package_record_v2 (id, name, latest_revision, removed, created_at, updated_at) VALUES (?, ?, ?, ?, {}, {})",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        let mut statement = sqlx::query(&insert_package)
            .bind("org.yin.default")
            .bind("Yin")
            .bind("rev-1");
        statement = if driver == Driver::Postgres {
            statement.bind(false)
        } else {
            statement.bind(0_i64)
        };
        statement
            .bind("2025-05-01 00:00:00")
            .bind("2025-05-01 00:00:00")
            .execute(pool)
            .await
            .expect("insert theme package");

        let insert_revision = format!(
            "INSERT INTO revision_record_v2 (id, package_id, version, manifest_json, tokens_json, verified, created_at) VALUES (?, ?, ?, ?, '{{}}', ?, {})",
            timestamp_bind(driver)
        );
        let insert_revision = query(driver, &insert_revision);
        let mut statement = sqlx::query(&insert_revision)
            .bind("rev-1")
            .bind("org.yin.default")
            .bind("1.0.0")
            .bind(r#"{"runtime":{"supportedModes":["trusted","sandbox"]}}"#);
        statement = if driver == Driver::Postgres {
            statement.bind(true)
        } else {
            statement.bind(1_i64)
        };
        statement
            .bind("2025-05-02 00:00:00")
            .execute(pool)
            .await
            .expect("insert theme revision");

        sqlx::query(&query(
            driver,
            &format!(
                "INSERT INTO activation_record_v2 (scope, package_id, active_revision_id, last_good_revision_id, pending_revision_id, trial_started_at, updated_at) VALUES ('instance', 'org.yin.default', 'rev-1', 'rev-1', '', '2025-05-03 00:00:00', '2025-05-03 00:00:00')"
            ),
        ))
        .execute(pool)
        .await
        .expect("insert activation");

        let state = theme_admin_state(pool, driver)
            .await
            .expect("read admin state");
        assert_eq!(state.active_revision, "rev-1");
        assert_eq!(state.default_package, "org.yin.default");
        assert_eq!(state.last_good_revision, "rev-1");
        assert_eq!(state.packages.len(), 1);
        assert_eq!(state.packages[0].version, "1.0.0");
        assert_eq!(state.packages[0].verified, 1);
        assert_eq!(state.revisions.len(), 1);
        assert_eq!(
            state.trial_started_at.as_deref(),
            Some("2025-05-03 00:00:00")
        );

        assert_eq!(
            user_theme_preference(pool, driver, 7).await.expect("empty"),
            None
        );
        save_theme_preference(
            pool,
            driver,
            7,
            "dark",
            "custom",
            "2025-05-04 00:00:00",
        )
        .await
        .expect("insert preference");
        assert_eq!(
            user_theme_preference(pool, driver, 7)
                .await
                .expect("read preference"),
            Some(("dark".to_string(), "custom".to_string()))
        );
        save_theme_preference(
            pool,
            driver,
            7,
            "light",
            "custom",
            "2025-05-05 00:00:00",
        )
        .await
        .expect("update mode only");
        assert_eq!(
            user_theme_preference(pool, driver, 7)
                .await
                .expect("read updated preference"),
            Some(("light".to_string(), "custom".to_string()))
        );
        assert_eq!(
            latest_revision(pool, driver, "org.yin.default")
                .await
                .expect("latest revision"),
            Some("rev-1".to_string())
        );
        assert_eq!(
            latest_revision(pool, driver, "missing")
                .await
                .expect("missing package"),
            None
        );
        assert_eq!(
            package_id_for_revision(pool, driver, "rev-1")
                .await
                .expect("package of revision"),
            Some("org.yin.default".to_string())
        );

        select_user_theme(
            pool,
            driver,
            7,
            "org.yin.default",
            "rev-1",
            "dark",
            "2025-05-06 00:00:00",
        )
        .await
        .expect("select theme twice-safe");
        select_user_theme(
            pool,
            driver,
            7,
            "org.yin.default",
            "rev-1",
            "light",
            "2025-05-07 00:00:00",
        )
        .await
        .expect("select theme again");
        assert_eq!(
            user_theme_preference(pool, driver, 7)
                .await
                .expect("preference after select"),
            Some(("light".to_string(), "custom".to_string()))
        );
        assert_eq!(
            activation_revision(pool, driver, "instance")
                .await
                .expect("instance activation"),
            Some("rev-1".to_string())
        );

        assert_eq!(
            revision_manifest(pool, driver, "rev-1")
                .await
                .expect("manifest"),
            Some(r#"{"runtime":{"supportedModes":["trusted","sandbox"]}}"#.to_string())
        );
        assert_eq!(
            trusted_enabled(pool, driver, "rev-1")
                .await
                .expect("no policy"),
            None
        );
        save_trusted_policy(pool, driver, "rev-1", true, 7, "2025-05-08 00:00:00")
            .await
            .expect("enable trusted runtime");
        assert_eq!(
            trusted_enabled(pool, driver, "rev-1")
                .await
                .expect("policy enabled"),
            Some(true)
        );
        sqlx::query(&query(
            driver,
            "INSERT INTO grant_record_v2 (user_id, revision_id, execution_mode, permissions_json, updated_at) VALUES (7, 'rev-1', 'trusted', '{}', '2025-05-08 00:00:00')",
        ))
        .execute(pool)
        .await
        .expect("insert trusted grant");
        save_trusted_policy(pool, driver, "rev-1", false, 7, "2025-05-09 00:00:00")
            .await
            .expect("disable trusted runtime");
        assert_eq!(
            trusted_enabled(pool, driver, "rev-1")
                .await
                .expect("policy disabled"),
            Some(false)
        );
        withdraw_trusted_grants(pool, driver, "rev-1")
            .await
            .expect("withdraw grants");
        let grants: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM grant_record_v2",
        ))
        .fetch_one(pool)
        .await
        .expect("count grants")
        .try_get("total")
        .expect("total");
        assert_eq!(grants, 0);

        let audit = audit_entries(pool, driver).await.expect("audit rows");
        assert_eq!(audit.len(), 2);
        assert_eq!(audit[0].action, "select");
        assert_eq!(audit[0].package_id, "org.yin.default");

        for table in [
            "audit_record",
            "grant_record_v2",
            "trusted_runtime_policy_v2",
            "user_theme_preference_v2",
            "activation_record_v2",
            "revision_record_v2",
            "package_record_v2",
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop theme fixture");
        }
    }

    async fn verify_user_admin_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (user_table, space_table, timestamp_type, id_type) = match driver {
            Driver::MySql => ("`user`", "`space`", "DATETIME(6)", "BIGINT AUTO_INCREMENT PRIMARY KEY"),
            Driver::Sqlite => ("\"user\"", "\"space\"", "TEXT", "INTEGER PRIMARY KEY AUTOINCREMENT"),
            Driver::Postgres => ("\"user\"", "\"space\"", "TIMESTAMPTZ", "BIGSERIAL PRIMARY KEY"),
        };
        let enabled_type = match driver {
            Driver::MySql => "TINYINT NOT NULL DEFAULT 0",
            Driver::Sqlite => "INTEGER NOT NULL DEFAULT 0",
            Driver::Postgres => "BOOLEAN NOT NULL DEFAULT false",
        };
        sqlx::query(&format!(
            "CREATE TABLE {user_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, password VARCHAR(255) NOT NULL, name VARCHAR(100) NOT NULL, head_image VARCHAR(255) NOT NULL, status BIGINT NOT NULL, role BIGINT NOT NULL, mail VARCHAR(255) NOT NULL, token_version BIGINT NOT NULL DEFAULT 0)"
        ))
        .execute(pool)
        .await
        .expect("create user admin fixture");
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, type VARCHAR(20) NOT NULL, name VARCHAR(100) NOT NULL, owner_user_id BIGINT NOT NULL, pair_id BIGINT NOT NULL DEFAULT 0, side VARCHAR(16) NOT NULL DEFAULT 'yin', public_enabled {enabled_type}, public_id VARCHAR(80), public_mode VARCHAR(32) NOT NULL DEFAULT 'direct', public_code_hash VARCHAR(128), search_config_json TEXT, wallpaper_json TEXT)"
        ))
        .execute(pool)
        .await
        .expect("create user space fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_member (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL, source VARCHAR(32) NOT NULL, joined_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create user member fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create user group fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create user item fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_o_id_c_group (id {id_type}, space_id BIGINT NOT NULL, provider VARCHAR(50) NOT NULL, group_name VARCHAR(100) NOT NULL, role VARCHAR(32) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create user oidc fixture");
        for table in ["o_auth_identity", "module_config", "file"] {
            sqlx::query(&format!(
                "CREATE TABLE {table} (id {id_type}, user_id BIGINT NOT NULL)"
            ))
            .execute(pool)
            .await
            .expect("create user child fixture");
        }

        let created = create_user(
            pool,
            driver,
            "new@example.test",
            "hash-one",
            "New User",
            "/uploads/head.png",
            2,
            "2025-04-01 00:00:00",
        )
        .await
        .expect("create user");
        assert!(created > 0);
        let view = admin_user_view(pool, driver, created)
            .await
            .expect("read created user")
            .expect("user exists");
        assert_eq!(view.mail, "new@example.test");
        assert_eq!(view.name, "New User");
        assert_eq!(view.status, 1);
        assert_eq!(view.role, 2);
        assert_eq!(view.password, "hash-one");
        assert_eq!(
            user_password_and_name(pool, driver, created)
                .await
                .expect("password and name"),
            Some(("hash-one".to_string(), "New User".to_string()))
        );
        assert!(user_mail_exists(pool, driver, "new@example.test", None)
            .await
            .expect("mail exists"));
        assert!(!user_mail_exists(pool, driver, "new@example.test", Some(created))
            .await
            .expect("own mail excluded"));
        let paired = paired_space_ids(pool, driver, 1)
            .await
            .expect("personal pair lookup");
        assert_eq!(paired.len(), 2, "personal space has a Yang twin");

        update_user_admin(
            pool,
            driver,
            created,
            "renamed@example.test",
            Some("Renamed"),
            Some("/uploads/new.png"),
            Some(2),
            Some(3),
            Some("hash-two"),
            "2025-04-02 00:00:00",
            Some(("Renamed", "Renamed-B")),
        )
        .await
        .expect("update user");
        let view = admin_user_view(pool, driver, created)
            .await
            .expect("read updated user")
            .expect("user remains");
        assert_eq!(view.mail, "renamed@example.test");
        assert_eq!(view.name, "Renamed");
        assert_eq!(view.status, 2);
        assert_eq!(view.role, 3);
        assert_eq!(view.password, "hash-two");
        assert_eq!(
            space_name(pool, driver, paired[0])
                .await
                .expect("renamed yin space"),
            Some("Renamed".to_string())
        );
        assert_eq!(
            space_name(pool, driver, paired[1])
                .await
                .expect("renamed yang space"),
            Some("Renamed-B".to_string())
        );

        update_user_profile(
            pool,
            driver,
            created,
            "Profile",
            "/uploads/profile.png",
            "2025-04-03 00:00:00",
        )
        .await
        .expect("update profile");
        assert_eq!(
            admin_user_view(pool, driver, created)
                .await
                .expect("read profile")
                .expect("user remains")
                .name,
            "Profile"
        );

        assert!(
            update_user_password(
                pool,
                driver,
                created,
                "hash-two",
                "hash-three",
                "2025-04-04 00:00:00",
            )
            .await
            .expect("change password")
        );
        let view = admin_user_view(pool, driver, created)
            .await
            .expect("read password change")
            .expect("user remains");
        assert_eq!(view.password, "hash-three");
        assert!(
            !update_user_password(
                pool,
                driver,
                created,
                "hash-two",
                "hash-four",
                "2025-04-05 00:00:00",
            )
            .await
            .expect("stale password is rejected")
        );

        let owned = spaces_owned_by(pool, driver, created)
            .await
            .expect("owned spaces");
        let required = !owned.iter().any(|(_, kind)| kind == "shared");
        assert!(required, "personal spaces only");
        let mut ids: Vec<i64> = owned.iter().map(|(id, _)| *id).collect();
        ids.extend(
            paired_ids_of(pool, driver, &ids)
                .await
                .expect("paired ids of owned"),
        );
        delete_user(pool, driver, created, &ids)
            .await
            .expect("delete user with spaces");
        assert_eq!(
            admin_user_view(pool, driver, created)
                .await
                .expect("deleted user"),
            None
        );
        let remaining_spaces: i64 = sqlx::query(&query(
            driver,
            &format!("SELECT COUNT(*) AS total FROM {space_table}"),
        ))
        .fetch_one(pool)
        .await
        .expect("count spaces")
        .try_get("total")
        .expect("total");
        assert_eq!(remaining_spaces, 0);

        for table in [
            "file",
            "module_config",
            "o_auth_identity",
            "space_o_id_c_group",
            "item_icon",
            "item_icon_group",
            "space_member",
            space_table,
            user_table,
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop user admin fixture");
        }
    }

    async fn verify_file_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        sqlx::query(&format!(
            "CREATE TABLE file (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, user_id BIGINT NOT NULL, file_name VARCHAR(191) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create file write fixture");

        insert_file(pool, driver, 7, "first.png", "2025-03-01 00:00:00")
            .await
            .expect("insert file");
        insert_file(pool, driver, 7, "second.png", "2025-03-02 00:00:00")
            .await
            .expect("insert second file");
        let (total, rows) = file_list(pool, driver, 0, 1)
            .await
            .expect("list files newest first");
        assert_eq!(total, 2);
        assert_eq!(rows[0].file_name, "second.png");
        let id = rows[0].id;
        assert_eq!(
            file_name_by_id(pool, driver, id)
                .await
                .expect("read file name"),
            Some("second.png".to_string())
        );
        delete_file_row(pool, driver, id)
            .await
            .expect("delete file row");
        assert_eq!(
            file_name_by_id(pool, driver, id).await.expect("deleted row"),
            None
        );
        let (remaining, _) = file_list(pool, driver, 0, 1)
            .await
            .expect("list after delete");
        assert_eq!(remaining, 1);

        sqlx::query("DROP TABLE file")
            .execute(pool)
            .await
            .expect("drop file fixture");
    }

    async fn verify_space_admin_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, user_table, timestamp_type, id_type) = match driver {
            Driver::MySql => ("`space`", "`user`", "DATETIME(6)", "BIGINT AUTO_INCREMENT PRIMARY KEY"),
            Driver::Sqlite => ("\"space\"", "\"user\"", "TEXT", "INTEGER PRIMARY KEY AUTOINCREMENT"),
            Driver::Postgres => ("\"space\"", "\"user\"", "TIMESTAMPTZ", "BIGSERIAL PRIMARY KEY"),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, type VARCHAR(20) NOT NULL, name VARCHAR(100) NOT NULL, owner_user_id BIGINT NOT NULL, pair_id BIGINT NOT NULL DEFAULT 0, side VARCHAR(16) NOT NULL DEFAULT 'yin', public_enabled BIGINT NOT NULL DEFAULT 0, public_id VARCHAR(80), public_mode VARCHAR(32) NOT NULL DEFAULT 'direct', public_code_hash VARCHAR(128), search_config_json TEXT, wallpaper_json TEXT)"
        ))
        .execute(pool)
        .await
        .expect("create admin space fixture");
        sqlx::query(&format!(
            "CREATE TABLE {user_table} (id {id_type}, mail VARCHAR(255) NOT NULL, name VARCHAR(100) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create admin user fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_member (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL, source VARCHAR(32) NOT NULL, joined_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create admin member fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create admin group fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create admin item fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_o_id_c_group (id {id_type}, space_id BIGINT NOT NULL, provider VARCHAR(50) NOT NULL, group_name VARCHAR(100) NOT NULL, role VARCHAR(32) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create admin oidc fixture");
        sqlx::query("CREATE TABLE file (id BIGINT PRIMARY KEY, file_name VARCHAR(191) NOT NULL)")
            .execute(pool)
            .await
            .expect("create admin file fixture");

        let insert_space = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, created_at, updated_at, type, name, owner_user_id, pair_id, side) VALUES (?, {}, {}, ?, ?, ?, ?, ?)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        for (id, name, kind, side) in [
            (11_i64, "Team", "team", "yin"),
            (12, "Team-B", "team", "yang"),
            (13, "Other", "team", "yin"),
        ] {
            sqlx::query(&insert_space)
                .bind(id)
                .bind("2025-02-01 00:00:00")
                .bind("2025-02-01 00:00:00")
                .bind(kind)
                .bind(name)
                .bind(7_i64)
                .bind(11_i64)
                .bind(side)
                .execute(pool)
                .await
                .expect("insert admin space");
        }
        let insert_user = query(
            driver,
            &format!("INSERT INTO {user_table} (id, mail, name) VALUES (?, ?, ?)"),
        );
        for (id, mail) in [(7_i64, "owner@example.test"), (8, "target@example.test")] {
            sqlx::query(&insert_user)
                .bind(id)
                .bind(mail)
                .bind("User")
                .execute(pool)
                .await
                .expect("insert admin user");
        }
        let insert_member = query(
            driver,
            &format!(
                "INSERT INTO space_member (id, created_at, updated_at, space_id, user_id, role, source, joined_at) VALUES (?, {}, {}, ?, ?, ?, 'manual', {})",
                timestamp_bind(driver),
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        for (id, space_id, user_id, role) in [
            (1_i64, 11_i64, 7_i64, "admin"),
            (2, 12, 7, "admin"),
            (3, 13, 7, "admin"),
        ] {
            sqlx::query(&insert_member)
                .bind(id)
                .bind("2025-02-01 00:00:00")
                .bind("2025-02-01 00:00:00")
                .bind(space_id)
                .bind(user_id)
                .bind(role)
                .bind("2025-02-01 00:00:00")
                .execute(pool)
                .await
                .expect("insert admin member");
        }

        assert_eq!(
            space_owner_and_type(pool, driver, 11)
                .await
                .expect("read owner and type"),
            Some((7, "team".to_string()))
        );
        // The fixture inserts explicit ids; PostgreSQL needs the sequence moved
        // forward exactly like the bootstrap does.
        crate::bootstrap::reset_sequences(pool, driver)
            .await
            .expect("reset sequences");

        transfer_space(pool, driver, 11, 7, 8, "2025-02-02 00:00:00")
            .await
            .expect("transfer space");
        assert_eq!(
            space_owner(pool, driver, 11).await.expect("new owner"),
            Some(8)
        );
        let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };
        let roles: Vec<String> = sqlx::query(
            &query(
                driver,
                &format!(
                    "SELECT CAST(role AS {text_type}) AS role FROM space_member WHERE space_id = 11 ORDER BY user_id"
                ),
            ),
        )
        .fetch_all(pool)
        .await
        .expect("read roles")
        .into_iter()
        .map(|row| row.try_get::<String, _>("role").expect("role"))
        .collect();
        assert_eq!(roles, vec!["editor".to_string(), "admin".to_string()]);

        let insert_group = query(
            driver,
            &format!(
                "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES ({}, {}, 'folder', 'Group', '', 0, ?, ?, NULL)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        sqlx::query(&insert_group)
            .bind("2025-02-03 00:00:00")
            .bind("2025-02-03 00:00:00")
            .bind(7_i64)
            .bind(13_i64)
            .execute(pool)
            .await
            .expect("insert cleared group");
        clear_space(pool, driver, 13).await.expect("clear space");
        assert!(
            groups_for_space(pool, driver, 13)
                .await
                .expect("cleared space has no groups")
                .is_empty()
        );

        sqlx::query("INSERT INTO file (id, file_name) VALUES (1, 'icon.png')")
            .execute(pool)
            .await
            .expect("insert icon file");
        let insert_item = query(
            driver,
            &format!(
                "INSERT INTO item_icon (created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES ({}, {}, ?, 'Item', 'https://example.test', '', '', '', 0, 0, 0, ?, ?)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        sqlx::query(&insert_item)
            .bind("2025-02-04 00:00:00")
            .bind("2025-02-04 00:00:00")
            .bind(r#"{"fileName":"icon.png"}"#)
            .bind(7_i64)
            .bind(11_i64)
            .execute(pool)
            .await
            .expect("insert icon item");

        let paired = paired_space_ids(pool, driver, 11)
            .await
            .expect("paired ids");
        let removed = delete_paired_space(pool, driver, &paired)
            .await
            .expect("delete paired space");
        // Go checks references before deleting the item rows, so a file named by
        // the Space itself is always still referenced and is never unlinked.
        assert!(removed.is_empty(), "kept for parity: {:?}", removed);
        assert_eq!(
            space_owner_and_type(pool, driver, 11)
                .await
                .expect("deleted space"),
            None
        );
        let files: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM file",
        ))
        .fetch_one(pool)
        .await
        .expect("count files")
        .try_get("total")
        .expect("total");
        assert_eq!(files, 1, "the referenced file row survives, as in Go");

        for table in [
            "file",
            "space_o_id_c_group",
            "item_icon",
            "item_icon_group",
            "space_member",
            user_table,
            space_table,
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop admin fixture");
        }
    }

    async fn verify_space_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, timestamp_type, id_type, enabled_type) = match driver {
            Driver::MySql => (
                "`space`",
                "DATETIME(6)",
                "BIGINT AUTO_INCREMENT PRIMARY KEY",
                "TINYINT NOT NULL DEFAULT 0",
            ),
            Driver::Sqlite => (
                "\"space\"",
                "TEXT",
                "INTEGER PRIMARY KEY AUTOINCREMENT",
                "INTEGER NOT NULL DEFAULT 0",
            ),
            Driver::Postgres => (
                "\"space\"",
                "TIMESTAMPTZ",
                "BIGSERIAL PRIMARY KEY",
                "BOOLEAN NOT NULL DEFAULT false",
            ),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, type VARCHAR(20) NOT NULL, name VARCHAR(100) NOT NULL, owner_user_id BIGINT NOT NULL, pair_id BIGINT NOT NULL DEFAULT 0, side VARCHAR(16) NOT NULL DEFAULT 'yin', public_enabled {enabled_type} NOT NULL, public_id VARCHAR(80), public_mode VARCHAR(32) NOT NULL DEFAULT 'direct', public_code_hash VARCHAR(128), search_config_json TEXT, wallpaper_json TEXT)"
        ))
        .execute(pool)
        .await
        .expect("create space write fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_member (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL, source VARCHAR(32) NOT NULL, joined_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create space member write fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon_group (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create space group write fixture");
        sqlx::query(&format!(
            "CREATE TABLE item_icon (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create space item write fixture");

        create_paired_space(
            pool,
            driver,
            7,
            "Created",
            "2025-01-01 00:00:00",
        )
        .await
        .expect("create paired space");
        let created = space_list(pool, driver, 7)
            .await
            .expect("list created spaces");
        let yin = created
            .iter()
            .find(|space| space.kind == "shared" && space.side == "yin")
            .expect("created yin space");
        assert_eq!(yin.name, "Created");
        assert_eq!(yin.pair_id, yin.id);
        assert_eq!(yin.paired_space_id, Some(yin.pair_id + 1));
        assert!(yin.can_edit);
        let paired = paired_space_ids(pool, driver, yin.id)
            .await
            .expect("paired ids");
        assert_eq!(paired, vec![yin.id, yin.id + 1]);
        let yang_id = paired[1];
        assert_eq!(
            space_name(pool, driver, yang_id)
                .await
                .expect("read yang name"),
            Some("Created-B".to_string())
        );
        let member_rows: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM space_member WHERE space_id = ? AND role = 'admin'",
        ))
        .bind(yin.id)
        .fetch_one(pool)
        .await
        .expect("count created members")
        .try_get("total")
        .expect("total");
        assert_eq!(member_rows, 1);

        rename_paired_space(pool, driver, yin.id, "Renamed", "2025-01-02 00:00:00")
            .await
            .expect("rename paired space");
        assert_eq!(
            space_name(pool, driver, yin.id)
                .await
                .expect("read renamed yin"),
            Some("Renamed".to_string())
        );
        assert_eq!(
            space_name(pool, driver, yang_id)
                .await
                .expect("read renamed yang"),
            Some("Renamed-B".to_string())
        );

        let group_sql = query(
            driver,
            &format!(
                "INSERT INTO item_icon_group (created_at, updated_at, icon, title, description, sort, user_id, space_id, parent_id) VALUES ({}, {}, 'folder', 'Links', '', 0, ?, ?, NULL)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        sqlx::query(&group_sql)
            .bind("2025-01-03 00:00:00")
            .bind("2025-01-03 00:00:00")
            .bind(7_i64)
            .bind(yin.id)
            .execute(pool)
            .await
            .expect("insert source group");
        let group_id: i64 = {
            use sqlx::Row;
            sqlx::query(&query(
                driver,
                "SELECT id FROM item_icon_group WHERE space_id = ?",
            ))
            .bind(yin.id)
            .fetch_one(pool)
            .await
            .expect("read source group")
            .try_get("id")
            .expect("group id")
        };
        let item_sql = query(
            driver,
            &format!(
                "INSERT INTO item_icon (created_at, updated_at, icon_json, title, url, lan_url, mobile_url, description, open_method, sort, item_icon_group_id, user_id, space_id) VALUES ({}, {}, '{{}}', 'Link', 'https://example.test', '', '', '', 0, 1, ?, ?, ?)",
                timestamp_bind(driver),
                timestamp_bind(driver)
            ),
        );
        sqlx::query(&item_sql)
            .bind("2025-01-03 00:00:00")
            .bind("2025-01-03 00:00:00")
            .bind(group_id)
            .bind(7_i64)
            .bind(yin.id)
            .execute(pool)
            .await
            .expect("insert source item");

        copy_space(
            pool,
            driver,
            yin.id,
            7,
            "Renamed Copy",
            "2025-01-04 00:00:00",
        )
        .await
        .expect("copy space");
        let after_copy = space_list(pool, driver, 7)
            .await
            .expect("list after copy");
        let copy = after_copy
            .iter()
            .find(|space| space.name == "Renamed Copy")
            .expect("copied space");
        assert_eq!(copy.pair_id, 0, "copy keeps the Go behaviour: no pair");
        let copied_members: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM space_member WHERE space_id = ?",
        ))
        .bind(copy.id)
        .fetch_one(pool)
        .await
        .expect("count copied members")
        .try_get("total")
        .expect("total");
        assert_eq!(copied_members, 1);
        let copied_groups = groups_for_space(pool, driver, copy.id)
            .await
            .expect("copied groups");
        // The created Yin space already had its default APP group.
        assert_eq!(copied_groups.len(), 2);
        assert!(
            copied_groups.iter().any(|group| group.title == "Links"),
            "the source group must be copied"
        );
        let copied_items = items_for_space(pool, driver, copy.id, None, 0, 0)
            .await
            .expect("copied items");
        assert_eq!(copied_items.len(), 1);
        assert_eq!(copied_items[0].title, "Link");
        assert_eq!(copied_items[0].space_id, copy.id);

        for table in ["item_icon", "item_icon_group", "space_member", space_table] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop space write fixture");
        }
    }

    async fn verify_config_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let timestamp_type = match driver {
            Driver::MySql => "DATETIME(6)",
            Driver::Sqlite => "TEXT",
            Driver::Postgres => "TIMESTAMPTZ",
        };
        let id_type = match driver {
            Driver::MySql => "BIGINT AUTO_INCREMENT PRIMARY KEY",
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        sqlx::query(&format!(
            "CREATE TABLE module_config (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, user_id BIGINT NOT NULL, name VARCHAR(255) NOT NULL, value_json TEXT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create module config write fixture");
        sqlx::query("CREATE TABLE user_config (user_id BIGINT PRIMARY KEY, panel_json TEXT NOT NULL, language VARCHAR(16) NOT NULL)")
            .execute(pool)
            .await
            .expect("create user config write fixture");

        save_module_config(
            pool,
            driver,
            91,
            "home",
            r#"{"layout":"grid"}"#,
            "2024-12-01 00:00:00",
        )
        .await
        .expect("insert module config");
        assert_eq!(
            module_config_value(pool, driver, 91, "home")
                .await
                .expect("read inserted module config"),
            Some(r#"{"layout":"grid"}"#.to_string())
        );
        save_module_config(
            pool,
            driver,
            91,
            "home",
            r#"{"layout":"list"}"#,
            "2024-12-02 00:00:00",
        )
        .await
        .expect("update module config");
        assert_eq!(
            module_config_value(pool, driver, 91, "home")
                .await
                .expect("read updated module config"),
            Some(r#"{"layout":"list"}"#.to_string())
        );
        let rows: i64 = sqlx::query(&query(
            driver,
            "SELECT COUNT(*) AS total FROM module_config WHERE user_id = ?",
        ))
        .bind(91_i64)
        .fetch_one(pool)
        .await
        .expect("count module config rows")
        .try_get("total")
        .expect("total");
        assert_eq!(rows, 1, "the update must not insert a second row");

        save_user_config(
            pool,
            driver,
            91,
            Some(r#"{"theme":"dark"}"#),
            "zh-CN",
        )
        .await
        .expect("insert user config");
        let config = user_config(pool, driver, 91)
            .await
            .expect("read inserted user config")
            .expect("user config exists");
        assert_eq!(config.panel_json, r#"{"theme":"dark"}"#);
        assert_eq!(config.language, "zh-CN");
        save_user_config(pool, driver, 91, None, "en-US")
            .await
            .expect("update language only");
        let config = user_config(pool, driver, 91)
            .await
            .expect("read updated user config")
            .expect("user config remains");
        assert_eq!(config.panel_json, r#"{"theme":"dark"}"#);
        assert_eq!(config.language, "en-US");

        for table in ["user_config", "module_config"] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop config fixture");
        }
    }

    async fn verify_member_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, user_table, timestamp_type, id_type) = match driver {
            Driver::MySql => (
                "`space`",
                "`user`",
                "DATETIME(6)",
                "BIGINT AUTO_INCREMENT PRIMARY KEY",
            ),
            Driver::Sqlite => (
                "\"space\"",
                "\"user\"",
                "TEXT",
                "INTEGER PRIMARY KEY AUTOINCREMENT",
            ),
            Driver::Postgres => (
                "\"space\"",
                "\"user\"",
                "TIMESTAMPTZ",
                "BIGSERIAL PRIMARY KEY",
            ),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id {id_type}, owner_user_id BIGINT NOT NULL, pair_id BIGINT NOT NULL DEFAULT 0, side VARCHAR(16) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create member space fixture");
        sqlx::query(&format!(
            "CREATE TABLE {user_table} (id {id_type}, mail VARCHAR(255) NOT NULL, name VARCHAR(100) NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create member user fixture");
        sqlx::query(&format!(
            "CREATE TABLE space_member (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL, source VARCHAR(32) NOT NULL, joined_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create member row fixture");

        let insert_space = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, owner_user_id, pair_id, side) VALUES (?, ?, ?, ?)"
            ),
        );
        for (id, owner, pair_id, side) in [
            (1_i64, 1_i64, 1_i64, "yin"),
            (2, 1, 1, "yang"),
            (3, 9, 3, "yin"),
        ] {
            sqlx::query(&insert_space)
                .bind(id)
                .bind(owner)
                .bind(pair_id)
                .bind(side)
                .execute(pool)
                .await
                .expect("insert member space");
        }
        sqlx::query(&query(
            driver,
            &format!("INSERT INTO {user_table} (id, mail, name) VALUES (?, ?, ?)"),
        ))
        .bind(5_i64)
        .bind("member@example.test")
        .bind("Member")
        .execute(pool)
        .await
        .expect("insert member account");

        assert_eq!(
            paired_space_ids(pool, driver, 1).await.expect("pair"),
            vec![1, 2]
        );
        assert_eq!(
            paired_space_ids(pool, driver, 3).await.expect("unpaired"),
            vec![3]
        );
        assert_eq!(
            user_id_by_mail(pool, driver, "member@example.test")
                .await
                .expect("lookup mail"),
            Some(5)
        );
        assert_eq!(
            user_id_by_mail(pool, driver, "missing@example.test")
                .await
                .expect("missing mail"),
            None
        );
        assert_eq!(
            space_owner(pool, driver, 1).await.expect("owner"),
            Some(1)
        );

        let pair = paired_space_ids(pool, driver, 1).await.expect("pair again");
        upsert_space_member(
            pool,
            driver,
            &pair,
            5,
            "editor",
            "2024-11-01 00:00:00",
        )
        .await
        .expect("add member to both spaces");
        let members = space_members(pool, driver, 2)
            .await
            .expect("read twin members");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].role, "editor");
        assert_eq!(members[0].source, "manual");
        upsert_space_member(
            pool,
            driver,
            &pair,
            5,
            "admin",
            "2024-11-02 00:00:00",
        )
        .await
        .expect("existing member takes the new role");
        let members = space_members(pool, driver, 2)
            .await
            .expect("read updated twin members");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].role, "admin");

        set_member_role(pool, driver, &pair, 5, "viewer")
            .await
            .expect("set role across the pair");
        let members = space_members(pool, driver, 1)
            .await
            .expect("read original members");
        assert_eq!(members[0].role, "viewer");

        remove_space_member(pool, driver, &pair, 5)
            .await
            .expect("remove from both spaces");
        assert!(space_members(pool, driver, 1)
            .await
            .expect("empty original")
            .is_empty());
        assert!(space_members(pool, driver, 2)
            .await
            .expect("empty twin")
            .is_empty());

        for table in ["space_member", user_table, space_table] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop member fixture");
        }
    }

    async fn verify_sort_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (group_table, item_table, timestamp_type, id_type) = match driver {
            Driver::MySql => (
                "item_icon_group",
                "item_icon",
                "DATETIME(6)",
                "BIGINT AUTO_INCREMENT PRIMARY KEY",
            ),
            Driver::Sqlite => (
                "item_icon_group",
                "item_icon",
                "TEXT",
                "INTEGER PRIMARY KEY AUTOINCREMENT",
            ),
            Driver::Postgres => (
                "item_icon_group",
                "item_icon",
                "TIMESTAMPTZ",
                "BIGSERIAL PRIMARY KEY",
            ),
        };
        sqlx::query(&format!(
            "CREATE TABLE {group_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create sort group fixture");
        sqlx::query(&format!(
            "CREATE TABLE {item_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create sort item fixture");

        let first = create_group(pool, driver, 78, 7, "folder", "First", None, "2024-10-01 00:00:00")
            .await
            .expect("create first group");
        let second = create_group(pool, driver, 78, 7, "folder", "Second", None, "2024-10-01 00:00:00")
            .await
            .expect("create second group");
        let siblings = group_siblings(pool, driver, 78, None)
            .await
            .expect("load root siblings");
        assert_eq!(siblings, vec![first, second]);

        sort_groups(
            pool,
            driver,
            78,
            &[(first, 2), (second, 1)],
            "2024-10-02 00:00:00",
        )
        .await
        .expect("sort groups");
        let sorted = groups_for_space(pool, driver, 78)
            .await
            .expect("read sorted groups");
        assert_eq!(
            sorted.iter().map(|group| group.id).collect::<Vec<_>>(),
            vec![second, first]
        );

        let item = create_item(
            pool,
            driver,
            78,
            7,
            &super::ItemWriteRecord {
                icon_json: "{}".to_string(),
                title: "Item".to_string(),
                url: "https://example.test".to_string(),
                lan_url: String::new(),
                mobile_url: String::new(),
                description: String::new(),
                open_method: 0,
                sort: 5,
                group_id: first,
            },
            "2024-10-03 00:00:00",
        )
        .await
        .expect("create item for sort");
        let other = create_item(
            pool,
            driver,
            78,
            7,
            &super::ItemWriteRecord {
                icon_json: "{}".to_string(),
                title: "Other".to_string(),
                url: "https://example.test".to_string(),
                lan_url: String::new(),
                mobile_url: String::new(),
                description: String::new(),
                open_method: 0,
                sort: 1,
                group_id: first,
            },
            "2024-10-03 00:00:00",
        )
        .await
        .expect("create second item");
        sort_items(
            pool,
            driver,
            78,
            first,
            &[(item, 1), (other, 2)],
            "2024-10-04 00:00:00",
        )
        .await
        .expect("sort items");
        let items = items_for_space(pool, driver, 78, Some(first), 0, 0)
            .await
            .expect("read sorted items");
        assert_eq!(
            items.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![item, other]
        );

        for table in [item_table, group_table] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop sort fixture");
        }
    }

    async fn verify_item_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (group_table, item_table, timestamp_type, id_type) = match driver {
            Driver::MySql => (
                "item_icon_group",
                "item_icon",
                "DATETIME(6)",
                "BIGINT AUTO_INCREMENT PRIMARY KEY",
            ),
            Driver::Sqlite => (
                "item_icon_group",
                "item_icon",
                "TEXT",
                "INTEGER PRIMARY KEY AUTOINCREMENT",
            ),
            Driver::Postgres => (
                "item_icon_group",
                "item_icon",
                "TIMESTAMPTZ",
                "BIGSERIAL PRIMARY KEY",
            ),
        };
        sqlx::query(&format!(
            "CREATE TABLE {group_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create item write group fixture");
        sqlx::query(&format!(
            "CREATE TABLE {item_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon_json TEXT NOT NULL, title VARCHAR(50) NOT NULL, url TEXT NOT NULL, lan_url TEXT NOT NULL, mobile_url TEXT NOT NULL, description TEXT NOT NULL, open_method BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create item write fixture");

        let group = create_group(
            pool,
            driver,
            78,
            7,
            "folder",
            "Links",
            None,
            "2024-09-01 00:00:00",
        )
        .await
        .expect("create item group");
        let record = super::ItemWriteRecord {
            icon_json: r#"{"itemType":2,"src":"favicon.ico"}"#.to_string(),
            title: "Link".to_string(),
            url: "https://example.test".to_string(),
            lan_url: "http://lan.example.test".to_string(),
            mobile_url: String::new(),
            description: "note".to_string(),
            open_method: 2,
            sort: 3,
            group_id: group,
        };
        let item = create_item(pool, driver, 78, 7, &record, "2024-09-02 00:00:00")
            .await
            .expect("create item");
        assert!(item > 0);
        let row = item_by_id(pool, driver, 78, item)
            .await
            .expect("read created item")
            .expect("item exists");
        assert_eq!(row.title, "Link");
        assert_eq!(row.icon_json, record.icon_json);
        assert_eq!(row.lan_url, "http://lan.example.test");
        assert_eq!(row.open_method, 2);
        assert_eq!(row.sort, 3);
        assert_eq!(row.user_id, 7);
        assert_eq!(
            item_by_id(pool, driver, 78, item + 1)
                .await
                .expect("missing item"),
            None
        );

        let updated = super::ItemWriteRecord {
            title: "Renamed".to_string(),
            sort: 9,
            ..record.clone()
        };
        update_item(pool, driver, 78, item, &updated, "2024-09-03 00:00:00")
            .await
            .expect("update item");
        let row = item_by_id(pool, driver, 78, item)
            .await
            .expect("read updated item")
            .expect("item remains");
        assert_eq!(row.title, "Renamed");
        assert_eq!(row.sort, 9);

        delete_item(pool, driver, 78, item)
            .await
            .expect("delete item");
        assert_eq!(
            item_by_id(pool, driver, 78, item).await.expect("item gone"),
            None
        );

        sqlx::query(&format!("DROP TABLE {item_table}"))
            .execute(pool)
            .await
            .expect("drop item write fixture");
        sqlx::query(&format!("DROP TABLE {group_table}"))
            .execute(pool)
            .await
            .expect("drop item group fixture");
    }

    async fn verify_group_writes(pool: &sqlx::AnyPool, driver: Driver) {
        let (group_table, item_table, timestamp_type, id_type) = match driver {
            Driver::MySql => (
                "item_icon_group",
                "item_icon",
                "DATETIME(6)",
                "BIGINT AUTO_INCREMENT PRIMARY KEY",
            ),
            Driver::Sqlite => (
                "item_icon_group",
                "item_icon",
                "TEXT",
                "INTEGER PRIMARY KEY AUTOINCREMENT",
            ),
            Driver::Postgres => (
                "item_icon_group",
                "item_icon",
                "TIMESTAMPTZ",
                "BIGSERIAL PRIMARY KEY",
            ),
        };
        sqlx::query(&format!(
            "CREATE TABLE {group_table} (id {id_type}, created_at {timestamp_type} NOT NULL, updated_at {timestamp_type} NOT NULL, icon VARCHAR(100) NOT NULL, title VARCHAR(100) NOT NULL, description TEXT NOT NULL, sort BIGINT NOT NULL, user_id BIGINT NOT NULL, space_id BIGINT NOT NULL, parent_id BIGINT)"
        ))
        .execute(pool)
        .await
        .expect("create group write fixture");
        sqlx::query(&format!(
            "CREATE TABLE {item_table} (id {id_type}, space_id BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create group item fixture");

        let root = create_group(
            pool,
            driver,
            78,
            7,
            "folder",
            "Root",
            None,
            "2024-08-01 00:00:00",
        )
        .await
        .expect("create root group");
        assert!(root > 0);
        let root_row = group_by_id(pool, driver, 78, root)
            .await
            .expect("read root group")
            .expect("root group exists");
        assert_eq!(root_row.title, "Root");
        assert_eq!(root_row.icon, "folder");
        assert_eq!(root_row.sort, 9999);
        assert_eq!(root_row.item_count, 0);
        assert_eq!(
            group_by_id(pool, driver, 78, root + 1)
                .await
                .expect("missing group"),
            None
        );

        let child = create_group(
            pool,
            driver,
            78,
            7,
            "file",
            "Child",
            Some(root),
            "2024-08-02 00:00:00",
        )
        .await
        .expect("create child group");
        assert!(child > root);
        assert!(
            group_belongs(pool, driver, 78, root)
                .await
                .expect("belongs in space")
        );
        assert!(
            !group_belongs(pool, driver, 78, root + 100)
                .await
                .expect("unknown group")
        );
        assert!(
            group_has_descendant(pool, driver, 78, root, child)
                .await
                .expect("cycle detected")
        );
        assert!(
            !group_has_descendant(pool, driver, 78, child, root)
                .await
                .expect("no cycle upwards")
        );

        update_group(
            pool,
            driver,
            78,
            child,
            "Renamed",
            "icon",
            Some(root),
            "2024-08-03 00:00:00",
        )
        .await
        .expect("rename child group");
        assert_eq!(
            group_by_id(pool, driver, 78, child)
                .await
                .expect("read renamed child")
                .expect("child exists")
                .title,
            "Renamed"
        );

        for id in [1_i64, 2] {
            sqlx::query(&query(
                driver,
                &format!(
                    "INSERT INTO {item_table} (id, space_id, item_icon_group_id) VALUES (?, ?, ?)"
                ),
            ))
            .bind(id)
            .bind(78_i64)
            .bind(root)
            .execute(pool)
            .await
            .expect("insert grouped item");
        }
        delete_group_tree(pool, driver, 78, root)
            .await
            .expect("delete group tree");
        assert_eq!(
            group_by_id(pool, driver, 78, root)
                .await
                .expect("root gone"),
            None
        );
        assert_eq!(
            group_by_id(pool, driver, 78, child)
                .await
                .expect("child gone"),
            None
        );
        let remaining: i64 = sqlx::query(&query(
            driver,
            &format!("SELECT COUNT(*) AS total FROM {item_table} WHERE space_id = ?"),
        ))
        .bind(78_i64)
        .fetch_one(pool)
        .await
        .expect("count remaining items")
        .try_get("total")
        .expect("total");
        assert_eq!(remaining, 0);

        sqlx::query(&format!("DROP TABLE {item_table}"))
            .execute(pool)
            .await
            .expect("drop group item fixture");
        sqlx::query(&format!("DROP TABLE {group_table}"))
            .execute(pool)
            .await
            .expect("drop group write fixture");
    }

    async fn verify_admin_and_config_reads(pool: &sqlx::AnyPool, driver: Driver) {
        let (user_table, text_type) = if driver == Driver::MySql {
            ("`user`", "CHAR")
        } else {
            ("\"user\"", "TEXT")
        };
        let mysql_unused = text_type;
        let _ = mysql_unused;
        sqlx::query("CREATE TABLE module_config (id BIGINT PRIMARY KEY, user_id BIGINT NOT NULL, name VARCHAR(64) NOT NULL, value_json TEXT NOT NULL)")
            .execute(pool)
            .await
            .expect("create module config fixture");
        sqlx::query("CREATE TABLE file (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, file_name TEXT NOT NULL)")
            .execute(pool)
            .await
            .expect("create file fixture");
        sqlx::query(&format!(
            "CREATE TABLE {user_table} (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, name VARCHAR(100) NOT NULL, head_image TEXT NOT NULL, status BIGINT NOT NULL, role BIGINT NOT NULL, mail VARCHAR(255) NOT NULL, token_version BIGINT NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create managed user fixture");
        sqlx::query("CREATE TABLE user_config (user_id BIGINT PRIMARY KEY, panel_json TEXT NOT NULL, language TEXT NOT NULL)")
            .execute(pool)
            .await
            .expect("create user config fixture");
        sqlx::query("CREATE TABLE item_icon_group (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, sort BIGINT NOT NULL, title VARCHAR(100) NOT NULL, icon VARCHAR(100) NOT NULL, parent_id BIGINT)")
            .execute(pool)
            .await
            .expect("create bookmark group fixture");
        sqlx::query("CREATE TABLE item_icon (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, sort BIGINT NOT NULL, item_icon_group_id BIGINT NOT NULL, title VARCHAR(100) NOT NULL, url TEXT NOT NULL)")
            .execute(pool)
            .await
            .expect("create bookmark item fixture");
        sqlx::query("CREATE TABLE space_o_id_c_group (id BIGINT PRIMARY KEY, created_at VARCHAR(40) NOT NULL, updated_at VARCHAR(40) NOT NULL, space_id BIGINT NOT NULL, provider VARCHAR(64) NOT NULL, group_name VARCHAR(100) NOT NULL, role VARCHAR(32) NOT NULL)")
            .execute(pool)
            .await
            .expect("create oidc group fixture");

        let insert_module = query(
            driver,
            "INSERT INTO module_config (id, user_id, name, value_json) VALUES (?, ?, ?, ?)",
        );
        for (id, name, value) in [
            (1_i64, "home", r#"{"layout":"grid"}"#),
            (2, "other", r#"{"layout":"list"}"#),
        ] {
            sqlx::query(&insert_module)
                .bind(id)
                .bind(7_i64)
                .bind(name)
                .bind(value)
                .execute(pool)
                .await
                .expect("insert module config");
        }
        let insert_file = query(
            driver,
            "INSERT INTO file (id, created_at, updated_at, file_name) VALUES (?, ?, ?, ?)",
        );
        for (id, created_at, name) in [
            (1_i64, "2024-01-01 00:00:00", "older.png"),
            (2, "2024-01-03 00:00:00", "newer.png"),
        ] {
            sqlx::query(&insert_file)
                .bind(id)
                .bind(created_at)
                .bind("2024-01-04 00:00:00")
                .bind(name)
                .execute(pool)
                .await
                .expect("insert file");
        }
        let insert_user = query(
            driver,
            &format!(
                "INSERT INTO {user_table} (id, created_at, updated_at, name, head_image, status, role, mail, token_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
            ),
        );
        for (id, name, role) in [(7_i64, "Owner", 2_i64), (8, "Admin", 1)] {
            sqlx::query(&insert_user)
                .bind(id)
                .bind("2024-01-01 00:00:00")
                .bind("2024-01-02 00:00:00")
                .bind(name)
                .bind("/uploads/head.png")
                .bind(1_i64)
                .bind(role)
                .bind("user@example.test")
                .bind(3_i64)
                .execute(pool)
                .await
                .expect("insert managed user");
        }
        sqlx::query(&query(
            driver,
            "INSERT INTO user_config (user_id, panel_json, language) VALUES (?, ?, ?)",
        ))
        .bind(7_i64)
        .bind(r#"{"theme":"dark"}"#)
        .bind("zh-CN")
        .execute(pool)
        .await
        .expect("insert user config");
        let insert_group = query(
            driver,
            "INSERT INTO item_icon_group (id, space_id, sort, title, icon, parent_id) VALUES (?, ?, ?, ?, ?, ?)",
        );
        for (id, sort, title, parent) in
            [(1_i64, 1_i64, "Root", None), (2, 2, "Child", Some(1_i64))]
        {
            sqlx::query(&insert_group)
                .bind(id)
                .bind(78_i64)
                .bind(sort)
                .bind(title)
                .bind("folder")
                .bind(parent)
                .execute(pool)
                .await
                .expect("insert bookmark group");
        }
        for (id, group_id, title) in [(1_i64, 1_i64, "Root link"), (2, 2, "Child link")] {
            sqlx::query(&query(
                driver,
                "INSERT INTO item_icon (id, space_id, sort, item_icon_group_id, title, url) VALUES (?, ?, ?, ?, ?, ?)",
            ))
            .bind(id)
            .bind(78_i64)
            .bind(id)
            .bind(group_id)
            .bind(title)
            .bind("https://example.test")
            .execute(pool)
            .await
            .expect("insert bookmark item");
        }
        sqlx::query(&query(
            driver,
            "INSERT INTO space_o_id_c_group (id, created_at, updated_at, space_id, provider, group_name, role) VALUES (?, ?, ?, ?, ?, ?, ?)",
        ))
        .bind(1_i64)
        .bind("2024-05-01 00:00:00")
        .bind("2024-05-02 00:00:00")
        .bind(78_i64)
        .bind("github")
        .bind("team-admins")
        .bind("editor")
        .execute(pool)
        .await
        .expect("insert oidc group");

        assert_eq!(
            module_config_value(pool, driver, 7, "home")
                .await
                .expect("read module config"),
            Some(r#"{"layout":"grid"}"#.to_string())
        );
        assert_eq!(
            module_config_value(pool, driver, 7, "missing")
                .await
                .expect("missing module config"),
            None
        );
        let (file_total, files) = file_list(pool, driver, 1, 1)
            .await
            .expect("first file page");
        assert_eq!(file_total, 2);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name, "newer.png");
        let (_, all_files) = file_list(pool, driver, 0, 1).await.expect("unpaged files");
        assert_eq!(all_files.len(), 2);
        let (user_total, users) = user_list(pool, driver, 2, 1)
            .await
            .expect("first user page");
        assert_eq!(user_total, 2);
        assert_eq!(
            users.iter().map(|user| user.id).collect::<Vec<_>>(),
            vec![7, 8]
        );
        assert_eq!(users[1].mail, "user@example.test");
        assert_eq!(users[0].head_image, "/uploads/head.png");
        let (_, unpaged_users) = user_list(pool, driver, 0, 0)
            .await
            .expect("users with no limit");
        assert!(unpaged_users.is_empty());
        let config = user_config(pool, driver, 7)
            .await
            .expect("read user config")
            .expect("config exists");
        assert_eq!(config.language, "zh-CN");
        assert_eq!(config.panel_json, r#"{"theme":"dark"}"#);
        assert_eq!(
            user_config(pool, driver, 8)
                .await
                .expect("missing user config"),
            None
        );
        let (groups, items) = bookmark_tree(pool, driver, 78)
            .await
            .expect("read bookmark tree");
        assert_eq!(
            groups.iter().map(|group| group.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(groups[1].parent_id, Some(1));
        assert_eq!(items[1].title, "Child link");
        assert_eq!(items[0].url, "https://example.test");
        let oidc = oidc_groups(pool, driver, 78)
            .await
            .expect("read oidc groups");
        assert_eq!(oidc.len(), 1);
        assert_eq!(oidc[0].provider, "github");
        assert_eq!(oidc[0].group_name, "team-admins");
        assert_eq!(oidc[0].role, "editor");
        assert!(
            oidc_groups(pool, driver, 999)
                .await
                .expect("empty oidc groups")
                .is_empty()
        );

        for table in [
            "space_o_id_c_group",
            "item_icon",
            "item_icon_group",
            "user_config",
            user_table,
            "file",
            "module_config",
        ] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(pool)
                .await
                .expect("drop misc read fixture");
        }
    }

    async fn verify_space_wallpaper(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, timestamp_type) = match driver {
            Driver::MySql => ("`space`", "DATETIME(6)"),
            Driver::Sqlite => ("\"space\"", "TEXT"),
            Driver::Postgres => ("\"space\"", "TIMESTAMPTZ"),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id BIGINT PRIMARY KEY, wallpaper_json TEXT NOT NULL, updated_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create wallpaper space fixture");
        let insert = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, wallpaper_json, updated_at) VALUES (?, ?, {})",
                if driver == Driver::Postgres {
                    "CAST(? AS TIMESTAMPTZ)"
                } else {
                    "?"
                }
            ),
        );
        for (id, stored) in [
            (80_i64, ""),
            (
                81,
                r#"{"mode":"library","libraryEntry":"pack:theme/orbit"}"#,
            ),
        ] {
            sqlx::query(&insert)
                .bind(id)
                .bind(stored)
                .bind("2024-06-01 00:00:00")
                .execute(pool)
                .await
                .expect("insert wallpaper space");
        }

        assert_eq!(
            space_wallpaper_json(pool, driver, 80)
                .await
                .expect("read empty wallpaper"),
            Some(String::new())
        );
        assert_eq!(
            space_wallpaper_json(pool, driver, 81)
                .await
                .expect("read wallpaper"),
            Some(r#"{"mode":"library","libraryEntry":"pack:theme/orbit"}"#.to_string())
        );
        assert_eq!(
            space_wallpaper_json(pool, driver, 999)
                .await
                .expect("missing wallpaper space"),
            None
        );

        update_space_wallpaper(
            pool,
            driver,
            80,
            r#"{"mode":"custom","source":"/uploads/custom.jpg"}"#,
            "2024-07-02 03:04:05.123456789+00:00",
        )
        .await
        .expect("write wallpaper and timestamp");
        assert_eq!(
            space_wallpaper_json(pool, driver, 80)
                .await
                .expect("read written wallpaper")
                .expect("space remains"),
            r#"{"mode":"custom","source":"/uploads/custom.jpg"}"#
        );
        update_space_wallpaper(pool, driver, 80, "", "2024-07-03 00:00:00+00:00")
            .await
            .expect("clear wallpaper for mode none");
        assert_eq!(
            space_wallpaper_json(pool, driver, 80)
                .await
                .expect("read cleared wallpaper"),
            Some(String::new())
        );

        sqlx::query(&format!("DROP TABLE {space_table}"))
            .execute(pool)
            .await
            .expect("drop wallpaper space fixture");
    }

    async fn verify_space_public(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, enabled_type) = match driver {
            Driver::MySql => ("`space`", "BOOLEAN"),
            Driver::Sqlite => ("\"space\"", "INTEGER"),
            Driver::Postgres => ("\"space\"", "BOOLEAN"),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id BIGINT PRIMARY KEY, owner_user_id BIGINT NOT NULL, public_enabled {enabled_type} NOT NULL, public_id VARCHAR(64), public_mode VARCHAR(20) NOT NULL, public_code_hash VARCHAR(128))"
        ))
        .execute(pool)
        .await
        .expect("create public config space fixture");
        sqlx::query("CREATE TABLE space_member (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL)")
            .execute(pool)
            .await
            .expect("create public config member fixture");
        let insert_space = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, owner_user_id, public_enabled, public_id, public_mode, public_code_hash) VALUES (?, ?, ?, ?, ?, ?)"
            ),
        );
        for (id, owner, enabled, public_id, mode) in [
            (90_i64, 7_i64, true, Some("mine"), "code"),
            (91, 8, false, None, "direct"),
            (92, 8, false, None, "direct"),
            (93, 8, false, None, "direct"),
        ] {
            let mut statement = sqlx::query(&insert_space).bind(id).bind(owner);
            statement = if driver == Driver::Postgres {
                statement.bind(enabled)
            } else {
                statement.bind(i64::from(enabled))
            };
            statement
                .bind(public_id)
                .bind(mode)
                .bind(Some("hash"))
                .execute(pool)
                .await
                .expect("insert public config space");
        }
        let insert_member = query(
            driver,
            "INSERT INTO space_member (id, space_id, user_id, role) VALUES (?, ?, ?, ?)",
        );
        for (id, space_id, role) in [
            (1_i64, 91_i64, "admin"),
            (2, 92, "editor"),
            (3, 93, "viewer"),
        ] {
            sqlx::query(&insert_member)
                .bind(id)
                .bind(space_id)
                .bind(7_i64)
                .bind(role)
                .execute(pool)
                .await
                .expect("insert public config member");
        }

        assert!(
            can_manage_space(pool, driver, 7, 90)
                .await
                .expect("owner manages")
        );
        assert!(
            can_manage_space(pool, driver, 7, 91)
                .await
                .expect("admin manages")
        );
        assert!(
            !can_manage_space(pool, driver, 7, 92)
                .await
                .expect("editor cannot manage")
        );
        assert!(
            !can_manage_space(pool, driver, 7, 93)
                .await
                .expect("viewer cannot manage")
        );
        assert!(
            !can_manage_space(pool, driver, 9, 90)
                .await
                .expect("outsider cannot manage")
        );
        let config = space_public_config(pool, driver, 90)
            .await
            .expect("read public config")
            .expect("public config exists");
        assert!(config.enabled);
        assert_eq!(config.public_id.as_deref(), Some("mine"));
        assert_eq!(config.mode, "code");
        assert_eq!(
            space_public_config(pool, driver, 999)
                .await
                .expect("missing public config"),
            None
        );

        update_space_public_config(
            pool,
            driver,
            91,
            true,
            Some("published"),
            "code",
            Some("new-hash"),
        )
        .await
        .expect("enable public access");
        let enabled = space_public_config(pool, driver, 91)
            .await
            .expect("read enabled config")
            .expect("space remains");
        assert!(enabled.enabled);
        assert_eq!(enabled.public_id.as_deref(), Some("published"));
        update_space_public_config(pool, driver, 91, false, None, "direct", Some(""))
            .await
            .expect("disable public access");
        let disabled = space_public_config(pool, driver, 91)
            .await
            .expect("read disabled config")
            .expect("space remains");
        assert!(!disabled.enabled);
        assert_eq!(disabled.public_id, None);
        assert_eq!(disabled.mode, "direct");

        sqlx::query("DROP TABLE space_member")
            .execute(pool)
            .await
            .expect("drop public config member fixture");
        sqlx::query(&format!("DROP TABLE {space_table}"))
            .execute(pool)
            .await
            .expect("drop public config space fixture");
    }

    async fn verify_search_config(pool: &sqlx::AnyPool, driver: Driver) {
        let (space_table, timestamp_type) = match driver {
            Driver::MySql => ("`space`", "DATETIME(6)"),
            Driver::Sqlite => ("\"space\"", "TEXT"),
            Driver::Postgres => ("\"space\"", "TIMESTAMPTZ"),
        };
        sqlx::query(&format!(
            "CREATE TABLE {space_table} (id BIGINT PRIMARY KEY, owner_user_id BIGINT NOT NULL, search_config_json TEXT NOT NULL, updated_at {timestamp_type} NOT NULL)"
        ))
        .execute(pool)
        .await
        .expect("create search config space fixture");
        sqlx::query("CREATE TABLE space_member (id BIGINT PRIMARY KEY, space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(32) NOT NULL)")
            .execute(pool)
            .await
            .expect("create search config membership fixture");
        let insert_space = query(
            driver,
            &format!(
                "INSERT INTO {space_table} (id, owner_user_id, search_config_json, updated_at) VALUES (?, ?, ?, {})",
                if driver == Driver::Postgres {
                    "CAST(? AS TIMESTAMPTZ)"
                } else {
                    "?"
                }
            ),
        );
        for (id, owner, config) in [
            (10_i64, 7_i64, ""),
            (
                11,
                8,
                r#"{"currentSearchEngine":{"url":"https://old.test"}}"#,
            ),
            (12, 8, ""),
            (13, 8, ""),
        ] {
            sqlx::query(&insert_space)
                .bind(id)
                .bind(owner)
                .bind(config)
                .bind("2024-01-01 00:00:00")
                .execute(pool)
                .await
                .expect("insert search config space");
        }
        let member_insert = query(
            driver,
            "INSERT INTO space_member (id, space_id, user_id, role) VALUES (?, ?, ?, ?)",
        );
        for (id, space_id, role) in [(1_i64, 11_i64, "editor"), (2, 12, "viewer")] {
            sqlx::query(&member_insert)
                .bind(id)
                .bind(space_id)
                .bind(7_i64)
                .bind(role)
                .execute(pool)
                .await
                .expect("insert search config member");
        }

        let owned = space_search_config(pool, driver, 7, 10)
            .await
            .expect("read owned config")
            .expect("owned space exists");
        assert_eq!(owned.owner_user_id, 7);
        assert!(!owned.is_member);
        assert!(owned.config_json.is_empty());
        let member = space_search_config(pool, driver, 7, 11)
            .await
            .expect("read member config")
            .expect("member space exists");
        assert!(member.is_member);
        assert!(
            can_edit_space(pool, driver, 7, 10)
                .await
                .expect("owner can edit")
        );
        assert!(
            can_edit_space(pool, driver, 7, 11)
                .await
                .expect("editor can edit")
        );
        assert!(
            !can_edit_space(pool, driver, 7, 12)
                .await
                .expect("viewer cannot edit")
        );
        assert!(
            !can_edit_space(pool, driver, 7, 13)
                .await
                .expect("outsider cannot edit")
        );
        assert_eq!(
            space_search_config(pool, driver, 7, 999)
                .await
                .expect("missing space lookup"),
            None
        );

        let updated_config = r#"{"currentSearchEngine":{"url":"https://new.test"}}"#;
        update_space_search_config(
            pool,
            driver,
            11,
            updated_config,
            "2024-04-05 06:07:08.123456789+00:00",
        )
        .await
        .expect("write search config and timestamp");
        assert_eq!(
            space_search_config(pool, driver, 7, 11)
                .await
                .expect("read updated config")
                .expect("space remains")
                .config_json,
            updated_config
        );

        sqlx::query("DROP TABLE space_member")
            .execute(pool)
            .await
            .expect("drop search config member fixture");
        sqlx::query(&format!("DROP TABLE {space_table}"))
            .execute(pool)
            .await
            .expect("drop search config fixture");
    }

    #[tokio::test]
    async fn shared_repository_slices_work_on_sqlite() {
        let path = std::env::temp_dir().join(format!("yin-user-repo-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        verify_user_repository(
            Driver::Sqlite,
            &format!("sqlite://{}?mode=rwc", path.display()),
        )
        .await;
        std::fs::remove_file(path).expect("remove repository fixture database");
    }

    #[tokio::test]
    async fn shared_repository_slices_work_on_configured_mysql_and_postgres() {
        if let Ok(url) = std::env::var("YIN_MYSQL_TEST_URL") {
            verify_user_repository(Driver::MySql, &url).await;
        } else {
            eprintln!("skipping MySQL repository integration: YIN_MYSQL_TEST_URL is unset");
        }
        if let Ok(url) = std::env::var("YIN_POSTGRES_TEST_URL") {
            verify_user_repository(Driver::Postgres, &url).await;
        } else {
            eprintln!("skipping PostgreSQL repository integration: YIN_POSTGRES_TEST_URL is unset");
        }
    }

    async fn verify_oauth_identity(pool: &sqlx::AnyPool, driver: Driver) {
        let identity_id = match driver {
            Driver::Sqlite => "INTEGER PRIMARY KEY AUTOINCREMENT",
            Driver::MySql => "BIGINT PRIMARY KEY AUTO_INCREMENT",
            Driver::Postgres => "BIGSERIAL PRIMARY KEY",
        };
        for statement in [
            format!("CREATE TABLE o_auth_identity (id {identity_id}, created_at VARCHAR(40), updated_at VARCHAR(40), user_id BIGINT NOT NULL, provider VARCHAR(50) NOT NULL, subject VARCHAR(255) NOT NULL, email VARCHAR(255))"),
            format!("CREATE TABLE space_member (id {identity_id}, created_at VARCHAR(40), updated_at VARCHAR(40), space_id BIGINT NOT NULL, user_id BIGINT NOT NULL, role VARCHAR(20) NOT NULL, source VARCHAR(20) NOT NULL DEFAULT 'manual', joined_at VARCHAR(40))"),
            format!("CREATE TABLE space_o_id_c_group (id {identity_id}, created_at VARCHAR(40), updated_at VARCHAR(40), space_id BIGINT NOT NULL, provider VARCHAR(50) NOT NULL, group_name VARCHAR(255) NOT NULL, role VARCHAR(20) NOT NULL)"),
        ] {
            sqlx::query(&statement)
                .execute(pool)
                .await
                .expect("create oauth fixture");
        }
        let now = "2024-01-01 00:00:00.000000000+00:00";
        let text_type = if driver == Driver::MySql { "CHAR" } else { "TEXT" };

        link_oauth_identity(pool, driver, 17, "authentik", "subject-17", "user@example.test", now)
            .await
            .expect("link oauth identity");
        let identity = oauth_identity(pool, driver, "authentik", "subject-17")
            .await
            .expect("read oauth identity")
            .expect("identity exists");
        assert_eq!(identity.user_id, 17);
        assert_eq!(identity.email, "user@example.test");

        link_oauth_identity(pool, driver, 18, "authentik", "subject-17", "second@example.test", now)
            .await
            .expect("relink oauth identity");
        let identity = oauth_identity(pool, driver, "authentik", "subject-17")
            .await
            .expect("read relinked identity")
            .expect("identity still exists");
        assert_eq!(identity.user_id, 18);
        assert_eq!(identity.email, "second@example.test");

        sqlx::query("INSERT INTO space_o_id_c_group (id, space_id, provider, group_name, role) VALUES (1, 80, 'authentik', 'team', 'editor')")
            .execute(pool)
            .await
            .expect("insert oidc group rule");
        sqlx::query("INSERT INTO space_o_id_c_group (id, space_id, provider, group_name, role) VALUES (2, 81, 'authentik', 'staff', 'admin')")
            .execute(pool)
            .await
            .expect("insert second oidc group rule");
        let rules = oidc_group_rules(pool, driver, "authentik")
            .await
            .expect("read oidc group rules");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].group_name, "team");
        assert_eq!(rules[0].role, "editor");

        grant_oidc_member(pool, driver, 80, 18, "viewer", now)
            .await
            .expect("insert oidc member");
        grant_oidc_member(pool, driver, 80, 18, "editor", now)
            .await
            .expect("upgrade oidc member");
        grant_oidc_member(pool, driver, 80, 18, "viewer", now)
            .await
            .expect("ignore a lower oidc role");
        let role: String = sqlx::query_scalar(&format!("SELECT CAST(role AS {text_type}) FROM space_member WHERE space_id = 80 AND user_id = 18"))
            .fetch_one(pool)
            .await
            .expect("read oidc member role");
        assert_eq!(role, "editor");
        let source: String = sqlx::query_scalar(&format!("SELECT CAST(source AS {text_type}) FROM space_member WHERE space_id = 80 AND user_id = 18"))
            .fetch_one(pool)
            .await
            .expect("read oidc member source");
        assert_eq!(source, "oidc");

        sqlx::query("INSERT INTO space_member (id, space_id, user_id, role, source) VALUES (11, 81, 18, 'viewer', 'manual')")
            .execute(pool)
            .await
            .expect("insert manual member");
        grant_oidc_member(pool, driver, 81, 18, "admin", now)
            .await
            .expect("leave manual member alone");
        let manual_role: String = sqlx::query_scalar(&format!("SELECT CAST(role AS {text_type}) FROM space_member WHERE id = 11"))
            .fetch_one(pool)
            .await
            .expect("read manual role");
        assert_eq!(manual_role, "viewer");

        for statement in [
            "DROP TABLE space_o_id_c_group",
            "DROP TABLE space_member",
            "DROP TABLE o_auth_identity",
        ] {
            sqlx::query(statement)
                .execute(pool)
                .await
                .expect("drop oauth fixture");
        }
    }
