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

`scripts/verify-upgrade.sh [port] [--keep]` (default port 3040) runs the whole
cycle on a scratch directory:

```
phase 1  Go on an empty directory: seed, Space/group/item, upload, theme
phase 2  stop Go, snapshot every table row and upload, start Rust on the same directory
phase 3  Rust serves the Go session cookie, the Go password, the Go Space,
         group, item, the upload (byte-for-byte) and theme selection, and the frontend
phase 3b Rust writes its own Space and group
phase 3c every Go row and upload still present, compared by content
phase 4  back to Go: the Rust-minted session is accepted and the Space and
         group Rust wrote are readable
phase 4b content comparison again, after Go has read the Rust state
```

### What "compared by content" means

`scripts/upgrade_compare.py snapshot` records every table with its columns and
every row as a column-name to value map; `compare` treats the rows as multisets.
A row count would not notice a value rewritten in place, or one row deleted and
another inserted. Both are failures here, and uploads are compared by sha256,
not by an HTTP status. Columns a schema upgrade adds or retires are reported
separately and the surviving rows are compared on the columns both snapshots
share.

The tool proves what it catches:

```
$ python3 scripts/upgrade_compare.py selftest
  ok   identical database passes
  ok   rewritten value is caught
  ok   delete plus insert is caught
  ok   added column is tolerated
  ok   changed upload is caught
  ok   deleted upload is caught
  ok   ignoring a primary key is refused
  ok   ignored column still detects a deleted row
  8/8 comparison self-checks pass
```

### The script only stops what it started

`up` refuses to run when the target port is busy, and the run fails if the
listener is not the child it started. It records one pid and kills only that
pid, so a shared host cannot lose an unrelated service to this test. A busy
port is an operator decision, never something the script resolves by killing a
process.

```
$ ./scripts/verify-upgrade.sh 3055        # 3055 already served by a dummy
FAIL: port 3055 is already in use by pid 25360; refusing to start a test that would stop it
$ curl -o /dev/null -w '%{http_code}' http://127.0.0.1:3055/   # dummy still up
200
```

A run against a fresh Go deployment:

```
schema work Rust reported   (nothing to change)
session, password, spaces, groups, items, upload bytes, theme, static: OK
lost rows: none
every row and upload present: 36 rows across 24 tables, 1 uploads
Rust session, spaces and groups: OK
every row and upload present: 36 rows across 24 tables, 1 uploads
upgrade keeps every row and byte, and is reversible
```

The in-place upgrade of the **real** deployment was also exercised: its
`space` table still carried the retired `team_id` column and the
`idx_space_team_id` index. The first Rust start dropped the index and the
column, reported it in the log, and the service came up with all existing data.

## Known differences

- **Go re-installs the built-in themes on every start; Rust does not.** Go's
  `EnsureBuiltinV2` calls `InstallPackageV2` for all three built-ins at boot,
  and the GORM update bumps `package_record_v2.updated_at` even when the revision
  is unchanged. Rust seeds them only on an empty database. Consequence: a
  database whose built-in revisions are older than the shipped ones keeps the
  old themes after a switch to Rust, where Go would have upgraded them. The
  timestamp refresh is named explicitly in the check
  (`--ignore-column package_record_v2.updated_at`) instead of being hidden;
  every other package column must match. This needs a Rust startup path that
  mirrors `EnsureBuiltinV2`.

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
