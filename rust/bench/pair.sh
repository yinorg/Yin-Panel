#!/usr/bin/env bash
# One Go deployment and one Rust deployment of the same database, on the same
# host, for a head-to-head measurement.
#
#   rust/bench/pair.sh up   --work DIR --source-db PATH [--go-port 3010] [--rust-port 3011]
#   rust/bench/pair.sh status --work DIR
#   rust/bench/pair.sh down --work DIR
#
# `up` refuses to start when either port is already in use, records the pid of
# each process it started in `$work/pids`, and only `down` stops those pids
# after checking they still belong to this pair. The database is snapshot
# through SQLite's own backup API, so a write-ahead log still in flight is
# included rather than missed.
set -Eeuo pipefail

root="$(git rev-parse --show-toplevel)"
work=""
source_db="$root/backend/database/database.db"
go_port=3010
rust_port=3011
go_bin="$root/build-output/yin-panel-go"
rust_bin="$root/backend/yin-panel"
command=""

die() { printf 'pair: %s\n' "$*" >&2; exit 1; }

while [[ $# -gt 0 ]]; do
    case "$1" in
        up|down|status) command="$1"; shift ;;
        --work) work="$2"; shift 2 ;;
        --source-db) source_db="$2"; shift 2 ;;
        --go-port) go_port="$2"; shift 2 ;;
        --rust-port) rust_port="$2"; shift 2 ;;
        --go-bin) go_bin="$2"; shift 2 ;;
        --rust-bin) rust_bin="$2"; shift 2 ;;
        --help|-h)
            sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
            exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done
[[ -n "$command" ]] || die "expected up, status or down"
[[ -n "$work" ]] || die "--work is required"

listener_pid() {
    ss -ltnp "sport = :$1" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1 || true
}

exe_of() { readlink -f "/proc/$1/exe" 2>/dev/null || true; }

snapshot_db() { # source destination
    python3 - "$1" "$2" <<'PY'
import sqlite3, sys, os
source, destination = sys.argv[1], sys.argv[2]
if not os.path.isfile(source):
    raise SystemExit(f"source database not found: {source}")
os.makedirs(os.path.dirname(destination), exist_ok=True)
src = sqlite3.connect(f"file:{source}?mode=ro", uri=True)
dst = sqlite3.connect(destination)
with dst:
    src.backup(dst)
dst.close()
src.close()
PY
}

write_conf() { # directory port
    python3 - "$root/backend/conf.yaml" "$1/conf.yaml" "$2" <<'PY'
import sys
source, destination, port = sys.argv[1], sys.argv[2], sys.argv[3]
lines = open(source, encoding="utf-8").read().splitlines()
output = []
for line in lines:
    stripped = line.strip()
    if stripped.startswith("http_port:"):
        indent = line[: len(line) - len(line.lstrip())]
        output.append(f"{indent}http_port: {port}")
    elif stripped.startswith("root_url:"):
        indent = line[: len(line) - len(line.lstrip())]
        output.append(f"{indent}root_url: http://127.0.0.1:{port}")
    elif stripped.startswith("file_path:"):
        indent = line[: len(line) - len(line.lstrip())]
        output.append(f"{indent}file_path: ./database/database.db")
    elif stripped.startswith("enable_monitor:"):
        # Both sides run with the monitor off so the comparison measures the
        # request path, not the collector. The monitor has its own check.
        indent = line[: len(line) - len(line.lstrip())]
        output.append(f"{indent}enable_monitor: false")
    else:
        output.append(line)
open(destination, "w", encoding="utf-8").write("\n".join(output) + "\n")
PY
}

start_one() { # binary directory label port -> prints pid, fails if not its own
    local bin="$1" directory="$2" label="$3" port="$4"
    [[ -z "$(listener_pid "$port")" ]] || die "port $port is already in use; refusing to start"
    ( cd "$directory" && exec "$bin" -c conf.yaml ) >"$directory/$label.log" 2>&1 < /dev/null &
    local pid=$!
    local waited=0
    while (( waited < 200 )); do
        kill -0 "$pid" 2>/dev/null || {
            tail -n 30 "$directory/$label.log" >&2
            die "$label ($bin) exited during startup"
        }
        if curl -fsS --noproxy '*' --max-time 1 -o /dev/null "http://127.0.0.1:$port/api/about" 2>/dev/null; then
            local owner
            owner="$(listener_pid "$port")"
            [[ "$owner" == "$pid" ]] ||
                die "port $port is served by pid $owner, not the $label process $pid"
            printf '%s\n' "$pid"
            return 0
        fi
        sleep 0.1
        waited=$((waited + 1))
    done
    tail -n 30 "$directory/$label.log" >&2
    die "$label did not answer /api/about within 20s"
}

case "$command" in
    up)
        [[ -x "$go_bin" ]] || die "missing Go binary: $go_bin"
        [[ -x "$rust_bin" ]] || die "missing Rust binary: $rust_bin"
        for port in "$go_port" "$rust_port"; do
            [[ -z "$(listener_pid "$port")" ]] ||
                die "port $port is already in use by pid $(listener_pid "$port"); refusing to start"
        done
        rm -rf "$work"
        for side in go rust; do
            mkdir -p "$work/$side/database" "$work/$side/uploads"
            snapshot_db "$source_db" "$work/$side/database/database.db"
            ln -s "$root/backend/web" "$work/$side/web"
            cp -a "$root/backend/lang" "$work/$side/lang"
        done
        write_conf "$work/go" "$go_port"
        write_conf "$work/rust" "$rust_port"
        go_pid="$(start_one "$go_bin" "$work/go" go "$go_port")"
        rust_pid="$(start_one "$rust_bin" "$work/rust" rust "$rust_port")"
        printf 'go_pid=%s rust_pid=%s\n' "$go_pid" "$rust_pid" > "$work/pids"
        printf 'go   %s pid %s\trust %s pid %s\n' \
            "http://127.0.0.1:$go_port" "$go_pid" "http://127.0.0.1:$rust_port" "$rust_pid"
        ;;
    status)
        [[ -f "$work/pids" ]] || die "no pids recorded in $work; was the pair started with this script?"
        # shellcheck disable=SC1090
        source "$work/pids"
        for side in go rust; do
            eval "pid=\$$side""_pid"
            printf '%s pid %s exe %s\n' "$side" "$pid" "$(exe_of "$pid")"
        done
        ;;
    down)
        [[ -f "$work/pids" ]] || die "no pids recorded in $work"
        # shellcheck disable=SC1090
        source "$work/pids"
        for side in go rust; do
            eval "pid=\$$side""_pid"
            [[ -n "$pid" ]] || continue
            if ! kill -0 "$pid" 2>/dev/null; then
                printf '%s pid %s already exited\n' "$side" "$pid"
                continue
            fi
            exe="$(exe_of "$pid")"
            [[ "$(basename "$exe")" == "yin-panel" ]] ||
                die "pid $pid is $exe, not the yin-panel this pair started; refusing to stop it"
            kill "$pid" 2>/dev/null || true
        done
        for side in go rust; do
            eval "pid=\$$side""_pid"
            for _ in $(seq 1 100); do
                kill -0 "$pid" 2>/dev/null || break
                sleep 0.1
            done
            kill -9 "$pid" 2>/dev/null || true
        done
        printf 'stopped the go/rust pair in %s\n' "$work"
        ;;
esac
