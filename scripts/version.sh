#!/usr/bin/env bash
#
# Version management for Dynamight
# Single command to bump the version in Cargo.toml, Cargo.lock,
# package.json and package-lock.json
#
# Usage:
#   ./scripts/version.sh major    # 0.1.0 → 1.0.0
#   ./scripts/version.sh minor    # 0.1.0 → 0.2.0
#   ./scripts/version.sh patch    # 0.1.0 → 0.1.1
#   ./scripts/version.sh set 2.1.0
#   ./scripts/version.sh show
#

set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

CARGO_TOML="$PROJECT_DIR/backend/Cargo.toml"
CARGO_LOCK="$PROJECT_DIR/Cargo.lock"
PACKAGE_JSON="$PROJECT_DIR/frontend/package.json"
PACKAGE_LOCK="$PROJECT_DIR/frontend/package-lock.json"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

die() { echo -e "${RED}[ERROR]${NC} $1" >&2; exit 1; }

# The version line of [package], not of any dependency table
get_version() {
    awk '/^\[/ { in_pkg = ($0 == "[package]") }
         in_pkg && /^version[[:space:]]*=/ { gsub(/.*=[[:space:]]*"|".*/, ""); print; exit }' "$CARGO_TOML"
}

# Replace a file only after the edit produced output
rewrite() {
    local file="$1"; shift
    local tmp
    tmp="$(mktemp "$file.XXXXXX")"
    if ! "$@" "$file" > "$tmp" || [[ ! -s "$tmp" ]]; then
        rm -f "$tmp"
        die "Could not update $file"
    fi
    cat "$tmp" > "$file"
    rm -f "$tmp"
}

set_version() {
    local new_version="$1"

    # Validate semver format
    if ! [[ "$new_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        die "Invalid version format: $new_version (expected: X.Y.Z)"
    fi

    # backend/Cargo.toml: only the [package] version
    rewrite "$CARGO_TOML" awk -v v="$new_version" '
        /^\[/ { in_pkg = ($0 == "[package]") }
        in_pkg && !done && /^version[[:space:]]*=/ { print "version = \"" v "\""; done = 1; next }
        { print }'

    # Cargo.lock: the version right after name = "dynamight"
    rewrite "$CARGO_LOCK" awk -v v="$new_version" '
        prev == "name = \"dynamight\"" && /^version = / { print "version = \"" v "\""; prev = ""; next }
        { print; prev = $0 }'

    # package.json and package-lock.json: only the top-level "version" fields
    # (npm does this when available; the fallback edits the same lines)
    if command -v npm &>/dev/null; then
        (cd "$PROJECT_DIR/frontend" && npm version "$new_version" --no-git-tag-version --allow-same-version >/dev/null)
    else
        rewrite "$PACKAGE_JSON" awk -v v="$new_version" '
            !done && /^  "version": / { sub(/"version": "[^"]*"/, "\"version\": \"" v "\""); done = 1 }
            { print }'
        rewrite "$PACKAGE_LOCK" awk -v v="$new_version" '
            /^  "version": / || (in_root && /^      "version": /) { sub(/"version": "[^"]*"/, "\"version\": \"" v "\"") }
            /^    "": \{/ { in_root = 1 }
            in_root && /^    \}/ { in_root = 0 }
            { print }'
    fi

    [[ "$(get_version)" == "$new_version" ]] || die "backend/Cargo.toml still reports $(get_version)"

    echo -e "${GREEN}[OK]${NC} Version set to ${BLUE}${new_version}${NC}"
    echo "  Updated: backend/Cargo.toml, Cargo.lock"
    echo "  Updated: frontend/package.json, frontend/package-lock.json"
}

bump_version() {
    local current
    current="$(get_version)"
    [[ "$current" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "Current version '$current' is not X.Y.Z"
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
        [[ -n "${2:-}" ]] || die "Usage: $0 set <version>"
        set_version "$2"
        ;;
    show)
        get_version
        ;;
    *)
        echo "Usage: $0 {major|minor|patch|set <version>|show}"
        exit 1
        ;;
esac
