#!/bin/sh
# Starts the Rust build as the subject under test, in the same throwaway working
# directory `run-original.sh` uses, so the recorded cookie secret and the
# database location are identical to the recording.
#
# Usage: rust/record/run-rust.sh
# Environment: PORT (required), YIN_PANEL_RECORD_DIR (optional).
set -eu

port="${PORT:-}"
if [ -z "$port" ]; then
    echo "PORT is required" >&2
    exit 2
fi

root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
bin="${YIN_PANEL_RUST_BIN:-$root/rust/app/target/release/run-original}"

if [ ! -x "$bin" ]; then
    echo "build the front door first: cargo build --release --manifest-path rust/app/Cargo.toml" >&2
    exit 2
fi

work="${YIN_PANEL_RECORD_DIR:-${TMPDIR:-/tmp}/yin-panel-record}"
mkdir -p "$work/uploads"
cp "$root/rust/record/conf.yaml" "$work/conf.yaml"
sed -i "s/__PORT__/$port/g" "$work/conf.yaml"
cp -a "$root/backend/lang" "$work/lang"

cd "$work"
exec "$bin"
