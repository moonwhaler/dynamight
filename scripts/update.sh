#!/usr/bin/env bash
#
# Update script for Dynamight
# Updates an existing installation with automatic rollback on failure
#

set -Eeuo pipefail

# Configuration (must match install.sh)
INSTALL_DIR="/opt/dynamight"
CONFIG_FILE="/etc/dynamight/dynamight.toml"
UNIT_FILE="/etc/systemd/system/dynamight.service"
BACKUP_DIR="/opt/dynamight-backups"
MAX_BACKUPS=3
SERVICE_NAME="dynamight"
HEALTH_CHECK_TIMEOUT=60
LOCK_FILE="/run/dynamight-update.lock"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }

# Detect script location (works whether run from package or source)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$(dirname "$SCRIPT_DIR")"
NEW_BINARY=""
NEW_STATIC=""
NEW_UNIT=""
if [[ -f "$SOURCE_DIR/dynamight" ]]; then
    NEW_BINARY="$SOURCE_DIR/dynamight"
    NEW_STATIC="$SOURCE_DIR/static"
    NEW_UNIT="$SOURCE_DIR/dynamight.service"
elif [[ -f "$SOURCE_DIR/target/release/dynamight" ]]; then
    NEW_BINARY="$SOURCE_DIR/target/release/dynamight"
    NEW_STATIC="$SOURCE_DIR/frontend/dist"
    NEW_UNIT="$SCRIPT_DIR/dynamight.service"
fi

# Global state
BACKUP_TIMESTAMP=""
NEW_VERSION=""
SERVICE_WAS_RUNNING=false
FILES_TOUCHED=false
FORCE=false

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

acquire_lock() {
    if command -v flock &>/dev/null; then
        exec 9>"$LOCK_FILE"
        if ! flock -n 9; then
            log_error "Another update or rollback is running"
            exit 1
        fi
    fi
}

confirm() {
    local reply
    if [[ "$FORCE" == true ]]; then
        return 0
    fi
    if [[ ! -t 0 ]]; then
        log_error "$1 (no terminal to ask; pass --force to continue)"
        exit 1
    fi
    read -r -p "$1 [y/N] " reply || reply=""
    [[ "$reply" =~ ^[Yy]$ ]]
}

# Value of a simple `key = value` line in a [section] of the config file
toml_get() {
    local section="$1" key="$2"
    [[ -r "$CONFIG_FILE" ]] || return 0
    awk -v section="$section" -v key="$key" '
        /^[[:space:]]*\[/ {
            line = $0
            gsub(/[[:space:]]|#.*/, "", line)
            in_section = (line == "[" section "]")
            next
        }
        in_section && $0 ~ "^[[:space:]]*" key "[[:space:]]*=" {
            sub(/^[^=]*=[[:space:]]*/, "")
            sub(/[[:space:]]+#.*$/, "")
            sub(/[[:space:]]+$/, "")
            gsub(/^"|"$/, "")
            print
            exit
        }' "$CONFIG_FILE"
}

# File path of a sqlite: URL, relative paths resolved like the service does
db_path_from_url() {
    local path="${1#sqlite:}"
    path="${path#//}"
    path="${path%%\?*}"
    if [[ "$path" != /* ]]; then
        path="$INSTALL_DIR/$path"
    fi
    echo "$path"
}

# Main and logs database files, one per line
database_files() {
    local url main
    url="$(toml_get database url)"
    main="$(db_path_from_url "${url:-sqlite:data/dynamight.db}")"
    echo "$main"
    url="$(toml_get database logs_url)"
    if [[ -n "$url" ]]; then
        db_path_from_url "$url"
    else
        echo "$(dirname "$main")/logs.db"
    fi
}

health_url() {
    local host port
    host="$(toml_get server host)"
    port="$(toml_get server port)"
    case "${host:-0.0.0.0}" in
        0.0.0.0) host="127.0.0.1" ;;
        ::|"[::]") host="[::1]" ;;
        \[*) ;;
        *:*) host="[$host]" ;;
    esac
    echo "http://${host}:${port:-8080}/api/system/health"
}

have_http_client() {
    command -v curl &>/dev/null || command -v wget &>/dev/null
}

http_get() {
    if command -v curl &>/dev/null; then
        curl -fsS --max-time 3 "$1"
    else
        wget -qO- -T 3 "$1"
    fi
}

# Version line ("1.2.0 (abc1234, 2026-01-31)") of a binary that is not
# installed yet. The in-memory databases keep even an old binary without
# --version support away from the real data.
probe_version() {
    local out
    out="$(cd / && DATABASE_URL=sqlite::memory: LOGS_DATABASE_URL=sqlite::memory: \
        timeout 10 "$1" --version 2>/dev/null)" || return 1
    [[ "$out" == "dynamight "* ]] || return 1
    echo "${out#dynamight }"
}

# Version of the installed program. Never runs the installed binary: versions
# before 1.2 do not know --version and would start a second server.
installed_version() {
    local version=""
    if [[ -f "$INSTALL_DIR/VERSION" ]]; then
        version="$(head -1 "$INSTALL_DIR/VERSION")"
    elif have_http_client && systemctl is-active --quiet "$SERVICE_NAME"; then
        version="$(http_get "$(health_url)" 2>/dev/null | sed -n 's/.*"version":"\([^"]*\)".*/\1/p' || true)"
    fi
    echo "${version:-unknown}"
}

# Copy a directory into place, replacing the old one only once the copy is complete
replace_dir() {
    local src="$1" dest="$2"
    rm -rf "$dest.new"
    cp -r "$src" "$dest.new"
    rm -rf "$dest"
    mv "$dest.new" "$dest"
}

replace_binary() {
    install -m 755 "$1" "$INSTALL_DIR/dynamight.new"
    mv -f "$INSTALL_DIR/dynamight.new" "$INSTALL_DIR/dynamight"
}

# ---------------------------------------------------------------------------
# Steps
# ---------------------------------------------------------------------------

check_installation() {
    if [[ ! -f "$INSTALL_DIR/dynamight" ]]; then
        log_error "Dynamight is not installed at $INSTALL_DIR"
        log_info "Run install.sh first to perform initial installation"
        exit 1
    fi
}

check_new_release() {
    if [[ -z "$NEW_BINARY" ]]; then
        log_error "Cannot find the new dynamight binary. Run scripts/build.sh first."
        exit 1
    fi
    if [[ ! -f "$NEW_STATIC/index.html" ]]; then
        log_error "Frontend files not found at $NEW_STATIC. Run scripts/build.sh first."
        exit 1
    fi
    if ! NEW_VERSION="$(probe_version "$NEW_BINARY")"; then
        log_error "$NEW_BINARY does not run on this system or is too old for this script"
        exit 1
    fi
}

check_running_backups() {
    command -v sqlite3 &>/dev/null || return 0
    local main_db count
    main_db="$(database_files | sed -n 1p)"
    [[ -f "$main_db" ]] || return 0
    count="$(sqlite3 -readonly "$main_db" \
        "SELECT COUNT(*) FROM job_runs WHERE status IN ('running', 'pending');" 2>/dev/null || echo 0)"
    if [[ "$count" =~ ^[0-9]+$ ]] && (( count > 0 )); then
        log_warn "$count backup run(s) are pending or running; the update stops them."
        confirm "Stop them and update anyway?" || { log_info "Update cancelled"; exit 0; }
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

# Runs with the service stopped, so the database copies are consistent
create_backup() {
    local old_version="$1"
    BACKUP_TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
    local backup_path="$BACKUP_DIR/$BACKUP_TIMESTAMP"

    log_info "Creating backup at $backup_path..."
    # The database copies hold password hashes and encrypted credentials
    install -d -m 700 "$BACKUP_DIR"
    install -d -m 700 "$backup_path" "$backup_path/data"

    cp -a "$INSTALL_DIR/dynamight" "$backup_path/"
    if [[ -d "$INSTALL_DIR/static" ]]; then
        cp -a "$INSTALL_DIR/static" "$backup_path/"
    fi
    if [[ -d "$INSTALL_DIR/migrations" ]]; then
        cp -a "$INSTALL_DIR/migrations" "$backup_path/"
    fi
    if [[ -f "$UNIT_FILE" ]]; then
        cp -a "$UNIT_FILE" "$backup_path/"
    fi
    echo "$old_version" > "$backup_path/VERSION"

    local db name ext
    : > "$backup_path/DATABASES"
    while IFS= read -r db; do
        if [[ -f "$db" ]]; then
            name="$(basename "$db")"
            for ext in "" -wal -shm; do
                if [[ -f "$db$ext" ]]; then
                    cp -a "$db$ext" "$backup_path/data/$name$ext"
                fi
            done
            echo "$db" >> "$backup_path/DATABASES"
            log_info "Backed up database $db"
        fi
    done < <(database_files)

    log_success "Backup created"
}

install_new_files() {
    log_info "Installing new files..."
    FILES_TOUCHED=true

    replace_binary "$NEW_BINARY"
    replace_dir "$NEW_STATIC" "$INSTALL_DIR/static"
    if [[ -d "$SOURCE_DIR/migrations" ]]; then
        replace_dir "$SOURCE_DIR/migrations" "$INSTALL_DIR/migrations"
    fi
    if [[ -f "$SOURCE_DIR/dynamight.toml.example" ]]; then
        install -m 644 "$SOURCE_DIR/dynamight.toml.example" "$INSTALL_DIR/dynamight.toml.example"
    fi
    echo "$NEW_VERSION" > "$INSTALL_DIR/VERSION"

    log_success "New files installed"

    if [[ -f "$NEW_UNIT" ]] && [[ -f "$UNIT_FILE" ]] && ! cmp -s "$NEW_UNIT" "$UNIT_FILE"; then
        log_warn "The service file in this release differs from $UNIT_FILE (left unchanged)."
        log_warn "Compare with: diff $UNIT_FILE $NEW_UNIT"
    fi
}

# Restore program files (and with restore_data=true the databases) from a backup
restore_from_backup() {
    local backup_path="$1" restore_data="$2"

    log_warn "Restoring from backup: $(basename "$backup_path")..."
    systemctl stop "$SERVICE_NAME" 2>/dev/null || true

    if [[ -f "$backup_path/dynamight" ]]; then
        replace_binary "$backup_path/dynamight"
    fi
    if [[ -d "$backup_path/static" ]]; then
        replace_dir "$backup_path/static" "$INSTALL_DIR/static"
    fi
    if [[ -d "$backup_path/migrations" ]]; then
        replace_dir "$backup_path/migrations" "$INSTALL_DIR/migrations"
    fi
    if [[ -f "$backup_path/VERSION" ]] && [[ "$(head -1 "$backup_path/VERSION")" != "unknown" ]]; then
        cp "$backup_path/VERSION" "$INSTALL_DIR/VERSION"
    else
        rm -f "$INSTALL_DIR/VERSION"
    fi
    log_success "Program files restored"

    if [[ "$restore_data" == true ]] && [[ -s "$backup_path/DATABASES" ]]; then
        local db name ext
        while IFS= read -r db; do
            name="$(basename "$db")"
            if [[ -f "$backup_path/data/$name" ]]; then
                rm -f "$db-wal" "$db-shm"
                for ext in "" -wal -shm; do
                    if [[ -f "$backup_path/data/$name$ext" ]]; then
                        cp -a "$backup_path/data/$name$ext" "$db$ext"
                    fi
                done
                log_success "Database restored: $db"
            fi
        done < "$backup_path/DATABASES"
    fi
}

# Waits until the service answers its health check, with the expected
# version if one is given (e.g. "1.2.0")
check_service_health() {
    local expected="$1"
    local url body
    url="$(health_url)"

    log_info "Waiting for the service to become healthy (timeout: ${HEALTH_CHECK_TIMEOUT}s)..."
    if ! have_http_client; then
        log_warn "Neither curl nor wget found; only checking that the service stays up"
    fi

    local deadline=$((SECONDS + HEALTH_CHECK_TIMEOUT))
    while (( SECONDS < deadline )); do
        sleep 2
        if systemctl is-active --quiet "$SERVICE_NAME"; then
            if have_http_client; then
                if body="$(http_get "$url" 2>/dev/null)" && [[ "$body" == *'"healthy"'* ]]; then
                    if [[ -z "$expected" ]] || [[ "$body" == *"\"version\":\"$expected\""* ]]; then
                        echo ""
                        log_success "Service is healthy ($url)"
                        return 0
                    fi
                fi
            else
                sleep 3
                if systemctl is-active --quiet "$SERVICE_NAME"; then
                    echo ""
                    log_success "Service is running"
                    return 0
                fi
            fi
        fi
        echo -n "."
    done

    echo ""
    log_error "Service did not become healthy within ${HEALTH_CHECK_TIMEOUT}s ($url)"
    log_info "Last log lines:"
    journalctl -u "$SERVICE_NAME" -n 20 --no-pager 2>/dev/null || true
    return 1
}

cleanup_old_backups() {
    [[ -d "$BACKUP_DIR" ]] || return 0
    local backups=()
    mapfile -t backups < <(find "$BACKUP_DIR" -mindepth 1 -maxdepth 1 -type d -name '[0-9]*-[0-9]*' | sort)
    local excess=$(( ${#backups[@]} - MAX_BACKUPS ))
    if (( excess > 0 )); then
        log_info "Cleaning up old backups (keeping last $MAX_BACKUPS)..."
        rm -rf "${backups[@]:0:excess}"
        log_success "Old backups removed"
    fi
}

latest_backup() {
    [[ -d "$BACKUP_DIR" ]] || return 0
    find "$BACKUP_DIR" -mindepth 1 -maxdepth 1 -type d -name '[0-9]*-[0-9]*' | sort | tail -1
}

# Called on any failed command or interrupt during an update: put the old
# version back, or at least start the service again
on_failure() {
    local code="$1"
    trap - ERR INT TERM
    set +e
    echo ""
    if [[ -n "${2:-}" ]]; then
        log_error "Failed at line $2: $3"
    fi

    if [[ "$FILES_TOUCHED" == true ]] && [[ -n "$BACKUP_TIMESTAMP" ]]; then
        log_warn "Attempting automatic rollback..."
        restore_from_backup "$BACKUP_DIR/$BACKUP_TIMESTAMP" true
        if [[ "$SERVICE_WAS_RUNNING" == true ]]; then
            start_service
            if check_service_health ""; then
                log_success "Automatic rollback succeeded; the previous version is running"
                log_warn "Please check the new release for issues before retrying"
            else
                log_error "Automatic rollback failed! Manual intervention required."
                log_error "Backup: $BACKUP_DIR/$BACKUP_TIMESTAMP"
            fi
        else
            log_success "Previous version restored (service left stopped, as before)"
        fi
    elif [[ "$SERVICE_WAS_RUNNING" == true ]] && ! systemctl is-active --quiet "$SERVICE_NAME"; then
        log_warn "Update aborted before any file was changed; starting the service again"
        systemctl start "$SERVICE_NAME"
    fi
    exit "$code"
}

list_backups() {
    if [[ ! -d "$BACKUP_DIR" ]]; then
        log_info "No backups found"
        return
    fi
    if [[ ! -r "$BACKUP_DIR" ]] || [[ ! -x "$BACKUP_DIR" ]]; then
        log_error "Cannot read $BACKUP_DIR (use sudo)"
        exit 1
    fi

    echo ""
    echo "Available backups (newest last):"
    echo ""
    local backup version databases
    while IFS= read -r backup; do
        version="unknown"
        if [[ -f "$backup/VERSION" ]]; then
            version="$(head -1 "$backup/VERSION")"
        fi
        databases="without databases"
        if [[ -s "$backup/DATABASES" ]]; then
            databases="with databases"
        elif [[ -f "$backup/dynamight.db" ]]; then
            databases="with database (old format)"
        fi
        echo "  $(basename "$backup")  version: $version, $databases, $(du -sh "$backup" | cut -f1)"
    done < <(find "$BACKUP_DIR" -mindepth 1 -maxdepth 1 -type d -name '[0-9]*-[0-9]*' | sort)
    echo ""
}

do_rollback() {
    local name="$1" restore_data="$2"
    local backup_path

    check_root
    acquire_lock
    check_installation

    if [[ -n "$name" ]]; then
        backup_path="$BACKUP_DIR/$(basename "$name")"
    else
        backup_path="$(latest_backup)"
    fi
    if [[ -z "$backup_path" ]] || [[ ! -f "$backup_path/dynamight" ]]; then
        log_error "No backup found to roll back to (see: $0 list)"
        exit 1
    fi

    local version="unknown"
    if [[ -f "$backup_path/VERSION" ]]; then
        version="$(head -1 "$backup_path/VERSION")"
    fi
    log_info "Rolling back to $(basename "$backup_path") (version: $version)"
    if [[ "$restore_data" == true ]]; then
        log_warn "The databases will be reset to the state of that backup; later changes are lost."
        confirm "Continue?" || { log_info "Rollback cancelled"; exit 0; }
    fi

    restore_from_backup "$backup_path" "$restore_data"
    if [[ "$restore_data" != true ]] && [[ -s "$backup_path/DATABASES" ]]; then
        log_info "Databases left as they are. Add --with-data to restore them from the backup too."
    fi
    start_service

    local expected=""
    if [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]; then
        expected="${BASH_REMATCH[0]}"
    fi
    if check_service_health "$expected"; then
        log_success "Rollback complete - service is running"
    else
        log_error "Rollback failed - service is not healthy. Manual intervention required!"
        exit 1
    fi
}

do_update() {
    local old_version

    echo ""
    echo -e "${BLUE}╔══════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║           Dynamight Update Script                ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════════╝${NC}"
    echo ""

    check_root
    acquire_lock
    check_installation
    check_new_release

    old_version="$(installed_version)"
    log_info "Current version: $old_version"
    log_info "New version:     $NEW_VERSION"
    echo ""

    if [[ "$old_version" == "$NEW_VERSION" ]]; then
        log_warn "This version is already installed."
        confirm "Install it again?" || { log_info "Update cancelled"; exit 0; }
    fi
    check_running_backups

    # From here on, any failure or Ctrl+C rolls back
    trap 'on_failure $? $LINENO "$BASH_COMMAND"' ERR
    trap 'on_failure 130' INT TERM

    stop_service
    create_backup "$old_version"
    install_new_files

    if [[ "$SERVICE_WAS_RUNNING" == true ]]; then
        start_service
        if ! check_service_health "${NEW_VERSION%% *}"; then
            log_error "Update failed! The new version is not healthy."
            on_failure 1
        fi
    fi
    trap - ERR INT TERM

    cleanup_old_backups
    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║            Update Successful!                    ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "  Previous version: $old_version"
    echo "  Current version:  $NEW_VERSION"
    echo "  Backup saved:     $BACKUP_DIR/$BACKUP_TIMESTAMP"
    echo ""
    if [[ "$SERVICE_WAS_RUNNING" == true ]]; then
        echo "  View logs: journalctl -u $SERVICE_NAME -f"
    else
        log_warn "Service was not running before update and was not started."
        echo "  Start manually: sudo systemctl start $SERVICE_NAME"
    fi
    echo ""
}

usage() {
    echo "Usage: $0 [command] [options]"
    echo ""
    echo "Commands:"
    echo "  update              Update Dynamight to the version in this package (default)"
    echo "  rollback [BACKUP]   Restore the most recent (or the named) backup"
    echo "  list                List available backups"
    echo ""
    echo "Options:"
    echo "  --force             Do not ask (same version, running backups, --with-data)"
    echo "  --with-data         rollback: also restore the databases from the backup"
    echo ""
    echo "Examples:"
    echo "  sudo $0                       # Update to new version"
    echo "  sudo $0 rollback              # Roll back to the previous version"
    echo "  sudo $0 list                  # Show available backups"
    echo ""
}

main() {
    local cmd="" name="" restore_data=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            update|rollback|list)
                if [[ -n "$cmd" ]]; then
                    log_error "Only one command at a time"; usage; exit 1
                fi
                cmd="$1" ;;
            --force) FORCE=true ;;
            --with-data) restore_data=true ;;
            -h|--help|help) usage; exit 0 ;;
            -*) log_error "Unknown option: $1"; usage; exit 1 ;;
            *)
                if [[ "$cmd" == rollback ]] && [[ -z "$name" ]]; then
                    name="$1"
                else
                    log_error "Unknown command: $1"; usage; exit 1
                fi ;;
        esac
        shift
    done

    case "${cmd:-update}" in
        update) do_update ;;
        rollback) do_rollback "$name" "$restore_data" ;;
        list) list_backups ;;
    esac
}

# Allow sourcing for tests
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
