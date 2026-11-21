#!/usr/bin/env bash
# Seamless upgrade: a Go deployment hands its directory to the Rust binary and
# back, and nothing may be lost.
#
#   scripts/verify-upgrade.sh [port] [--keep]
#
# Phases:
#   1. Go on an empty directory: seed, create a Space/group/item, upload a file,
#      select a theme, mint a session.
#   2. Stop Go, snapshot every table row and every uploaded file, start Rust on
#      the same directory.
#   3. Rust must accept the Go session, the Go password, the Go-written rows and
#      the Go upload, and write its own rows without touching the Go ones.
#   4. Stop Rust, start Go again: the Go build must accept the Rust session and
#      read everything Rust wrote. That proves the upgrade is reversible.
#
# Nothing here is proven by row counts. `scripts/upgrade_compare.py` compares
# rows by column name as multisets and uploads by sha256, so a rewritten value,
# a delete plus insert, or a truncated file fails the check. Run
# `python3 scripts/upgrade_compare.py selftest` to see that it does.
#
# The script only ever stops the process it started itself. If the port is
# already in use it refuses to run rather than kill an unrelated service, and
# if the listener turns out not to be its own child it fails.
set -Eeuo pipefail

port="3040"
keep=0
for arg in "$@"; do
    case "$arg" in
        --keep) keep=1 ;;
        --*) printf 'unknown option: %s\n' "$arg" >&2; exit 2 ;;
        *) port="$arg" ;;
    esac
done

root="$(git rev-parse --show-toplevel)"
compare="$root/scripts/upgrade_compare.py"
go_bin="${YIN_PANEL_GO_BIN:-$root/build-output/yin-panel-go}"
rust_bin="${YIN_PANEL_RUST_BIN:-$root/backend/yin-panel}"
work="/tmp/yin-panel-upgrade-$port"
api="http://127.0.0.1:$port"
server_pid=""

# Go's `EnsureBuiltinV2` installs the built-in themes on every start, and the
# GORM update that follows bumps `package_record_v2.updated_at` even when the
# revision did not change. Only the Go restart gets that one-column exception;
# the Rust phase is compared strictly. Rust does not refresh the timestamp yet,
# a behaviour difference recorded in `rust/UPGRADE.md`.
COMPARE_POLICY_GO_RESTART=(--ignore-column package_record_v2.updated_at)

for bin in "$go_bin" "$rust_bin"; do
    [[ -x "$bin" ]] || { printf 'missing binary: %s\n' "$bin" >&2; exit 1; }
done
[[ -x "$compare" ]] || { printf 'missing helper: %s\n' "$compare" >&2; exit 1; }

# `ss -ltnp "sport = :PORT"` prints only the listener on that port, or nothing.
listener_pid() {
    ss -ltnp "sport = :$1" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1 || true
}

say() { printf '\n== %s\n' "$*"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

cleanup() {
    local status=$?
    if [[ -n "$server_pid" ]]; then
        kill "$server_pid" 2>/dev/null || true
        for _ in $(seq 1 50); do
            kill -0 "$server_pid" 2>/dev/null || break
            sleep 0.1
        done
        kill -9 "$server_pid" 2>/dev/null || true
    fi
    if [[ "$keep" -eq 0 ]]; then
        rm -rf "$work" /tmp/upgrade-icon.png
    else
        printf 'kept the work directory: %s\n' "$work"
    fi
    return "$status"
}
trap cleanup EXIT

# The port preflight runs before anything is created or stopped. A busy port
# is an error the operator must resolve, never a process to kill.
if [[ -n "$(listener_pid "$port")" ]]; then
    printf 'FAIL: port %s is already in use by pid %s; refusing to start a test that would stop it\n' \
        "$port" "$(listener_pid "$port")" >&2
    exit 1
fi

rm -rf "$work"
mkdir -p "$work/database" "$work/uploads"
cp -a "$root/backend/lang" "$work/lang"
ln -s "$root/backend/web" "$work/web"
cat > "$work/conf.yaml" <<EOF
base:
  http_port: $port
  root_url: http://127.0.0.1:$port
  database_drive: sqlite
  enable_static_server: true
  enable_monitor: false
  url_prefix: /uploads/
sqlite:
  file_path: ./database/database.db
  read_pool_size: -1
jwt:
  secret: upgrade-check-secret
  expire: 24
rclone:
  type: local
  bucket: uploads
EOF

start() { # binary label
    local bin="$1" label="$2" log="$work/$2.log"
    [[ -z "$(listener_pid "$port")" ]] ||
        fail "port $port was taken before starting $label; refusing to continue"
    # `exec` keeps the subshell's pid, so `$!` is the server itself and cleanup
    # can never reach a process this script did not start.
    ( cd "$work" && exec "$bin" -c conf.yaml ) >"$log" 2>&1 < /dev/null &
    server_pid=$!
    local waited=0
    while (( waited < 200 )); do
        if ! kill -0 "$server_pid" 2>/dev/null; then
            printf '%s exited during startup; log:\n' "$label" >&2
            tail -n 40 "$log" >&2
            fail "$label did not start"
        fi
        if curl -fsS --noproxy '*' --max-time 2 -o /dev/null "$api/api/about" 2>/dev/null; then
            local owner
            owner="$(listener_pid "$port")"
            [[ "$owner" == "$server_pid" ]] ||
                fail "$label listens as pid $owner, not its own pid $server_pid; refusing to test an unrelated service"
            return 0
        fi
        sleep 0.1
        waited=$((waited + 1))
    done
    printf '%s did not answer /api/about within 20s; log:\n' "$label" >&2
    tail -n 40 "$log" >&2
    fail "$label did not start"
}

stop() { # label
    local label="$1"
    [[ -n "$server_pid" ]] || return 0
    kill "$server_pid" 2>/dev/null || true
    for _ in $(seq 1 100); do
        kill -0 "$server_pid" 2>/dev/null || break
        sleep 0.1
    done
    kill -0 "$server_pid" 2>/dev/null && fail "$label (pid $server_pid) did not stop"
    server_pid=""
    [[ -z "$(listener_pid "$port")" ]] || fail "port $port is still held after stopping $label"
}

json() { # path -> body
    curl -fsS --noproxy '*' -H "Cookie: yin_token=$TOKEN" "$api$1"
}

login() {
    curl -fsS --noproxy '*' -H 'Content-Type: application/json' \
        -d '{"mail":"admin@yiniot.com","password":"admin@yiniot.com"}' "$api/api/login"
}

start "$go_bin" go
say "phase 1: Go seeds and creates data"
TOKEN="$(login | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["token"])')"
GO_TOKEN="$TOKEN"

curl -fsS --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d '{"name":"Upgrade Space"}' "$api/api/spaces/shared" > /dev/null
SPACE="$(json /api/spaces | python3 -c 'import json,sys
data=json.load(sys.stdin)["data"]
print([s["id"] for s in data if s["name"]=="Upgrade Space"][0])')"
printf 'space %s\n' "$SPACE"

curl -fsS --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d '{"title":"Upgrade Group","icon":"material-symbols:folder"}' \
    "$api/api/spaces/$SPACE/groups" > /dev/null
GROUP="$(json "/api/spaces/$SPACE/groups" | python3 -c 'import json,sys
data=json.load(sys.stdin)["data"]
print([g["id"] for g in data if g["title"]=="Upgrade Group"][0])')"
printf 'group %s\n' "$GROUP"

curl -fsS --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d "{\"title\":\"Go Item\",\"url\":\"https://go.example\",\"openMethod\":2,\"itemIconGroupId\":$GROUP}" \
    "$api/api/spaces/$SPACE/items" > /dev/null

python3 - <<'PY'
import base64
open('/tmp/upgrade-icon.png','wb').write(base64.b64decode(
 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII='))
PY
UPLOAD="$(curl -fsS --noproxy '*' -H "Cookie: yin_token=$TOKEN" \
    -F "imgfile=@/tmp/upgrade-icon.png;filename=upgrade.png" "$api/api/file/uploadImg" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["fileName"])')"
printf 'upload %s\n' "$UPLOAD"
curl -fsS --noproxy '*' -H "Cookie: yin_token=$TOKEN" \
    -o "$work/go-upload.bin" "$api/uploads/$UPLOAD"
GO_UPLOAD_SHA="$(sha256sum "$work/go-upload.bin" | cut -d' ' -f1)"
printf 'upload sha256 %s\n' "$GO_UPLOAD_SHA"

curl -fsS --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d '{"packageId":"org.yin.glass","mode":"dark"}' "$api/api/theme/v2/preference" > /dev/null

say "phase 1: snapshot every Go-written row and upload"
python3 "$compare" snapshot "$work/database/database.db" "$work/before.json" \
    --uploads "$work/uploads" | tee "$work/before.txt"

stop go
say "phase 2: Rust on the same directory"
start "$rust_bin" rust
say "schema work Rust reported"
grep -E 'schema|seed' "$work/rust.log" || echo "(nothing to change)"

say "phase 3: Rust must serve what Go wrote"
[[ "$(json /api/user/getUser | python3 -c 'import json,sys; print(json.load(sys.stdin)["code"])')" == "0" ]] ||
    fail "Rust rejected the Go session cookie"
[[ "$(login | python3 -c 'import json,sys; print(json.load(sys.stdin)["code"])')" == "0" ]] ||
    fail "Rust rejected the Go password hash"
[[ "$(json /api/spaces | grep -c 'Upgrade Space')" -gt 0 ]] || fail "Rust lost the Go Space"
[[ "$(json "/api/spaces/$SPACE/groups" | grep -c 'Upgrade Group')" -gt 0 ]] || fail "Rust lost the Go group"
[[ "$(json "/api/spaces/$SPACE/items?page=1&pageSize=50" | grep -c 'Go Item')" -gt 0 ]] || fail "Rust lost the Go item"
curl -fsS --noproxy '*' -o "$work/rust-upload.bin" "$api/uploads/$UPLOAD" ||
    fail "Rust cannot serve the Go upload"
RUST_UPLOAD_SHA="$(sha256sum "$work/rust-upload.bin" | cut -d' ' -f1)"
[[ "$RUST_UPLOAD_SHA" == "$GO_UPLOAD_SHA" ]] ||
    fail "Rust served different bytes for the Go upload ($GO_UPLOAD_SHA vs $RUST_UPLOAD_SHA)"
[[ "$(json /api/theme/v2/preference | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"].get("mode"))')" == "dark" ]] ||
    fail "Rust lost the Go theme selection"
[[ "$(curl -fsS --noproxy '*' -o /dev/null -w '%{http_code}' "$api/")" == "200" ]] ||
    fail "Rust cannot serve the frontend"
printf '  session, password, spaces, groups, items, upload bytes, theme, static: OK\n'

say "phase 3b: Rust writes its own rows"
TOKEN="$GO_TOKEN"
curl -fsS --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d '{"title":"Rust Group","icon":"material-symbols:folder"}' \
    "$api/api/spaces/$SPACE/groups" > /dev/null
curl -fsS --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d '{"name":"Rust Space"}' "$api/api/spaces/shared" > /dev/null
RUST_TOKEN="$(login | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["token"])')"
printf '  Rust also minted a session\n'

stop rust
say "phase 3c: content comparison, Rust phase"
python3 "$compare" snapshot "$work/database/database.db" "$work/after-rust.json" \
    --uploads "$work/uploads"
python3 "$compare" compare "$work/before.json" "$work/after-rust.json"

say "phase 4: back to Go, the Rust-written state must be readable"
start "$go_bin" go2
[[ "$(curl -fsS --noproxy '*' -H "Cookie: yin_token=$RUST_TOKEN" "$api/api/user/getUser" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["code"])')" == "0" ]] ||
    fail "Go rejected the Rust session cookie"
[[ "$(curl -fsS --noproxy '*' -H "Cookie: yin_token=$RUST_TOKEN" "$api/api/spaces" | grep -c 'Rust Space')" -gt 0 ]] ||
    fail "Go cannot read the Space Rust created"
[[ "$(curl -fsS --noproxy '*' -H "Cookie: yin_token=$RUST_TOKEN" "$api/api/spaces/$SPACE/groups" | grep -c 'Rust Group')" -gt 0 ]] ||
    fail "Go cannot read the group Rust created"
printf '  Rust session, spaces and groups: OK\n'

say "phase 4b: content comparison after Go read the Rust state"
python3 "$compare" snapshot "$work/database/database.db" "$work/after-go.json" \
    --uploads "$work/uploads"
python3 "$compare" compare "$work/before.json" "$work/after-go.json" \
    "${COMPARE_POLICY_GO_RESTART[@]}"

stop go2
say "upgrade keeps every row and byte, and is reversible"
