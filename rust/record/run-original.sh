#!/bin/sh
# Starts the Go backend as the reference implementation for rust-anything.
#
# `rust-anything app record` runs this once per recording run and once per
# replay of the original. It always starts from a throwaway working directory:
# the recording never reads or writes the deployment database, uploads or
# configuration.
#
# Usage: rust/record/run-original.sh <port>
#
# Environment:
#   YIN_PANEL_GO_BIN   use this binary instead of building build-output/yin-panel-go
#   YIN_PANEL_RECORD_DIR  working directory (default: $TMPDIR/yin-panel-record)
#   YIN_PANEL_RECORD_CONF  alternate recorder-only config for focused scenarios
set -eu

port="${1:-}"
if [ -z "$port" ]; then
    echo "usage: $0 <port>" >&2
    exit 2
fi

root="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
bin="${YIN_PANEL_GO_BIN:-$root/build-output/yin-panel-go}"

if [ ! -x "$bin" ]; then
    mkdir -p "$(dirname -- "$bin")"
    (cd "$root/backend" && go build -trimpath -o "$bin" .)
fi

work="${YIN_PANEL_RECORD_DIR:-${TMPDIR:-/tmp}/yin-panel-record}"
conf="${YIN_PANEL_RECORD_CONF:-$root/rust/record/conf.yaml}"
rm -rf "$work"
mkdir -p "$work/uploads"
cp "$conf" "$work/conf.yaml"
sed -i "s/__PORT__/$port/g" "$work/conf.yaml"
cp -a "$root/backend/lang" "$work/lang"

cd "$work"
exec "$bin" -c conf.yaml
