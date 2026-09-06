#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="MacClean"
APP_DIR="$ROOT/dist/$APP_NAME.app"
CONTENTS="$APP_DIR/Contents"

cargo build --release --manifest-path "$ROOT/Cargo.toml"
swift build -c release --package-path "$ROOT/MacCleanApp"

rm -rf "$APP_DIR"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"
cp "$ROOT/MacCleanApp/Resources/Info.plist" "$CONTENTS/Info.plist"
cp "$ROOT/MacCleanApp/.build/release/MacCleanApp" "$CONTENTS/MacOS/MacCleanApp"
cp "$ROOT/target/release/macclean" "$CONTENTS/Resources/macclean"
chmod +x "$CONTENTS/MacOS/MacCleanApp" "$CONTENTS/Resources/macclean"

if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$APP_DIR"
fi

echo "Built $APP_DIR"
