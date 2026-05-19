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
rm -f "$ARCHIVE_PATH"
mkdir -p "$STAGE_DIR/data"

(
    cd "$REPO_ROOT"
    cargo build --release
    target/release/cli4all build-index \
        --input data/commands.source.json \
        --index data/commands.c4idx \
        --data data/commands.c4dat
)

install -m 755 "$REPO_ROOT/target/release/cli4all" "$STAGE_DIR/cli4all"
install -m 644 "$REPO_ROOT/README.md" "$STAGE_DIR/README.md"
install -m 644 "$REPO_ROOT/PACKAGING.md" "$STAGE_DIR/PACKAGING.md"
install -m 755 "$REPO_ROOT/scripts/install_macos.sh" "$STAGE_DIR/install_macos.sh"
install -m 644 "$REPO_ROOT/data/commands.source.json" "$STAGE_DIR/data/commands.source.json"
install -m 644 "$REPO_ROOT/data/commands.yaml" "$STAGE_DIR/data/commands.yaml"
install -m 644 "$REPO_ROOT/data/commands.c4idx" "$STAGE_DIR/data/commands.c4idx"
install -m 644 "$REPO_ROOT/data/commands.c4dat" "$STAGE_DIR/data/commands.c4dat"
install -m 644 "$REPO_ROOT/data/risks.yaml" "$STAGE_DIR/data/risks.yaml"
LC_ALL=C tar -C "$DIST_DIR" -czf "$ARCHIVE_PATH" "cli4all-macos-$PACKAGE_ARCH"

echo "Created $ARCHIVE_PATH"
