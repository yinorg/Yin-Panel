#!/usr/bin/env bash
set -Eeuo pipefail

repo_root="$(git rev-parse --show-toplevel)"
env_file="${YIN_PANEL_ENV_FILE:-$repo_root/.env.local}"
if [[ -f "$env_file" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "$env_file"
    set +a
fi

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

command -v sudo >/dev/null 2>&1 || die "sudo is required"
command -v ss >/dev/null 2>&1 || die "ss is required"
command -v curl >/dev/null 2>&1 || die "curl is required"
command -v npm >/dev/null 2>&1 || die "npm is required"
command -v go >/dev/null 2>&1 || die "go is required"
command -v setsid >/dev/null 2>&1 || die "setsid is required"

[[ -f "$repo_root/frontend/package.json" ]] || die "frontend/package.json is missing"
[[ -f "$repo_root/backend/go.mod" ]] || die "backend/go.mod is missing"
[[ -f "$run_dir/conf.yaml" ]] || die "runtime config is missing: $run_dir/conf.yaml"

sudo -v

printf 'Building frontend...\n'
npm run build-only --prefix "$repo_root/frontend"

printf 'Testing backend...\n'
(cd "$repo_root/backend" && go test ./...)

build_dir="$(mktemp -d /tmp/yin-panel-build.XXXXXX)"
trap 'rm -rf "$build_dir"' EXIT
printf 'Building backend...\n'
(cd "$repo_root/backend" && go build -o "$build_dir/yin-panel" .)

[[ -f "$repo_root/backend/web/index.html" ]] || die "frontend build did not produce backend/web/index.html"
[[ -x "$build_dir/yin-panel" ]] || die "backend build did not produce an executable"

pid="$(ss -ltnp | awk '/:'"$port"' / {match($0,/pid=[0-9]+/); if (RSTART) {print substr($0,RSTART+4,RLENGTH-4); exit}}')"
if [[ -n "$pid" ]]; then
    exe="$(readlink -f "/proc/$pid/exe")"
    [[ "$(basename "$exe")" == "yin-panel" ]] || die "port $port is used by a non-Yin-Panel process: $exe"
    printf 'Stopping Yin-Panel PID %s...\n' "$pid"
    sudo kill "$pid"
    for _ in $(seq 1 100); do
        if ! sudo kill -0 "$pid" 2>/dev/null; then
            break
        fi
        sleep 0.1
    done
    sudo kill -0 "$pid" 2>/dev/null && die "Yin-Panel process did not exit: $pid"
fi

printf 'Replacing runtime binary and frontend files...\n'
sudo rm -f "$run_dir/yin-panel"
sudo rm -rf "$run_dir/web"
sudo cp -a "$build_dir/yin-panel" "$run_dir/yin-panel"
sudo cp -a "$repo_root/backend/web" "$run_dir/web"

printf 'Starting Yin-Panel from %s...\n' "$run_dir"
(cd "$run_dir" && setsid ./yin-panel > yin-panel.log 2>&1 < /dev/null &)

for _ in $(seq 1 100); do
    if ss -ltnp | grep -q ":$port "; then
        break
    fi
    sleep 0.1
done
ss -ltnp | grep -q ":$port " || die "Yin-Panel did not listen on port $port"

curl --noproxy '*' --fail --silent --show-error "http://127.0.0.1:$port/" -o /tmp/yin-panel-home.html
route_status="$(curl --noproxy '*' --silent --output /tmp/yin-panel-search-config-response --write-out '%{http_code}' "http://127.0.0.1:$port/api/spaces/1/search-config")"
[[ "$route_status" != "404" ]] || die "search-config route returned 404"
grep -q "Listening and serving HTTP" "$run_dir/yin-panel.log" || die "startup log did not confirm HTTP readiness"

printf 'Yin-Panel is running on port %s. search-config HTTP status: %s\n' "$port" "$route_status"
