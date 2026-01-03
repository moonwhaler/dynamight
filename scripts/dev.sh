#!/usr/bin/env bash
#
# Development script for Dynamight
# Starts both backend and frontend with hot-reloading
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Track child PIDs for cleanup
BACKEND_PID=""
FRONTEND_PID=""
CLEANUP_DONE=false

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

# Recursively kill a process and all its children
kill_tree() {
    local pid=$1
    local signal=${2:-TERM}

    # Get all child PIDs
    local children
    children=$(pgrep -P "$pid" 2>/dev/null || true)

    # Kill children first (depth-first)
    for child in $children; do
        kill_tree "$child" "$signal"
    done

    # Kill the process itself
    if kill -0 "$pid" 2>/dev/null; then
        kill -"$signal" "$pid" 2>/dev/null || true
    fi
}

cleanup() {
    # Prevent running cleanup multiple times
    if [[ "$CLEANUP_DONE" == "true" ]]; then
        return
    fi
    CLEANUP_DONE=true

    echo ""
    log_info "Shutting down..."

    # Kill backend process tree
    if [[ -n "$BACKEND_PID" ]] && kill -0 "$BACKEND_PID" 2>/dev/null; then
        log_info "Stopping backend (PID: $BACKEND_PID)..."
        kill_tree "$BACKEND_PID" TERM
    fi

    # Kill frontend process tree
    if [[ -n "$FRONTEND_PID" ]] && kill -0 "$FRONTEND_PID" 2>/dev/null; then
        log_info "Stopping frontend (PID: $FRONTEND_PID)..."
        kill_tree "$FRONTEND_PID" TERM
    fi

    # Wait briefly for graceful shutdown
    sleep 1

    # Force kill if still running
    if [[ -n "$BACKEND_PID" ]] && kill -0 "$BACKEND_PID" 2>/dev/null; then
        log_warn "Force killing backend..."
        kill_tree "$BACKEND_PID" KILL
    fi

    if [[ -n "$FRONTEND_PID" ]] && kill -0 "$FRONTEND_PID" 2>/dev/null; then
        log_warn "Force killing frontend..."
        kill_tree "$FRONTEND_PID" KILL
    fi

    wait 2>/dev/null || true
    log_success "Cleanup complete"
}

trap cleanup SIGINT SIGTERM
trap 'cleanup; exit 0' EXIT

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
            cat > "$PROJECT_DIR/.env" << EOF
JWT_SECRET=dev-secret-$(openssl rand -hex 32)
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
        exec cargo run 2>&1 | sed -u "s/^/$(printf "${BLUE}[backend]${NC} ")/"
    ) &
    BACKEND_PID=$!
    log_info "Backend started with PID: $BACKEND_PID"
}

run_frontend() {
    log_info "Starting frontend dev server..."
    (
        cd "$PROJECT_DIR/frontend"
        exec npm run dev 2>&1 | sed -u "s/^/$(printf "${GREEN}[frontend]${NC} ")/"
    ) &
    FRONTEND_PID=$!
    log_info "Frontend started with PID: $FRONTEND_PID"
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
