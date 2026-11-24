# Stage 1: Build Frontend
FROM node:22-alpine AS frontend-builder

WORKDIR /app/frontend

# Install dependencies exactly as locked
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --no-audit --no-fund

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

# Build the dependencies with a dummy main, so this layer is cached until
# Cargo.toml/Cargo.lock change. Errors here are not fatal: the real build
# below reports them.
RUN mkdir -p backend/src && echo "fn main() {}" > backend/src/main.rs
RUN cargo build --release --locked --package dynamight || true

# Build actual application
COPY backend/src ./backend/src
ARG BUILD_GIT_HASH=unknown
ARG BUILD_DATE=unknown
ENV BUILD_GIT_HASH=${BUILD_GIT_HASH}
ENV BUILD_DATE=${BUILD_DATE}
RUN touch backend/src/main.rs backend/src/lib.rs \
    && cargo build --release --locked --package dynamight \
    && ./target/release/dynamight --version

# Stage 3: Runtime
FROM alpine:3.22

# Runtime tools the server calls. coreutils: GNU df/du options (busybox lacks
# them); e2fsprogs-extra: dumpe2fs; tini: reaps orphaned child processes
RUN apk add --no-cache \
    rsync \
    util-linux \
    coreutils \
    e2fsprogs \
    e2fsprogs-extra \
    ntfs-3g \
    ntfs-3g-progs \
    exfatprogs \
    ca-certificates \
    tzdata \
    tar \
    zip \
    openssl \
    tini

WORKDIR /app

# Copy built artifacts
COPY --from=backend-builder /app/target/release/dynamight ./
COPY --from=frontend-builder /app/frontend/dist ./static
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

# Health check: port from $PORT, else [server] port in the config file, else 8080
COPY <<'EOF' /app/healthcheck.sh
#!/bin/sh
PORT="${PORT:-$(awk '
    /^[[:space:]]*\[/ { s = $0; gsub(/[[:space:]]|#.*/, "", s); in_server = (s == "[server]"); next }
    in_server && /^[[:space:]]*port[[:space:]]*=/ { sub(/^[^=]*=/, ""); gsub(/[^0-9]/, ""); print; exit }
' /app/config/dynamight.toml 2>/dev/null)}"
wget -qO /dev/null -T 2 "http://127.0.0.1:${PORT:-8080}/api/system/health"
EOF
RUN chmod +x /app/healthcheck.sh

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s \
    CMD /app/healthcheck.sh

ARG APP_VERSION=unknown
ARG BUILD_GIT_HASH=unknown
LABEL org.opencontainers.image.title="Dynamight" \
      org.opencontainers.image.description="Self-hosted backup management with a web UI" \
      org.opencontainers.image.source="https://github.com/moonwhaler/dynamight" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${APP_VERSION}" \
      org.opencontainers.image.revision="${BUILD_GIT_HASH}"

# Run as root (required for mount operations)
ENTRYPOINT ["/sbin/tini", "--", "./dynamight"]
