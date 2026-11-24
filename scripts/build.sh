#!/usr/bin/env bash
#
# Build script for Dynamight
# Creates a production-ready release package
#

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_DIR/dist"

# Read version from Cargo.toml (single source of truth): the [package] version
APP_VERSION="$(awk '/^\[/ { in_pkg = ($0 == "[package]") } in_pkg && /^version[[:space:]]*=/ { gsub(/.*=[[:space:]]*"|".*/, ""); print; exit }' "$PROJECT_DIR/backend/Cargo.toml")"
if [[ -z "$APP_VERSION" ]]; then
    echo "[ERROR] Could not read the version from backend/Cargo.toml" >&2
    exit 1
fi
VERSION="${VERSION:-${APP_VERSION}}"
PACKAGE_NAME="dynamight-${VERSION}"

# Docker settings
DOCKER_REGISTRY="${DOCKER_REGISTRY:-ghcr.io}"
DOCKER_IMAGE="${DOCKER_IMAGE:-moonwhaler/dynamight}"
DOCKER_TAG="${DOCKER_TAG:-latest}"
DOCKER_FULL_IMAGE="${DOCKER_REGISTRY}/${DOCKER_IMAGE}:${DOCKER_TAG}"
DOCKER_VERSIONED_IMAGE="${DOCKER_REGISTRY}/${DOCKER_IMAGE}:${APP_VERSION}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }

trap 'log_error "Build failed (line $LINENO: $BASH_COMMAND)"' ERR

# Short commit hash, marked "-dirty" when the working tree has uncommitted changes
GIT_HASH="$(git -C "$PROJECT_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown")"
if [[ "$GIT_HASH" != "unknown" ]] && [[ -n "$(git -C "$PROJECT_DIR" status --porcelain --untracked-files=no 2>/dev/null)" ]]; then
    GIT_HASH="${GIT_HASH}-dirty"
fi
BUILD_DATE="$(date +%Y-%m-%d)"
# build.rs picks these up, so native and Docker builds report the same build
export BUILD_GIT_HASH="$GIT_HASH"
export BUILD_DATE

check_dependencies() {
    local do_native="$1"
    local do_docker="$2"
    local missing=()

    if [[ "$do_native" == true ]]; then
        command -v cargo &>/dev/null || missing+=("cargo (Rust)")
        command -v node &>/dev/null || missing+=("node (Node.js)")
        command -v npm &>/dev/null || missing+=("npm")
    fi

    if [[ "$do_docker" == true ]] && ! select_engine; then
        missing+=("docker or podman")
    fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies:"
        for dep in "${missing[@]}"; do
            echo "  - $dep"
        done
        exit 1
    fi

    if [[ "$do_docker" == true ]]; then
        if ! "$ENGINE" info &>/dev/null; then
            log_error "'$ENGINE info' fails. Is the Docker daemon running, and may this user use it?"
            exit 1
        fi
        log_info "Container engine: $ENGINE"
    fi
}

# Container engine: $CONTAINER_ENGINE if set, else docker when its daemon
# answers, else podman
ENGINE=""
ENGINE_BUILD_ARGS=()
select_engine() {
    if [[ -n "${CONTAINER_ENGINE:-}" ]]; then
        ENGINE="$CONTAINER_ENGINE"
    elif command -v docker &>/dev/null && docker info &>/dev/null; then
        ENGINE=docker
    elif command -v podman &>/dev/null; then
        ENGINE=podman
    elif command -v docker &>/dev/null; then
        ENGINE=docker
    else
        return 1
    fi
    command -v "$ENGINE" &>/dev/null || return 1
    # Podman builds OCI images by default, and the OCI format has no
    # HEALTHCHECK; the docker format keeps it
    if [[ "$("$ENGINE" --version 2>/dev/null)" == podman* ]]; then
        ENGINE_BUILD_ARGS=(--format docker)
    fi
}

clean() {
    log_info "Cleaning previous build..."
    rm -rf "${BUILD_DIR:?}/$PACKAGE_NAME" "$BUILD_DIR/${PACKAGE_NAME}.tar.gz"
    mkdir -p "$BUILD_DIR/$PACKAGE_NAME"
}

build_frontend() {
    log_info "Building frontend..."
    (
        cd "$PROJECT_DIR/frontend"
        # Clean install from package-lock.json, so the build never uses stale modules
        npm ci --no-audit --no-fund
        npm run build
    )
    log_success "Frontend built"
}

build_backend() {
    log_info "Building backend (release mode)..."
    (
        cd "$PROJECT_DIR"
        cargo build --release --locked --package dynamight
    )
    log_success "Backend built"
}

create_package() {
    local pkg_dir="$BUILD_DIR/$PACKAGE_NAME"

    log_info "Creating package..."

    # Binary, and its version line for install.sh/update.sh (they never run an
    # installed binary to ask for its version)
    cp "$PROJECT_DIR/target/release/dynamight" "$pkg_dir/"
    "$pkg_dir/dynamight" --version | sed 's/^dynamight //' > "$pkg_dir/VERSION"

    cp -r "$PROJECT_DIR/frontend/dist" "$pkg_dir/static"
    cp -r "$PROJECT_DIR/migrations" "$pkg_dir/"
    cp "$PROJECT_DIR/dynamight.toml.example" "$pkg_dir/dynamight.toml.example"
    cp "$PROJECT_DIR/LICENSE" "$pkg_dir/"

    mkdir -p "$pkg_dir/scripts"
    cp "$SCRIPT_DIR/install.sh" "$SCRIPT_DIR/update.sh" "$pkg_dir/scripts/"
    cp "$SCRIPT_DIR/dynamight.service" "$pkg_dir/"

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
export DYNAMIGHT_CONFIG="$SCRIPT_DIR/dynamight.toml"

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
   Without HTTPS, also set secure_cookies = false, or the browser drops the
   login cookie.
3. Run: ./run.sh
4. Open http://localhost:8080 in your browser
5. Complete the initial setup to create your admin account

System Service Installation:
1. Run: sudo ./scripts/install.sh
2. Review: /etc/dynamight/dynamight.toml (jwt_secret is generated for you)
3. Start: sudo systemctl enable --now dynamight
4. Open http://your-server:8080 in your browser
5. Complete the initial setup to create your admin account

Updating an Existing Installation:
1. Extract the new release
2. Run: sudo ./scripts/update.sh
   - Stops the service and backs up program files and databases
   - Rolls back automatically if the new version does not come up healthy
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

For more information, visit https://github.com/moonwhaler/dynamight
EOF

    log_success "Package created at $pkg_dir"
}

create_archive() {
    log_info "Creating archive..."
    (
        cd "$BUILD_DIR"
        tar -czf "${PACKAGE_NAME}.tar.gz" "$PACKAGE_NAME"
        sha256sum "${PACKAGE_NAME}.tar.gz" > "${PACKAGE_NAME}.tar.gz.sha256"
    )
    log_success "Archive created: $BUILD_DIR/${PACKAGE_NAME}.tar.gz (+ .sha256)"
}

build_docker() {
    log_info "Building Docker image: ${DOCKER_FULL_IMAGE} (${APP_VERSION}+${GIT_HASH})..."
    "$ENGINE" build "${ENGINE_BUILD_ARGS[@]}" \
        --build-arg BUILD_GIT_HASH="${GIT_HASH}" \
        --build-arg BUILD_DATE="${BUILD_DATE}" \
        --build-arg APP_VERSION="${APP_VERSION}" \
        -t "${DOCKER_FULL_IMAGE}" "$PROJECT_DIR"

    # Always tag with the app version from Cargo.toml
    if [[ "$DOCKER_VERSIONED_IMAGE" != "$DOCKER_FULL_IMAGE" ]]; then
        "$ENGINE" tag "${DOCKER_FULL_IMAGE}" "${DOCKER_VERSIONED_IMAGE}"
        log_success "Tagged: ${DOCKER_VERSIONED_IMAGE}"
    fi

    # The image must start and report the expected version
    local reported
    reported="$("$ENGINE" run --rm "${DOCKER_FULL_IMAGE}" --version)"
    if [[ "$reported" != "dynamight ${APP_VERSION} "* ]]; then
        log_error "Image reports '${reported}', expected version ${APP_VERSION}"
        exit 1
    fi
    log_success "Docker image built: ${DOCKER_FULL_IMAGE} (${reported})"
}

push_docker() {
    if [[ "$GIT_HASH" == *-dirty ]] && [[ "${ALLOW_DIRTY:-}" != 1 ]]; then
        log_error "Refusing to push an image built from uncommitted changes."
        log_error "Commit first, or set ALLOW_DIRTY=1 to push anyway."
        exit 1
    fi

    log_info "Pushing Docker image: ${DOCKER_FULL_IMAGE}..."
    if ! "$ENGINE" push "${DOCKER_FULL_IMAGE}"; then
        log_error "Push failed. Are you logged in? Run: $ENGINE login ${DOCKER_REGISTRY}"
        exit 1
    fi
    log_success "Pushed: ${DOCKER_FULL_IMAGE}"

    if [[ "$DOCKER_VERSIONED_IMAGE" != "$DOCKER_FULL_IMAGE" ]]; then
        "$ENGINE" push "${DOCKER_VERSIONED_IMAGE}"
        log_success "Pushed: ${DOCKER_VERSIONED_IMAGE}"
    fi
}

docker_size() {
    "$ENGINE" image inspect "${DOCKER_FULL_IMAGE}" --format='{{.Size}}' 2>/dev/null | awk '{printf "%.1fMB", $1/1024/1024}'
}

show_summary() {
    local do_native="$1"
    local do_docker="$2"
    local do_push="$3"

    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║         Build Complete!              ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════╝${NC}"
    echo ""
    echo "  Version: ${APP_VERSION} (${GIT_HASH}, ${BUILD_DATE})"
    if [[ "$do_native" == true ]]; then
        echo "  Package: $BUILD_DIR/${PACKAGE_NAME}.tar.gz ($(du -sh "$BUILD_DIR/${PACKAGE_NAME}.tar.gz" | cut -f1))"
    fi
    if [[ "$do_docker" == true ]]; then
        echo "  Image:   ${DOCKER_FULL_IMAGE}, ${DOCKER_VERSIONED_IMAGE} ($(docker_size))"
        [[ "$do_push" == true ]] && echo "           pushed to ${DOCKER_REGISTRY}"
    fi
    echo ""
    if [[ "$do_native" == true ]]; then
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
    fi
    if [[ "$do_docker" == true ]]; then
        echo "Docker usage:"
        echo "  Run locally:  $ENGINE compose up -d"
        [[ "$do_push" == true ]] || echo "  Push to registry: $0 --docker-only --push"
        echo ""
    fi
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
    echo "  CONTAINER_ENGINE docker or podman (default: docker if its daemon runs, else podman)"
    echo "  ALLOW_DIRTY=1    Allow --push with uncommitted changes"
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
            --help|-h)    usage; exit 0 ;;
            *)            log_error "Unknown option: $1"; usage; exit 1 ;;
        esac
    done

    if [[ "$do_docker" == false ]] && [[ "$do_push" == true ]]; then
        log_error "--push needs the Docker build; drop --no-docker"
        exit 1
    fi
    if [[ "$do_docker" == false ]] && [[ "$do_native" == false ]]; then
        log_error "--docker-only and --no-docker together leave nothing to build"
        exit 1
    fi

    echo ""
    echo -e "${BLUE}╔══════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║       Dynamight Build Script         ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════╝${NC}"
    echo ""

    cd "$PROJECT_DIR"

    check_dependencies "$do_native" "$do_docker"
    if [[ "$GIT_HASH" == *-dirty ]]; then
        log_warn "Working tree has uncommitted changes; the build is marked ${GIT_HASH}"
    fi

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

    show_summary "$do_native" "$do_docker" "$do_push"
}

main "$@"
