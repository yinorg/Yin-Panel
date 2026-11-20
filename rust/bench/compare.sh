#!/usr/bin/env bash
# Alternating repeated measurement of a Go and a Rust deployment of the same
# service.
#
#   rust/bench/compare.sh --go http://127.0.0.1:3010 --rust http://127.0.0.1:3011 \
#       [--reps 5] [--requests 4000] [--concurrency 32] [--endpoint /api/about]...
#
# Each repetition runs one side and then the other, alternating which goes
# first, so a slow drift on the host hits both sides equally. The report gives
# the median of each repetition's RPS/latency, plus the observed range, so a
# claim of "faster" has to survive the noise. It exits non-zero when either
# side served a request that was not a complete success.
set -Eeuo pipefail

root="$(git rev-parse --show-toplevel)"
go_url=""
rust_url=""
reps=5
requests=4000
concurrency=32
token=""
mail="admin@yiniot.com"
password="admin@yiniot.com"
endpoints=()
loadgen="$root/rust/bench/loadgen/loadgen"

die() { printf 'compare: %s\n' "$*" >&2; exit 1; }

while [[ $# -gt 0 ]]; do
    case "$1" in
        --go) go_url="$2"; shift 2 ;;
        --rust) rust_url="$2"; shift 2 ;;
        --reps) reps="$2"; shift 2 ;;
        --requests) requests="$2"; shift 2 ;;
        --concurrency) concurrency="$2"; shift 2 ;;
        --endpoint) endpoints+=("$2"); shift 2 ;;
        --token) token="$2"; shift 2 ;;
        --mail) mail="$2"; shift 2 ;;
        --password) password="$2"; shift 2 ;;
        --loadgen) loadgen="$2"; shift 2 ;;
        --help|-h) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done

[[ -n "$go_url" ]] || die "--go is required"
[[ -n "$rust_url" ]] || die "--rust is required"
[[ "$reps" -ge 1 ]] || die "--reps must be positive"
[[ "${#endpoints[@]}" -gt 0 ]] || endpoints=(
    /api/about
    /
    /api/theme/v2/current
    /api/spaces
)

if [[ ! -x "$loadgen" ]]; then
    printf 'building the load generator...\n'
    ( cd "$root/rust/bench/loadgen" && go build -trimpath -o loadgen . ) || die "could not build loadgen"
fi

port_of() {
    local rest="${1#http://}"
    rest="${rest%%/*}"
    printf '%s\n' "${rest##*:}"
}
pid_of() {
    ss -ltnp "sport = :$1" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1 || true
}

go_port="$(port_of "$go_url")"
rust_port="$(port_of "$rust_url")"
go_pid="$(pid_of "$go_port")"
rust_pid="$(pid_of "$rust_port")"
[[ -n "$go_pid" ]] || die "nothing listening on $go_port"
[[ -n "$rust_pid" ]] || die "nothing listening on $rust_port"

if [[ -z "$token" ]]; then
    token="$(curl -fsS --noproxy '*' -H 'Content-Type: application/json' \
        -d "{\"mail\":\"$mail\",\"password\":\"$password\"}" "$go_url/api/login" |
        python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["token"])')" ||
        die "could not log in on $go_url"
fi

work="$(mktemp -d "${TMPDIR:-/tmp}/yin-panel-compare.XXXXXX")"
results="$work/results.jsonl"
: > "$results"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

printf 'load: %s requests, %s connections, %s repetitions per side\n' "$requests" "$concurrency" "$reps"
printf 'go   %s pid %s\nrust %s pid %s\n' "$go_url" "$go_pid" "$rust_url" "$rust_pid"

for index in "${!endpoints[@]}"; do
    endpoint="${endpoints[$index]}"
    for side in go rust; do
        if [[ "$side" == "go" ]]; then
            base="$go_url"; pid="$go_pid"
        else
            base="$rust_url"; pid="$rust_pid"
        fi
        "$loadgen" -url "$base$endpoint" -n 100 -c 4 -pid "$pid" \
            -header "Cookie: yin_token=$token" -allow-failures > /dev/null 2>&1 || true
    done

    for rep in $(seq 1 "$reps"); do
        # Alternate the order so a thermal or scheduler drift is shared.
        if (( rep % 2 == 1 )); then order=(go rust); else order=(rust go); fi
        for side in "${order[@]}"; do
            if [[ "$side" == "go" ]]; then
                base="$go_url"; pid="$go_pid"
            else
                base="$rust_url"; pid="$rust_pid"
            fi
            set +e
            body="$("$loadgen" -url "$base$endpoint" -n "$requests" -c "$concurrency" \
                -pid "$pid" -header "Cookie: yin_token=$token" -json 2>"$work/error")"
            status=$?
            set -e
            [[ -n "$body" ]] || { cat "$work/error" >&2; die "loadgen produced no result for $side $endpoint"; }
            printf '%s' "$body" > "$work/body.json"
            python3 - "$endpoint" "$side" "$rep" "$work/body.json" >> "$results" <<'PY'
import json, sys
data = json.load(open(sys.argv[4], encoding="utf-8"))
data["endpoint"] = sys.argv[1]
data["side"] = sys.argv[2]
data["rep"] = int(sys.argv[3])
print(json.dumps(data))
PY
            if (( status != 0 )); then
                printf '%s %s had failed requests in repetition %s\n' "$side" "$endpoint" "$rep" >&2
            fi
        done
    done
done

python3 - "$results" <<'PY'
import json, statistics, sys

rows = []
for line in open(sys.argv[1], encoding="utf-8"):
    rows.append(json.loads(line))

by = {}
for row in rows:
    by.setdefault((row["endpoint"], row["side"]), []).append(row)

endpoints = []
for endpoint, _side in by:
    if endpoint not in endpoints:
        endpoints.append(endpoint)

def med(values):
    return statistics.median(values) if values else 0.0

problems = []
print()
print(f"{'endpoint':32} {'side':4} {'ok/fail':>14} {'rps median':>11} {'rps range':>17} "
      f"{'p50ms':>7} {'p95ms':>7} {'p99ms':>7} {'cpu/req':>9}")
print("-" * 122)
for endpoint in endpoints:
    for side in ("go", "rust"):
        group = by.get((endpoint, side), [])
        if not group:
            continue
        ok = sum(row["ok"] for row in group)
        failed = sum(row["failed"] for row in group)
        rps = [row["rps"] for row in group]
        range_text = f"{min(rps):.0f}-{max(rps):.0f}"
        print(f"{endpoint:32} {side:4} {str(ok) + '/' + str(failed):>14} {med(rps):11.0f} "
              f"{range_text:>17} {med([r['p50_ms'] for r in group]):7.2f} "
              f"{med([r['p95_ms'] for r in group]):7.2f} {med([r['p99_ms'] for r in group]):7.2f} "
              f"{med([r['cpu_us_per_request'] for r in group]):9.1f}")
        if failed:
            reasons = {}
            for row in group:
                for reason, count in (row.get("failures") or {}).items():
                    reasons[reason] = reasons.get(reason, 0) + count
            problems.append(f"{side} {endpoint}: {failed} failed request(s): {reasons}")

print()
for endpoint in endpoints:
    go = [row["rps"] for row in by.get((endpoint, "go"), [])]
    rust = [row["rps"] for row in by.get((endpoint, "rust"), [])]
    if go and rust and med(go) > 0:
        ratio = med(rust) / med(go)
        verdict = "rust faster" if ratio > 1.05 else ("go faster" if ratio < 0.95 else "level")
        print(f"{endpoint:32} rust/go median rps = {ratio:.2f}x  ({verdict})")

if problems:
    print()
    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    sys.exit(1)
PY
