# Handoff

Where the port stands. Read this first, then `MAP.md` and the ADRs that matter.

## State

| Thing | Value |
| --- | --- |
| Fallback rate | 0.0% (0/384), pure Rust with no Go process |
| Replay | 299/299 against the Go baseline and against the front door |
| Units ported | 2 (`internal/biz/theme/wallpaperpack`, `internal/util`) + the Rust Core spine |
| Routes owned by Rust | 43 registrations (about, capabilities, oauth config, login, logout, getUser, spaces, search-config, groups, items, members, public, moduleConfig, file list, bookmark tree, user list, user config, monitor status, theme preference, theme space, wallpaper library) |
| Wave | 0 — leaf units |
| Runtime | pure Rust: no Go child, no proxy; `src/bootstrap.rs` migrates and seeds a new install |
| Database tooling | `rust/db-tool` upgrade + copy to SQLite/MySQL/PostgreSQL; PostgreSQL verified live, MySQL DDL-tested |
| Database unit | `rust/database-rs` (`MigrateDB`) gate **passed**, 233/233; `migrate-db` is a command in the binary |
| Monitor | live snapshot ported (CPU/memory/net); verified against Go on the same host |
| Favicon | `getSiteFavicon` ported; manual comparison via a local fixture (no recorded cases) |
| Theme packages | parse/validate/install/preview ported; multipart verified manually, DTCG check narrower than Go |

## What is in place

- `rust/record/` — the reproducible recording harness (ADR-005).
- `rust/app-cases/` — 299 recorded HTTP cases; commit `replay-report.md`.
- `rust/app/` — axum front door. `src/units/mod.rs` and `rust/app/rust-anything.json`
  are rewritten by `app wire`; `src/original.rs` is the proxy.
- `rust/wallpaperpack-rs/` — first ported unit. `go/category.go` exists because
  `refresh` regenerates `go/wallpaperpack.go` without the element type.
- `rust/util-rs/` — `Md5`, `PathExists`, `PasswordEncryption`, `RedirectURL`
  (`gate` passed, 868/868). `InArray` and `ToJSONString` stay in Go (generic /
  `any`). One known diff: the Go bridge truncates a string at an embedded NUL, so
  `PathExists("\0")` arrives empty; recorded in `known-diffs.jsonl`.

## Next

1. `internal/util/i18n` (50 lines) — then `internal/infra/zaplog`.
2. `internal/constant/`, `internal/web/model/param/*` carry no functions, so
   they are ported as Rust consts/structs alongside their first consumer, not as
   units. `internal/web/model/response` takes `*gin.Context`, so its envelope is
   ported with the first route that needs it (see `rust/app/src/units/wallpaperpack.rs`).
3. `internal/util/monitor` (507 lines, readiness 36) needs `sysinfo`; decide
   whether the snapshot shape stays identical before porting.
4. Replace the proxy's `rust/record/run-original.sh` dependency only at
   `app lift`; until then the Go binary is the reference.

## Known gaps

- **E2E is behind Core.** The E2E repository's newest commit is 2026-10-11;
  Core has 82 commits after it, many of them home/theme surface changes
  (`1aa8dc1` one surface transparency, `212e827` colouring every surface from
  the scheme, `6957009`/`e7f*` home performance). A run against a dedicated
  fresh instance gives **32 passed / 20 failed / 1 flaky**; the failures are
  that drift (space-pill colour, layout preference shape, logo text, theme
  surface timeouts), plus two environment cases: the visual baseline differs by
  5% on this machine, and the PWA specs need an HTTPS origin (they have their
  own config). Updating those expectations is a design decision for the E2E
  repository, not something to bless from here.
- Recording harness note: `rust/record/run-original.sh` compiles the Go binary
  on first use. `app replay` kills a start that has not listened within 60 s, so
  build `build-output/yin-panel-go` once before the first replay on a cold
  checkout.
- `gate` `error cases` cannot pass for units whose only Go error path is
  impossible at runtime (ADR-006). Record the exception in `MAP.md` and use
  `--skip-gate` **only** after `diff` is 100%.
- `rust/app` has no CI job yet. Add `cargo fmt --check`, `cargo clippy -D
  warnings` and `rust-anything app replay --build rust/app/target/release/run-original`
  to `.github/workflows/` once the first wave is wired.
- Deployment still ships the Go binary. `scripts/deploy-local.sh` and
  `.github/workflows/ci-docker.yml` change only at cutover.
