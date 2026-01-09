#!/usr/bin/env bash
#
# Update script for Dynamight
# Updates an existing installation with automatic rollback on failure
#

set -e

# Configuration (must match install.sh)
INSTALL_DIR="/opt/dynamight"
CONFIG_DIR="/etc/dynamight"
DATA_DIR="/var/lib/dynamight"
BACKUP_DIR="/opt/dynamight-backups"
MAX_BACKUPS=3
SERVICE_NAME="dynamight"
HEALTH_CHECK_TIMEOUT=30

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Detect script location
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -f "$SCRIPT_DIR/../dynamight" ]]; then
    SOURCE_DIR="$(dirname "$SCRIPT_DIR")"
elif [[ -f "$SCRIPT_DIR/../target/release/dynamight" ]]; then
    SOURCE_DIR="$(dirname "$SCRIPT_DIR")"
else
    log_error "Cannot find dynamight binary. Run build.sh first."
    exit 1
fi

# Global state
BACKUP_TIMESTAMP=""
SERVICE_WAS_RUNNING=false

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

check_installation() {
    if [[ ! -d "$INSTALL_DIR" ]] || [[ ! -f "$INSTALL_DIR/dynamight" ]]; then
        log_error "Dynamight is not installed at $INSTALL_DIR"
        log_info "Run install.sh first to perform initial installation"
        exit 1
    fi
}

get_version() {
    local binary="$1"
    if [[ -f "$binary" ]] && [[ -x "$binary" ]]; then
        "$binary" --version 2>/dev/null | head -1 || echo "unknown"
    else
        echo "unknown"
    fi
}

create_backup() {
    BACKUP_TIMESTAMP=$(date +%Y%m%d-%H%M%S)
    local backup_path="$BACKUP_DIR/$BACKUP_TIMESTAMP"

    log_info "Creating backup at $backup_path..."
    mkdir -p "$backup_path"

    # Backup binary
    if [[ -f "$INSTALL_DIR/dynamight" ]]; then
        cp "$INSTALL_DIR/dynamight" "$backup_path/"
    fi

    # Backup static files
    if [[ -d "$INSTALL_DIR/static" ]]; then
        cp -r "$INSTALL_DIR/static" "$backup_path/"
    fi

    # Backup migrations
    if [[ -d "$INSTALL_DIR/migrations" ]]; then
        cp -r "$INSTALL_DIR/migrations" "$backup_path/"
    fi

    # Backup database files
    if [[ -f "$DATA_DIR/dynamight.db" ]]; then
        log_info "Backing up database..."
        cp "$DATA_DIR/dynamight.db" "$backup_path/"
        # Also backup SQLite WAL files if they exist
        [[ -f "$DATA_DIR/dynamight.db-wal" ]] && cp "$DATA_DIR/dynamight.db-wal" "$backup_path/"
        [[ -f "$DATA_DIR/dynamight.db-shm" ]] && cp "$DATA_DIR/dynamight.db-shm" "$backup_path/"
        log_success "Database backed up"
    fi

    # Store version info
    get_version "$INSTALL_DIR/dynamight" > "$backup_path/VERSION"

    log_success "Backup created"
}

cleanup_old_backups() {
    if [[ ! -d "$BACKUP_DIR" ]]; then
        return
    fi

    local backup_count
    backup_count=$(find "$BACKUP_DIR" -maxdepth 1 -type d ! -path "$BACKUP_DIR" | wc -l)

    if [[ $backup_count -gt $MAX_BACKUPS ]]; then
        log_info "Cleaning up old backups (keeping last $MAX_BACKUPS)..."
        find "$BACKUP_DIR" -maxdepth 1 -type d ! -path "$BACKUP_DIR" | \
            sort | head -n -$MAX_BACKUPS | xargs rm -rf
        log_success "Old backups removed"
    fi
}

stop_service() {
    if systemctl is-active --quiet "$SERVICE_NAME"; then
        SERVICE_WAS_RUNNING=true
        log_info "Stopping $SERVICE_NAME service..."
        systemctl stop "$SERVICE_NAME"
        log_success "Service stopped"
    else
        log_info "Service was not running"
    fi
}

start_service() {
    log_info "Starting $SERVICE_NAME service..."
    systemctl start "$SERVICE_NAME"
}

check_service_health() {
    log_info "Waiting for service to become healthy (timeout: ${HEALTH_CHECK_TIMEOUT}s)..."

    local elapsed=0
    local interval=2

    while [[ $elapsed -lt $HEALTH_CHECK_TIMEOUT ]]; do
        sleep $interval
        elapsed=$((elapsed + interval))

        if systemctl is-active --quiet "$SERVICE_NAME"; then
            # Additional check: see if the process is still running after a moment
            sleep 2
            if systemctl is-active --quiet "$SERVICE_NAME"; then
                log_success "Service is running"
                return 0
            fi
        fi

        echo -n "."
    done

    echo ""
    log_error "Service failed to start within ${HEALTH_CHECK_TIMEOUT}s"
    return 1
}

install_new_files() {
    log_info "Installing new files..."

    # Install binary
    if [[ -f "$SOURCE_DIR/dynamight" ]]; then
        cp "$SOURCE_DIR/dynamight" "$INSTALL_DIR/"
    elif [[ -f "$SOURCE_DIR/target/release/dynamight" ]]; then
        cp "$SOURCE_DIR/target/release/dynamight" "$INSTALL_DIR/"
    fi
    chmod +x "$INSTALL_DIR/dynamight"

    # Install static files
    if [[ -d "$SOURCE_DIR/static" ]]; then
        rm -rf "$INSTALL_DIR/static"
        cp -r "$SOURCE_DIR/static" "$INSTALL_DIR/"
    elif [[ -d "$SOURCE_DIR/frontend/dist" ]]; then
        rm -rf "$INSTALL_DIR/static"
        cp -r "$SOURCE_DIR/frontend/dist" "$INSTALL_DIR/static"
    fi

    # Install migrations
    if [[ -d "$SOURCE_DIR/migrations" ]]; then
        rm -rf "$INSTALL_DIR/migrations"
        cp -r "$SOURCE_DIR/migrations" "$INSTALL_DIR/"
    fi

    log_success "New files installed"
}

rollback() {
    local backup_path="$1"

    if [[ -z "$backup_path" ]]; then
        # Find most recent backup
        backup_path=$(find "$BACKUP_DIR" -maxdepth 1 -type d ! -path "$BACKUP_DIR" | sort | tail -1)
    fi

    if [[ -z "$backup_path" ]] || [[ ! -d "$backup_path" ]]; then
        log_error "No backup found to rollback to"
        return 1
    fi

    log_warn "Rolling back to backup: $(basename "$backup_path")..."

    # Stop service if running
    systemctl stop "$SERVICE_NAME" 2>/dev/null || true

    # Restore binary
    if [[ -f "$backup_path/dynamight" ]]; then
        cp "$backup_path/dynamight" "$INSTALL_DIR/"
        chmod +x "$INSTALL_DIR/dynamight"
    fi

    # Restore static files
    if [[ -d "$backup_path/static" ]]; then
        rm -rf "$INSTALL_DIR/static"
        cp -r "$backup_path/static" "$INSTALL_DIR/"
    fi

    # Restore migrations
    if [[ -d "$backup_path/migrations" ]]; then
        rm -rf "$INSTALL_DIR/migrations"
        cp -r "$backup_path/migrations" "$INSTALL_DIR/"
    fi

    log_success "Files restored from backup"

    # Start service
    start_service

    if check_service_health; then
        log_success "Rollback complete - service is running"
        return 0
    else
        log_error "Rollback failed - service still not healthy"
        log_error "Manual intervention required!"
        return 1
    fi
}

list_backups() {
    if [[ ! -d "$BACKUP_DIR" ]]; then
        log_info "No backups found"
        return
    fi

    echo ""
    echo "Available backups:"
    echo ""

    for backup in $(find "$BACKUP_DIR" -maxdepth 1 -type d ! -path "$BACKUP_DIR" | sort); do
        local name=$(basename "$backup")
        local version="unknown"
        if [[ -f "$backup/VERSION" ]]; then
            version=$(cat "$backup/VERSION")
        fi
        echo "  $name (version: $version)"
    done
    echo ""
}

do_update() {
    local old_version new_version

    echo ""
    echo -e "${BLUE}╔══════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║           Dynamight Update Script                ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════════╝${NC}"
    echo ""

    check_root
    check_installation

    # Show version info
    old_version=$(get_version "$INSTALL_DIR/dynamight")
    if [[ -f "$SOURCE_DIR/dynamight" ]]; then
        new_version=$(get_version "$SOURCE_DIR/dynamight")
    elif [[ -f "$SOURCE_DIR/target/release/dynamight" ]]; then
        new_version=$(get_version "$SOURCE_DIR/target/release/dynamight")
    else
        new_version="unknown"
    fi

    log_info "Current version: $old_version"
    log_info "New version:     $new_version"
    echo ""

    if [[ "$old_version" == "$new_version" ]] && [[ "$old_version" != "unknown" ]]; then
        log_warn "Same version detected. Continue anyway? [y/N]"
        read -r response
        if [[ ! "$response" =~ ^[Yy]$ ]]; then
            log_info "Update cancelled"
            exit 0
        fi
    fi

    # Perform update
    create_backup
    stop_service
    install_new_files

    # Only restart and health check if service was running before
    if [[ "$SERVICE_WAS_RUNNING" == true ]]; then
        start_service

        if check_service_health; then
            cleanup_old_backups
            echo ""
            echo -e "${GREEN}╔══════════════════════════════════════════════════╗${NC}"
            echo -e "${GREEN}║            Update Successful!                    ║${NC}"
            echo -e "${GREEN}╚══════════════════════════════════════════════════╝${NC}"
            echo ""
            echo "  Previous version: $old_version"
            echo "  Current version:  $new_version"
            echo "  Backup saved:     $BACKUP_DIR/$BACKUP_TIMESTAMP"
            echo ""
            echo "  View logs: journalctl -u $SERVICE_NAME -f"
            echo ""
        else
            log_error "Update failed! Service is not healthy."
            log_warn "Attempting automatic rollback..."

            if rollback "$BACKUP_DIR/$BACKUP_TIMESTAMP"; then
                log_success "Automatic rollback succeeded"
                log_warn "Please check the new binary for issues before retrying"
            else
                log_error "Automatic rollback failed!"
                log_error "Manual intervention required"
            fi
            exit 1
        fi
    else
        # Service was not running, just complete the update without starting
        cleanup_old_backups
        echo ""
        echo -e "${GREEN}╔══════════════════════════════════════════════════╗${NC}"
        echo -e "${GREEN}║            Update Successful!                    ║${NC}"
        echo -e "${GREEN}╚══════════════════════════════════════════════════╝${NC}"
        echo ""
        echo "  Previous version: $old_version"
        echo "  Current version:  $new_version"
        echo "  Backup saved:     $BACKUP_DIR/$BACKUP_TIMESTAMP"
        echo ""
        log_warn "Service was not running before update and was not started."
        echo "  Start manually: sudo systemctl start $SERVICE_NAME"
        echo ""
    fi
}

usage() {
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  update     Update Dynamight to new version (default)"
    echo "  rollback   Rollback to the most recent backup"
    echo "  list       List available backups"
    echo ""
    echo "Options:"
    echo "  --force    Skip version check confirmation"
    echo ""
    echo "Examples:"
    echo "  $0                    # Update to new version"
    echo "  $0 rollback           # Rollback to previous version"
    echo "  $0 list               # Show available backups"
    echo ""
}

main() {
    local cmd="${1:-update}"

    case "$cmd" in
        update|"")
            do_update
            ;;
        rollback)
            check_root
            rollback ""
            ;;
        list)
            list_backups
            ;;
        -h|--help|help)
            usage
            ;;
        *)
            log_error "Unknown command: $cmd"
            usage
            exit 1
            ;;
    esac
}

main "$@"
