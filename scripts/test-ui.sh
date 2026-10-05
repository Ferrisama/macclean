#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export MACCLEAN_RENDERED_UI_TESTS=1
export MACCLEAN_UI_SNAPSHOTS="$ROOT/dist/ui-snapshots"
cd "$ROOT/MacCleanApp"
swift test --filter RenderedScrollingTests
