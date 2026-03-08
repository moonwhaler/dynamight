#!/usr/bin/env bash
#
# Version management for Dynamight
# Single command to bump version across Cargo.toml and package.json
#
# Usage:
#   ./scripts/version.sh major    # 0.1.0 → 1.0.0
#   ./scripts/version.sh minor    # 0.1.0 → 0.2.0
#   ./scripts/version.sh patch    # 0.1.0 → 0.1.1
#   ./scripts/version.sh set 2.1.0
#   ./scripts/version.sh show
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

CARGO_TOML="$PROJECT_DIR/backend/Cargo.toml"
PACKAGE_JSON="$PROJECT_DIR/frontend/package.json"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

get_version() {
    grep '^version' "$CARGO_TOML" | head -1 | sed 's/.*"\(.*\)".*/\1/'
}

set_version() {
    local new_version="$1"

    # Validate semver format
    if ! [[ "$new_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo -e "${RED}[ERROR]${NC} Invalid version format: $new_version (expected: X.Y.Z)"
        exit 1
    fi

    # Update Cargo.toml
    sed -i "s/^version = \".*\"/version = \"${new_version}\"/" "$CARGO_TOML"

    # Update package.json
    sed -i "s/\"version\": \".*\"/\"version\": \"${new_version}\"/" "$PACKAGE_JSON"

    echo -e "${GREEN}[OK]${NC} Version set to ${BLUE}${new_version}${NC}"
    echo "  Updated: backend/Cargo.toml"
    echo "  Updated: frontend/package.json"
}

bump_version() {
    local current
    current=$(get_version)
    local major minor patch
    IFS='.' read -r major minor patch <<< "$current"

    case "$1" in
        major) major=$((major + 1)); minor=0; patch=0 ;;
        minor) minor=$((minor + 1)); patch=0 ;;
        patch) patch=$((patch + 1)) ;;
    esac

    local new_version="${major}.${minor}.${patch}"
    echo -e "${BLUE}[INFO]${NC} Bumping: ${current} → ${new_version}"
    set_version "$new_version"
}

case "${1:-show}" in
    major|minor|patch)
        bump_version "$1"
        ;;
    set)
        if [[ -z "$2" ]]; then
            echo -e "${RED}[ERROR]${NC} Usage: $0 set <version>"
            exit 1
        fi
        set_version "$2"
        ;;
    show)
        echo "$(get_version)"
        ;;
    *)
        echo "Usage: $0 {major|minor|patch|set <version>|show}"
        exit 1
        ;;
esac
