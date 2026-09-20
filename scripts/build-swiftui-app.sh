#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="MacClean"
APP_DIR="$ROOT/dist/$APP_NAME.app"
CONTENTS="$APP_DIR/Contents"
LOCAL_IDENTITY="MacClean Local Development"

cargo build --release --manifest-path "$ROOT/Cargo.toml"
swift build -c release --package-path "$ROOT/MacCleanApp"

rm -rf "$APP_DIR"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"
cp "$ROOT/MacCleanApp/Resources/Info.plist" "$CONTENTS/Info.plist"
cp "$ROOT/MacCleanApp/.build/release/MacCleanApp" "$CONTENTS/MacOS/MacCleanApp"
cp "$ROOT/target/release/macclean" "$CONTENTS/Resources/macclean"
chmod +x "$CONTENTS/MacOS/MacCleanApp" "$CONTENTS/Resources/macclean"

if command -v codesign >/dev/null 2>&1; then
  SIGN_IDENTITY="${MACCLEAN_SIGN_IDENTITY:-}"
  if [[ -z "$SIGN_IDENTITY" ]] && \
    security find-identity -v -p codesigning 2>/dev/null | grep -Fq "\"$LOCAL_IDENTITY\""; then
    SIGN_IDENTITY="$LOCAL_IDENTITY"
  fi

  if [[ -n "$SIGN_IDENTITY" ]]; then
    SIGN_ARGS=(--force --options runtime --sign "$SIGN_IDENTITY")
    if [[ "$SIGN_IDENTITY" == Developer\ ID\ Application:* ]]; then
      SIGN_ARGS+=(--timestamp)
    else
      SIGN_ARGS+=(--timestamp=none)
    fi
    # Sign nested code first so the outer signature seals the final backend.
    codesign "${SIGN_ARGS[@]}" --identifier dev.macclean.backend \
      "$CONTENTS/Resources/macclean"
    codesign "${SIGN_ARGS[@]}" "$APP_DIR"
    codesign --verify --deep --strict --verbose=2 "$APP_DIR"
    echo "Signed $APP_DIR with $SIGN_IDENTITY"
  else
    codesign --force --deep --sign - "$APP_DIR"
    echo "Warning: ad-hoc signature used; Full Disk Access may need approval after each rebuild." >&2
    echo "Run scripts/setup-local-signing.sh once for stable local development identity." >&2
  fi
fi

echo "Built $APP_DIR"
