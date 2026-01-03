#!/usr/bin/env bash
#
# Build script for Dynamight
# Creates a production-ready release package
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_DIR/dist"
VERSION="${VERSION:-$(date +%Y%m%d-%H%M%S)}"
PACKAGE_NAME="dynamight-${VERSION}"

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
    local missing=()

    command -v cargo &>/dev/null || missing+=("cargo (Rust)")
    command -v node &>/dev/null || missing+=("node (Node.js)")
    command -v npm &>/dev/null || missing+=("npm")

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

    # Copy example .env from project root
    cp "$PROJECT_DIR/.env.example" "$pkg_dir/.env.example"

    # Copy scripts
    mkdir -p "$pkg_dir/scripts"
    cp "$SCRIPT_DIR/install.sh" "$pkg_dir/scripts/" 2>/dev/null || true

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

# Load environment
if [[ -f .env ]]; then
    set -a
    source .env
    set +a
fi

# Set defaults
export DATABASE_URL="${DATABASE_URL:-sqlite:data/dynamight.db}"
export STATIC_FILES_DIR="${STATIC_FILES_DIR:-static}"
export HOST="${HOST:-0.0.0.0}"
export PORT="${PORT:-8080}"
export RUST_LOG="${RUST_LOG:-info,dynamight=debug}"
export TZ="${TZ:-UTC}"
# MAX_RUNS_PER_JOB is optional, no default needed

echo "Starting Dynamight on http://${HOST}:${PORT}"
exec ./dynamight
EOF
    chmod +x "$pkg_dir/run.sh"

    # Create README
    cat > "$pkg_dir/README.txt" << 'EOF'
Dynamight - Backup Management System
=====================================

Quick Start:
1. Copy .env.example to .env
2. Edit .env and set secure values for JWT_SECRET and ADMIN_PASSWORD
3. Run: ./run.sh

System Service Installation:
1. Run: sudo ./scripts/install.sh
2. Edit: /etc/dynamight/.env
3. Start: sudo systemctl start dynamight

Default login:
  Username: admin
  Password: (value of ADMIN_PASSWORD in .env)

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

show_summary() {
    local size
    size=$(du -sh "$BUILD_DIR/${PACKAGE_NAME}.tar.gz" | cut -f1)

    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║         Build Complete!              ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════╝${NC}"
    echo ""
    echo "  Package: $BUILD_DIR/${PACKAGE_NAME}.tar.gz"
    echo "  Size:    $size"
    echo ""
    echo "To deploy:"
    echo "  1. Copy ${PACKAGE_NAME}.tar.gz to your server"
    echo "  2. Extract: tar -xzf ${PACKAGE_NAME}.tar.gz"
    echo "  3. cd ${PACKAGE_NAME}"
    echo "  4. Run: sudo ./scripts/install.sh"
    echo "     or manually: ./run.sh"
    echo ""
}

main() {
    echo ""
    echo -e "${BLUE}╔══════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║       Dynamight Build Script         ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════╝${NC}"
    echo ""

    cd "$PROJECT_DIR"

    check_dependencies
    clean
    build_frontend
    build_backend
    create_package
    create_archive
    show_summary
}

main "$@"
