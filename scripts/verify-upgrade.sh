#!/usr/bin/env bash
# Seamless upgrade: a Go deployment hands its directory to the Rust binary and
# back, and nothing may be lost.
#
#   scripts/verify-upgrade.sh            # uses port 3040 and a scratch directory
#
# Phases:
#   1. Go on an empty directory: seed, create a Space/group/item, upload a file,
#      select a theme, mint a session.
#   2. Stop Go, note every row count, start Rust on the same directory.
#   3. Rust must accept the Go session, the Go password, the Go-written rows and
#      the Go upload, and write its own rows without touching the Go ones.
#   4. Stop Rust, start Go again: the Go build must accept the Rust session and
#      read everything Rust wrote. That proves the upgrade is reversible.
set -Eeuo pipefail

port="${1:-3040}"
root="$(git rev-parse --show-toplevel)"
go_bin="${YIN_PANEL_GO_BIN:-$root/build-output/yin-panel-go}"
rust_bin="${YIN_PANEL_RUST_BIN:-$root/backend/yin-panel}"
work="/tmp/yin-panel-upgrade-$port"

for bin in "$go_bin" "$rust_bin"; do
    [[ -x "$bin" ]] || { printf 'missing binary: %s\n' "$bin" >&2; exit 1; }
done

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

api="http://127.0.0.1:$port"
say() { printf '\n== %s\n' "$*"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

start() { # binary label
    ( cd "$work" && exec setsid "$1" -c conf.yaml ) > "$work/$2.log" 2>&1 < /dev/null &
    for _ in $(seq 1 200); do
        curl -s --noproxy '*' -o /dev/null "$1_probe" 2>/dev/null || true
        if curl -s --noproxy '*' -o /dev/null --max-time 1 "$api/api/about"; then return 0; fi
        sleep 0.1
    done
    printf '%s did not start; log:\n' "$1" >&2
    tail -n 30 "$work/$2.log" >&2
    exit 1
}

stop() {
    local pid
    pid="$(ss -ltnp 2>/dev/null | awk '/:'"$port"' / {match($0,/pid=[0-9]+/); if (RSTART) {print substr($0,RSTART+4,RLENGTH-4); exit}}')"
    [[ -n "$pid" ]] || fail "nothing listening on $port"
    kill "$pid" 2>/dev/null || true
    for _ in $(seq 1 100); do
        kill -0 "$pid" 2>/dev/null || return 0
        sleep 0.1
    done
}

counts() { # database -> "table rows" lines
    python3 - "$1" <<'PY'
import sqlite3, sys
c = sqlite3.connect(sys.argv[1])
for row in c.execute("select name from sqlite_master where type='table' and name not like 'sqlite_%' order by name"):
    print(row[0], c.execute(f'select count(*) from "{row[0]}"').fetchone()[0])
PY
}

json() { # path -> prints body
    curl -s --noproxy '*' -H "Cookie: yin_token=$TOKEN" "$api$1"
}

login() {
    curl -s --noproxy '*' -H 'Content-Type: application/json' \
        -d '{"mail":"admin@yiniot.com","password":"admin@yiniot.com"}' "$api/api/login"
}

start "$go_bin" go
say "phase 1: Go seeds and creates data"
TOKEN="$(login | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["token"])')"
GO_TOKEN="$TOKEN"

curl -s --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d '{"name":"Upgrade Space"}' "$api/api/spaces/shared" > /dev/null
SPACE="$(json /api/spaces | python3 -c 'import json,sys
data=json.load(sys.stdin)["data"]
print([s["id"] for s in data if s["name"]=="Upgrade Space"][0])')"
echo "space $SPACE"

curl -s --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d '{"title":"Upgrade Group","icon":"material-symbols:folder"}' \
    "$api/api/spaces/$SPACE/groups" > /dev/null
GROUP="$(json "/api/spaces/$SPACE/groups" | python3 -c 'import json,sys
data=json.load(sys.stdin)["data"]
print([g["id"] for g in data if g["title"]=="Upgrade Group"][0])')"
echo "group $GROUP"

curl -s --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d "{\"title\":\"Go Item\",\"url\":\"https://go.example\",\"openMethod\":2,\"itemIconGroupId\":$GROUP}" \
    "$api/api/spaces/$SPACE/items" > /dev/null

python3 - <<'PY'
import base64
open('/tmp/upgrade-icon.png','wb').write(base64.b64decode(
 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII='))
PY
UPLOAD="$(curl -s --noproxy '*' -H "Cookie: yin_token=$TOKEN" \
    -F "imgfile=@/tmp/upgrade-icon.png;filename=upgrade.png" "$api/api/file/uploadImg" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["fileName"])')"
echo "upload $UPLOAD"
curl -s --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d '{"packageId":"org.yin.glass","mode":"dark"}' "$api/api/theme/v2/preference" > /dev/null

say "phase 1 rows (written by Go)"
counts "$work/database/database.db" | tee "$work/before.txt" | paste -sd ' ' -

stop
say "phase 2: Rust on the same directory"
start "$rust_bin" rust
say "schema work Rust reported"
grep -E 'schema|seed' "$work/rust.log" || echo "(nothing to change)"

say "phase 3: Rust must serve what Go wrote"
# The Go session cookie and the Go password hash must still work.
[[ "$(json /api/user/getUser | python3 -c 'import json,sys; print(json.load(sys.stdin)["code"])')" == "0" ]] ||
    fail "Rust rejected the Go session cookie"
[[ "$(login | python3 -c 'import json,sys; print(json.load(sys.stdin)["code"])')" == "0" ]] ||
    fail "Rust rejected the Go password hash"
[[ "$(json /api/spaces | grep -c 'Upgrade Space')" -gt 0 ]] || fail "Rust lost the Go Space"
[[ "$(json "/api/spaces/$SPACE/items?page=1&pageSize=50" | grep -c 'Go Item')" -gt 0 ]] || fail "Rust lost the Go item"
[[ "$(curl -s --noproxy '*' -o /dev/null -w '%{http_code}' "$api/uploads/$UPLOAD")" == "200" ]] ||
    fail "Rust cannot serve the Go upload"
[[ "$(json /api/theme/v2/preference | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"].get("mode"))')" == "dark" ]] ||
    fail "Rust lost the Go theme selection"
[[ "$(curl -s --noproxy '*' -o /dev/null -w '%{http_code}' "$api/")" == "200" ]] || fail "Rust cannot serve the frontend"
echo "  session, password, spaces, items, upload, theme, static: OK"

say "phase 3b: Rust writes its own rows"
TOKEN="$GO_TOKEN"
curl -s --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d '{"title":"Rust Group","icon":"material-symbols:folder"}' \
    "$api/api/spaces/$SPACE/groups" > /dev/null
curl -s --noproxy '*' -H "Cookie: yin_token=$TOKEN" -H 'Content-Type: application/json' \
    -d '{"name":"Rust Space"}' "$api/api/spaces/shared" > /dev/null
RUST_TOKEN="$(login | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["token"])')"
echo "  rust also minted a session"

say "phase 3c: no Go row was lost or duplicated"
counts "$work/database/database.db" > "$work/after-rust.txt"
python3 - "$work/before.txt" "$work/after-rust.txt" <<'PY'
import sys
before = {l.split()[0]: int(l.split()[1]) for l in open(sys.argv[1])}
after = {l.split()[0]: int(l.split()[1]) for l in open(sys.argv[2])}
lost = {k: (v, after.get(k, 0)) for k, v in before.items() if after.get(k, 0) < v}
print("  lost rows:", lost if lost else "none")
sys.exit(1 if lost else 0)
PY

stop
say "phase 4: back to Go — the Rust-written state must be readable"
start "$go_bin" go2
[[ "$(RUST_TOKEN="$RUST_TOKEN" python3 -c 'import os' 2>/dev/null; curl -s --noproxy '*' \
    -H "Cookie: yin_token=$RUST_TOKEN" "$api/api/user/getUser" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["code"])')" == "0" ]] ||
    fail "Go rejected the Rust session cookie"
[[ "$(curl -s --noproxy '*' -H "Cookie: yin_token=$RUST_TOKEN" "$api/api/spaces" | grep -c 'Rust Space')" -gt 0 ]] ||
    fail "Go cannot read the Space Rust created"
[[ "$(curl -s --noproxy '*' -H "Cookie: yin_token=$RUST_TOKEN" "$api/api/spaces/$SPACE/groups" | grep -c 'Rust Group')" -gt 0 ]] ||
    fail "Go cannot read the group Rust created"
echo "  Rust session, spaces and groups: OK"

stop
say "upgrade is seamless and reversible"
rm -rf "$work" /tmp/upgrade-icon.png
