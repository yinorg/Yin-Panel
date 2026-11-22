#!/usr/bin/env sh
# Replay the recorded contract. `rust-anything` looks for `<repo>/rust/app-cases`
# by convention; this project keeps the backend under `backend/`, so the tool is
# pointed at a throwaway staging directory whose `rust` entry links to `backend`.
# The report is written through that link into `backend/app-cases/`.
#
# Usage: scripts/replay.sh [extra replay options]
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
staging="$(mktemp -d "${TMPDIR:-/tmp}/yin-panel-replay.XXXXXX")"
trap 'rm -rf "$staging"' EXIT
ln -s "$repo_root/backend" "$staging/rust"
ln -s "$repo_root/backend" "$staging/backend"

exec rust-anything app replay "$staging" \
    --build "sh backend/record/run-rust.sh" "$@"
