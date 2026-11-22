# Rust port of the Go backend

The Core backend now runs on Rust: every registered route is served by the
Rust binary (`rust/app`), the shared repository layer covers SQLite, MySQL and
PostgreSQL, and recorded replay is green. The Go tree has been removed; recorded replay is the contract.

Read `DECISIONS.md` first, then `MAP.md` (the queue) and `HANDOFF.md` (where the
last session stopped).

## Requirements

- Rust 1.97+ (`rustup`) and `cargo install rust-anything` for the recorded
  replay. The Go oracle is gone; re-recording requires the historical tree.
- crates.io reachable, or a mirror in `~/.cargo/config.toml` (ADR-008).

## The loop

```sh
# What the Go backend does today, recorded once (299 cases).
rust-anything app record . --http \
  --inputs rust/record/inputs.jsonl \
  --normalize timestamps \
  --normalize 'jwt=eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+' \
  -- sh rust/record/run-original.sh '{port}'

# The original must pass its own recording.
rust-anything app replay .

# One unit: record what it does, write the Rust, prove it, gate it.
rust-anything bridge backend internal/biz/theme/wallpaperpack/ -o rust/wallpaperpack-rs
rust-anything record rust/wallpaperpack-rs
rust-anything build rust/wallpaperpack-rs
rust-anything diff rust/wallpaperpack-rs
rust-anything gate rust/wallpaperpack-rs

# Hand the route to Rust and prove the whole app again.
rust-anything app wire . wallpaperpack-rs \
  --route 'GET /api/theme/v2/wallpaper/library'
rust-anything app replay . --build rust/app/target/release/run-original
```

The preview API creates an opaque random token, so its upload and follow-up
reads need a stateful differential run rather than independent recorded cases:

```sh
python3 rust/record/replay-preview-sequence.py
python3 rust/record/replay-monitor-sequence.py
```

This uploads the same package to Go and Rust independently, follows each
response's token to the preview and declared asset, and compares the results
after normalizing only the generated token and its URL. The monitor sequence
uses an isolated recorder config with monitoring enabled and compares disk
responses exactly; snapshot metrics are compared by structure and type because
they change between collection times.

`RUST_ANYTHING_UNIT_WALLPAPERPACK=original` sends a wired unit back to Go while
the port is suspect.

## Continuous checks

| Workflow | Trigger | What it proves |
| --- | --- | --- |
| `rust-check.yml` | every pull request and push | parity metrics, `cargo test`, release build, route inventory, recorded coverage, 665-case replay |
| `rust-db.yml` | changes under `rust/app/src`, `rust/app/Cargo*` or `rust/db-tool` | repository slices and bootstrap on real MySQL 8.4 and PostgreSQL 17 |
| `rust-nightly.yml` | nightly schedule and manual dispatch | HTTP login/session smoke on SQLite, MySQL and PostgreSQL, plus `migrate-db` into both servers |
| `ci-docker.yml` | version tags and manual dispatch | multi-arch release image |
| `e2e.yml` | nightly schedule and manual dispatch | Core + extension Playwright acceptance, including OIDC through the mock provider |
| `perf.yml` | nightly schedule and manual dispatch | k6 smoke/load thresholds against the Rust binary |

`rust-db.yml` is path-filtered because pulling both database images costs more
than the tests themselves; `rust-nightly.yml` carries the heavier acceptance run.

## Layout

| Path | What it is |
| --- | --- |
| `rust/app/` | axum front door; `src/original.rs` proxies to Go |
| `rust/app-cases/` | recorded HTTP behaviour, `replay-report.md` committed |
| `rust/record/` | throwaway-database recording harness (ADR-005) |
| `rust/wallpaperpack-rs/` | first ported unit, with its own cases |
| `rust/MAP.md` | every unit, in port order, with state |
| `rust/HANDOFF.md` | what the next session should do |
| `rust/DATABASE.md` | shared SQLx layer: dialect boundaries, migrated slices, verification gate |
| `rust/GOAL_REPLACE_GO.md` | goal-executable plan to replace the Go backend |

## Rules

- Never record against the deployment database or its uploads; the harness uses
  a fresh `$TMPDIR` directory.
- Never change a Go file to make a port easier. The original is the oracle.
- A unit is done when `diff` is 100% **and** `gate` passes; an exception is
  recorded in `MAP.md` and `DECISIONS.md`, never silently skipped.
