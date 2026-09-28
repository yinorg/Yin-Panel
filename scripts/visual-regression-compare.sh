#!/usr/bin/env bash
# Compares the current build against the confirmed old-version ruler.
#
# This config can never overwrite the ruler: playwright.compare.config.ts pins
# updateSnapshots to 'none'. Regenerating the ruler is only possible through
# scripts/visual-regression-ruler.sh.
#
# Prerequisites: the current build must be serving on YIN_PANEL_PORT (3002).
#
# Usage:
#   ./scripts/visual-regression-compare.sh
#   ./scripts/visual-regression-compare.sh --list     # show diff artefacts only

set -Eeuo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"

set +x
ENV_FILE="${YIN_PANEL_ENV_FILE:-${REPO_ROOT}/.env.local}"
if [[ -f "${ENV_FILE}" ]]; then
  set -a
  # shellcheck disable=SC1090
  . "${ENV_FILE}"
  set +a
fi
export -n SUDO_PASSWORD SUDO_PASSWD 2>/dev/null || true

PORT="${YIN_PANEL_PORT:-3002}"
BASE_URL="http://127.0.0.1:${PORT}"

BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

log() { printf "${BLUE}[compare]${NC} %s\n" "$*"; }
success() { printf "${GREEN}[compare]${NC} %s\n" "$*"; }
error() { printf "${RED}[compare]${NC} %s\n" "$*" >&2; }

if ! curl --noproxy '*' -sf -o /dev/null "${BASE_URL}/"; then
  error "No build is serving on ${BASE_URL}."
  error "Deploy first with scripts/deploy-local.sh, then re-run this comparison."
  exit 1
fi

cd "${REPO_ROOT}/frontend"
if [[ ! -f package.json ]]; then
  error "frontend/package.json not found under ${REPO_ROOT}"
  exit 1
fi

if [[ ! -d tests/visual/ruler-snapshots ]]; then
  error "Ruler baselines are missing."
  error "Regenerate them with: ./scripts/visual-regression-ruler.sh --update"
  exit 1
fi

log "Comparing the current build against the old-version ruler..."
log "Diff artefacts (actual / expected / diff) land in frontend/test-results/."
npx playwright test --config=playwright.compare.config.ts "${1:-}" || {
  error "Visual differences found against the ruler."
  error "Review frontend/test-results/ for the per-scenario diff images."
  exit 1
}

success "No visual differences against the ruler."
