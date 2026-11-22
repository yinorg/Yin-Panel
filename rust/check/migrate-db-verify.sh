#!/usr/bin/env bash
# Verifies the Rust `migrate-db`: a seeded SQLite deployment is copied into
# MySQL and PostgreSQL, row counts are compared per table, and the Rust binary
# is started against each target to prove the copy is a working installation.
#
# Environment:
#   MIGRATE_MYSQL_PASSWORD / MIGRATE_POSTGRES_PASSWORD (required)
#   MIGRATE_MYSQL_HOST/PORT/DB, MIGRATE_POSTGRES_HOST/PORT/DB (defaults match
#   the containers used by the goal run)
set -Eeuo pipefail

repo_root="$(git rev-parse --show-toplevel)"
env_file="${YIN_PANEL_ENV_FILE:-$repo_root/.env.local}"
if [[ -f "$env_file" ]]; then
    set +x
    set -a
    source "$env_file"
    set +a
fi
export -n SUDO_PASSWORD SUDO_PASSWD 2>/dev/null || true

mysql_host="${MIGRATE_MYSQL_HOST:-127.0.0.1}"
mysql_port="${MIGRATE_MYSQL_PORT:-13306}"
mysql_db="${MIGRATE_MYSQL_DB:-yin_test}"
pg_host="${MIGRATE_POSTGRES_HOST:-127.0.0.1}"
pg_port="${MIGRATE_POSTGRES_PORT:-15432}"
pg_db="${MIGRATE_POSTGRES_DB:-yin_test}"

die() {
    printf 'migrate-db-verify: %s\n' "$*" >&2
    exit 1
}

work="$(mktemp -d /tmp/yin-migrate-verify.XXXXXX)"
cleanup() {
    [[ -n "${server_pid:-}" ]] && kill "$server_pid" 2>/dev/null || true
    rm -rf "$work"
}
trap cleanup EXIT

source_conf="${MIGRATE_SOURCE_CONF:-$repo_root/runtime/conf.yaml}"
[[ -f "$source_conf" ]] || die "source configuration is missing: $source_conf"
if [[ "$source_conf" == "$repo_root/runtime/conf.yaml" ]]; then
    [[ -f "$repo_root/runtime/database/database.db" ]] || die "the deployment database is missing"
fi

printf 'Building yin-db and yin-panel...\n'
cargo build --quiet --release --manifest-path "$repo_root/rust/db-tool/Cargo.toml"
cargo build --quiet --release --manifest-path "$repo_root/rust/app/Cargo.toml"
yin_db="$repo_root/rust/db-tool/target/release/yin-db"
panel="$repo_root/rust/app/target/release/yin-panel"
[[ -x "$yin_db" && -x "$panel" ]] || die "build did not produce both binaries"

# 1. Consistent SQLite snapshot of the live deployment (read-only).
cat >"$work/snapshot.yaml" <<CONF
base:
  http_port: 3099
  database_drive: sqlite
sqlite:
  file_path: $work/source.db
jwt:
  secret: migrate-db-verify-secret
  expire: 1
rclone:
  type: local
  bucket: $work/uploads
CONF
printf 'Snapshotting the SQLite deployment...\n'
"$yin_db" copy --source "$source_conf" --target "$work/snapshot.yaml"

mysql_conf="$work/mysql.yaml"
cat >"$mysql_conf" <<CONF
base:
  http_port: 3098
  database_drive: mysql
mysql:
  host: $mysql_host
  port: $mysql_port
  username: root
  password: ${MIGRATE_MYSQL_PASSWORD:?MIGRATE_MYSQL_PASSWORD is required}
  db_name: $mysql_db
jwt:
  secret: migrate-db-verify-secret
  expire: 1
rclone:
  type: local
  bucket: $work/uploads
CONF

pg_conf="$work/postgres.yaml"
cat >"$pg_conf" <<CONF
base:
  http_port: 3097
  database_drive: postgres
postgres:
  host: $pg_host
  port: $pg_port
  username: postgres
  password: ${MIGRATE_POSTGRES_PASSWORD:?MIGRATE_POSTGRES_PASSWORD is required}
  db_name: $pg_db
  ssl_mode: disable
jwt:
  secret: migrate-db-verify-secret
  expire: 1
rclone:
  type: local
  bucket: $work/uploads
CONF

printf 'Copying into MySQL...\n'
"$yin_db" copy --source "$work/snapshot.yaml" --target "$mysql_conf"
printf 'Copying into PostgreSQL...\n'
"$yin_db" copy --source "$work/snapshot.yaml" --target "$pg_conf"

# 2. Compare row counts per table and a few identity rows.
python3 - "$work/source.db" "$mysql_db" "$pg_db" "$MIGRATE_MYSQL_PASSWORD" "$MIGRATE_POSTGRES_PASSWORD" <<'PY'
import os, sqlite3, subprocess, sys

source, mysql_db, pg_db, mysql_password, pg_password = sys.argv[1:6]
conn = sqlite3.connect(source)
tables = [
    row[0] for row in conn.execute(
        "select name from sqlite_master where type='table' and name not like 'sqlite_%' order by name"
    )
]
counts = {table: conn.execute(f'select count(*) from "{table}"').fetchone()[0] for table in tables}

def find_container(image):
    found = subprocess.run(
        ["docker", "ps", "--filter", f"ancestor={image}", "--format", "{{.ID}}"],
        capture_output=True, text=True, check=True,
    ).stdout.strip().splitlines()
    if not found:
        raise SystemExit(f"no running container for {image}")
    return found[0]

mysql_container = os.environ.get("MIGRATE_MYSQL_CONTAINER") or find_container("mysql:8.4")
pg_container = os.environ.get("MIGRATE_POSTGRES_CONTAINER") or find_container("postgres:17")

def mysql_count(table):
    answer = subprocess.run(
        ["docker", "exec", mysql_container,
         "mysql", "-N", "-B", "-uroot", f"-p{mysql_password}", mysql_db,
         "-e", f"select count(*) from `{table}`"],
        capture_output=True, text=True, check=True,
    )
    return int(answer.stdout.strip())

def pg_count(table):
    answer = subprocess.run(
        ["docker", "exec", pg_container,
         "psql", "-U", "postgres", "-d", pg_db, "-tAc",
         f'select count(*) from "{table}"'],
        capture_output=True, text=True, check=True,
    )
    return int(answer.stdout.strip())

mismatch = False
for table in tables:
    expected = counts[table]
    for label, actual in (("mysql", mysql_count(table)), ("postgres", pg_count(table))):
        if actual != expected:
            print(f"MISMATCH {label}.{table}: {actual} != {expected}")
            mismatch = True
print(f"tables compared: {len(tables)}")
print(f"rows compared:   {sum(counts.values())}")

# Identity rows that must survive the copy byte-for-byte.
user = conn.execute("select mail, password, token_version from \"user\" order by id limit 1").fetchone()
space = conn.execute("select name, type, side from \"space\" order by id limit 1").fetchone()
print(f"sqlite identity: {user[0]} {user[1]} v{user[2]} / {space[0]} {space[1]} {space[2]}")
sys.exit(1 if mismatch else 0)
PY

# 3. The copied database must be a working installation for the Rust binary.
printf 'Starting yin-panel against the MySQL copy...\n'
cat >"$work/mysql-run.yaml" <<CONF
base:
  http_port: 3098
  root_url: http://127.0.0.1:3098
  database_drive: mysql
sqlite:
  file_path: ./unused.db
mysql:
  host: $mysql_host
  port: $mysql_port
  username: root
  password: ${MIGRATE_MYSQL_PASSWORD}
  db_name: $mysql_db
jwt:
  secret: migrate-db-verify-secret
  expire: 1
rclone:
  type: local
  bucket: $work/uploads
CONF
(cd "$work" && exec env PORT=3098 YIN_PANEL_CONFIG="$work/mysql-run.yaml" "$panel" >server.log 2>&1) &
server_pid=$!
for _ in $(seq 1 60); do
    if curl -s --noproxy '*' -o /dev/null "http://127.0.0.1:3098/api/about"; then break; fi
    sleep 0.5
done
if ! curl -s --noproxy '*' --fail "http://127.0.0.1:3098/api/about" >/dev/null; then
    tail -n 20 "$work/server.log" >&2 || true
    die "server did not start on the MySQL copy"
fi
login="$(curl -s --noproxy '*' -H 'Content-Type: application/json' -X POST \
    -d '{"mail":"admin@yiniot.com","password":"admin@yiniot.com"}' \
    "http://127.0.0.1:3098/api/login")"
token="$(printf '%s' "$login" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["data"]["token"] if d.get("code")==0 else "")')"
[[ -n "$token" ]] || die "login against the MySQL copy failed: $login"
curl -s --noproxy '*' --fail -H "Authorization: Bearer $token" "http://127.0.0.1:3098/api/spaces" >/dev/null \
    || die "spaces against the MySQL copy failed"
printf 'MySQL copy: login and spaces OK\n'
kill "$server_pid" 2>/dev/null || true
server_pid=""

printf 'Starting yin-panel against the PostgreSQL copy...\n'
cat >"$work/pg-run.yaml" <<CONF
base:
  http_port: 3097
  root_url: http://127.0.0.1:3097
  database_drive: postgres
sqlite:
  file_path: ./unused.db
postgres:
  host: $pg_host
  port: $pg_port
  username: postgres
  password: ${MIGRATE_POSTGRES_PASSWORD}
  db_name: $pg_db
  ssl_mode: disable
jwt:
  secret: migrate-db-verify-secret
  expire: 1
rclone:
  type: local
  bucket: $work/uploads
CONF
(cd "$work" && exec env PORT=3097 YIN_PANEL_CONFIG="$work/pg-run.yaml" "$panel" >server-pg.log 2>&1) &
server_pid=$!
for _ in $(seq 1 60); do
    if curl -s --noproxy '*' -o /dev/null "http://127.0.0.1:3097/api/about"; then break; fi
    sleep 0.5
done
if ! curl -s --noproxy '*' --fail "http://127.0.0.1:3097/api/about" >/dev/null; then
    tail -n 20 "$work/server-pg.log" >&2 || true
    die "server did not start on the PostgreSQL copy"
fi
login="$(curl -s --noproxy '*' -H 'Content-Type: application/json' -X POST \
    -d '{"mail":"admin@yiniot.com","password":"admin@yiniot.com"}' \
    "http://127.0.0.1:3097/api/login")"
token="$(printf '%s' "$login" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["data"]["token"] if d.get("code")==0 else "")')"
[[ -n "$token" ]] || die "login against the PostgreSQL copy failed: $login"
curl -s --noproxy '*' --fail -H "Authorization: Bearer $token" "http://127.0.0.1:3097/api/spaces" >/dev/null \
    || die "spaces against the PostgreSQL copy failed"
printf 'PostgreSQL copy: login and spaces OK\n'

printf 'migrate-db verification passed\n'
