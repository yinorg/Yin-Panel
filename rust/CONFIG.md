# Configuration: what the Rust build honours

The Rust binary reads the same `conf.yaml` as the Go build. "Reads" is not the
same as "implements", so this table states, per setting, what actually happens.
Anything marked **refused** stops the process at startup with a message; it
never falls back silently to a different backend.

| Setting | Go build | Rust build | Evidence |
| --- | --- | --- | --- |
| `base.http_port` | listens there | listens there | live service on `:3002` |
| `base.root_url` | used for OAuth redirects and absolute links | parsed; used by the routes that emit absolute URLs | `scripts/verify-upgrade.sh` |
| `base.database_drive: sqlite` | SQLite | SQLite | `scripts/verify-upgrade.sh` |
| `base.database_drive: mysql` / `postgres` | serves it | **refused**: `unsupported database drive: mysql` | startup check in `src/main.rs`; `rust/db-tool` can still migrate *to* those databases |
| `sqlite.file_path` | opens it | opens it | pair/upgrade runs |
| `sqlite.read_pool_size` | `>= 0` opens a read-only pool of that size, negative disables the split | **ignored**; one pool with four connections | see below |
| `base.enable_static_server` | serves `./web` | serves `./web` | `src/site.rs` |
| `base.enable_monitor` | background cache refreshed every 3 s | same | snapshot fell from 1.0037 s to < 1 ms |
| `base.monitor_refresh_interval` | only returned to the client | only returned to the client | matches Go |
| `base.url_prefix` | upload URL prefix | same, default `/uploads/` | `upload_prefix()` |
| `jwt.secret`, `jwt.expire` | HS256 sessions | same secret and claims | Go cookie accepted by Rust and back |
| `rclone.type: local` | local bucket directory | same, default `uploads` | uploads served byte-identically |
| `rclone.type: s3` (or any non-local) | errors: "Core only supports local storage" | **refused**, same wording | startup check |
| `oauth.enable` | `/api/oauth/config` plus `:provider` redirect and callback | `/api/oauth/config` only; the login and callback routes do not exist here and answer Gin's 404 | `grep` in `src/routes.rs`; **not implemented** |
| `oauth.providers[].name` | listed by `/api/oauth/config` | listed by `/api/oauth/config` | same route |
| `migration.enabled` | imports a legacy SQLite installation at startup | not implemented; already-migrated deployments are unaffected | see below |

## Why `read_pool_size` is ignored

Go enables four read-only connections by default and routes non-transactional
reads to them. The Rust build keeps one pool of four connections instead of
splitting, because the split is a routing decision inside every handler rather
than a flag on the connection.

The measurement in `BENCHMARKS.md` section 6 does not support adding it: with
both builds on their shipped configuration the SQLite reads are level, and Rust
has the tighter tail (`/api/spaces` p99 7.20 ms against Go's 12.10 ms). Adding a
second pool would change which connection each query uses for no measured win, so
this stays a documented difference rather than a speculative one. Revisit only
with a workload where reads demonstrably queue behind writes.

## Known gaps that do not fail startup

These are behaviour differences, not silent fallbacks, and each has a record:

- **OAuth login.** `oauth.enable: true` in the shipped configuration, but only
  the provider list is served. A user who clicks a provider reaches Gin's 404
  instead of the provider's consent screen. Local password login is unaffected.
- **Legacy migration.** `migration.enabled: true` asks the Go build to import a
  pre-existing installation. The Rust build only runs the schema upgrade
  (`rust/db-tool`) and the first-run seed, so an already-migrated deployment is
  correct while a legacy import is not performed.
- **Built-in theme refresh.** Go re-installs the built-in themes on every start
  and bumps `package_record_v2.updated_at`; Rust seeds them only on an empty
  database. A database holding older built-in revisions keeps them. Recorded in
  `rust/UPGRADE.md`.
