#!/bin/bash
# Build ModernTensor Docker Image

set -e

echo "Building ModernTensor Docker image..."

# Get version from pyproject.toml or use default
VERSION=$(grep '^version' ../pyproject.toml | head -1 | cut -d'"' -f2 || echo "0.1.0")

# Build image
docker build -t moderntensor:${VERSION} -t moderntensor:latest -f Dockerfile ..

echo "✅ Successfully built moderntensor:${VERSION}"
echo "   Tagged as: moderntensor:latest"

# Show image details
docker images moderntensor --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedAt}}"
