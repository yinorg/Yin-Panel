# Upgrading a deployment from Go to Rust

The Rust binary reads the same `conf.yaml`, the same SQLite file, the same
`uploads/` directory and the same `web/` build. An upgrade is: stop the Go
process, replace `yin-panel`, start it. Going back is the same with the Go
binary. Nothing else moves.

## What is carried over

| State | Where it lives | Why it survives |
| --- | --- | --- |
| Accounts, passwords, `token_version` | `user` table | Rust runs the same schema upgrade (`rust/db-tool`) and reads the same columns |
| Sessions already issued | `yin_token` JWT | Same `jwt.secret` from `conf.yaml`, same claim set, so a Go-minted cookie is valid in Rust and vice versa |
| Spaces, groups, items, members, OIDC rules | their tables | Same schema; Rust writes the same rows |
| Theme packages, revisions, assets | `*_record_v2` | The upgrade installs built-in themes only when the table is empty |
| Selected theme and colour scheme | `activation_record_v2`, `user_theme_preference_v2` | Same rows, same scope strings (`instance`, `user:<id>`) |
| Uploaded files | `uploads/` + `file` table | The Rust static routes serve the same names from the same bucket path |
| Frontend | `web/` | Rust serves it with the same cache policies (`src/site.rs`) |

## Two things the upgrade does on first start

1. **Schema upgrade** — `yin-db-tool` adds columns newer releases introduced,
   removes rows that would block the unique indexes, normalises login
   addresses, merges duplicate accounts, retires legacy columns and their
   indexes, and only then creates `uk_user_mail`. It prints each step; a
   correctly upgraded database prints nothing.
2. **Seed** — only when `user` is empty. A deployed database is never touched.

## Verified

`scripts/verify-upgrade.sh [port]` (default 3040) runs the whole cycle on a
scratch directory:

```
phase 1  Go on an empty directory: seed, Space/group/item, upload, theme
phase 2  stop Go, note every row count, start Rust on the same directory
phase 3  Rust serves the Go session cookie, the Go password, the Go Space,
         item, upload and theme selection, and the frontend
phase 3b Rust writes its own Space and group
phase 3c no Go row is lost or duplicated (all 24 tables compared)
phase 4  back to Go: the Rust-minted session is accepted and the Space and
         group Rust wrote are readable
```

A run against a fresh Go deployment:

```
schema work Rust reported   (nothing to change)
session, password, spaces, items, upload, theme, static: OK
lost rows: none
Rust session, spaces and groups: OK
upgrade is seamless and reversible
```

The in-place upgrade of the **real** deployment was also exercised: its
`space` table still carried the retired `team_id` column and the
`idx_space_team_id` index. The first Rust start dropped the index and the
column, reported it in the log, and the service came up with all existing data.

## Known limits

- **Stop the Go process before switching.** A plain file copy taken while Go is
  writing misses frames still in the write-ahead log (this bit the
  `migrate-db` work twice). Both the deploy script and the Go binary's own
  `migrate-db` snapshot through SQLite (`VACUUM INTO` / `sqlite3_backup`) for
  that reason.
- **Do not run both binaries against the same file at once.** They are separate
  deployments of the same service, not a cluster.
- The rollback binary must be kept somewhere until the upgrade is validated:
  `build-output/yin-panel-go`, or rebuild with
  `cd backend && go build -o ../build-output/yin-panel-go .`.
