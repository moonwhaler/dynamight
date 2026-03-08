#!/usr/bin/env bash
#
# Build script for Dynamight
# Creates a production-ready release package
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_DIR/dist"

# Read version from Cargo.toml (single source of truth)
APP_VERSION="$(grep '^version' "$PROJECT_DIR/backend/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
VERSION="${VERSION:-${APP_VERSION}}"
PACKAGE_NAME="dynamight-${VERSION}"

# Docker settings
DOCKER_REGISTRY="${DOCKER_REGISTRY:-ghcr.io}"
DOCKER_IMAGE="${DOCKER_IMAGE:-moonwhaler/dynamight}"
DOCKER_TAG="${DOCKER_TAG:-latest}"
DOCKER_FULL_IMAGE="${DOCKER_REGISTRY}/${DOCKER_IMAGE}:${DOCKER_TAG}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

check_dependencies() {
    local do_native="$1"
    local do_docker="$2"
    local missing=()

    if [[ "$do_native" == true ]]; then
        command -v cargo &>/dev/null || missing+=("cargo (Rust)")
        command -v node &>/dev/null || missing+=("node (Node.js)")
        command -v npm &>/dev/null || missing+=("npm")
    fi

    if [[ "$do_docker" == true ]]; then
        command -v docker &>/dev/null || missing+=("docker")
    fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies:"
        for dep in "${missing[@]}"; do
            echo "  - $dep"
        done
        exit 1
    fi
}

clean() {
    log_info "Cleaning previous build..."
    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR/$PACKAGE_NAME"
}

build_frontend() {
    log_info "Building frontend..."
    (
        cd "$PROJECT_DIR/frontend"

        # Install dependencies if needed
        if [[ ! -d "node_modules" ]]; then
            npm install
        fi

        # Build for production
        npm run build
    )
    log_success "Frontend built"
}

build_backend() {
    log_info "Building backend (release mode)..."
    (
        cd "$PROJECT_DIR/backend"
        cargo build --release
    )
    log_success "Backend built"
}

create_package() {
    local pkg_dir="$BUILD_DIR/$PACKAGE_NAME"

    log_info "Creating package..."

    # Copy binary
    cp "$PROJECT_DIR/target/release/dynamight" "$pkg_dir/"

    # Copy frontend dist
    cp -r "$PROJECT_DIR/frontend/dist" "$pkg_dir/static"

    # Copy migrations
    cp -r "$PROJECT_DIR/migrations" "$pkg_dir/"

    # Create default directories
    mkdir -p "$pkg_dir/data"

    # Copy example config from project root
    cp "$PROJECT_DIR/dynamight.toml.example" "$pkg_dir/dynamight.toml.example"

    # Copy scripts
    mkdir -p "$pkg_dir/scripts"
    cp "$SCRIPT_DIR/install.sh" "$pkg_dir/scripts/" 2>/dev/null || true
    cp "$SCRIPT_DIR/update.sh" "$pkg_dir/scripts/" 2>/dev/null || true

    # Copy systemd service file
    cp "$SCRIPT_DIR/dynamight.service" "$pkg_dir/" 2>/dev/null || true

    # Create run script
    cat > "$pkg_dir/run.sh" << 'EOF'
#!/usr/bin/env bash
#
# Run Dynamight server
#

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Check for config file
if [[ ! -f "dynamight.toml" ]]; then
    echo "ERROR: dynamight.toml not found!"
    echo "Please copy dynamight.toml.example to dynamight.toml and configure it."
    echo ""
    echo "  cp dynamight.toml.example dynamight.toml"
    echo "  # Edit dynamight.toml and set jwt_secret"
    echo ""
    exit 1
fi

# Optional environment variable overrides (these take precedence over config file)
export RUST_LOG="${RUST_LOG:-info,dynamight=debug}"
export TZ="${TZ:-UTC}"

echo "Starting Dynamight..."
echo "Config: dynamight.toml"
exec ./dynamight
EOF
    chmod +x "$pkg_dir/run.sh"

    # Create README
    cat > "$pkg_dir/README.txt" << 'EOF'
Dynamight - Backup Management System
=====================================

Quick Start:
1. Copy dynamight.toml.example to dynamight.toml
2. Edit dynamight.toml and set jwt_secret (generate with: openssl rand -base64 32)
3. Run: ./run.sh
4. Open http://localhost:8080 in your browser
5. Complete the initial setup to create your admin account

System Service Installation:
1. Run: sudo ./scripts/install.sh
2. Edit: /etc/dynamight/dynamight.toml (set jwt_secret)
3. Start: sudo systemctl enable --now dynamight
4. Open http://your-server:8080 in your browser
5. Complete the initial setup to create your admin account

Updating an Existing Installation:
1. Extract the new release
2. Run: sudo ./scripts/update.sh
   - Creates automatic backup before updating
   - Rolls back automatically if service fails to start
3. To rollback manually: sudo ./scripts/update.sh rollback
4. To list backups: ./scripts/update.sh list

First-Time Setup:
  On first launch, Dynamight will prompt you to create an admin
  account through the web interface. Choose a strong password!

Configuration:
  All settings are in dynamight.toml. See the comments in the file
  for detailed documentation of each option.

  Environment variables can override config file settings.
  Useful for Docker or CI/CD deployments.

For more information, visit the project repository.
EOF

    log_success "Package created at $pkg_dir"
}

create_archive() {
    log_info "Creating archive..."
    (
        cd "$BUILD_DIR"
        tar -czf "${PACKAGE_NAME}.tar.gz" "$PACKAGE_NAME"
    )
    log_success "Archive created: $BUILD_DIR/${PACKAGE_NAME}.tar.gz"
}

build_docker() {
    local git_hash
    git_hash=$(git -C "$PROJECT_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown")
    local build_date
    build_date=$(date +%Y-%m-%d)

    log_info "Building Docker image: ${DOCKER_FULL_IMAGE} (${APP_VERSION}+${git_hash})..."
    docker build \
        --build-arg BUILD_GIT_HASH="${git_hash}" \
        --build-arg BUILD_DATE="${build_date}" \
        -t "${DOCKER_FULL_IMAGE}" "$PROJECT_DIR"

    # Always tag with the app version from Cargo.toml
    local versioned="${DOCKER_REGISTRY}/${DOCKER_IMAGE}:${APP_VERSION}"
    docker tag "${DOCKER_FULL_IMAGE}" "${versioned}"
    log_success "Tagged: ${versioned}"

    log_success "Docker image built: ${DOCKER_FULL_IMAGE}"
}

push_docker() {
    log_info "Pushing Docker image: ${DOCKER_FULL_IMAGE}..."

    if ! docker push "${DOCKER_FULL_IMAGE}"; then
        log_error "Push failed. Are you logged in? Run: docker login ${DOCKER_REGISTRY}"
        return 1
    fi

    # Push the versioned tag too
    local versioned="${DOCKER_REGISTRY}/${DOCKER_IMAGE}:${APP_VERSION}"
    docker push "${versioned}"
    log_success "Pushed: ${versioned}"

    log_success "Docker image pushed: ${DOCKER_FULL_IMAGE}"
}

show_summary() {
    local size
    size=$(du -sh "$BUILD_DIR/${PACKAGE_NAME}.tar.gz" | cut -f1)
    local docker_size
    docker_size=$(docker image inspect "${DOCKER_FULL_IMAGE}" --format='{{.Size}}' 2>/dev/null | awk '{printf "%.1fMB", $1/1024/1024}')

    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║         Build Complete!              ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════╝${NC}"
    echo ""
    echo "  Package: $BUILD_DIR/${PACKAGE_NAME}.tar.gz"
    echo "  Size:    $size"
    echo "  Docker:  ${DOCKER_FULL_IMAGE} (${docker_size})"
    echo ""
    echo "To deploy (new installation):"
    echo "  1. Copy ${PACKAGE_NAME}.tar.gz to your server"
    echo "  2. Extract: tar -xzf ${PACKAGE_NAME}.tar.gz"
    echo "  3. cd ${PACKAGE_NAME}"
    echo "  4. Run: sudo ./scripts/install.sh"
    echo "     or manually: ./run.sh"
    echo ""
    echo "To update existing installation:"
    echo "  1. Copy and extract as above"
    echo "  2. Run: sudo ./scripts/update.sh"
    echo ""
    echo "Docker usage:"
    echo "  Run locally:  docker compose up -d"
    echo "  Push to registry: $0 --push"
    echo ""
}

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --push           Build and push Docker image to registry"
    echo "  --docker-only    Only build Docker image (skip native build)"
    echo "  --no-docker      Skip Docker image build"
    echo "  --help           Show this help"
    echo ""
    echo "Version is read from backend/Cargo.toml (use ./scripts/version.sh to bump)."
    echo "Docker images are tagged as both :latest and :<version>."
    echo ""
    echo "Environment variables:"
    echo "  DOCKER_REGISTRY  Registry (default: ghcr.io)"
    echo "  DOCKER_IMAGE     Image name (default: moonwhaler/dynamight)"
    echo "  DOCKER_TAG       Base tag (default: latest)"
}

main() {
    local do_push=false
    local do_docker=true
    local do_native=true

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --push)       do_push=true; shift ;;
            --docker-only) do_native=false; shift ;;
            --no-docker)  do_docker=false; shift ;;
            --help)       usage; exit 0 ;;
            *)            log_error "Unknown option: $1"; usage; exit 1 ;;
        esac
    done

    echo ""
    echo -e "${BLUE}╔══════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║       Dynamight Build Script         ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════╝${NC}"
    echo ""

    cd "$PROJECT_DIR"

    check_dependencies "$do_native" "$do_docker"

    if [[ "$do_native" == true ]]; then
        clean
        build_frontend
        build_backend
        create_package
        create_archive
    fi

    if [[ "$do_docker" == true ]]; then
        build_docker
        if [[ "$do_push" == true ]]; then
            push_docker
        fi
    fi

    if [[ "$do_native" == true ]]; then
        show_summary
    elif [[ "$do_docker" == true ]]; then
        local docker_size
        docker_size=$(docker image inspect "${DOCKER_FULL_IMAGE}" --format='{{.Size}}' 2>/dev/null | awk '{printf "%.1fMB", $1/1024/1024}')
        echo ""
        echo -e "${GREEN}[OK]${NC} Docker image ready: ${DOCKER_FULL_IMAGE} (${docker_size})"
        echo -e "${GREEN}[OK]${NC} Tagged: ${DOCKER_REGISTRY}/${DOCKER_IMAGE}:${APP_VERSION}"
        if [[ "$do_push" == true ]]; then
            echo -e "${GREEN}[OK]${NC} Pushed to registry"
        fi
        echo ""
    fi
}

main "$@"
