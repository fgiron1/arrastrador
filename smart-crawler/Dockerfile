FROM rust:1.70-slim as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source file to build dependencies
RUN mkdir -p src && \
    echo "fn main() {println!(\"dummy\")}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src/

# Build the actual application - all dependencies are now mandatory
RUN touch src/main.rs && \
    cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates libssl1.1 && \
    rm -rf /var/lib/apt/lists/*

# Create app directory and a non-root user
RUN mkdir -p /app/config/sites && \
    useradd -m crawler && \
    chown -R crawler:crawler /app

# Copy the built binary
COPY --from=builder /app/target/release/crawler /usr/local/bin/

# Copy configuration files
COPY config/default.yaml /app/config/
COPY config/sites/ /app/config/sites/

WORKDIR /app
USER crawler

# Environment variables for connecting to services
ENV REDIS_URL="redis://redis-service:6379"
ENV MONGODB_URL="mongodb://mongodb-service:27017"
ENV POSTGRES_URL="postgresql://postgres:postgres@postgres-service:5432/crawler"
ENV BROWSER_SERVICE_URL="http://browser-service:5000"
ENV RUST_LOG="info,crawler=debug"

# Health check to ensure the service is running
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD crawler status health || exit 1

# Default command - set this to what you want the crawler to do when started
ENTRYPOINT ["crawler"]
CMD ["serve", "--host", "0.0.0.0", "--port", "8080"]