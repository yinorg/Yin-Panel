#!/usr/bin/env bash
# One command for the Rust release gate: Go->Rust->Go state, replay parity,
# compression/range HTTP parity, and a repeated Go-vs-Rust performance report.
#
#   scripts/verify-rust-release.sh [--skip-perf]
#
# Requires .env.local (as the repository's agent protocol specifies), the Go
# rollback binary, a deployed Rust binary, the separate temporary ports 3040
# and 3010/3011, Python 3, curl, ss, rust-anything and Go. The existing live
# service on YIN_PANEL_PORT is not stopped. The benchmark pair is always
# cleaned up on exit and refuses any port already in use.
set -Eeuo pipefail

root="$(git rev-parse --show-toplevel)"
env_file="${YIN_PANEL_ENV_FILE:-$root/.env.local}"
[[ -f "$env_file" ]] || { printf 'missing local environment: %s\n' "$env_file" >&2; exit 1; }
set +x
set -a
# shellcheck disable=SC1090
source "$env_file"
set +a
export -n SUDO_PASSWORD SUDO_PASSWD 2>/dev/null || true

skip_perf=0
for arg in "$@"; do
    case "$arg" in
        --skip-perf) skip_perf=1 ;;
        *) printf 'unknown option: %s\n' "$arg" >&2; exit 2 ;;
    esac
done

work="/tmp/opencode/rust-release-$$"
bench_work="$work/pair"
bench_up=0
cleanup() {
    status=$?
    if [[ "$bench_up" -eq 1 ]]; then
        "$root/backend/bench/pair.sh" down --work "$bench_work" || true
    fi
    rm -rf "$work"
    return "$status"
}
trap cleanup EXIT
mkdir -p "$work"

say() { printf '\n== %s\n' "$*"; }

say "tool self-tests"
python3 "$root/scripts/upgrade_compare.py" selftest
bash -n "$root/scripts/verify-upgrade.sh" "$root/backend/bench/pair.sh" "$root/backend/bench/compare.sh"

say "Go -> Rust -> Go content-preserving upgrade"
"$root/scripts/verify-upgrade.sh" 3040

say "build and record/replay parity"
export PATH="$HOME/.cargo/bin:$PATH"
(cd "$root/backend/app" && cargo build --release)
export YIN_PANEL_RECORD_DIR="$work/record"
"$root/scripts/replay.sh"

say "Go/Rust gzip and body parity"
"$root/backend/bench/pair.sh" up \
    --work "$bench_work" \
    --source-db "$root/runtime/database/database.db" \
    --go-port 3010 --rust-port 3011
bench_up=1
go_token="$(curl -fsS --noproxy '*' -H 'Content-Type: application/json' \
    -d '{"mail":"admin@yiniot.com","password":"admin@yiniot.com"}' \
    http://127.0.0.1:3010/api/login |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["token"])')"
python3 "$root/backend/check/gzipcheck.py" \
    http://127.0.0.1:3010 http://127.0.0.1:3011 "$go_token"
python3 "$root/backend/check/rangecheck.py" \
    http://127.0.0.1:3010 http://127.0.0.1:3011 "$go_token"

if [[ "$skip_perf" -eq 0 ]]; then
    say "repeated, alternating head-to-head performance baseline"
    "$root/backend/bench/compare.sh" \
        --go http://127.0.0.1:3010 --rust http://127.0.0.1:3011 \
        --token "$go_token" --reps 5 --requests 1500 --concurrency 1
    "$root/backend/bench/compare.sh" \
        --go http://127.0.0.1:3010 --rust http://127.0.0.1:3011 \
        --token "$go_token" --reps 5 --requests 3000 --concurrency 32
fi

say "Rust release gate passed"
