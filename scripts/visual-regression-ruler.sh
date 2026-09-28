#!/usr/bin/env bash
# Captures the old-version visual ruler from the official release image.
#
# The ruler is the ground truth for pixel comparison against the current build.
# It is deliberately sourced from the published image rather than rebuilt from
# a git commit, because rebuilding an old commit on a modern toolchain drifts
# from what users actually ran.
#
# Usage:
#   ./scripts/visual-regression-ruler.sh            # start the image if needed, then capture
#   ./scripts/visual-regression-ruler.sh --stop     # stop and remove the container
#   ./scripts/visual-regression-ruler.sh --update   # regenerate the ruler baselines
#
# Env:
#   YIN_RULER_IMAGE  default ghcr.io/yinorg/yin-panel-ce:0.4.2
#   YIN_RULER_CONTAINER default yin-ruler
#   YIN_RULER_PORT   default 3003

set -Eeuo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
IMAGE="${YIN_RULER_IMAGE:-ghcr.io/yinorg/yin-panel-ce:0.4.2}"
CONTAINER="${YIN_RULER_CONTAINER:-yin-ruler}"
PORT="${YIN_RULER_PORT:-3003}"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

log() { printf "${BLUE}[ruler]${NC} %s\n" "$*"; }
success() { printf "${GREEN}[ruler]${NC} %s\n" "$*"; }
error() { printf "${RED}[ruler]${NC} %s\n" "$*" >&2; }

container_running() {
  [[ "$(docker inspect -f '{{.State.Running}}' "${CONTAINER}" 2>/dev/null || echo false)" == "true" ]]
}

stop_container() {
  if docker ps -a --format '{{.Names}}' | grep -qx "${CONTAINER}"; then
    log "Removing container ${CONTAINER}..."
    docker rm -f "${CONTAINER}" >/dev/null
  fi
}

start_container() {
  if container_running; then
    log "Container ${CONTAINER} already running."
    return
  fi
  stop_container
  log "Pulling ${IMAGE}..."
  docker pull "${IMAGE}" >/dev/null
  log "Starting ${CONTAINER} on port ${PORT}..."
  docker run -d --name "${CONTAINER}" -p "${PORT}:3002" "${IMAGE}" >/dev/null
  for _ in $(seq 1 60); do
    if curl --noproxy '*' -sf -o /dev/null "http://127.0.0.1:${PORT}/"; then
      success "Ruler source is serving on http://127.0.0.1:${PORT}"
      return
    fi
    sleep 1
  done
  error "Container did not become ready in time."
  docker logs "${CONTAINER}" || true
  exit 1
}

case "${1:-}" in
  --stop)
    stop_container
    success "Container removed."
    exit 0
    ;;
  --update)
    UPDATE_FLAG=(--update-snapshots)
    ;;
  *)
    UPDATE_FLAG=()
    ;;
esac

start_container

cd "${REPO_ROOT}/frontend"
if [[ ! -f package.json ]]; then
  error "frontend/package.json not found under ${REPO_ROOT}"
  exit 1
fi

log "Running the ruler suite (compares against existing baselines)..."
npx playwright test --config=playwright.ruler.config.ts tests/visual/ruler-capture.spec.ts "${UPDATE_FLAG[@]}"

success "Ruler check complete. Baselines live in tests/visual/ruler-snapshots/."
