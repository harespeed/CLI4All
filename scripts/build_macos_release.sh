#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "build_macos_release.sh must be run on macOS." >&2
    exit 1
fi

HOST_ARCH="$(uname -m)"
case "$HOST_ARCH" in
    arm64)
        PACKAGE_ARCH="aarch64"
        ;;
    x86_64)
        PACKAGE_ARCH="x86_64"
        ;;
    *)
        echo "Unsupported macOS architecture: $HOST_ARCH" >&2
        exit 1
        ;;
esac

DIST_DIR="$REPO_ROOT/dist"
STAGE_DIR="$DIST_DIR/cli4all-macos-$PACKAGE_ARCH"
ARCHIVE_PATH="$DIST_DIR/cli4all-macos-$PACKAGE_ARCH.tar.gz"

mkdir -p "$DIST_DIR"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/data" "$STAGE_DIR/scripts"

(
    cd "$REPO_ROOT"
    cargo build --release
)

cp "$REPO_ROOT/target/release/cli4all" "$STAGE_DIR/cli4all"
cp "$REPO_ROOT/README.md" "$STAGE_DIR/README.md"
cp "$REPO_ROOT/PACKAGING.md" "$STAGE_DIR/PACKAGING.md"
cp "$REPO_ROOT/data/"*.yaml "$STAGE_DIR/data/"
cp "$REPO_ROOT/scripts/install_macos.sh" "$STAGE_DIR/scripts/install_macos.sh"
tar -C "$DIST_DIR" -czf "$ARCHIVE_PATH" "cli4all-macos-$PACKAGE_ARCH"

echo "Created $ARCHIVE_PATH"
