#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/dist/MacClean.app"
BACKEND="$APP/Contents/Resources/macclean"
SMOKE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/macclean-smoke.XXXXXX")"
STATE_DIR="$SMOKE_ROOT/state"
SCAN_ROOT="$SMOKE_ROOT/scan"
CACHE_PATH="$SMOKE_ROOT/Library/Caches/dev.macclean.smoke"
RACE_PATH="$SMOKE_ROOT/Library/Caches/dev.macclean.identity-race"
MODULE_CACHE="$SMOKE_ROOT/module-cache"
TRASH_PATH=""
DUPLICATE_TRASH_PATH=""
DUPLICATE_ORIGINAL_PATH=""

cleanup() {
  if [[ ! -e "$CACHE_PATH" && -n "$TRASH_PATH" && -e "$TRASH_PATH" ]]; then
    mkdir -p "$(dirname "$CACHE_PATH")"
    mv "$TRASH_PATH" "$CACHE_PATH"
  fi
  if [[ -n "$DUPLICATE_ORIGINAL_PATH" && ! -e "$DUPLICATE_ORIGINAL_PATH" \
      && -n "$DUPLICATE_TRASH_PATH" && -e "$DUPLICATE_TRASH_PATH" ]]; then
    mkdir -p "$(dirname "$DUPLICATE_ORIGINAL_PATH")"
    mv "$DUPLICATE_TRASH_PATH" "$DUPLICATE_ORIGINAL_PATH"
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

mkdir -p "$SCAN_ROOT/folder" "$SCAN_ROOT/duplicates" "$CACHE_PATH" "$RACE_PATH"
printf 'scan fixture\n' > "$SCAN_ROOT/folder/file.txt"
printf 'duplicate fixture\n' > "$SCAN_ROOT/duplicates/a.txt"
cp "$SCAN_ROOT/duplicates/a.txt" "$SCAN_ROOT/duplicates/b.txt"
printf 'cleanup fixture\n' > "$CACHE_PATH/payload.txt"
printf 'original identity\n' > "$RACE_PATH/payload.txt"

MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-scan "$SCAN_ROOT" \
  --depth 1 --limit 10 --no-system-data --no-health \
  | jq -e '.root_scan.tree.size_bytes > 0 and .root_scan.partial == false' >/dev/null

DUPLICATE_SCAN_JSON="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-dupes "$SCAN_ROOT" --min 0)"
jq -e '.partial == false and (.groups | length == 1)' <<<"$DUPLICATE_SCAN_JSON" >/dev/null

MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-dupes "$SCAN_ROOT" --min 0 --progress \
  | jq -s -e 'any(.[]; .type == "progress" and .data.stage == "hashing")
      and any(.[]; .type == "result" and (.data.groups | length == 1))' >/dev/null

DUPLICATE_GROUP_ID="$(jq -er '.groups[0].id' <<<"$DUPLICATE_SCAN_JSON")"
DUPLICATE_KEEPER="$(jq -er '.groups[0].files[0].path' <<<"$DUPLICATE_SCAN_JSON")"
DUPLICATE_ORIGINAL_PATH="$(jq -er '.groups[0].files[1].path' <<<"$DUPLICATE_SCAN_JSON")"
DUPLICATE_FILES="$(jq -c '[.groups[0].files[].path]' <<<"$DUPLICATE_SCAN_JSON")"
DUPLICATE_REQUEST="$SMOKE_ROOT/duplicate-request.json"
jq -n \
  --arg id "$DUPLICATE_GROUP_ID" \
  --arg keeper "$DUPLICATE_KEEPER" \
  --arg selected "$DUPLICATE_ORIGINAL_PATH" \
  --argjson files "$DUPLICATE_FILES" \
  '{groups: [{id: $id, keeper_path: $keeper, files: $files,
    selected: [{path: $selected}]}]}' >"$DUPLICATE_REQUEST"

DUPLICATE_PREFLIGHT="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" --dry-run \
  app-dupes-trash --request-file "$DUPLICATE_REQUEST")"
jq -e '.dry_run == true and .failed_count == 0 and .moved_count == 0
  and (.outcomes[0].review_token | length > 0)' <<<"$DUPLICATE_PREFLIGHT" >/dev/null
DUPLICATE_TOKEN="$(jq -er '.outcomes[0].review_token' <<<"$DUPLICATE_PREFLIGHT")"
jq --arg token "$DUPLICATE_TOKEN" '.groups[0].selected[0].review_token = $token' \
  "$DUPLICATE_REQUEST" >"$DUPLICATE_REQUEST.reviewed"

DUPLICATE_MOVE="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-dupes-trash \
  --request-file "$DUPLICATE_REQUEST.reviewed")"
DUPLICATE_SESSION="$(jq -er '.session_id' <<<"$DUPLICATE_MOVE")"
DUPLICATE_TRASH_PATH="$(jq -er '.outcomes[0].trash_path' <<<"$DUPLICATE_MOVE")"
jq -e '.moved_count == 1 and .failed_count == 0 and .moved_bytes > 0
  and .reclaimed_bytes == 0' <<<"$DUPLICATE_MOVE" >/dev/null
test -f "$DUPLICATE_KEEPER"
test ! -e "$DUPLICATE_ORIGINAL_PATH"
test -e "$DUPLICATE_TRASH_PATH"
MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-history --limit 10 \
  | jq -e --arg session "$DUPLICATE_SESSION" \
      'any(.[]; .session_id == $session and .restorable_count == 1)' >/dev/null
MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-restore "$DUPLICATE_SESSION" \
  | jq -e '.restored_count == 1 and .failed_count == 0' >/dev/null
test -f "$DUPLICATE_ORIGINAL_PATH"
DUPLICATE_TRASH_PATH=""

PREFLIGHT_JSON="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" --dry-run app-trash "$CACHE_PATH")"
jq -e '.dry_run == true and .failed_count == 0 and .moved_count == 0
  and (.outcomes[0].review_token | length > 0)' <<<"$PREFLIGHT_JSON" >/dev/null
REVIEW_TOKEN="$(jq -er '.outcomes[0].review_token' <<<"$PREFLIGHT_JSON")"

RACE_PREFLIGHT="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" --dry-run app-trash "$RACE_PATH")"
RACE_TOKEN="$(jq -er '.outcomes[0].review_token' <<<"$RACE_PREFLIGHT")"
mv "$RACE_PATH" "$RACE_PATH.reviewed"
mkdir -p "$RACE_PATH"
printf 'replacement identity\n' > "$RACE_PATH/payload.txt"
MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-trash "$RACE_PATH" --review-token "$RACE_TOKEN" \
  | jq -e '.moved_count == 0 and .failed_count == 1
      and (.outcomes[0].error | contains("changed after review"))' >/dev/null
test -f "$RACE_PATH/payload.txt"

MOVE_JSON="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-trash "$CACHE_PATH" --review-token "$REVIEW_TOKEN")"
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
