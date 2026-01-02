#!/usr/bin/env bash
#
# Development script for Dynamight
# Starts both backend and frontend with hot-reloading
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

cleanup() {
    log_info "Shutting down..."
    # Kill all background jobs
    jobs -p | xargs -r kill 2>/dev/null || true
    wait 2>/dev/null || true
    log_success "Cleanup complete"
    exit 0
}

trap cleanup SIGINT SIGTERM

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

setup_env() {
    if [[ ! -f "$PROJECT_DIR/.env" ]]; then
        if [[ -f "$PROJECT_DIR/.env.example" ]]; then
            log_warn ".env file not found, copying from .env.example"
            cp "$PROJECT_DIR/.env.example" "$PROJECT_DIR/.env"
        else
            log_warn "Creating default .env file"
            cat > "$PROJECT_DIR/.env" << 'EOF'
JWT_SECRET=dev-secret-change-in-production-$(openssl rand -hex 32)
ADMIN_PASSWORD=admin
DATABASE_URL=sqlite:data/dynamight.db
STATIC_FILES_DIR=frontend/dist
HOST=127.0.0.1
PORT=3000
EOF
        fi
    fi

    # Source .env for this script
    set -a
    source "$PROJECT_DIR/.env"
    set +a
}

install_frontend_deps() {
    if [[ ! -d "$PROJECT_DIR/frontend/node_modules" ]]; then
        log_info "Installing frontend dependencies..."
        (cd "$PROJECT_DIR/frontend" && npm install)
        log_success "Frontend dependencies installed"
    fi
}

run_backend() {
    log_info "Starting backend on http://${HOST:-127.0.0.1}:${PORT:-3000}"
    (
        cd "$PROJECT_DIR/backend"
        cargo run 2>&1 | while IFS= read -r line; do
            echo -e "${BLUE}[backend]${NC} $line"
        done
    ) &
}

run_frontend() {
    log_info "Starting frontend dev server..."
    (
        cd "$PROJECT_DIR/frontend"
        npm run dev 2>&1 | while IFS= read -r line; do
            echo -e "${GREEN}[frontend]${NC} $line"
        done
    ) &
}

main() {
    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║     Dynamight Development Server     ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════╝${NC}"
    echo ""

    cd "$PROJECT_DIR"

    check_dependencies
    setup_env
    install_frontend_deps

    echo ""
    log_info "Starting services..."
    echo ""

    run_backend
    sleep 2  # Give backend time to start
    run_frontend

    echo ""
    log_success "Development servers started!"
    echo ""
    echo "  Backend:  http://${HOST:-127.0.0.1}:${PORT:-3000}"
    echo "  Frontend: http://localhost:5173 (with hot-reload)"
    echo ""
    echo "Press Ctrl+C to stop all services"
    echo ""

    # Wait for any background job to exit
    wait
}

main "$@"
