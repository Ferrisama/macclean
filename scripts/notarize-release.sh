#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
TARGET="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
IDENTITY="${MACCLEAN_SIGN_IDENTITY:-Developer ID Application}"
APPLE_ID="${APPLE_ID:-}"
APPLE_TEAM_ID="${APPLE_TEAM_ID:-}"
APPLE_APP_PASSWORD="${APPLE_APP_PASSWORD:-}"
DIST="$ROOT/dist"
WORK="$DIST/notarize-v$VERSION-$TARGET"

if [[ "$TARGET" != *"apple-darwin" ]]; then
  echo "Notarization only applies to macOS targets; got $TARGET" >&2
  exit 1
fi

for required in APPLE_ID APPLE_TEAM_ID APPLE_APP_PASSWORD; do
  if [[ -z "${!required:-}" ]]; then
    echo "Missing $required" >&2
    exit 1
  fi
done

cargo build --release --target "$TARGET" --manifest-path "$ROOT/Cargo.toml"
rm -rf "$WORK"
mkdir -p "$WORK"
cp "$ROOT/target/$TARGET/release/macclean" "$WORK/macclean"

codesign --force --timestamp --options runtime --sign "$IDENTITY" "$WORK/macclean"
codesign --verify --verbose "$WORK/macclean"

zip_path="$DIST/macclean-v$VERSION-$TARGET-notarize.zip"
ditto -c -k --keepParent "$WORK/macclean" "$zip_path"

xcrun notarytool submit "$zip_path" \
  --apple-id "$APPLE_ID" \
  --team-id "$APPLE_TEAM_ID" \
  --password "$APPLE_APP_PASSWORD" \
  --wait

final_dir="$DIST/macclean-v$VERSION-$TARGET"
rm -rf "$final_dir"
mkdir -p "$final_dir"
cp "$WORK/macclean" "$final_dir/macclean"
cp "$ROOT/README.md" "$final_dir/README.md"
cp "$ROOT/LICENSE" "$final_dir/LICENSE"

tarball="$DIST/macclean-v$VERSION-$TARGET.tar.gz"
tar -C "$DIST" -czf "$tarball" "macclean-v$VERSION-$TARGET"
shasum -a 256 "$tarball" > "$tarball.sha256"

echo "Signed and notarized binary packaged:"
echo "  $tarball"
echo "  $tarball.sha256"
