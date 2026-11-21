# Rust port of the Go backend

The backend is being rewritten in Rust with
[rust-anything](https://github.com/erkamyaman/rust-anything), one verified unit
at a time. The Go backend stays the reference implementation and answers every
route the Rust front door has not taken over yet.

Read `DECISIONS.md` first, then `MAP.md` (the queue) and `HANDOFF.md` (where the
last session stopped).

## Requirements

- Rust 1.97+ (`rustup`), Go (the version in `backend/go.mod`), and
  `cargo install rust-anything`.
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
