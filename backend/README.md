# Yin-Panel backend (Rust)

The Core backend runs on Rust: `backend/app` serves every registered route,
the shared repository layer covers SQLite, MySQL 8.4 and PostgreSQL 17, and
`backend/app-cases` is the recorded replay contract (665 cases, 0 fallbacks).

## Requirements

- Rust 1.97+ (`rustup`).
- `cargo install rust-anything` for recording and replay.
- crates.io reachable, or a mirror in `~/.cargo/config.toml` (ADR-008).

## The loop

```sh
# Build the binary used by the replay.
cargo build --release --manifest-path backend/app/Cargo.toml

# Run the recorded contract.
sh scripts/replay.sh

# One ported unit, with its own recorded cases.
rust-anything record backend/wallpaperpack-rs
rust-anything build backend/wallpaperpack-rs
rust-anything diff backend/wallpaperpack-rs
rust-anything gate backend/wallpaperpack-rs
```

The preview API mints an opaque token, so its upload and follow-up reads need a
stateful differential run rather than independent recorded cases:

```sh
python3 backend/record/replay-preview-sequence.py
python3 backend/record/replay-monitor-sequence.py
```

## Continuous checks

| Workflow | Trigger | What it proves |
| --- | --- | --- |
| `rust-check.yml` | every pull request and push | parity metrics, app and db-tool tests, release build, route inventory, recorded coverage, 665-case replay |
| `rust-db.yml` | changes under `backend/app/src`, `backend/app/Cargo*` or `backend/db-tool` | repository slices and bootstrap on real MySQL 8.4 and PostgreSQL 17 |
| `rust-nightly.yml` | nightly schedule and manual dispatch | HTTP login/session smoke on SQLite, MySQL and PostgreSQL, plus `migrate-db` into both servers |
| `ci-docker.yml` | version tags and manual dispatch | multi-arch release image from `docker/Dockerfile` |
| `e2e.yml` | nightly schedule and manual dispatch | Core + extension Playwright acceptance, including OIDC through the mock provider |
| `perf.yml` | nightly schedule and manual dispatch | k6 smoke/load thresholds against the Rust binary |

## Layout

| Path | What it is |
| --- | --- |
| `backend/app/` | axum front door, routes and repository wiring |
| `backend/app/src/routes/` | route modules by domain |
| `backend/app/src/repository/` | shared SQLx repository modules |
| `backend/app-cases/` | recorded HTTP behaviour; `replay-report.md` is committed |
| `backend/record/` | throwaway-database recording harness (ADR-005) |
| `backend/check/` | route parity, coverage, smoke and migration checks |
| `backend/db-tool/` | `yin-db` upgrade/copy tool |
| `backend/wallpaperpack-rs/` | packaged wallpaper library |
| `../docs/engineering/DATABASE.md` | shared SQLx layer and verification gate |
| `../docs/engineering/migration/MAP.md` | migration history, in port order |
| `../docs/engineering/migration/GOAL_REPLACE_GO.md` | the completed replacement plan |

## Rules

- Never record against the deployment database or its uploads; the harness uses
  a fresh `$TMPDIR` directory.
- The recorded cases are the contract. Do not change a case to make the Rust
  build pass; fix the build or record a documented exception.
