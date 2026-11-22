# Goal: replace the Go backend with the Rust binary, verifiably

This is the execution plan for a goal-driven agent. It is written so every task
is independently verifiable, ordered by dependency, and safe to resume. The
authoritative batch order stays in `PLAN.md`; this file turns "perfect
replacement" into concrete, testable conditions.

## Definition of "perfect replacement"

All six must be true at the same time. Until then, the Go backend is still the
product.

1. **Runtime database parity** — the binary starts and serves with
   `base.database_drive` set to `sqlite`, `mysql` and `postgres`, with no
   hard-coded refusal and no SQLite-only pool on any served route.
2. **Route parity** — every `(method, path)` in `rust/app/src/go_routes.rs`
   (`GO_ROUTES`, the Go route inventory) is served by a real handler. No
   request may reach the "unimplemented" fallback.
3. **Behaviour parity** — recorded replay passes 100% with zero unimplemented
   hits, and the recording set covers every route with at least one success and
   one failure case (or is listed in `rust/record/coverage-exceptions.md`).
4. **No Go build** — `scripts/deploy-local.sh`, `.github/workflows/*`,
   `docker/**` build and ship the Rust binary only; deleting `backend/` breaks
   nothing.
5. **Upgrade parity** — an existing SQLite deployment upgrades to the Rust
   binary, and `migrate-db` moves data to MySQL/PostgreSQL, both verified on
   real files/databases.
6. **Docs parity** — `AGENTS.md`, `frontend/THEME_SYSTEM.md`, `rust/README.md`
   and `PLAN.md` describe the Rust runtime, not the Go one.

## Baseline (measured before starting)

| Metric | Value | How to recount |
| --- | --- | --- |
| Go route registrations | 105 (`GO_ROUTES`, including the conditional OAuth pair) | `grep -c '("' rust/app/src/go_routes.rs` |
| Rust hand-written routes | 87 `.route(` calls | `grep -c '\.route(' rust/app/src/routes.rs` |
| Wire-generated routes | 1 | `wc -l rust/app/src/units/mod.rs` |
| Handlers on SQLite pool | 55 | `grep -c 'ctx\.pool()\.await' rust/app/src/routes.rs` |
| Handlers on shared pool | 19 | `grep -c 'ctx\.shared_pool()\.await' rust/app/src/routes.rs` |
| SQLite-only insert ids | 10 | `grep -c 'last_insert_rowid' rust/app/src/routes.rs` |
| Recorded replay | 642/642, "fallback" 0% | `rust-anything app replay .` |
| Runtime MySQL/PostgreSQL | refused at startup | `main.rs` driver guard |

The replay "fallback rate" is currently meaningless: there is no proxy any
more, so it can only report 0. Phase 0 replaces it with a real gate.

## Phase 0 — Build the gates first (S)

Without these, later phases can look green while routes are missing.

**T0.1 Metrics script** — add `scripts/rust-parity-metrics.sh` printing the
table above (counts + wire route count). Acceptance: running it twice gives
identical output; it exits non-zero if any count increases relative to a
committed baseline file `rust/parity-baseline.txt`.

**T0.2 Unimplemented-fallback marker** — in `routes::go_fallback`, add a header
such as `x-yin-unimplemented: 1` to every response it produces. Do not change
status codes or bodies: replay must still pass unchanged.

**T0.3 Inventory probe test** — add a test that starts the binary (or calls the
router directly) and probes every `GO_ROUTES` entry with a harmless method/path
request, asserting no response carries the unimplemented marker. Protected
routes answer the envelope (`code` 1000/1005), which is fine; only the marker
is failure. Acceptance: the test fails today with a list of unimplemented
routes, and is expected to fail until Phase 3 completes.

**T0.4 CI wiring** — run T0.1 and T0.3 in CI as required checks; store the
failing-route list as a snapshot file `rust/parity-unimplemented.txt` that may
only shrink.

Evidence: `cargo test parity`, `scripts/rust-parity-metrics.sh`, CI log.

## Phase 1 — Runtime database parity (L)

**T1.1 Remove the SQLite-only startup guard** in `main.rs`; select the pool via
`Ctx::shared_pool()` and `Ctx::sqlx_driver()`. Unsupported storage combinations
(non-local `rclone`) keep their explicit refusal.

**T1.2 Driver-aware bootstrap** — `bootstrap::prepare` currently assumes a
SQLite file. Add schema DDL and seed inserts per driver (reuse
`rust/db-tool/src/schema.rs` for the SQLite text and translate it once, not per
route). Acceptance: a fresh, empty MySQL and PostgreSQL database boots to a
seeded, login-capable instance.

**T1.3 HTTP smoke suite on three drivers** — one test binary that starts the
server against each driver and exercises: login → getUser → spaces → items →
create/update/delete item → logout. Acceptance: identical JSON envelopes on
`sqlite`, `mysql`, `postgres`; run in CI with service containers.

**T1.4 Config surface** — `conf.yaml` MySQL/PostgreSQL settings documented in
`rust/CONFIG.md`; `database_drive` values `sqlite|mysql|postgres` accepted.
Acceptance: `rust/CONFIG.md` matches the parser in `appcfg.rs`.

Evidence: T1.3 log lines per driver, `cargo test`, boot logs without refusal.

## Phase 2 — Migrate the remaining write routes (XL)

Order is dependency-first. Each task is one commit-sized slice with its own
three-driver fixture in `repository.rs` and its own replay run.

For every task: replace `ctx.pool()` with `ctx.shared_pool()`, move SQL into
`repository.rs`, use `query(driver, …)`, `text()`/`text_opt()`,
`timestamp_bind()`, and repository insert helpers. Handlers keep their current
status codes and error strings.

| Task | Routes | Notes |
| --- | --- | --- |
| T2.1 | `sort_items`, `sort_groups` | multi-row update inside one `AnyTransaction` |
| T2.2 | member add / role update / remove | paired Yin/Yang rows; verify both sides |
| T2.3 | space create / rename / copy | insert-id plus paired row creation |
| T2.4 | space transfer / share / team / delete / clear | ownership and pairing invariants |
| T2.5 | file upload / delete / stored-file reads | disk writes stay outside transactions |
| T2.6 | user create / update / delete | password hashing parity; insert-id |
| T2.7 | `moduleConfig/save`, `userConfig/setConfig` | JSON text columns |
| T2.8 | theme v2 preference + admin revisions/packages/audit/trusted | read + write split |
| T2.9 | theme install / rollback / trial / default / preview | file system + DB transaction ordering |
| T2.10 | wallpaper web upload / asset serving; wallpaper library | blob storage, content types |
| T2.11 | item icon groups / items / favicon | final `last_insert_rowid` removals |

Exit criteria for Phase 2: `ctx.pool()` count is 0, `last_insert_rowid` count is
0, `SqlitePool` does not appear in `routes.rs`, and the T0.3 unimplemented list
has shrunk accordingly.

## Phase 3 — Route parity (L)

**T3.1** Wire or hand-port every remaining `GO_ROUTES` entry. Prefer `app wire`
for unchanged units; hand-port where the wire output would need behaviour the
unit does not cover.

**T3.2** Extend `rust/record/inputs.jsonl` so every route has a success and a
failure case, then re-record with the Go binary as the oracle. Update
`rust/record/coverage-exceptions.md` only for genuinely failure-free public
reads.

**T3.3** Make T0.3 pass: no response may carry the unimplemented marker for any
entry in `GO_ROUTES`.

Exit criteria: T0.3 green, replay green, `rust/parity-unimplemented.txt` empty.

> Finding from Phase 0: `rust/check/route-parity.py` already reports 0
> unimplemented out of the then-inventoried 103 routes. The OAuth `:provider` and
> callback routes were missing from the inventory even though the Go router
> registered them under `oauth.enable`; the implementation and inventory now
> count 105. T3.1/T3.2 remain: behaviour parity per route, and success+failure
> recording coverage.

## Phase 4 — Remove Go (M)

Preconditions: Phase 3 exit criteria green on all three drivers; `cargo test`
green; replay 100%; user has approved deleting Go.

**T4.1** `scripts/deploy-local.sh`: build `cargo build --release`, copy the Rust
binary, drop all `go test`/`go build` steps.

**T4.2** CI: replace Go build steps in `ci-docker.yml`, `cache-warm.yml`,
`visual-regression.yml` with Rust; publish the Rust binary at the same path
(`build-output/yin-panel`) so tags and image aliases keep working.

**T4.3** Docker/compose: runtime image contains the Rust binary; no Go
toolchain.

**T4.4** Delete `backend/` in a separate commit, after the T4.1–T4.3 gates are
green and the user explicitly approves. `rust/database-rs` supplies the
`MigrateDB` entry point the Go wrapper used.

**T4.5** Update `AGENTS.md`, `frontend/THEME_SYSTEM.md`, `rust/PLAN.md`,
`rust/README.md` to describe the Rust runtime and remove Go build instructions.

Exit criteria: `grep -r 'go build' scripts .github docker` returns nothing;
`./scripts/deploy-local.sh` deploys and verifies the Rust binary; the release
tag flow still publishes.

## Phase 5 — Acceptance run (S)

Run, in order, and paste the output into the final report:

```sh
cargo test --manifest-path rust/app/Cargo.toml
bash scripts/rust-parity-metrics.sh
rust-anything app replay .
rust-anything app replay . --build 'sh rust/record/run-rust.sh'
bash scripts/deploy-local.sh
# three-driver HTTP smoke from T1.3
# upgrade test from T5.1
```

**T5.1** Upgrade test: take a pre-change SQLite database, start the Rust binary
against it, verify content survives; then `migrate-db` to MySQL and PostgreSQL
and verify the same content on both.

**T5.2** Publish the final status table (the six conditions above, each with the
command that proves it).

## Execution protocol for the goal agent

1. Pick the lowest-numbered unfinished task whose dependencies are green.
2. Inspect `git status --short --branch` first; preserve unrelated work.
3. Implement the task only; do not opportunistically refactor neighbours.
4. Run the task's acceptance command and the regression gate
   (`cargo test`, three-driver fixture for write slices, replay for handler
   changes).
5. Report the exact commands and their output; never claim an unrun check.
6. Commit one task per commit on `<username>-dev`, only after the manifest
   (`git diff --cached --name-only`) equals the reviewed, deployed scope.
7. If a task needs a product decision (delete Go, change an API shape, drop a
   route), stop and ask instead of guessing.
8. Never edit recorded case bodies to make a test pass. Fixture payload
   whitespace is data, not style.

## Risk register

| Risk | Impact | Mitigation |
| --- | --- | --- |
| sqlx `Any` runtime decoding | type bugs surface only in production | three-driver fixture for every slice; no slice without it |
| `Any` dispatch overhead | throughput regression | compare with `rust/BENCHMARKS.md`; keep repository API stable so typed pools can replace `Any` later |
| PostgreSQL strict typing | binds rejected at runtime | `timestamp_bind`, explicit casts, boolean 0/1 expressions |
| insert-id divergence | wrong row updated after create | `RETURNING id` on SQLite/PostgreSQL, `last_insert_id` on MySQL, tested on all three |
| seeding/schema drift between drivers | MySQL/PostgreSQL cannot boot | one schema source translated per driver (T1.2) |
| replay 404 masking | missing routes look green | T0.2/T0.3 unimplemented marker gate |
| manifest drift | deployed files differ from committed files | AGENTS.md manifest invariant, enforced before every commit |

## Six-condition sign-off

| Condition | Evidence |
| --- | --- |
| 1. Runtime database parity | `rust/check/db-smoke.sh` passes on SQLite, MySQL 8.4 and PostgreSQL 17 |
| 2. Route parity | `rust/check/route-parity.py`: 105 inventoried, 0 unimplemented |
| 3. Behaviour parity | `rust-anything app replay . --build 'sh rust/record/run-rust.sh'`: 665/665, fallback 0% |
| 4. No Go build | `backend/` removed; no `go build/test/mod/vet` in scripts, `.github` or `distribution` |
| 5. Upgrade parity | Go→Rust→Go upgrade run preserved 24 tables/43 rows/1 upload; `migrate-db` copied 24 tables/839 rows into MySQL and PostgreSQL and booted on both |
| 6. Docs parity | `AGENTS.md`, `rust/README.md`, `rust/PLAN.md`, `frontend/THEME_SYSTEM.md` describe the Rust runtime |

## Status table (update as work lands)

| Phase | Task | Status |
| --- | --- | --- |
| 0 | T0.1 metrics script | done — `scripts/rust-parity-metrics.sh`, baseline `rust/parity-baseline.txt` |
| 0 | T0.2 fallback marker | done — `x-yin-unimplemented` only when `YIN_UNIMPLEMENTED_MARKER` is set |
| 0 | T0.3 inventory probe | done — `rust/check/route-parity.py`, 105 routes, 0 unimplemented |
| 0 | T0.4 CI wiring | done — `rust-check.yml` (unit + recorded replay), `rust-db.yml` (repository/bootstrap on MySQL 8.4 and PostgreSQL 17, path-filtered), `rust-nightly.yml` (three-driver HTTP smoke + `migrate-db`) |
| 1 | T1.1 startup guard | done — `main.rs` boots `prepare_shared` for MySQL/PostgreSQL |
| 1 | T1.2 driver bootstrap | done — `schema_shared.rs` translates DDL, seed runs on all three |
| 1 | T1.3 three-driver HTTP smoke | done — `rust/check/db-smoke.sh` passes on MySQL 8.4 and PostgreSQL 17 |
| 1 | T1.4 config docs | done — `rust/CONFIG.md` |
| 2 | T2.1–T2.11 (Phase 2) | done — every handler runs on the shared AnyPool; `ctx.pool()` and `last_insert_rowid` are 0 |
| 2 | T2.2 members | done — paired-space upsert, role update and removal on the shared pool |
| 2 | T2.3 space create/rename/copy | done — `create_paired_space`, `rename_paired_space`, `copy_space` |
| 2 | T2.4 space transfer/share/team/delete/clear | done — transfer, clear, cascading delete on the shared pool; `shared`/`teams` reuse the T2.3 creator |
| 2 | T2.5 files | done — upload row, delete row and stored-file reads on the shared pool |
| 2 | T2.6 users | done — create/update/delete, updateInfo and updatePassword on the shared pool |
| 2 | T2.7 configs | done — `save_module_config`, `save_user_config` on the shared pool |
| 2 | T2.8 theme reads | done — preference, admin revisions/packages, audit and trusted policy on the shared pool |
| 2 | T2.9 theme writes | done — install, remove, default, trial, confirm, rollback on the shared pool |
| 2 | T2.10 wallpaper | done — web wallpaper upload/asset on the shared pool; library assets are in-memory |
| 2 | T2.11 icons/favicon | done — OIDC rules, item-with-icon and bookmark import; no `last_insert_rowid` left |
| 3 | T3.1–T3.3 route parity | done — 105 routes served, 0 unimplemented, recorded coverage `routes missing a pair: 0` |
| 4 | T4.1 deploy-local.sh builds Rust | done — `cargo test` + `cargo build`, deploys `yin-panel`; local cutover verified on :3002 |
| 4 | T4.2 CI builds Rust | done — `ci-docker.yml`, `cache-warm.yml`, `visual-regression.yml` |
| 4 | T4.3 Docker runtime | done — glibc base, `bash /dev/tcp` healthcheck; image built, started and reported `healthy` |
| 4 | T4.4 delete `backend/` | done — runtime moved to `runtime/`, Go tree, `backend-check.yml`, Go recorder and the Go arm of the upgrade check removed |
| 4 | T4.5 docs | done — AGENTS.md, rust/README.md, rust/PLAN.md, frontend/THEME_SYSTEM.md describe the Rust runtime |
| 5 | T5.1 three-driver HTTP acceptance | done — `rust/check/db-smoke.sh` passes on SQLite, MySQL 8.4 and PostgreSQL 17 |
| 5 | T5.1 upgrade verification | done — `scripts/verify-upgrade.sh 3040`: Go→Rust→Go, 24 tables/43 rows/1 upload preserved |
| 5 | T5.1 migrate-db verification | done and re-run after the runtime move — 24 tables / 839 rows, login+spaces on both copies |
| 5 | T5.2 six-condition sign-off | done — see the sign-off table below |

Already landed before this plan (kept for context): shared `Any` layer, auth,
space list/members/groups/items reads, search-config/public/wallpaper, admin and
config reads, and group/item CRUD writes.
