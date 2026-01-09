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

setup_config() {
    local config_file=""
    local example_file=""

    # Look for config file with fallbacks:
    # 1. Project root directory
    # 2. Backend directory
    if [[ -f "$PROJECT_DIR/dynamight.toml" ]]; then
        config_file="$PROJECT_DIR/dynamight.toml"
        log_info "Found config at: $config_file"
    elif [[ -f "$PROJECT_DIR/backend/dynamight.toml" ]]; then
        config_file="$PROJECT_DIR/backend/dynamight.toml"
        log_info "Found config at: $config_file"
    fi

    # Look for example file with same fallbacks
    if [[ -f "$PROJECT_DIR/dynamight.toml.example" ]]; then
        example_file="$PROJECT_DIR/dynamight.toml.example"
    elif [[ -f "$PROJECT_DIR/backend/dynamight.toml.example" ]]; then
        example_file="$PROJECT_DIR/backend/dynamight.toml.example"
    fi

    if [[ -z "$config_file" ]]; then
        if [[ -n "$example_file" ]]; then
            # Create config in project root by default
            config_file="$PROJECT_DIR/dynamight.toml"
            log_warn "dynamight.toml not found, copying from $example_file"
            cp "$example_file" "$config_file"

            # Generate a random JWT secret for development
            local jwt_secret
            jwt_secret="dev-$(openssl rand -hex 32)"

            # Update the jwt_secret in the config file
            if [[ "$(uname)" == "Darwin" ]]; then
                # macOS sed
                sed -i '' "s/jwt_secret = \"CHANGE-ME-generate-with-openssl-rand-base64-32\"/jwt_secret = \"$jwt_secret\"/" "$config_file"
            else
                # GNU sed
                sed -i "s/jwt_secret = \"CHANGE-ME-generate-with-openssl-rand-base64-32\"/jwt_secret = \"$jwt_secret\"/" "$config_file"
            fi

            log_success "Generated development config with random JWT secret"
        else
            log_error "No config file found. Please create dynamight.toml in project root or backend directory"
            exit 1
        fi
    fi

    # Export config path for the backend to use
    export DYNAMIGHT_CONFIG="$config_file"

    # Set development-specific overrides via environment variables
    export HOST="${HOST:-127.0.0.1}"
    export PORT="${PORT:-3000}"
    export STATIC_FILES_DIR="${STATIC_FILES_DIR:-frontend/dist}"
    export SECURE_COOKIES="${SECURE_COOKIES:-false}"
    export RUST_LOG="${RUST_LOG:-info,dynamight=debug}"
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
        exec npm run dev -- --host 2>&1 | sed -u "s/^/$(printf "${GREEN}[frontend]${NC} ")/"
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
    setup_config
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
    echo "            Also available on local network via --host"
    echo ""
    echo "Press Ctrl+C to stop all services"
    echo ""

    # Wait for any background job to exit
    wait
}

main "$@"
