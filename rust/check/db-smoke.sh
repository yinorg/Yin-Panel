#!/usr/bin/env sh
# HTTP smoke test of the Rust binary against a real MySQL or PostgreSQL.
#
# Proves runtime database parity, not just repository parity: the server boots
# from an empty database, seeds it, then answers login/session/space routes.
#
# Environment:
#   DB_SMOKE_DRIVE     mysql | postgres
#   DB_SMOKE_HOST      default 127.0.0.1
#   DB_SMOKE_PORT      default 13306 (mysql) / 15432 (postgres)
#   DB_SMOKE_USER      default root / postgres
#   DB_SMOKE_PASSWORD  required
#   DB_SMOKE_NAME      default yin_test
#   DB_SMOKE_PORT_HTTP default 18092
set -eu

drive="${DB_SMOKE_DRIVE:-}"
if [ -z "$drive" ]; then
    echo "DB_SMOKE_DRIVE is required (mysql|postgres)" >&2
    exit 2
fi

root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
bin="${YIN_PANEL_RUST_BIN:-$root/rust/app/target/release/yin-panel}"
if [ ! -x "$bin" ]; then
    echo "missing $bin; run: cargo build --release --manifest-path rust/app/Cargo.toml" >&2
    exit 2
fi

host="${DB_SMOKE_HOST:-127.0.0.1}"
name="${DB_SMOKE_NAME:-yin_test}"
password="${DB_SMOKE_PASSWORD:-}"
http_port="${DB_SMOKE_PORT_HTTP:-18092}"
work="${TMPDIR:-/tmp}/yin-db-smoke-${drive}"
rm -rf "$work"
mkdir -p "$work/uploads"

case "$drive" in
mysql)
    db_port="${DB_SMOKE_PORT:-13306}"
    user="${DB_SMOKE_USER:-root}"
    ;;
postgres)
    db_port="${DB_SMOKE_PORT:-15432}"
    user="${DB_SMOKE_USER:-postgres}"
    ;;
*)
    echo "unsupported DB_SMOKE_DRIVE: $drive" >&2
    exit 2
    ;;
esac

cat >"$work/conf.yaml" <<CONF
base:
  http_port: $http_port
  root_url: http://127.0.0.1:$http_port
  database_drive: $drive
  enable_static_server: false
  enable_monitor: false
  monitor_refresh_interval: 10
  url_prefix: /uploads/
sqlite:
  file_path: ./unused.db
mysql:
  host: $host
  port: $db_port
  username: $user
  password: $password
  db_name: $name
postgres:
  host: $host
  port: $db_port
  username: $user
  password: $password
  db_name: $name
  ssl_mode: disable
jwt:
  secret: smoke-only-secret
  expire: 1
rclone:
  type: local
  bucket: ./uploads
CONF

cd "$work"
"$bin" >server.log 2>&1 &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT

ready=0
for _ in $(seq 1 60); do
    if curl -s -o /dev/null "http://127.0.0.1:$http_port/api/about"; then
        ready=1
        break
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
        break
    fi
    sleep 0.5
done
if [ "$ready" -ne 1 ]; then
    echo "server did not start against $drive:" >&2
    tail -n 30 server.log >&2
    exit 1
fi

assert_code() {
    label="$1"
    expected="$2"
    shift 2
    body="$(curl -s -H 'Content-Type: application/json' "$@")"
    got="$(printf '%s' "$body" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code"))')"
    if [ "$got" != "$expected" ]; then
        echo "$label: expected code $expected, got $got: $body" >&2
        exit 1
    fi
    printf '%s: code %s\n' "$label" "$got"
}

assert_code "about" 0 "http://127.0.0.1:$http_port/api/about"
assert_code "login wrong password" 1003 \
    -X POST -d '{"mail":"admin@yiniot.com","password":"wrong"}' \
    "http://127.0.0.1:$http_port/api/login"

login_body="$(curl -s -H 'Content-Type: application/json' -X POST \
    -d '{"mail":"admin@yiniot.com","password":"admin@yiniot.com"}' \
    "http://127.0.0.1:$http_port/api/login")"
token="$(printf '%s' "$login_body" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["data"]["token"] if d.get("code")==0 else "")')"
if [ -z "$token" ]; then
    echo "login with the seeded administrator failed: $login_body" >&2
    exit 1
fi
printf 'login seeded administrator: code 0\n'

assert_code "getUser" 0 -H "Authorization: Bearer $token" \
    "http://127.0.0.1:$http_port/api/user/getUser"
assert_code "spaces" 0 -H "Authorization: Bearer $token" \
    "http://127.0.0.1:$http_port/api/spaces"
assert_code "logout" 0 -X POST -H "Authorization: Bearer $token" \
    "http://127.0.0.1:$http_port/api/logout"

echo "$drive runtime smoke passed"
