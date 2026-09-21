#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/dist/MacClean.app"
BUILD_SCRIPT="$ROOT/scripts/build-swiftui-app.sh"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/macclean-beta-upgrade.XXXXXX")"
STATE_DIR="$WORK_DIR/state"
SCAN_ROOT="$WORK_DIR/scan"
FIRST_REQUIREMENT="$WORK_DIR/first-designated-requirement.txt"
SECOND_REQUIREMENT="$WORK_DIR/second-designated-requirement.txt"
SKIP_INITIAL_BUILD=false

cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

usage() {
  echo "Usage: scripts/verify-beta-upgrade.sh [--skip-initial-build]"
  echo
  echo "Builds MacClean twice by default and verifies beta metadata, stable signing,"
  echo "and compatibility with state written by the first build."
}

case "${1:-}" in
  "") ;;
  --skip-initial-build) SKIP_INITIAL_BUILD=true ;;
  -h|--help)
    usage
    exit 0
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Required command not found: $1" >&2
    exit 1
  fi
}

for command in codesign jq shasum; do
  require_command "$command"
done

if [[ ! -x "$BUILD_SCRIPT" ]]; then
  echo "Build script is not executable: $BUILD_SCRIPT" >&2
  exit 1
fi

bundle_value() {
  /usr/libexec/PlistBuddy -c "Print :$1" "$APP/Contents/Info.plist"
}

source_bundle_value() {
  /usr/libexec/PlistBuddy -c "Print :$1" "$ROOT/MacCleanApp/Resources/Info.plist"
}

cargo_version() {
  awk '
    /^\[package\][[:space:]]*$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && $1 == "version" {
      value = $3
      gsub(/"/, "", value)
      print value
      exit
    }
  ' "$ROOT/Cargo.toml"
}

extract_designated_requirement() {
  local app="$1"
  local destination="$2"
  local raw="$destination.raw"
  codesign --display --requirements - "$app" >"$raw" 2>&1
  sed -n 's/^designated => //p' "$raw" >"$destination"
  if [[ ! -s "$destination" ]]; then
    echo "Unable to extract the designated requirement for $app" >&2
    cat "$raw" >&2
    exit 1
  fi
}

verify_bundle() {
  local expected_version="$1"
  local backend="$APP/Contents/Resources/macclean"
  local short_version
  local build_version

  test -x "$APP/Contents/MacOS/MacCleanApp"
  test -x "$backend"
  codesign --verify --deep --strict --verbose=2 "$APP"

  if codesign --display --verbose=4 "$APP" 2>&1 | grep -Fq "Signature=adhoc"; then
    echo "The beta app is ad-hoc signed, so its identity cannot remain stable across rebuilds." >&2
    echo "Run scripts/setup-local-signing.sh, then rerun this verifier." >&2
    exit 1
  fi

  [[ "$(bundle_value CFBundleIdentifier)" == "dev.macclean.app" ]]
  [[ "$(bundle_value CFBundleExecutable)" == "MacCleanApp" ]]
  short_version="$(bundle_value CFBundleShortVersionString)"
  build_version="$(bundle_value CFBundleVersion)"
  [[ "$short_version" == "$expected_version" ]]
  [[ "$build_version" =~ ^[1-9][0-9]*$ ]]
  [[ "$($backend --version)" == "macclean $expected_version" ]]
}

state_manifest() {
  if [[ ! -d "$STATE_DIR" ]]; then
    return
  fi
  find "$STATE_DIR" -type f -exec shasum -a 256 {} \; | LC_ALL=C sort
}

EXPECTED_VERSION="$(cargo_version)"
SOURCE_SHORT_VERSION="$(source_bundle_value CFBundleShortVersionString)"
SOURCE_BUILD_VERSION="$(source_bundle_value CFBundleVersion)"

if [[ -z "$EXPECTED_VERSION" || "$SOURCE_SHORT_VERSION" != "$EXPECTED_VERSION" ]]; then
  echo "Version mismatch: Cargo.toml=$EXPECTED_VERSION Info.plist=$SOURCE_SHORT_VERSION" >&2
  exit 1
fi
if [[ ! "$SOURCE_BUILD_VERSION" =~ ^[1-9][0-9]*$ ]]; then
  echo "CFBundleVersion must be a positive integer; found: $SOURCE_BUILD_VERSION" >&2
  exit 1
fi

if [[ "$SKIP_INITIAL_BUILD" == false ]]; then
  "$BUILD_SCRIPT"
elif [[ ! -d "$APP" ]]; then
  echo "--skip-initial-build requires an existing bundle at $APP" >&2
  exit 1
fi

verify_bundle "$EXPECTED_VERSION"
extract_designated_requirement "$APP" "$FIRST_REQUIREMENT"

mkdir -p "$SCAN_ROOT/folder" "$STATE_DIR"
printf 'beta upgrade scan fixture\n' >"$SCAN_ROOT/folder/file.txt"
FIRST_BACKEND="$APP/Contents/Resources/macclean"
MACCLEAN_STATE_DIR="$STATE_DIR" "$FIRST_BACKEND" app-scan "$SCAN_ROOT" \
  --depth 1 --limit 10 --no-system-data --no-health \
  | jq -e '.schema_version == 1 and .root_scan.tree.size_bytes > 0' >/dev/null

# The seven-field form predates recorded Trash destinations. It must remain
# readable, but it must never be represented as automatically restorable.
printf '1\tbeta-upgrade-legacy\tupgrade-test\tlegacy item\t/nonexistent/macclean-beta-item\t123\tlegacy\n' \
  >"$STATE_DIR/history.tsv"
MACCLEAN_STATE_DIR="$STATE_DIR" "$FIRST_BACKEND" app-history --limit 10 \
  | jq -e 'any(.[]; .session_id == "beta-upgrade-legacy"
      and .item_count == 1 and .restorable_count == 0)' >/dev/null

STATE_BEFORE_REBUILD="$(state_manifest)"
"$BUILD_SCRIPT"
STATE_AFTER_REBUILD="$(state_manifest)"

if [[ "$STATE_BEFORE_REBUILD" != "$STATE_AFTER_REBUILD" ]]; then
  echo "Building the replacement app changed persisted beta state." >&2
  diff -u <(printf '%s\n' "$STATE_BEFORE_REBUILD") \
    <(printf '%s\n' "$STATE_AFTER_REBUILD") >&2 || true
  exit 1
fi

verify_bundle "$EXPECTED_VERSION"
extract_designated_requirement "$APP" "$SECOND_REQUIREMENT"
codesign --verify --strict --verbose=2 -R "$FIRST_REQUIREMENT" "$APP"
cmp -s "$FIRST_REQUIREMENT" "$SECOND_REQUIREMENT"

SECOND_BACKEND="$APP/Contents/Resources/macclean"
MACCLEAN_STATE_DIR="$STATE_DIR" "$SECOND_BACKEND" app-cache \
  | jq -e '.schema_version == 1 and .root_scan.tree.size_bytes > 0' >/dev/null
MACCLEAN_STATE_DIR="$STATE_DIR" "$SECOND_BACKEND" app-history --limit 10 \
  | jq -e 'any(.[]; .session_id == "beta-upgrade-legacy"
      and .item_count == 1 and .restorable_count == 0)' >/dev/null

echo "MacClean beta upgrade verification passed."
echo "Version: $EXPECTED_VERSION ($SOURCE_BUILD_VERSION)"
echo "Bundle identity and isolated state remained compatible across rebuilds."
