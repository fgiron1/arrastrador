#!/bin/bash
set -e

# Configuration
IMAGE_NAME="crawler-browser-service"
IMAGE_TAG="${1:-latest}"
REGISTRY="${2:-}"  # Optional registry prefix

FULL_IMAGE_NAME="${REGISTRY}${IMAGE_NAME}:${IMAGE_TAG}"

# Display build information
echo "Building browser service image: ${FULL_IMAGE_NAME}"
echo "========================================"

# Navigate to project root
cd "$(dirname "$0")/.."

# Ensure drivers in browser-service/drivers are executable
chmod +x browser-service/drivers/* 2>/dev/null || echo "No drivers found or already executable"

# Build the Docker image
echo "Building Docker image..."
docker build -t "${FULL_IMAGE_NAME}" -f build/browser-service.Dockerfile ./browser-service

# Push the image if registry is provided
if [ -n "$REGISTRY" ]; then
  echo "Pushing image to registry..."
  docker push "${FULL_IMAGE_NAME}"
fi

echo "Browser service image built successfully: ${FULL_IMAGE_NAME}"