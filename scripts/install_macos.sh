#!/usr/bin/env bash
set -euo pipefail

SCRIPT_PATH="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" && pwd)"
PACKAGE_ROOT="$SCRIPT_DIR"

if [[ ! -f "$PACKAGE_ROOT/cli4all" && -f "$PACKAGE_ROOT/../cli4all" ]]; then
    PACKAGE_ROOT="$(cd "$PACKAGE_ROOT/.." && pwd)"
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "install_macos.sh is intended for macOS only." >&2
    exit 1
fi

if [[ -z "${HOME:-}" ]]; then
    echo "HOME is not set. install_macos.sh needs HOME to install into ~/.local." >&2
    exit 1
fi

if [[ ! -f "$PACKAGE_ROOT/cli4all" ]]; then
    echo "cli4all binary not found next to install_macos.sh." >&2
    exit 1
fi

if [[ ! -d "$PACKAGE_ROOT/data" ]]; then
    echo "data directory not found next to install_macos.sh." >&2
    exit 1
fi

REQUIRED_DATA_FILES=(
    "commands.c4idx"
    "commands.c4dat"
    "commands.source.json"
    "commands.yaml"
    "risks.yaml"
)

for file_name in "${REQUIRED_DATA_FILES[@]}"; do
    if [[ ! -f "$PACKAGE_ROOT/data/$file_name" ]]; then
        echo "Missing required data file: $PACKAGE_ROOT/data/$file_name" >&2
        exit 1
    fi
done

BIN_DIR="$HOME/.local/bin"
SHARE_DIR="$HOME/.local/share/cli4all"
DATA_DIR="$SHARE_DIR/data"

echo "Installing cli4all into user-local directories"
install -d "$BIN_DIR" "$DATA_DIR"
install -m 755 "$PACKAGE_ROOT/cli4all" "$BIN_DIR/cli4all"
install -m 644 "$PACKAGE_ROOT/README.md" "$SHARE_DIR/README.md"
install -m 644 "$PACKAGE_ROOT/PACKAGING.md" "$SHARE_DIR/PACKAGING.md"

for file_name in "${REQUIRED_DATA_FILES[@]}"; do
    install -m 644 "$PACKAGE_ROOT/data/$file_name" "$DATA_DIR/$file_name"
done

echo "Installed cli4all"
echo "Binary: $BIN_DIR/cli4all"
echo "Data: $DATA_DIR"

case ":${PATH:-}:" in
    *":$BIN_DIR:"*)
        ;;
    *)
        echo
        echo "$BIN_DIR is not currently in PATH."
        echo "Add this line to ~/.zprofile or ~/.zshrc:"
        echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        ;;
esac

echo
echo "Test the installation with:"
echo "  cli4all --help"
echo "  cli4all check \"ipconfig\""
echo "  cli4all shell"
