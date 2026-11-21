#!/bin/sh
# Starts the Rust build as the subject under test, in the same throwaway working
# directory `run-original.sh` uses, so the recorded cookie secret and the
# database location are identical to the recording.
#
# Usage: rust/record/run-rust.sh
# Environment: PORT (required), YIN_PANEL_RECORD_DIR and YIN_PANEL_RECORD_CONF (optional).
set -eu

port="${PORT:-}"
if [ -z "$port" ]; then
    echo "PORT is required" >&2
    exit 2
fi

root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
bin="${YIN_PANEL_RUST_BIN:-$root/rust/app/target/release/yin-panel}"

if [ ! -x "$bin" ]; then
    echo "build the front door first: cargo build --release --manifest-path rust/app/Cargo.toml" >&2
    exit 2
fi

work="${YIN_PANEL_RECORD_DIR:-${TMPDIR:-/tmp}/yin-panel-record}"
conf="${YIN_PANEL_RECORD_CONF:-$root/rust/record/conf.yaml}"
# The recorded cases are a sequence, so every run starts from an empty
# installation, exactly as `run-original.sh` does.
rm -rf "$work"
mkdir -p "$work/uploads"
cp "$conf" "$work/conf.yaml"
sed -i "s/__PORT__/$port/g" "$work/conf.yaml"
cp -a "$root/backend/lang" "$work/lang"

cd "$work"
exec "$bin"
