#!/bin/bash
set -e

# Configuration
IMAGE_NAME="smart-crawler"
IMAGE_TAG="${1:-latest}"
REGISTRY="${2:-}"  # Optional registry prefix, e.g., "your-registry.com/"

FULL_IMAGE_NAME="${REGISTRY}${IMAGE_NAME}:${IMAGE_TAG}"

# Display build information
echo "Building crawler image: ${FULL_IMAGE_NAME}"
echo "========================================"

# Navigate to project root
cd "$(dirname "$0")/.."

# Build the Docker image
echo "Building Docker image..."
docker build -t "${FULL_IMAGE_NAME}" -f Dockerfile .

# Push the image if registry is provided
if [ -n "$REGISTRY" ]; then
  echo "Pushing image to registry..."
  docker push "${FULL_IMAGE_NAME}"
fi

echo "Crawler image built successfully: ${FULL_IMAGE_NAME}"