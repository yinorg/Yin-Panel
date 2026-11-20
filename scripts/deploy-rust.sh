#!/usr/bin/env bash
# Local cutover to the Rust binary.
#
# Builds `rust/app` in release mode, installs it as the runtime `yin-panel`,
# keeps the existing `web/`, database, uploads and `conf.yaml`, restarts the
# service and verifies the running HTTP endpoints.
#
# The Go binary is left in `build-output/yin-panel-go` (or can be rebuilt with
# `cd backend && go build -o ../build-output/yin-panel-go .`) as the rollback.
set -Eeuo pipefail

repo_root="$(git rev-parse --show-toplevel)"
env_file="${YIN_PANEL_ENV_FILE:-$repo_root/.env.local}"
if [[ -f "$env_file" ]]; then
    set +x
    set -a
    # shellcheck disable=SC1090
    source "$env_file"
    set +a
fi

export -n SUDO_PASSWORD SUDO_PASSWD 2>/dev/null || true

port="${YIN_PANEL_PORT:-3002}"
run_dir="${YIN_PANEL_RUNTIME_DIR:-$repo_root/backend}"
if [[ "$run_dir" != /* ]]; then
    run_dir="$repo_root/$run_dir"
fi
run_dir="$(readlink -f "$run_dir")"

die() {
    printf 'deploy-rust: %s\n' "$*" >&2
    exit 1
}

command -v curl >/dev/null 2>&1 || die "curl is required"
command -v cargo >/dev/null 2>&1 || die "cargo is required"
[[ -f "$run_dir/conf.yaml" ]] || die "runtime config is missing: $run_dir/conf.yaml"
[[ -d "$run_dir/web" ]] || die "frontend files are missing: $run_dir/web"

if command -v ss >/dev/null 2>&1; then
    listening_pid() { ss -ltnp 2>/dev/null | awk '/:'"$1"' / {match($0,/pid=[0-9]+/); if (RSTART) {print substr($0,RSTART+4,RLENGTH-4); exit}}' || true; }
    process_exe() { readlink -f "/proc/$1/exe" 2>/dev/null || true; }
else
    command -v lsof >/dev/null 2>&1 || die "either ss (Linux) or lsof (macOS/BSD) is required"
    listening_pid() { lsof -nP -iTCP:"$1" -sTCP:LISTEN -t 2>/dev/null | head -1 || true; }
    process_exe() { ps -o comm= -p "$1" 2>/dev/null || true; }
fi

needs_privilege=0
if [[ ! -w "$run_dir" ]] || [[ -e "$run_dir/yin-panel" && ! -w "$run_dir/yin-panel" ]]; then
    needs_privilege=1
fi
if [[ "$needs_privilege" == 1 ]]; then
    command -v sudo >/dev/null 2>&1 || die "sudo is required to replace root-owned runtime files"
    privileged() { sudo "$@"; }
    sudo_password="${SUDO_PASSWORD:-${SUDO_PASSWD:-}}"
    if [[ -n "$sudo_password" ]]; then
        sudo -S -v <<< "$sudo_password"
        sudo_status=$?
        unset sudo_password SUDO_PASSWORD SUDO_PASSWD
        [[ "$sudo_status" -eq 0 ]] || exit "$sudo_status"
    else
        sudo -v
    fi
else
    privileged() { "$@"; }
    printf 'Runtime directory is user-writable; continuing without sudo.\n'
fi

version="$(git -C "$repo_root" describe --tags --always 2>/dev/null || echo unknown)"
commit="$(git -C "$repo_root" rev-parse --short=8 HEAD 2>/dev/null || echo unknown)"
build_time="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

printf 'Building the Rust binary...\n'
(cd "$repo_root/rust/app" && \
    YIN_PANEL_VERSION="$version" \
    YIN_PANEL_COMMIT="$commit" \
    YIN_PANEL_BUILD_TIME="$build_time" \
    cargo build --release)

binary="$repo_root/rust/app/target/release/run-original"
[[ -x "$binary" ]] || die "cargo build did not produce $binary"

pid="$(listening_pid "$port")"
if [[ -n "$pid" ]]; then
    exe="$(process_exe "$pid")"
    if [[ -z "$exe" ]]; then
        die "port $port is in use by pid $pid, whose executable could not be identified; refusing to stop it"
    fi
    [[ "$(basename "$exe")" == "yin-panel" ]] || die "port $port is used by a non-Yin-Panel process: $exe"
    printf 'Stopping Yin-Panel PID %s...\n' "$pid"
    privileged kill "$pid"
    for _ in $(seq 1 100); do
        privileged kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
    done
    privileged kill -0 "$pid" 2>/dev/null && die "Yin-Panel process did not exit: $pid"
fi

printf 'Replacing the runtime binary (web/, database, uploads and conf.yaml stay in place)...\n'
privileged rm -f "$run_dir/yin-panel"
privileged cp -a "$binary" "$run_dir/yin-panel"

printf 'Starting Yin-Panel from %s...\n' "$run_dir"
if command -v setsid >/dev/null 2>&1; then
    (cd "$run_dir" && exec setsid ./yin-panel) > "$run_dir/yin-panel.log" 2>&1 < /dev/null &
else
    (cd "$run_dir" && exec nohup ./yin-panel) > "$run_dir/yin-panel.log" 2>&1 < /dev/null &
fi
disown 2>/dev/null || true

for _ in $(seq 1 200); do
    [[ -n "$(listening_pid "$port")" ]] && break
    sleep 0.1
done
[[ -n "$(listening_pid "$port")" ]] || die "Yin-Panel did not listen on port $port"

curl --noproxy '*' --fail --silent --show-error "http://127.0.0.1:$port/" -o /tmp/yin-panel-home.html
about_status="$(curl --noproxy '*' --silent --output /tmp/yin-panel-about.json --write-out '%{http_code}' "http://127.0.0.1:$port/api/about")"
[[ "$about_status" == "200" ]] || die "/api/about answered $about_status"
python3 - "/tmp/yin-panel-about.json" <<'PY' || die "/api/about did not answer the version envelope"
import json, sys
body = json.load(open(sys.argv[1]))
assert body.get("code") == 0 and body.get("data", {}).get("version"), body
PY

printf 'Yin-Panel (Rust) is running on port %s. /api/about: %s\n' "$port" "$about_status"
