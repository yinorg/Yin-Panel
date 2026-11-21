#!/usr/bin/env bash
# Parity metrics for the Go -> Rust replacement.
#
# Prints the counts `rust/GOAL_REPLACE_GO.md` tracks and compares them with
# `rust/parity-baseline.txt`. Any count that grows fails the run, so progress
# can only move forward.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
routes="$repo_root/rust/app/src/routes.rs"
go_routes="$repo_root/rust/app/src/go_routes.rs"
units="$repo_root/rust/app/src/units/mod.rs"
baseline="$repo_root/rust/parity-baseline.txt"

go_route_count=$(grep -c '("' "$go_routes")
rust_route_count=$(grep -c '\.route(' "$routes")
wire_route_count=$(grep -c '\.route(' "$units" || true)
sqlite_pool=$(
    grep -c 'ctx\.pool()\.await' "$routes" || true
)
shared_pool=$(
    grep -c 'ctx\.shared_pool()\.await' "$routes" || true
)
insert_rowid=$(
    grep -c 'last_insert_rowid' "$routes" || true
)
sqlite_params=$(
    grep -c 'SqlitePool' "$routes" || true
)

print_row() {
    printf '%-24s %s\n' "$1" "$2"
}

print_row "go_routes" "$go_route_count"
print_row "rust_routes" "$rust_route_count"
print_row "wire_routes" "$wire_route_count"
print_row "sqlite_pool_handlers" "$sqlite_pool"
print_row "shared_pool_handlers" "$shared_pool"
print_row "last_insert_rowid" "$insert_rowid"
print_row "sqlite_pool_params" "$sqlite_params"

if [[ "${1:-}" == "--write-baseline" ]]; then
    {
        echo "go_routes=$go_route_count"
        echo "rust_routes=$rust_route_count"
        echo "wire_routes=$wire_route_count"
        echo "sqlite_pool_handlers=$sqlite_pool"
        echo "shared_pool_handlers=$shared_pool"
        echo "last_insert_rowid=$insert_rowid"
        echo "sqlite_pool_params=$sqlite_params"
    } >"$baseline"
    echo "baseline written to ${baseline#"$repo_root"/}"
    exit 0
fi

if [[ ! -f "$baseline" ]]; then
    echo "missing ${baseline#"$repo_root"/}; run with --write-baseline once" >&2
    exit 2
fi

declare -A current_values=(
    [go_routes]="$go_route_count"
    [rust_routes]="$rust_route_count"
    [wire_routes]="$wire_route_count"
    [sqlite_pool_handlers]="$sqlite_pool"
    [shared_pool_handlers]="$shared_pool"
    [last_insert_rowid]="$insert_rowid"
    [sqlite_pool_params]="$sqlite_params"
)

status=0
while IFS='=' read -r key value; do
    [[ -z "$key" ]] && continue
    current="${current_values[$key]-}"
    if [[ -z "$current" ]]; then
        echo "unknown baseline key: $key" >&2
        status=1
        continue
    fi
    if [[ "$current" -gt "$value" ]]; then
        echo "regression: $key is $current (baseline $value)" >&2
        status=1
    fi
done <"$baseline"

if [[ "$status" -ne 0 ]]; then
    exit 1
fi
echo "metrics within baseline"
