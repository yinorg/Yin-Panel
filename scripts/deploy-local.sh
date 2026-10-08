#!/usr/bin/env bash
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
    printf 'deploy-local: %s\n' "$*" >&2
    exit 1
}

command -v curl >/dev/null 2>&1 || die "curl is required"
command -v npm >/dev/null 2>&1 || die "npm is required"
command -v go >/dev/null 2>&1 || die "go is required"

# Port and process introspection. Linux provides `ss` plus /proc; macOS/BSD
# provide neither, so `lsof` plus `ps` stand in. Both paths must agree, because
# the script refuses to touch a port owned by anything other than Yin-Panel.
if command -v ss >/dev/null 2>&1; then
    listening_pid() { ss -ltnp 2>/dev/null | awk '/:'"$1"' / {match($0,/pid=[0-9]+/); if (RSTART) {print substr($0,RSTART+4,RLENGTH-4); exit}}' || true; }
    process_exe() { readlink -f "/proc/$1/exe" 2>/dev/null || true; }
else
    command -v lsof >/dev/null 2>&1 || die "either ss (Linux) or lsof (macOS/BSD) is required"
    # `lsof` exits non-zero when nothing matches, which under `set -e` combined
    # with a command substitution would abort the script, so the status is
    # deliberately discarded: "no listener" is an expected answer here.
    listening_pid() { lsof -nP -iTCP:"$1" -sTCP:LISTEN -t 2>/dev/null | head -1 || true; }
    process_exe() { ps -o comm= -p "$1" 2>/dev/null || true; }
fi

# Detaching the daemon: `setsid` on Linux, `nohup` everywhere else.
if ! command -v setsid >/dev/null 2>&1 && ! command -v nohup >/dev/null 2>&1; then
    die "either setsid (Linux) or nohup (macOS/BSD) is required"
fi

[[ -f "$repo_root/frontend/package.json" ]] || die "frontend/package.json is missing"
[[ -f "$repo_root/backend/go.mod" ]] || die "backend/go.mod is missing"
[[ -f "$run_dir/conf.yaml" ]] || die "runtime config is missing: $run_dir/conf.yaml"

# Replacing the runtime artifacts normally needs root, because on a packaged
# install the binary and web/ are owned by root. When the runtime directory is
# already owned by the current user — the usual case for a dev checkout — sudo
# is not required at all, so the script also works without a sudo password.
needs_privilege=0
if [[ ! -w "$run_dir" || ( -e "$run_dir/web" && ! -w "$run_dir/web" ) ]]; then
    needs_privilege=1
fi

if [[ "$needs_privilege" -eq 1 ]]; then
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

build_dir="$(mktemp -d /tmp/yin-panel-build.XXXXXX)"
trap 'rm -rf "$build_dir"' EXIT

printf 'Building frontend...\n'
(cd "$repo_root/frontend" && npm run build-only)

[[ -f "$repo_root/backend/web/index.html" ]] || die "frontend build did not produce backend/web/index.html"
cp -a "$repo_root/backend/web" "$build_dir/web"

printf 'Testing backend...\n'
(cd "$repo_root/backend" && go test ./...)

printf 'Building backend...\n'
(cd "$repo_root/backend" && go build -o "$build_dir/yin-panel" .)

[[ -x "$build_dir/yin-panel" ]] || die "backend build did not produce an executable"

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
        if ! privileged kill -0 "$pid" 2>/dev/null; then
            break
        fi
        sleep 0.1
    done
    privileged kill -0 "$pid" 2>/dev/null && die "Yin-Panel process did not exit: $pid"
fi

printf 'Replacing runtime binary and frontend files...\n'
privileged rm -f "$run_dir/yin-panel"
privileged rm -rf "$run_dir/web"
privileged cp -a "$build_dir/yin-panel" "$run_dir/yin-panel"
privileged cp -a "$build_dir/web" "$run_dir/web"

printf 'Starting Yin-Panel from %s...\n' "$run_dir"
# Start the daemon detached from this shell's stdio. The redirections belong on
# the subshell invocation: if they sit on the inner command, the wrapper
# subshell keeps this script's stdout/stderr open and can outlive it, so any
# caller that pipes the output (for example `... | tail`) blocks forever waiting
# for EOF, and an orphaned wrapper is left behind. `exec` replaces the wrapper
# with the daemon, so nothing lingers.
if command -v setsid >/dev/null 2>&1; then
    (cd "$run_dir" && exec setsid ./yin-panel) > "$run_dir/yin-panel.log" 2>&1 < /dev/null &
else
    (cd "$run_dir" && exec nohup ./yin-panel) > "$run_dir/yin-panel.log" 2>&1 < /dev/null &
fi
disown 2>/dev/null || true

for _ in $(seq 1 100); do
    if [[ -n "$(listening_pid "$port")" ]]; then
        break
    fi
    sleep 0.1
done
[[ -n "$(listening_pid "$port")" ]] || die "Yin-Panel did not listen on port $port"

curl --noproxy '*' --fail --silent --show-error "http://127.0.0.1:$port/" -o /tmp/yin-panel-home.html
route_status="$(curl --noproxy '*' --silent --output /tmp/yin-panel-search-config-response --write-out '%{http_code}' "http://127.0.0.1:$port/api/spaces/1/search-config")"
[[ "$route_status" != "404" ]] || die "search-config route returned 404"
grep -q "Listening and serving HTTP" "$run_dir/yin-panel.log" || die "startup log did not confirm HTTP readiness"

printf 'Yin-Panel is running on port %s. search-config HTTP status: %s\n' "$port" "$route_status"
