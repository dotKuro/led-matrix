#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_DIR="$PROJECT_ROOT/deploy"
TARGET="aarch64-unknown-linux-gnu"

echo "==> Building frontend (WASM)"
( cd "$PROJECT_ROOT/frontend" && trunk build --release )

echo "==> Cross-compiling server for $TARGET"
cross build --manifest-path "$PROJECT_ROOT/server/Cargo.toml" --release --target "$TARGET"

echo "==> Assembling $DEPLOY_DIR"
rm -rf "$DEPLOY_DIR"
mkdir -p "$DEPLOY_DIR/static"
cp "$PROJECT_ROOT/target/$TARGET/release/matrix-server" "$DEPLOY_DIR/"
cp -r "$PROJECT_ROOT/frontend/dist/." "$DEPLOY_DIR/static/"

echo "==> Done."
echo "    scp -r $DEPLOY_DIR/. pi@<pi-host>:/home/pi/matrix/"
