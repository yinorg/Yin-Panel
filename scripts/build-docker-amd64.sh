#!/usr/bin/env bash
set -euo pipefail

TAG="${1:-local}"
IMAGE_ROOT="yin-panel-ce"

echo "Building Yin Panel monolith image for linux/amd64 (tag: ${TAG})"

docker build --platform linux/amd64 \
  --build-arg VERSION="${TAG}" \
  -f distribution/docker-image/Dockerfile_monolith \
  -t "${IMAGE_ROOT}:${TAG}" .

echo "Built: ${IMAGE_ROOT}/monolith:${TAG}"
