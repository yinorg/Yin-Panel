#!/usr/bin/env bash
set -euo pipefail

TAG="${1:-local}"
IMAGE_ROOT="ghcr.io/phantommaa/yin-panel"

echo "Building Yin Panel Docker image for linux/amd64 (tag: ${TAG})"

docker build --platform linux/amd64 \
  -f distribution/docker-image/Dockerfile_temp \
  -t "${IMAGE_ROOT}/temp:${TAG}" .

docker build --platform linux/amd64 \
  --build-arg VERSION="${TAG}" \
  -f distribution/docker-image/Dockerfile_backend \
  -t "${IMAGE_ROOT}/backend:${TAG}" .

docker build --platform linux/amd64 \
  --build-arg REPO_LOWER=phantommaa/yin-panel \
  --build-arg VERSION="${TAG}" \
  -f distribution/docker-image/Dockerfile_monolith \
  -t "${IMAGE_ROOT}/monolith:${TAG}" .

echo "Built: ${IMAGE_ROOT}/monolith:${TAG}"
