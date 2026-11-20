# Decisions

Architecture decisions for the Go -> Rust port of the backend. Each one is
binding until an ADR that supersedes it is added here.

The port is driven by [rust-anything](https://github.com/erkamyaman/rust-anything):
record what the Go backend does, port one unit, prove the port against the
recording, then hand the route over to Rust. Nothing is rewritten blind.

## ADR-001: Rust front door, one unit at a time

**Status:** accepted

The Rust binary owns the listening socket and proxies every route it does not
answer yet to the Go binary, which runs on a private port. Each ported unit is
wired with `rust-anything app wire <repo> <unit> --route ...`; the fallback rate
is the progress bar and `rust-anything app lift` is only allowed at 0%.

Rejected: a big-bang rewrite (no verifiable intermediate state, one very large
cutover) and a "hot path only" port (leaves two languages in the serving path
forever).

## ADR-002: Enterprise extensions are frozen

**Status:** accepted

`backend/pkg/extension` is a Go-typed contract: modules receive a
`*gin.RouterGroup` and a `*gorm.DB` and register `gin.HandlerFunc`s. A Rust
process cannot honour that ABI without an FFI layer around Gin and GORM, which
would keep the whole Go dependency tree alive.

Therefore `pkg/extension` is not ported. The Rust backend answers
`/api/system/capabilities` with `edition: "core"` and no capabilities, which is
exactly what Core does today when no module is registered. The EE contract is
re-designed after the Core port; until then EE images keep the Go backend.

## ADR-003: sqlx and hand-written SQL, schema-compatible

**Status:** accepted

The database schema, the migration order and the `migrate-db` command are
contracts with existing deployments. The Rust port keeps them: `sqlx` with
explicit SQL, no ORM, no regenerated migrations. Anything that would change a
column, an index or a default is a separate, reviewed migration with a rollback
plan, never a side effect of the port.

`migrate-db` (SQLite -> MySQL/PostgreSQL) is ported last, after every table's
reader and writer exist in Rust.

## ADR-004: axum replaces Gin

**Status:** accepted

`rust-anything analyze` maps Gin to axum with a partial fit: routing and
middleware differ. The behavioural gaps are covered by recorded HTTP cases:
longest-prefix routing, the `:param` / `*filepath` distinction, trailing-slash
redirects, automatic 405, gzip threshold, and the static-file cache headers.

Gin middleware is ported as axum layers with one dedicated unit per concern
(security headers, no-store, gzip, auth, public access), not as one blob.

## ADR-005: The recording runs against a throwaway database

**Status:** accepted

`rust/record/run-original.sh` builds the Go binary and starts it in a fresh
`$TMPDIR` directory with `rust/record/conf.yaml`, a fixed recording JWT secret
and an empty SQLite file. The deployment database, uploads and `conf.yaml` are
never read or written by a recording.

`rust/record/inputs.jsonl` therefore contains a session token signed with the
recording secret. It is not a credential for any deployment.

## ADR-006: `wallpaperpack` — validation at build time, `Asset` stays in Go

**Status:** accepted

`Catalogue` and `Find` (ported, `rust/wallpaperpack-rs`) read a catalogue that
`build.rs` validates and emits as Rust statics. Go validates on first use and
returns an error for a malformed pack; the pack is compiled in, so the Rust
port fails the build instead. This makes both functions total and keeps
`port.rs` free of an error path no recorded case can reach.

Consequence for `gate`: its `error cases` check still sees the Go signature's
`error` and fails for `Catalogue`. The port is wired with `--skip-gate` after
`diff` reached `212/212`; this exception applies only to units whose sole error
path is a compile-time-invalid asset and must be listed in `MAP.md`.

`Asset` was kept in Go at first, because the catalogue questions need only
media types. The asset route is now ported too: `build.rs` embeds each packaged
file with `include_bytes!`, exactly as Go's `embed.FS` does, which keeps the
route free of a runtime directory and made the front door binary 142 MiB. The
bytes and their headers were compared against the Go service (HTML and JPEG).

## ADR-007: JSON key order follows the Go encoder

**Status:** accepted

The Core returns maps (`gin.H`), and Go's `encoding/json` sorts map keys; it
returns structs, and those keep field order. The Rust side reproduces this per
response instead of choosing one global order:

- `serde_json` keeps its default sorted `Map` (no `preserve_order`) so the
  wired route serialises `gin.H`-shaped bodies byte for byte.
- Where Go marshals a struct, the Rust code builds a struct-shaped value and the
  recorded case decides the order.

## ADR-008: Cargo uses a mirror on this host only

**Status:** accepted

`crates.io` and `static.crates.io` are unreachable from the development host.
`~/.cargo/config.toml` replaces the registry with the USTC mirror. That file is
machine-local and is not committed; a checkout on a host with normal network
access needs nothing.

## ADR-009: `repository` and `database` are not unit-ported

**Status:** accepted, with a consequence for `app lift`

`internal/biz/repository` and `internal/infra/database` expose methods on types
that hold a package-level `*gorm.DB`; the bridge sees only constructors and
`CalcOffset`. Bridging them would produce a crate that proves nothing: the
behaviour under test is SQL against a live database, and the Go functions cannot
run without their global handle.

Those units are therefore not ported as crates. Their behaviour is proven at the
HTTP contract level (384 recorded cases, fallback 0.0%) and, for the schema, by
`rust/db-tool`'s upgrade and copy tests. The consequence is real: `app lift`
refuses to run while a unit has not passed `gate`, so the lift itself is blocked
until either the gate's unit model is satisfied by hand-written crates with
their own golden tests, or the team accepts route-level parity as the port's
definition of done for these two units.

## ADR-010: the Rust binary bootstraps and seeds its own database

**Status:** accepted

The Go build migrated and seeded at startup: schema upgrade, the three built-in
themes with their revisions and assets, the default administrator, its personal
Space pair and the default group. Removing the Go child therefore removed the
only thing that created a database.

`src/bootstrap.rs` now does the same: it runs `yin-db-tool`'s upgrade (the same
schema) and, when the database has no account, inserts a seed generated from a
freshly initialised Go database. The seed is not hand-written; it is captured
from the oracle, and the tests compare the result: 24 tables with identical row
counts, and identical `revision_record_v2` JSON and `asset_record_v2` bytes.

The alternative — shipping a prebuilt `.db` template — was rejected: a template
drifts from the schema the code expects, and every upgrade would have to patch
two shapes.

## ADR-011: `internal/infra/database` is one bridged unit (`MigrateDB`)

**Status:** accepted

`bridge` on `internal/infra/database` yields two exports: `MigrateDB`, which takes
the two configuration paths, and `CreateDefaultUser`, which needs the global
repositories that only `app.Init` sets up. Everything else takes or returns a
`*gorm.DB` and stays in Go with that reason recorded in
`rust/database-rs/rust-anything.json`.

`rust/database-rs` ports `MigrateDB` by delegating to `rust/db-tool`, so the copy
implementation is single-sourced, and it passes `rust-anything gate`
(233/233 recorded cases, including the failing configuration paths). The Go
binary's `migrate-db` subcommand now exists in the Rust binary too, with the same
flags, messages and exit codes.

Verified against Go on a live PostgreSQL 16: both produce the same 23 tables with
identical row counts and identical `user`, `space` and `revision_record_v2` rows.
Two differences were found and fixed by that comparison: Go resolves the source
path against the working directory (not the config file), and its target schema
has no `schema_migration` table.
