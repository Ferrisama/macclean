#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/dist/MacClean.app"
BACKEND="$APP/Contents/Resources/macclean"
SMOKE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/macclean-smoke.XXXXXX")"
SMOKE_ROOT="$(cd "$SMOKE_ROOT" && pwd -P)"
STATE_DIR="$SMOKE_ROOT/state"
SCAN_ROOT="$SMOKE_ROOT/scan"
CACHE_PATH="$SMOKE_ROOT/Library/Caches/dev.macclean.smoke"
RACE_PATH="$SMOKE_ROOT/Library/Caches/dev.macclean.identity-race"
MODULE_CACHE="$SMOKE_ROOT/module-cache"
TRASH_PATH=""
DUPLICATE_TRASH_PATH=""
DUPLICATE_ORIGINAL_PATH=""
EXTRA_PATH=""
EXTRA_TRASH_PATH=""
TEMP_BUILD_PATH=""
READER_PID=""

cleanup() {
  if [[ -n "$READER_PID" ]]; then
    kill "$READER_PID" 2>/dev/null || true
    wait "$READER_PID" 2>/dev/null || true
  fi
  if [[ -n "$EXTRA_PATH" && ! -e "$EXTRA_PATH" && -n "$EXTRA_TRASH_PATH" && -e "$EXTRA_TRASH_PATH" ]]; then
    mv "$EXTRA_TRASH_PATH" "$EXTRA_PATH"
  fi
  if [[ -n "$TEMP_BUILD_PATH" && "$TEMP_BUILD_PATH" == /private/tmp/macclean-smoke-build.*-target ]]; then
    rm -rf "$TEMP_BUILD_PATH"
  fi
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
trap 'echo "Packaged-app smoke check failed at line $LINENO." >&2' ERR

if [[ "${1:-}" != "--skip-build" ]]; then
  mkdir -p "$MODULE_CACHE"
  CLANG_MODULE_CACHE_PATH="$MODULE_CACHE" \
    SWIFTPM_MODULECACHE_OVERRIDE="$MODULE_CACHE" \
    "$ROOT/scripts/build-swiftui-app.sh"
fi

test -x "$APP/Contents/MacOS/MacCleanApp"
test -x "$BACKEND"
codesign --verify --deep --strict "$APP"

mkdir -p "$SCAN_ROOT/folder" "$SCAN_ROOT/duplicates" \
  "$SCAN_ROOT/Library/Caches/Hidden" "$CACHE_PATH" "$RACE_PATH"
printf 'scan fixture\n' > "$SCAN_ROOT/folder/file.txt"
printf 'duplicate fixture\n' > "$SCAN_ROOT/duplicates/a.txt"
cp "$SCAN_ROOT/duplicates/a.txt" "$SCAN_ROOT/duplicates/b.txt"
printf 'cleanup fixture\n' > "$CACHE_PATH/payload.txt"
printf 'original identity\n' > "$RACE_PATH/payload.txt"
printf 'hidden cache fixture\n' > "$SCAN_ROOT/Library/Caches/Hidden/payload.txt"

MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-scan "$SCAN_ROOT" \
  --depth 1 --limit 10 --no-system-data --no-health \
  | jq -e '.root_scan.tree.size_bytes > 0 and .root_scan.partial == false' >/dev/null

MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-scan "$SCAN_ROOT" \
  --depth 1 --limit 10 --deep --no-system-data --no-health \
  | jq -e 'any(.cleanup_candidates[];
      .name == "Caches" and .safety == "safe"
      and .cleanup_action == "Move to Trash" and .size_bytes > 0)' >/dev/null

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

# Exercise the new targets through the packaged app's exact review/Trash/restore commands.
EDITOR_CACHE="$SMOKE_ROOT/Library/Application Support/Code/Cache"
EDITOR_USER="$SMOKE_ROOT/Library/Application Support/Code/User"
PROJECT="$SMOKE_ROOT/Desktop/project"
PROJECT_TARGET="$PROJECT/target"
TEMP_BUILD_PATH="$(mktemp -d /private/tmp/macclean-smoke-build.XXXXXX)-target"
mv "${TEMP_BUILD_PATH%-target}" "$TEMP_BUILD_PATH"
mkdir -p "$TEMP_BUILD_PATH" "$EDITOR_CACHE" "$EDITOR_USER" \
  "$PROJECT_TARGET/debug/.fingerprint" "$PROJECT_TARGET/debug/deps" \
  "$TEMP_BUILD_PATH/debug/.fingerprint" "$TEMP_BUILD_PATH/debug/deps"
printf 'cache fixture\n' > "$EDITOR_CACHE/payload.txt"
printf 'keep settings\n' > "$EDITOR_USER/settings.json"
printf '[package]\nname="fixture"\nversion="0.1.0"\n' > "$PROJECT/Cargo.toml"
printf 'keep source\n' > "$PROJECT/source.rs"
printf 'build fixture\n' > "$PROJECT_TARGET/debug/deps/payload.txt"
printf 'temporary build fixture\n' > "$TEMP_BUILD_PATH/debug/deps/payload.txt"
printf '{}\n' > "$TEMP_BUILD_PATH/.rustc_info.json"

MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-scan "$SMOKE_ROOT" \
  --deep --depth 1 --limit 1 --no-system-data --no-health \
  | jq -e --arg target "$PROJECT_TARGET" --arg cache "$EDITOR_CACHE" \
      'any(.cleanup_candidates[]; .path == $target and .safety == "review")
       and any(.cleanup_candidates[]; .path == $cache and .safety == "safe")' >/dev/null

for EXTRA_PATH in "$EDITOR_CACHE" "$PROJECT_TARGET" "$TEMP_BUILD_PATH"; do
  REVIEW_JSON="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" --dry-run app-trash "$EXTRA_PATH")"
  jq -e '.failed_count == 0 and .dry_run == true' <<<"$REVIEW_JSON" >/dev/null
  EXTRA_TOKEN="$(jq -er '.outcomes[0].review_token' <<<"$REVIEW_JSON")"
  if [[ "$EXTRA_PATH" == "$EDITOR_CACHE" ]]; then
    tail -f "$EDITOR_CACHE/payload.txt" >/dev/null &
    READER_PID=$!
    # Wait for the fixture reader to hold the file, then prove both review and execution reject it.
    for _ in {1..30}; do
      if /usr/sbin/lsof -p "$READER_PID" -Fn 2>/dev/null | rg -q -F "$EDITOR_CACHE/payload.txt"; then break; fi
      sleep 0.1
    done
    for EXTRA_MODE in review execution; do
      if [[ "$EXTRA_MODE" == review ]]; then
        BLOCKED="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" --dry-run app-trash "$EXTRA_PATH")"
      else
        BLOCKED="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-trash "$EXTRA_PATH" --review-token "$EXTRA_TOKEN")"
      fi
      jq -e '.moved_count == 0 and .failed_count == 1 and (.outcomes[0].error | contains("in use"))' <<<"$BLOCKED" >/dev/null
      test -f "$EDITOR_CACHE/payload.txt"
    done
    kill "$READER_PID"
    wait "$READER_PID" 2>/dev/null || true
    READER_PID=""
  fi
  EXTRA_MOVE="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-trash "$EXTRA_PATH" --review-token "$EXTRA_TOKEN")"
  jq -e '.moved_count == 1 and .failed_count == 0 and .reclaimed_bytes == 0' <<<"$EXTRA_MOVE" >/dev/null
  EXTRA_TRASH_PATH="$(jq -er '.outcomes[0].trash_path' <<<"$EXTRA_MOVE")"
  EXTRA_SESSION="$(jq -er '.session_id' <<<"$EXTRA_MOVE")"
  test ! -e "$EXTRA_PATH"
  MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-restore "$EXTRA_SESSION" \
    | jq -e '.restored_count == 1 and .failed_count == 0' >/dev/null
  test -d "$EXTRA_PATH"
  EXTRA_TRASH_PATH=""
done
test -f "$PROJECT/Cargo.toml"
test -f "$PROJECT/source.rs"
test -f "$EDITOR_USER/settings.json"
MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" --dry-run app-trash "$EDITOR_USER" \
  | jq -e '.failed_count == 1 and .moved_count == 0' >/dev/null

# Version removal must re-check project pins after identity review.
VERSION_HOME="$SMOKE_ROOT/version-home"
OLD_RUST="$VERSION_HOME/.rustup/toolchains/nightly-2024-01-01-aarch64-apple-darwin"
OLD_NODE="$VERSION_HOME/.nvm/versions/node/v18.20.0"
OLD_PYTHON="$VERSION_HOME/.pyenv/versions/3.9.0"
OLD_EXTENSION="$VERSION_HOME/.vscode/extensions/acme.fixture-1.0.0"
mkdir -p "$OLD_RUST/bin" "$OLD_NODE/bin" "$OLD_PYTHON/bin" "$OLD_EXTENSION" \
  "$VERSION_HOME/.nvm/alias" "$VERSION_HOME/.pyenv" "$VERSION_HOME/Projects/test/.vscode" \
  "$VERSION_HOME/.vscode/extensions/acme.fixture-2.0.0"
printf 'fixture\n' > "$OLD_RUST/bin/rustc"
printf 'fixture\n' > "$OLD_NODE/bin/node"
printf 'fixture\n' > "$OLD_PYTHON/bin/python"
printf 'default_toolchain="stable-aarch64-apple-darwin"\n' > "$VERSION_HOME/.rustup/settings.toml"
printf '20\n' > "$VERSION_HOME/.nvm/alias/default"
printf '3.12.0\n' > "$VERSION_HOME/.pyenv/version"
printf '{"publisher":"acme","name":"fixture","version":"1.0.0"}\n' > "$OLD_EXTENSION/package.json"
printf '{"publisher":"acme","name":"fixture","version":"2.0.0"}\n' > "$VERSION_HOME/.vscode/extensions/acme.fixture-2.0.0/package.json"
printf '{"acme.fixture-1.0.0":true}\n' > "$VERSION_HOME/.vscode/extensions/.obsolete"
printf '[{"relativeLocation":"acme.fixture-2.0.0"}]\n' > "$VERSION_HOME/.vscode/extensions/extensions.json"
for EXTRA_PATH in "$OLD_RUST" "$OLD_NODE" "$OLD_PYTHON" "$OLD_EXTENSION"; do
  REVIEW_JSON="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" --dry-run app-trash "$EXTRA_PATH")"
  jq -e '.failed_count == 0 and .dry_run == true' <<<"$REVIEW_JSON" >/dev/null
  EXTRA_TOKEN="$(jq -er '.outcomes[0].review_token' <<<"$REVIEW_JSON")"
  case "$EXTRA_PATH" in
    "$OLD_RUST")
      PIN_FILE="$VERSION_HOME/Projects/test/rust-toolchain.toml"
      printf '[toolchain]\nchannel="nightly-2024-01-01"\n' > "$PIN_FILE" ;;
    "$OLD_NODE")
      PIN_FILE="$VERSION_HOME/Projects/test/.nvmrc"
      printf '18\n' > "$PIN_FILE" ;;
    "$OLD_PYTHON")
      PIN_FILE="$VERSION_HOME/Projects/test/.python-version"
      printf '3.9.0\n' > "$PIN_FILE" ;;
    "$OLD_EXTENSION")
      PIN_FILE="$VERSION_HOME/Projects/test/.vscode/extensions.json"
      printf '{"recommendations":["acme.fixture@1.0.0"]}\n' > "$PIN_FILE" ;;
  esac
  MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-trash "$EXTRA_PATH" --review-token "$EXTRA_TOKEN" \
    | jq -e '.moved_count == 0 and .failed_count == 1' >/dev/null
  test -d "$EXTRA_PATH"
  rm "$PIN_FILE"
  EXTRA_MOVE="$(MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-trash "$EXTRA_PATH" --review-token "$EXTRA_TOKEN")"
  jq -e '.moved_count == 1 and .failed_count == 0' <<<"$EXTRA_MOVE" >/dev/null
  EXTRA_TRASH_PATH="$(jq -er '.outcomes[0].trash_path' <<<"$EXTRA_MOVE")"
  EXTRA_SESSION="$(jq -er '.session_id' <<<"$EXTRA_MOVE")"
  test ! -e "$EXTRA_PATH"
  MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" app-restore "$EXTRA_SESSION" \
    | jq -e '.restored_count == 1 and .failed_count == 0' >/dev/null
  test -d "$EXTRA_PATH"
  EXTRA_TRASH_PATH=""
done
MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" --dry-run app-trash "$VERSION_HOME/.rustup" \
  | jq -e '.failed_count == 1 and .moved_count == 0' >/dev/null

echo "MacClean packaged-app smoke test passed."
