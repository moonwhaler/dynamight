#!/usr/bin/env bash
#
# Install script for Dynamight
# Installs as a systemd service on Linux
#

set -e

# Configuration
INSTALL_DIR="/opt/dynamight"
CONFIG_DIR="/etc/dynamight"
DATA_DIR="/var/lib/dynamight"
LOG_DIR="/var/log/dynamight"
SERVICE_USER="dynamight"
SERVICE_GROUP="dynamight"

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

# Detect script location (works whether run from package or source)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -f "$SCRIPT_DIR/../dynamight" ]]; then
    # Running from package
    SOURCE_DIR="$(dirname "$SCRIPT_DIR")"
elif [[ -f "$SCRIPT_DIR/../target/release/dynamight" ]]; then
    # Running from source after build
    SOURCE_DIR="$(dirname "$SCRIPT_DIR")"
else
    log_error "Cannot find dynamight binary. Run build.sh first."
    exit 1
fi

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

check_systemd() {
    if ! command -v systemctl &>/dev/null; then
        log_error "systemd is required but not found"
        exit 1
    fi
}

create_user() {
    if ! id "$SERVICE_USER" &>/dev/null; then
        log_info "Creating service user '$SERVICE_USER'..."
        useradd --system --no-create-home --shell /usr/sbin/nologin "$SERVICE_USER"
        log_success "User created"
    else
        log_info "Service user '$SERVICE_USER' already exists"
    fi
}

create_directories() {
    log_info "Creating directories..."

    mkdir -p "$INSTALL_DIR"
    mkdir -p "$CONFIG_DIR"
    mkdir -p "$DATA_DIR"
    mkdir -p "$LOG_DIR"

    log_success "Directories created"
}

install_files() {
    log_info "Installing files..."

    # Install binary
    if [[ -f "$SOURCE_DIR/dynamight" ]]; then
        cp "$SOURCE_DIR/dynamight" "$INSTALL_DIR/"
    elif [[ -f "$SOURCE_DIR/target/release/dynamight" ]]; then
        cp "$SOURCE_DIR/target/release/dynamight" "$INSTALL_DIR/"
    fi
    chmod +x "$INSTALL_DIR/dynamight"

    # Install static files
    if [[ -d "$SOURCE_DIR/static" ]]; then
        cp -r "$SOURCE_DIR/static" "$INSTALL_DIR/"
    elif [[ -d "$SOURCE_DIR/frontend/dist" ]]; then
        cp -r "$SOURCE_DIR/frontend/dist" "$INSTALL_DIR/static"
    fi

    # Install migrations
    if [[ -d "$SOURCE_DIR/migrations" ]]; then
        cp -r "$SOURCE_DIR/migrations" "$INSTALL_DIR/"
    fi

    log_success "Files installed"
}

setup_config() {
    if [[ ! -f "$CONFIG_DIR/dynamight.toml" ]]; then
        log_info "Creating configuration..."

        # Generate secure JWT secret
        JWT_SECRET=$(openssl rand -base64 32 2>/dev/null || head -c 48 /dev/urandom | base64 | tr -d '\n' | head -c 44)

        cat > "$CONFIG_DIR/dynamight.toml" << EOF
# Dynamight Configuration
# Generated on $(date)
# See dynamight.toml.example for all available options

[security]
# JWT secret for authentication (auto-generated, keep secure!)
jwt_secret = "${JWT_SECRET}"

# Paths users are allowed to browse
allowed_browse_paths = ["/mnt", "/home", "/media"]

[server]
host = "0.0.0.0"
port = 8080
static_files_dir = "${INSTALL_DIR}/static"

[database]
url = "sqlite:${DATA_DIR}/dynamight.db"

[logging]
level = "info,dynamight=debug"
EOF

        chmod 600 "$CONFIG_DIR/dynamight.toml"
        log_success "Configuration created at $CONFIG_DIR/dynamight.toml"
    else
        log_info "Configuration already exists, preserving..."
    fi
}

install_service() {
    log_info "Installing systemd service..."

    cat > /etc/systemd/system/dynamight.service << EOF
[Unit]
Description=Dynamight Backup Management System
Documentation=https://github.com/your-repo/dynamight
After=network.target

[Service]
Type=simple
User=${SERVICE_USER}
Group=${SERVICE_GROUP}
WorkingDirectory=${INSTALL_DIR}
Environment=DYNAMIGHT_CONFIG=${CONFIG_DIR}/dynamight.toml
Environment=RUST_LOG=info,dynamight=debug
ExecStart=${INSTALL_DIR}/dynamight
Restart=on-failure
RestartSec=5
TimeoutStartSec=30
TimeoutStopSec=30

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ReadWritePaths=${DATA_DIR} ${LOG_DIR}

# Allow mounting drives (required for backup functionality)
CapabilityBoundingSet=CAP_SYS_ADMIN
AmbientCapabilities=CAP_SYS_ADMIN

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=dynamight

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    log_success "Service installed"
}

set_permissions() {
    log_info "Setting permissions..."

    chown -R "$SERVICE_USER:$SERVICE_GROUP" "$INSTALL_DIR"
    chown -R "$SERVICE_USER:$SERVICE_GROUP" "$DATA_DIR"
    chown -R "$SERVICE_USER:$SERVICE_GROUP" "$LOG_DIR"
    chown -R root:$SERVICE_GROUP "$CONFIG_DIR"
    chmod 750 "$CONFIG_DIR"

    log_success "Permissions set"
}

show_status() {
    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║          Installation Complete!                  ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Installation summary:"
    echo "  Binary:     $INSTALL_DIR/dynamight"
    echo "  Static:     $INSTALL_DIR/static/"
    echo "  Config:     $CONFIG_DIR/dynamight.toml"
    echo "  Data:       $DATA_DIR/"
    echo "  Logs:       journalctl -u dynamight"
    echo ""
    echo "Next steps:"
    echo "  1. Review config:       sudo nano $CONFIG_DIR/dynamight.toml"
    echo "  2. Enable service:      sudo systemctl enable dynamight"
    echo "  3. Start service:       sudo systemctl start dynamight"
    echo "  4. Check status:        sudo systemctl status dynamight"
    echo "  5. Open http://your-server:8080 in your browser"
    echo "  6. Complete the initial setup to create your admin account"
    echo ""
}

uninstall() {
    log_warn "Uninstalling Dynamight..."

    systemctl stop dynamight 2>/dev/null || true
    systemctl disable dynamight 2>/dev/null || true
    rm -f /etc/systemd/system/dynamight.service
    systemctl daemon-reload

    read -p "Remove installation directory ($INSTALL_DIR)? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "$INSTALL_DIR"
        log_success "Installation directory removed"
    fi

    read -p "Remove data directory ($DATA_DIR)? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "$DATA_DIR"
        log_success "Data directory removed"
    fi

    read -p "Remove configuration ($CONFIG_DIR)? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "$CONFIG_DIR"
        log_success "Configuration removed"
    fi

    read -p "Remove service user ($SERVICE_USER)? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        userdel "$SERVICE_USER" 2>/dev/null || true
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
            check_systemd
            create_user
            create_directories
            install_files
            setup_config
            install_service
            set_permissions
            show_status
            ;;
        uninstall)
            check_root
            uninstall
            ;;
        *)
            usage
            exit 1
            ;;
    esac
}

main "$@"
