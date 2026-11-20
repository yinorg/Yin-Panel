# Port map

Every unit `rust-anything analyze backend --lang go --whole-app` lists, in port
order. State is updated at each checkpoint; `HANDOFF.md` says what is next.

Regenerate the plan with:

```sh
rust-anything analyze backend --lang go --whole-app
```

| Wave | Unit | LOC | Readiness | Status | Port state |
| ---: | --- | ---: | ---: | --- | --- |
| 0 | `internal/util/monitor/` | 507 | 36 | caution | todo |
| 0 | `internal/biz/theme/wallpaperpack/` | 179 | 79 | ready | ported: diff 212/212, route wired (gate: error-cases exception) |
| 0 | `pkg/storage/` | 127 | 64 | ready | todo |
| 0 | `internal/infra/kvcache/` | 89 | 69 | caution | todo |
| 0 | `internal/web/model/base/` | 73 | 68 | caution | todo |
| 0 | `internal/util/` | 47 | 78 | ready | ported: gate passed, 868/868 (`InArray`/`ToJSONString` kept in Go) |
| 0 | `internal/infra/zaplog/` | 46 | 81 | ready | todo |
| 0 | `internal/web/model/param/commonApi/` | 25 | 100 | ready | todo |
| 0 | `internal/constant/` | 14 | 99 | ready | todo |
| 0 | `internal/web/model/param/systemApi/` | 4 | 100 | ready | todo |
| 1 | `internal/biz/repository/` | 705 | 4 | caution | todo |
| 1 | `internal/biz/cache/` | 150 | 63 | ready | todo |
| 1 | `internal/infra/config/` | 140 | 78 | caution | todo |
| 1 | `pkg/extension/` | 97 | 78 | ready | frozen (ADR-002) |
| 1 | `internal/util/i18n/` | 50 | 87 | ready | todo |
| 2 | `internal/biz/service/` | 811 | 3 | caution | todo |
| 2 | `internal/web/model/response/` | 104 | 70 | ready | todo |
| 2 | `internal/util/jwt/` | 70 | 70 | ready | todo |
| 2 | `internal/web/model/param/panelApi/` | 20 | 100 | ready | todo |
| 3 | `internal/global/` | 40 | 87 | ready | todo |
| 4 | `internal/biz/theme/` | 2507 | 0 | caution | todo |
| 4 | `internal/web/interceptor/` | 235 | 68 | ready | todo |
| 4 | `internal/util/favicon/` | 134 | 73 | caution | todo |
| 5 | `internal/web/router/panel/` | 2411 | 0 | caution | todo |
| 5 | `internal/web/router/system/` | 1874 | 0 | caution | todo |
| 5 | `internal/infra/database/` | 1806 | 0 | caution | todo |
| 6 | `internal/web/router/` | 243 | 49 | caution | todo |
| 7 | `pkg/app/` | 194 | 75 | ready | todo |
| 8 | `main.go/` | 61 | 95 | ready | todo |

## App-level state

- Original recorded: 299 HTTP cases in `rust/app-cases/` (35 named inputs, 264
  traps), `299/299` passing against the Go baseline.
- Front door: `rust/app` (axum), `299/299`, fallback `99.7%` (298/299).
- Recorded cases: 384 (120 named inputs + 264 traps); original 384/384, and the
  pure Rust binary 384/384 with fallback 0.0% (run it with
  `--build "sh rust/record/run-rust.sh"`, which puts it in the recording working
  directory).
- Ported units and their gate status:
  | crate | gate | cases |
  | --- | --- | --- |
  | `util-rs` | **pass** | 868/868 |
  | `database-rs` (`MigrateDB`) | **pass** | 233/233 |
  | `response-rs` (`GetErrorMsgByCode`) | **pass** | 25/25 |
  | `storage-rs` (`ContentAddressedKey`) | **pass** | 100/100 |
  | `theme-rs` (manifest/grant/scope/web-wallpaper) | **pass** | 111/111 |
  | `repository-rs` (7 repository methods, via a non-product Go adapter) | **pass** | 344/344 |
  | `wallpaperpack-rs` | fails **only** `error cases` | 212/212 |
  | `internal/biz/service` | cannot be bridged: every export depends on a repository interface | — |
  | `internal/web/model/base` | cannot be bridged: `interface{}` and `*gin.Context` | — |
  | `pkg/app` | `Init` starts the server; nothing to record | — |
  | `internal/infra/zaplog` | `InitLog` only fails when `./logs` cannot be created, which no input reaches | — |
- Database unit: `rust/database-rs` is bridged from `internal/infra/database` and
  **passes gate** (233/233 recorded cases, error cases included). It ports
  `MigrateDB`; the rest of that package takes the package-level `*gorm.DB` and
  stays in Go, with the reason recorded per export in its manifest. The binary's
  `migrate-db` command calls the same function.
- `repository` is ported through `backend/internal/tools/repositoryport`, a
  non-product adapter: `bridge` on `internal/biz/repository` itself yields only
  the constructors (ADR-009), so the adapter opens the handle for one call and
  delegates to the real methods. `rust/repository-rs` passes the gate with 344
  cases (ADR-013).
- `app lift` builds a copy of `rust/app` (without `rust/record/`) and replays it
  from the repository root, so the configuration must exist in the recording
  directory it falls back to. Prepare it first:
  ```
  rm -rf /tmp/yin-panel-record && mkdir -p /tmp/yin-panel-record/uploads
  cp rust/record/conf.yaml /tmp/yin-panel-record/conf.yaml
  rust-anything app lift .
  ```
  With that, the lift replay passes 384/384 and the only blocker left is:
- `app lift` blockers, as the tool reports them: `wallpaperpack-rs` fails the
  `error cases` gate check. Every other check passes; that one asks for a
  recorded case where `Catalogue` fails, and its only error comes from a broken
  compile-time `embed`, so no input can reach it. ADR-009 also leaves
  `repository`/`database` without unit gates.
- Rust answers directly (43 route registrations):
  - `GET /api/theme/v2/wallpaper/library` -> `rust/wallpaperpack-rs`
  - `GET /api/about`, `GET /api/system/capabilities`, `GET /api/oauth/config`,
    `POST /api/login`, `POST /api/logout`, `GET /api/user/getUser`,
    `GET /api/spaces`, `GET /api/spaces/{id}/search-config`,
    `GET /api/spaces/{id}/groups`, `GET /api/spaces/{id}/items`,
    `GET /api/spaces/{id}/members`, `GET /api/spaces/{id}/public`,
    `GET /api/system/moduleConfig/getByName`, `GET /api/file/getList`,
    `GET /api/spaces/{id}/bookmarks/tree`, `GET /api/panel/users/getList`,
    `GET /api/panel/userConfig/getConfig`, the monitor status readers,
    `GET /api/theme/v2/preference`, `GET /api/theme/v2/space/{id}`,
    the theme reads (`current`, `packages`, `package/{id}`, `mine`, `effective`)
    -> `rust/app`
- Ported library units not yet wired to a route: `rust/util-rs`.
- The binary is **pure Rust**: the Go child process, the proxy and the
  `State<Original>` plumbing are gone. `src/bootstrap.rs` creates the database,
  runs the schema upgrade and seeds a new installation; the seed was generated
  from a freshly initialised Go database and is byte-identical (revision JSON,
  token documents and every theme asset).
- Fallback rate: **0.0% (0/384)** over the recorded contract, with no Go process
  running. The front door now
  recognises the Go route table: a registered path is proxied, anything else
  answers Gin's 404, so the 264 trap cases are Rust's.
- Every one of the 104 routes the Go build registers now has a Rust
  implementation. The recorded contract (384 cases) is answered entirely by Rust.
- Monitor: the live snapshot (`CPU_INFO`, `MEMORY_INFO`, `NETWORK_INFO`) is
  collected in Rust; memory and CPU identity verified against Go on the same
  host.
- Wallpaper library assets: the packaged artwork is embedded and served by Rust
  (verified byte-identical against Go for HTML and JPEG, with the same 404s).
- Favicon: `getSiteFavicon` ported and compared against Go through the fixture
  in `rust/record/favicon-fixture/` (not recordable, so verified by hand).
- Theme package pipeline: archive parsing, manifest and resource validation,
  Ed25519 signature check, preview store, `admin/preview`, `preview/:token`, its
  asset route and `admin/install` are ported and verified against Go. The DTCG
  validator is narrower than `dtcg_v2.go`. Installing has no recorded cases
  (the recorder cannot attach files) and is verified manually.
- `rust/db-tool`: SQLite schema upgrade (all steps) and `migrate-db` into
  SQLite, MySQL and PostgreSQL, with tests. The PostgreSQL path was verified
  against a real server (24 tables, row counts matched, re-runnable). MySQL is
  covered by DDL unit tests only.

## Static site

`rust/app/src/site.rs` serves the built frontend when `enable_static_server` is
on: `/assets/*` and `/workbox-*.js` immutable for 30 days, `/custom/*` for one
day, the entry documents, PWA control files and SPA fallbacks (`/`, `/login`,
`/oauth/callback`, `/theme/:view`, `/:publicId`) uncached, and `/clear.html`
also expiring the session cookie. Compared against the running Go service on
the same build output: identical bytes for `/`, `/sw.js` and a hashed asset,
identical cache/type/length/last-modified headers, and identical empty 404s
with the group's cache policy for a missing asset.

## Cutover

`scripts/deploy-rust.sh` builds `rust/app` in release mode, stops only the
Yin-Panel listener, installs the binary as `backend/yin-panel` and starts it,
keeping `web/`, the database, `uploads/` and `conf.yaml`. The build metadata is
read from the environment at start, so the same binary serves a deployment (with
`YIN_PANEL_VERSION` and friends) and a replay (without them, which must see the
recorded defaults).

Two differences were found by comparing the running services and fixed: Gin
answers a method mismatch with its NoRoute (404 `text/plain`), not 405 with an
`Allow` header; and Gin registers `HEAD` only for the static files, while axum
answers `HEAD` from every `GET` handler.

Rollback is `cp build-output/yin-panel-go backend/yin-panel` and restart, or
`cd backend && go build -o ../build-output/yin-panel-go .` first if it is stale.
