# Execution plan to a pure-Rust Core backend

Definition of done, in order. Each batch must end with `diff`/`gate` green for its
units, `app replay` green, and the run committed. A batch that cannot reach those
three does not close.

## Definition of done

1. Every Core unit listed in `MAP.md` is ported or explicitly frozen (only
   `pkg/extension`, ADR-002).
2. `rust-anything app replay` passes with **fallback 0%** against a recording set
   that covers every Core endpoint, not only the 35 sampled today.
3. `rust-anything app lift` completes: the proxy and the Go binary are gone,
   `cargo test` runs the recorded cases.
4. `scripts/deploy-local.sh`, `.github/workflows/*`, `docker/*` and the release
   tag flow build and ship the Rust binary; the Go build steps are removed.
5. `migrate-db` (SQLite -> MySQL/PostgreSQL) exists in Rust, or is documented as
   a one-time Go-side tool with its removal scheduled.
6. `AGENTS.md` and `frontend/THEME_SYSTEM.md` describe the Rust runtime.

## Recording coverage (do this first, it gates every later step)

The 299 cases only exercise 35 requests. Extend `rust/record/inputs.jsonl` to
every endpoint in the route inventory, with authenticated flows and the paired
Space (yin/yang) semantics, then re-record. Target: every route in
`backend/internal/web/router/**` has at least one success and one failure case.

## Batches

| Batch | Content | Exit criteria |
| --- | --- | --- |
| B0 | Recording coverage for all endpoints | replay of the original 100%, case count per route >= 2 |
| B1 | Spine: config, sqlx pools (sqlite/mysql/postgres), JWT verify/issue, response envelope, security + no-store headers, error codes, i18n loader | unit tests against a temp SQLite DB; tokens from the Go build verify in Rust |
| B2 | `internal/biz/repository` (705) + `internal/infra/database` read paths: users, spaces, members, items, groups, files, configs | repository parity tests on a fixture DB; schema compatibility test |
| B3 | `internal/biz/service` (811) + `internal/util/jwt` + `internal/biz/cache` | unit diff/gate; login/logout/session parity |
| B4 | system routes: about, capabilities, login, logout, user, moduleConfig, monitor, oauth config | those routes wired, replay green, fallback drops |
| B5 | panel routes: itemIcon, userConfig, users, space (the 1,930-line router, split by resource) | as above |
| B6 | `internal/biz/theme` (2,507): DTCG, revision, package, store, wallpaper | unit diff/gate over the Go tests as cases |
| B7 | theme routes (1,082) + wallpaper pack `Asset` + storage | as above |
| B8 | `internal/util/monitor`, favicon, kvcache, zaplog, config | unit diff/gate |
| B9 | database write paths + `schema_upgrade` + `migrate-db` | upgrade test from a pre-change SQLite file |
| B10 | `app lift`: remove the proxy, make the Rust binary the only server | fallback 0, `cargo test` green |
| B11 | Deployment: script, CI, Docker, tags, docs | CI builds and publishes the Rust image |

## Rules that keep it safe

- Recorded cases are the contract. A behaviour that no case covers is not ported
  until a case exists.
- Never edit Go to make a port easier; the Go build is the oracle until B10.
- The database schema, migration order and `migrate-db` output stay
  byte-compatible until B11 has been shipped and verified on a real upgrade.
- EE stays frozen at `edition: core` (ADR-002) until the Core port is pure.

## Honest estimate

B0-B3 are the load-bearing work: a data layer that behaves like GORM's and a
session layer that accepts tokens the Go build issued. B6 is the second peak.
The remaining router code is volume, not risk. This is multi-session work; the
per-batch exit criteria are what make each session resumable.
