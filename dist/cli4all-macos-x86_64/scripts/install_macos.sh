#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ ! -f "$PACKAGE_ROOT/cli4all" ]]; then
    echo "cli4all binary not found next to install_macos.sh" >&2
    exit 1
fi

if [[ ! -d "$PACKAGE_ROOT/data" ]]; then
    echo "data directory not found next to install_macos.sh" >&2
    exit 1
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "install_macos.sh is intended for macOS only" >&2
    exit 1
fi

BIN_DIR="/usr/local/bin"
SHARE_DIR="/usr/local/share/cli4all"

if [[ -d "/opt/homebrew/bin" && -w "/opt/homebrew/bin" ]]; then
    BIN_DIR="/opt/homebrew/bin"
    SHARE_DIR="/opt/homebrew/share/cli4all"
fi

if [[ ! -w "$BIN_DIR" ]]; then
    echo "Permission is required to install into $BIN_DIR." >&2
    echo "Run this installer with sudo:" >&2
    echo "  sudo ./scripts/install_macos.sh" >&2
    exit 1
fi

PARENT_SHARE_DIR="$(dirname "$SHARE_DIR")"
if [[ ! -d "$PARENT_SHARE_DIR" || ! -w "$PARENT_SHARE_DIR" ]]; then
    echo "Permission is required to install data into $SHARE_DIR." >&2
    echo "Run this installer with sudo:" >&2
    echo "  sudo ./scripts/install_macos.sh" >&2
    exit 1
fi

echo "Installing cli4all to $BIN_DIR"
install -d "$BIN_DIR" "$SHARE_DIR/data"
install -m 755 "$PACKAGE_ROOT/cli4all" "$BIN_DIR/cli4all"
install -m 644 "$PACKAGE_ROOT/README.md" "$SHARE_DIR/README.md"
install -m 644 "$PACKAGE_ROOT/PACKAGING.md" "$SHARE_DIR/PACKAGING.md"
install -m 644 "$PACKAGE_ROOT/data/commands.yaml" "$SHARE_DIR/data/commands.yaml"
install -m 644 "$PACKAGE_ROOT/data/risks.yaml" "$SHARE_DIR/data/risks.yaml"

echo "Installed cli4all"
echo "Binary: $BIN_DIR/cli4all"
echo "Data: $SHARE_DIR/data"
