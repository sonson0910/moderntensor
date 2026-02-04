# ModernTensor Layer 1 Node - Dockerfile
FROM python:3.11-slim

# Set working directory
WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy requirements first for better caching
COPY requirements.txt pyproject.toml ./

# Install Python dependencies
RUN pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir -r requirements.txt && \
    pip install --no-cache-dir pytest pytest-asyncio

# Copy application code
COPY sdk/ ./sdk/
COPY tests/ ./tests/
COPY *.md ./
COPY *.py ./

# Create data directory
RUN mkdir -p /data/blockchain /data/state

# Expose ports
# 30303: P2P networking
# 8545: JSON-RPC API
# 8080: GraphQL API (optional)
# 9090: Prometheus metrics
EXPOSE 30303 8545 8080 9090

# Set environment variables
ENV PYTHONPATH=/app
ENV MT_DATA_DIR=/data
ENV MT_LOG_LEVEL=INFO

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:8545/health || exit 1

# Default command (can be overridden)
CMD ["python", "-m", "uvicorn", "sdk.api.rpc:app", "--host", "0.0.0.0", "--port", "8545"]
