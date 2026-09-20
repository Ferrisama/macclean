#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/dist/MacClean.app"
BACKEND="$APP/Contents/Resources/macclean"
SMOKE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/macclean-smoke.XXXXXX")"
STATE_DIR="$SMOKE_ROOT/state"
SCAN_ROOT="$SMOKE_ROOT/scan"
CACHE_PATH="$SMOKE_ROOT/Library/Caches/dev.macclean.smoke"
MODULE_CACHE="$SMOKE_ROOT/module-cache"
TRASH_PATH=""

cleanup() {
  if [[ ! -e "$CACHE_PATH" && -n "$TRASH_PATH" && -e "$TRASH_PATH" ]]; then
    mkdir -p "$(dirname "$CACHE_PATH")"
    mv "$TRASH_PATH" "$CACHE_PATH"
  fi
  rm -rf "$SMOKE_ROOT"
}
trap cleanup EXIT

if [[ "${1:-}" != "--skip-build" ]]; then
  mkdir -p "$MODULE_CACHE"
  CLANG_MODULE_CACHE_PATH="$MODULE_CACHE" \
    SWIFTPM_MODULECACHE_OVERRIDE="$MODULE_CACHE" \
    "$ROOT/scripts/build-swiftui-app.sh"
fi

test -x "$APP/Contents/MacOS/MacCleanApp"
test -x "$BACKEND"
codesign --verify --deep --strict "$APP"

mkdir -p "$SCAN_ROOT/folder" "$CACHE_PATH"
printf 'scan fixture\n' > "$SCAN_ROOT/folder/file.txt"
printf 'cleanup fixture\n' > "$CACHE_PATH/payload.txt"

MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-scan "$SCAN_ROOT" \
  --depth 1 --limit 10 --no-system-data --no-health \
  | jq -e '.root_scan.tree.size_bytes > 0 and .root_scan.partial == false' >/dev/null

MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" --dry-run app-trash "$CACHE_PATH" \
  | jq -e '.dry_run == true and .failed_count == 0 and .moved_count == 0' >/dev/null

MOVE_JSON="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-trash "$CACHE_PATH")"
SESSION_ID="$(jq -er '.session_id' <<<"$MOVE_JSON")"
TRASH_PATH="$(jq -er '.outcomes[0].trash_path' <<<"$MOVE_JSON")"
jq -e '.moved_count == 1 and .failed_count == 0 and .moved_bytes > 0 and .reclaimed_bytes == 0' \
  <<<"$MOVE_JSON" >/dev/null
test ! -e "$CACHE_PATH"
test -e "$TRASH_PATH"
test -f "$STATE_DIR/receipts/$SESSION_ID.json"

MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-history --limit 10 \
  | jq -e --arg session "$SESSION_ID" 'any(.[]; .session_id == $session and .restorable_count == 1)' >/dev/null

MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-restore "$SESSION_ID" \
  | jq -e '.restored_count == 1 and .failed_count == 0' >/dev/null
test -f "$CACHE_PATH/payload.txt"
TRASH_PATH=""

echo "MacClean packaged-app smoke test passed."
