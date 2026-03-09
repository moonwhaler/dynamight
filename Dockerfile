# Stage 1: Build Frontend
FROM node:20-alpine AS frontend-builder

WORKDIR /app/frontend

# Install dependencies
COPY frontend/package.json frontend/package-lock.json* ./
RUN npm ci || npm install

# Build frontend
COPY frontend/ ./
RUN npm run build

# Stage 2: Build Backend
FROM rust:1-alpine AS backend-builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY backend/Cargo.toml backend/build.rs ./backend/
COPY migrations ./migrations

# Create dummy main to cache dependencies
RUN mkdir -p backend/src && echo "fn main() {}" > backend/src/main.rs
RUN cargo build --release --package dynamight 2>/dev/null || true

# Build actual application
COPY backend/src ./backend/src
ARG BUILD_GIT_HASH=unknown
ARG BUILD_DATE=unknown
ENV BUILD_GIT_HASH=${BUILD_GIT_HASH}
ENV BUILD_DATE=${BUILD_DATE}
RUN touch backend/src/main.rs && cargo build --release --package dynamight

# Stage 3: Runtime
FROM alpine:3.20

# Install runtime dependencies
RUN apk add --no-cache \
    rsync \
    util-linux \
    e2fsprogs \
    ntfs-3g \
    ntfs-3g-progs \
    exfatprogs \
    ca-certificates \
    tzdata \
    tar \
    zip \
    openssl

WORKDIR /app

# Copy built artifacts
COPY --from=backend-builder /app/target/release/dynamight ./
COPY --from=frontend-builder /app/frontend/dist ./static
COPY migrations ./migrations
COPY dynamight.toml.example ./dynamight.toml.example

# Create necessary directories
RUN mkdir -p /app/data /app/logs /mnt /app/config

# Default environment (can be overridden or use config file)
ENV DATABASE_URL=sqlite:/app/data/dynamight.db
ENV STATIC_FILES_DIR=/app/static
ENV RUST_LOG=info,dynamight=debug
ENV DYNAMIGHT_CONFIG=/app/config/dynamight.toml

# Expose default port
EXPOSE 8080

# Health check script (reads port from config file, falls back to 8080)
COPY <<'EOF' /app/healthcheck.sh
#!/bin/sh
PORT=$(grep -E '^[[:space:]]*port[[:space:]]*=' /app/config/dynamight.toml 2>/dev/null | head -1 | tr -dc '0-9')
wget -qO /dev/null "http://127.0.0.1:${PORT:-8080}/api/system/health"
EOF
RUN chmod +x /app/healthcheck.sh

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s \
    CMD /app/healthcheck.sh

# Run as root (required for mount operations)
ENTRYPOINT ["./dynamight"]
