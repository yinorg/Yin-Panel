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

`Asset` (973 files, 132 MB) stays in Go: this crate must not embed the artwork
to answer two metadata questions. The asset route stays proxied until storage
is ported, and the Rust port then reads the pack from a directory shipped next
to the binary.

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
