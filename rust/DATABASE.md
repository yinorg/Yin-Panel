# Database layer: one query path across SQLite, MySQL and PostgreSQL

This document is the working plan for the shared database layer that the Rust
port uses. `PLAN.md` owns the batch order (B0–B11); this file owns the "how" of
the data layer: architecture, dialect boundaries, what is already migrated, what
is left, and how every slice is verified.

Status snapshot: written after the authentication, space, member, group, item,
search-config, public-config, wallpaper and admin/config read slices were
migrated and verified on SQLite, MySQL 8.4 and PostgreSQL 17.

## 1. Goal

One repository implementation serves all three supported databases. A route
handler should never see a driver-specific pool type or write driver-specific
SQL inline; it asks the repository for domain records.

Non-goals at this stage:

- No ORM. SQL stays hand-written so it matches the SQL GORM produces in the Go
  oracle. Recorded replay cases compare bytes, including row order, timestamps
  and NULL handling.
- No schema changes. Column names, defaults and migration order stay as they are
  until B9/B11.

## 1.1 Versioned migrations (MySQL and PostgreSQL)

SQLite keeps its in-place upgrade in `yin-db-tool`. For the shared drivers,
`rust/app/src/migrations.rs` owns an ordered list of migrations recorded in
`schema_migration`: each version is applied once, in order, and skipped when
already present. `bootstrap::prepare_shared` runs it after creating or finding
the schema and before the seed, so a release that changes a column can append a
migration instead of requiring a fresh database.

`0001_baseline` is a no-statement marker stamped on any existing shared
deployment; later entries append to `MIGRATIONS` and are never edited in place.
Statements are written in SQLite spelling and translated by the same
`schema_shared::table_statement` used for the initial schema. The SQLite unit
test proves ordering and idempotency, and the MySQL/PostgreSQL bootstrap test
asserts the baseline is recorded exactly once across two starts.

## 2. Architecture

```
handler (routes.rs)
    │  domain records + shared guard
    ▼
repository (repository.rs)
    │  SQL text, driver-aware casts, row decoding, transactions
    ▼
query layer (db.rs)
    │  driver selection, pool, `?` → `$n` rewrite
    ▼
sqlx Any  →  sqlite / mysql / postgres driver
```

`db.rs`:

- `Driver` enum: `Sqlite`, `MySql`, `Postgres`; `Driver::parse` and
  `url_scheme` for configuration.
- `connect(driver, url)`: installs the sqlx default drivers once (`OnceLock`)
  and builds an `AnyPool` with 4 connections for SQLite, 10 otherwise.
- `query(driver, sql)`: rewrites positional `?` into PostgreSQL `$n`, skipping
  quoted text, quoted identifiers and SQL comments.

`appcfg.rs`:

- `Ctx::shared_pool()` builds the `AnyPool` from `conf.yaml`:
  `base.database_drive` selects the driver; SQLite resolves the same file the
  Go binary uses; MySQL/PostgreSQL build a URL from host/port/user/db, with
  `ssl_mode` mapped to `sslmode`.
- `Ctx::sqlx_driver()` maps the configured driver to `db::Driver`.
- `Ctx::pool()` (SQLite-only) still exists for the routes not yet migrated.

`repository.rs` holds every migrated query. Records are plain structs, so
handlers do not depend on sqlx row types.

## 3. Dialect boundaries (all of these were hit in practice)

| Concern | What Any does not unify | Mitigation |
| --- | --- | --- |
| Placeholders | PostgreSQL needs `$1`; Any does not rewrite `?` | every SQL passes through `db::query(driver, …)` |
| Text columns | MySQL `TEXT` reports as `BLOB`; `String` decode fails | `text()` / `text_opt()` decode String first, then bytes → UTF-8 |
| Insert id | `last_insert_id` is `None` on SQLite and PostgreSQL under Any | `RETURNING id` on SQLite/PostgreSQL, `last_insert_id` on MySQL |
| Timestamps | PostgreSQL rejects text bound into `timestamptz` | `timestamp_bind()` emits `CAST(… AS TIMESTAMP WITH TIME ZONE)`; MySQL uses `DATETIME(6)` |
| Booleans | MySQL/SQLite store numeric, PostgreSQL has `boolean` | write binds bool on PostgreSQL and 0/1 elsewhere; read selects a driver-specific 0/1 expression |
| Identifiers | `"user"` vs `` `user` `` | table/column quoting chosen per driver |
| Cyclic writes | — | group/item path walks parent links with a hop cap (256) |

Rules that follow from this:

1. Never call `sqlx::query(...)` with `?` directly; always `sqlx::query(&query(driver, sql))`.
   A fixture that skipped this failed on PostgreSQL with `syntax error at or near ","`.
2. Never decode a text column with `row.try_get::<String, _>` alone; use `text()`.
3. Never assume an insert returns an id; use the repository insert helpers.
4. Keep driver branches in `repository.rs`, not in `routes.rs`.

## 4. What is migrated

Handlers on the shared pool:

| Area | Routes | Repository entry points |
| --- | --- | --- |
| Auth | `POST /api/login`, `POST /api/logout`, `GET /api/user/getUser` | `user_by_credentials`, `revoke_user_sessions`, `user_by_id` |
| Spaces | `GET /api/spaces`, `GET …/members` | `space_list`, `space_members`, `can_access_space` |
| Space config | `GET/POST …/search-config`, `GET/POST …/public`, `GET/POST …/wallpaper` | `space_search_config`, `update_space_search_config`, `can_manage_space`, `space_public_config`, `update_space_public_config`, `space_wallpaper_json`, `update_space_wallpaper` |
| Groups | `GET …/groups`, create/update/delete | `groups_for_space`, `create_group`, `group_by_id`, `update_group`, `delete_group_tree`, `group_belongs`, `group_has_descendant` |
| Items | `GET …/items`, create/update/delete | `items_for_space`, `create_item`, `item_by_id`, `update_item`, `delete_item` |
| Admin/config reads | `GET /api/system/moduleConfig/getByName`, `/api/file/getList`, `/api/panel/users/getList`, `/api/panel/userConfig/getConfig`, `…/bookmarks/tree`, `…/oidc/groups` | `module_config_value`, `file_list`, `user_list`, `user_config`, `bookmark_tree`, `oidc_groups` |

Progress metrics (recount before trusting them):

```sh
grep -c 'ctx\.pool()\.await'          rust/app/src/routes.rs   # 55 remaining
grep -c 'ctx\.shared_pool()\.await'   rust/app/src/routes.rs   # 19 migrated
grep -c 'last_insert_rowid'           rust/app/src/routes.rs   # 10 remaining
grep -c 'SqlitePool'                  rust/app/src/routes.rs   # helper signatures
```

## 5. What is left, in order

### 5.1 Remaining B2 write routes

Work resource by resource; each resource is one commit-sized slice.

1. **Sorting**: `sort_items`, `sort_groups` (multi-row updates in one transaction).
2. **Members**: add (paired-space insert), role update, remove (paired-space delete).
3. **Space lifecycle**: create (insert-id + paired Yang row), rename, copy, transfer,
   share/team, delete, clear.
4. **Files**: upload (insert-id + disk write), delete, S3/stored-file reads.
5. **Users**: create (insert-id + password hash), update, delete.
6. **Configs**: `moduleConfig/save`, `userConfig/setConfig`.
7. **Theme v2**: preference, admin revisions/packages/audit/trusted/install/
   rollback/trial/default, wallpaper web upload and asset serving.
8. **Icon packs**: `itemIcon` group/item operations, `create_item_with_icon`,
   site favicon fetch.

### 5.2 Then B3–B11

- B3 service layer and session parity (login/logout/version semantics).
- B4–B5 remaining system/panel routes.
- B6–B7 theme engine, wallpaper pack, storage.
- B8 monitor, favicon, cache, logging.
- B9 write paths, `schema_upgrade`, `migrate-db`.
- B10 app lift: remove the proxy and the Go process. Go deletion happens only
  after every route is on the shared layer and replay is green without fallback.
- B11 deployment: script, CI, Docker, tags, docs.

## 6. Verification gate for every slice

A slice is not done until all four pass:

```sh
# 1. unit + slice tests (SQLite path always runs)
cd rust/app && cargo test

# 2. real MySQL 8.4 and PostgreSQL 17
docker run --rm -d --name yin-mysql -e MYSQL_ROOT_PASSWORD=… \
  -e MYSQL_ROOT_HOST=% -e MYSQL_DATABASE=yin_test -p 127.0.0.1:13306:3306 mysql:8.4
docker run --rm -d --name yin-pg -e POSTGRES_PASSWORD=… \
  -e POSTGRES_DB=yin_test -p 127.0.0.1:15432:5432 postgres:17
YIN_MYSQL_TEST_URL='mysql://root:…@127.0.0.1:13306/yin_test' \
YIN_POSTGRES_TEST_URL='postgres://postgres:…@127.0.0.1:15432/yin_test' \
  cargo test repository::tests::shared_repository_slices_work_on_configured_mysql_and_postgres \
  -- --exact --nocapture

# 3. release build and byte-exact replay
cargo build --release
python3 -m http.server 18080 --bind 127.0.0.1 \
  --directory rust/record/favicon-fixture &
rust-anything app replay .
rust-anything app replay . --build 'sh rust/record/run-rust.sh'

# 4. whitespace
git diff --check -- rust/app/src
```

Operational notes:

- MySQL readiness must use TCP inside the container:
  `mysqladmin ping --protocol=tcp -h127.0.0.1 -P3306`. The default socket probe
  answers during the init phase and the first real TCP connection then fails.
- The favicon replay case expects the fixture server on **port 18080**. Running
  it on another port fails exactly one case with a body mismatch; that is an
  environment error, not a regression.
- Do not delete `rust/app-cases/**` whitespace "errors": recorded bodies are
  byte-exact payloads (HTML/CSS/request bodies). `git diff --check` flags them
  by design; replay equality is the real gate.

## 7. Known risks and decision points

- **Any is dynamic**: row decoding happens at runtime (`ColumnDecode`), so a
  missing three-DB test is the only way a type bug ships.
- **Performance**: Any adds dispatch overhead. If throughput in `BENCHMARKS.md`
  regresses materially, evaluate typed pools behind an enum, keeping the
  repository API unchanged.
- **Write semantics**: insert-id and timestamp conversion now work on all three
  drivers, but statement-level differences (e.g. MySQL `ON DUPLICATE KEY` vs
  PostgreSQL `ON CONFLICT`) must stay behind driver branches.
- **Transaction shape**: `AnyPool::begin()` returns `AnyTransaction`; loops over
  dynamic `IN` placeholders must reuse the same `driver` so PostgreSQL numbering
  stays consistent.

## 8. Resuming a session

1. `git status --short --branch`; the branch is `<username>-dev`.
2. Recount the metrics in §4 and pick the first unmigrated resource in §5.1.
3. Read the handler, then add the repository function before touching SQL.
4. Add the three-driver fixture to `verify_*` in `repository.rs`; fixtures
   create their own tables with driver-specific types and drop them again.
5. Run the full gate in §6; report numbers, not adjectives.
