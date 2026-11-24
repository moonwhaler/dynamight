#!/usr/bin/env bash
#
# Install script for Dynamight
# Installs as a systemd service on Linux
#

set -Eeuo pipefail

# Configuration (update.sh uses the same paths)
INSTALL_DIR="/opt/dynamight"
CONFIG_DIR="/etc/dynamight"
CONFIG_FILE="$CONFIG_DIR/dynamight.toml"
DATA_DIR="/var/lib/dynamight"
UNIT_FILE="/etc/systemd/system/dynamight.service"
# Older versions created this user, but the service runs as root (mounting
# drives needs it). uninstall still offers to remove it.
LEGACY_USER="dynamight"

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

trap 'log_error "Installation failed (line $LINENO: $BASH_COMMAND)"' ERR

# Detect script location (works whether run from package or source)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$(dirname "$SCRIPT_DIR")"
if [[ -f "$SOURCE_DIR/dynamight" ]]; then
    # Running from an extracted package
    NEW_BINARY="$SOURCE_DIR/dynamight"
    NEW_STATIC="$SOURCE_DIR/static"
    NEW_UNIT="$SOURCE_DIR/dynamight.service"
elif [[ -f "$SOURCE_DIR/target/release/dynamight" ]]; then
    # Running from the source tree after a build
    NEW_BINARY="$SOURCE_DIR/target/release/dynamight"
    NEW_STATIC="$SOURCE_DIR/frontend/dist"
    NEW_UNIT="$SCRIPT_DIR/dynamight.service"
else
    NEW_BINARY=""
fi

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

check_requirements() {
    if [[ -z "$NEW_BINARY" ]]; then
        log_error "Cannot find the dynamight binary. Run scripts/build.sh first."
        exit 1
    fi
    if [[ ! -f "$NEW_STATIC/index.html" ]]; then
        log_error "Frontend files not found at $NEW_STATIC. Run scripts/build.sh first."
        exit 1
    fi
    if [[ ! -f "$NEW_UNIT" ]]; then
        log_error "Service file not found at $NEW_UNIT"
        exit 1
    fi
    if ! command -v systemctl &>/dev/null; then
        log_error "systemd is required but not found"
        exit 1
    fi

    # The new binary must run on this system before anything is touched.
    # The in-memory databases keep even a misbehaving binary away from real data.
    if ! NEW_VERSION="$(cd / && DATABASE_URL=sqlite::memory: LOGS_DATABASE_URL=sqlite::memory: \
        timeout 10 "$NEW_BINARY" --version 2>/dev/null)" || [[ "$NEW_VERSION" != "dynamight "* ]]; then
        log_error "$NEW_BINARY does not run on this system (wrong architecture or libraries?)"
        exit 1
    fi
    NEW_VERSION="${NEW_VERSION#dynamight }"

    local missing=()
    command -v rsync &>/dev/null || missing+=("rsync (required for all backups)")
    command -v mount &>/dev/null || missing+=("mount (util-linux, for USB drives)")
    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing required tools:"
        printf '  - %s\n' "${missing[@]}"
        exit 1
    fi

    local optional=()
    command -v mkfs.ext4 &>/dev/null || optional+=("mkfs.ext4 (e2fsprogs)")
    command -v mkfs.exfat &>/dev/null || optional+=("mkfs.exfat (exfatprogs)")
    command -v ntfs-3g &>/dev/null || optional+=("ntfs-3g (NTFS drives)")
    command -v tar &>/dev/null || optional+=("tar (compressed backups)")
    command -v zip &>/dev/null || optional+=("zip (compressed backups)")
    if [[ ${#optional[@]} -gt 0 ]]; then
        log_warn "Optional tools not found; the features that need them will fail:"
        printf '  - %s\n' "${optional[@]}"
    fi
}

check_not_installed() {
    if [[ -f "$INSTALL_DIR/dynamight" ]]; then
        log_error "Dynamight is already installed at $INSTALL_DIR"
        log_info "Use scripts/update.sh to update it (with backup and rollback)"
        exit 1
    fi
}

create_directories() {
    log_info "Creating directories..."
    install -d -m 755 "$INSTALL_DIR"
    # The config holds the JWT secret, the data dir the encrypted credentials
    install -d -m 700 "$CONFIG_DIR" "$DATA_DIR"
    log_success "Directories created"
}

install_files() {
    log_info "Installing files..."

    install -m 755 "$NEW_BINARY" "$INSTALL_DIR/dynamight"
    rm -rf "$INSTALL_DIR/static"
    cp -r "$NEW_STATIC" "$INSTALL_DIR/static"
    rm -rf "$INSTALL_DIR/migrations"
    if [[ -d "$SOURCE_DIR/migrations" ]]; then
        cp -r "$SOURCE_DIR/migrations" "$INSTALL_DIR/"
    fi
    echo "$NEW_VERSION" > "$INSTALL_DIR/VERSION"

    log_success "Files installed ($NEW_VERSION)"
}

detect_timezone() {
    local tz=""
    if command -v timedatectl &>/dev/null; then
        tz="$(timedatectl show --property=Timezone --value 2>/dev/null || true)"
    fi
    if [[ -z "$tz" ]] && [[ -L /etc/localtime ]]; then
        tz="$(readlink /etc/localtime | sed 's|.*/zoneinfo/||')"
    fi
    echo "${tz:-UTC}"
}

setup_config() {
    if [[ -f "$CONFIG_FILE" ]]; then
        log_info "Configuration already exists, preserving..."
        return
    fi

    log_info "Creating configuration..."

    local jwt_secret timezone
    jwt_secret="$(openssl rand -base64 32 2>/dev/null || head -c 32 /dev/urandom | base64 | tr -d '\n')"
    timezone="$(detect_timezone)"

    (
        umask 077
        cat > "$CONFIG_FILE" << EOF
# Dynamight Configuration
# Generated on $(date)
# See ${INSTALL_DIR}/dynamight.toml.example for all available options

[security]
# JWT secret for authentication and credential encryption (auto-generated).
# Keep it secret, and keep it: changing it makes stored credentials unreadable.
jwt_secret = "${jwt_secret}"

# Login cookies are only sent over HTTPS. Set to false if you open the web UI
# over plain HTTP from another machine, or the browser drops the login cookie.
secure_cookies = true

# Paths users are allowed to browse
allowed_browse_paths = ["/mnt", "/home", "/media"]

[server]
host = "0.0.0.0"
port = 8080
static_files_dir = "${INSTALL_DIR}/static"
# Timezone for schedules (detected from this system)
timezone = "${timezone}"

[database]
url = "sqlite:${DATA_DIR}/dynamight.db"

[logging]
level = "info,dynamight=debug"
EOF
    )

    if [[ -f "$SOURCE_DIR/dynamight.toml.example" ]]; then
        install -m 644 "$SOURCE_DIR/dynamight.toml.example" "$INSTALL_DIR/dynamight.toml.example"
    fi
    log_success "Configuration created at $CONFIG_FILE (timezone: $timezone)"
}

install_service() {
    log_info "Installing systemd service..."
    install -m 644 "$NEW_UNIT" "$UNIT_FILE"
    systemctl daemon-reload
    log_success "Service installed"
}

show_status() {
    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║          Installation Complete!                  ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Installation summary:"
    echo "  Version:    $NEW_VERSION"
    echo "  Binary:     $INSTALL_DIR/dynamight"
    echo "  Static:     $INSTALL_DIR/static/"
    echo "  Config:     $CONFIG_FILE"
    echo "  Data:       $DATA_DIR/"
    echo "  Logs:       journalctl -u dynamight"
    echo ""
    echo "Next steps:"
    echo "  1. Review config:       sudo nano $CONFIG_FILE"
    echo "     Without HTTPS (e.g. http://your-server:8080), set secure_cookies = false"
    echo "  2. Enable and start:    sudo systemctl enable --now dynamight"
    echo "  3. Check status:        sudo systemctl status dynamight"
    echo "  4. Open http://your-server:8080 in your browser"
    echo "  5. Complete the initial setup to create your admin account"
    echo ""
}

confirm() {
    local reply
    read -r -p "$1 [y/N] " reply || reply=""
    [[ "$reply" =~ ^[Yy]$ ]]
}

uninstall() {
    log_warn "Uninstalling Dynamight..."

    systemctl stop dynamight 2>/dev/null || true
    systemctl disable dynamight 2>/dev/null || true
    rm -f "$UNIT_FILE"
    systemctl daemon-reload

    if confirm "Remove installation directory ($INSTALL_DIR)?"; then
        rm -rf "$INSTALL_DIR"
        log_success "Installation directory removed"
    fi

    if confirm "Remove data directory ($DATA_DIR)? This deletes all jobs, history and stored credentials."; then
        rm -rf "$DATA_DIR"
        log_success "Data directory removed"
    fi

    if confirm "Remove configuration ($CONFIG_DIR)?"; then
        rm -rf "$CONFIG_DIR"
        log_success "Configuration removed"
    fi

    if [[ -d /opt/dynamight-backups ]] && confirm "Remove update backups (/opt/dynamight-backups)?"; then
        rm -rf /opt/dynamight-backups
        log_success "Update backups removed"
    fi

    if id "$LEGACY_USER" &>/dev/null && confirm "Remove the unused '$LEGACY_USER' user created by older versions?"; then
        userdel "$LEGACY_USER" 2>/dev/null || true
        log_success "User removed"
    fi

    log_success "Uninstallation complete"
}

usage() {
    echo "Usage: $0 [install|uninstall]"
    echo ""
    echo "Commands:"
    echo "  install    Install Dynamight as a system service (default)"
    echo "  uninstall  Remove Dynamight from the system"
    echo ""
    echo "To update an existing installation, use scripts/update.sh."
    echo ""
}

main() {
    local cmd="${1:-install}"

    case "$cmd" in
        install)
            echo ""
            echo -e "${BLUE}╔══════════════════════════════════════════════════╗${NC}"
            echo -e "${BLUE}║       Dynamight Installation Script              ║${NC}"
            echo -e "${BLUE}╚══════════════════════════════════════════════════╝${NC}"
            echo ""

            check_root
            check_not_installed
            check_requirements
            create_directories
            install_files
            setup_config
            install_service
            show_status
            ;;
        uninstall)
            check_root
            uninstall
            ;;
        -h|--help|help)
            usage
            ;;
        *)
            usage
            exit 1
            ;;
    esac
}

main "$@"
