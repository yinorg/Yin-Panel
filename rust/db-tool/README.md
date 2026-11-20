# yin-db

The Rust counterpart of the Go backend's in-place schema upgrade and its
`migrate-db` command.

```sh
cargo build --release
./target/release/yin-db upgrade ./conf.yaml
./target/release/yin-db copy --source ./conf.yaml --target ./conf.target.yaml
```

`upgrade` performs the same idempotent steps as `internal/infra/database`:
it creates the tables, adds the columns newer releases introduced, removes the
rows that would block the unique indexes, normalises the login address, copies
retired provider columns into `o_auth_identity`, merges accounts that share an
address, retires the legacy columns and the `team` table, and only then creates
`uk_user_mail`. A second run reports `Already up to date.`

`copy` writes a SQLite deployment into another configured database. SQLite
targets are a file copy followed by `upgrade`; MySQL/MariaDB and PostgreSQL
targets are not implemented yet and are rejected with a clear message.

The schema statements in `src/schema.rs` are generated from a fully migrated
database and are idempotent, so they describe both a fresh install and the
target of an upgrade.

Tests: `cargo test` builds a legacy-shaped database (retired columns, mixed-case
duplicate addresses, duplicate configuration rows, no unique mail index) and
asserts the upgrade and the copy.
