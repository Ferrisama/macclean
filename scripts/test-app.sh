#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo fmt --check --manifest-path "$ROOT/Cargo.toml"
cargo test --all-targets --manifest-path "$ROOT/Cargo.toml"
MODULE_CACHE="$ROOT/MacCleanApp/.build/module-cache"
mkdir -p "$MODULE_CACHE"
(
  cd "$ROOT/MacCleanApp"
  CLANG_MODULE_CACHE_PATH="$MODULE_CACHE" \
    SWIFTPM_MODULECACHE_OVERRIDE="$MODULE_CACHE" \
    swift test
)
"$ROOT/scripts/smoke-swiftui-app.sh"

echo "MacClean app checks passed. Complete the manual window checks in docs/APP_TESTING.md for a full UI pass."
