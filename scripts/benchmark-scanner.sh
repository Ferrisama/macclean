#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND="${MACCLEAN_BENCH_BACKEND:-$ROOT/target/release/macclean}"
RUNS=5
TARGET=""
OUTPUT_DIR=""

usage() {
  echo "Usage: scripts/benchmark-scanner.sh [--runs N] [--output-dir NEW_DIRECTORY] <directory>"
  echo
  echo "Runs the same complete inventory scan repeatedly and reports raw JSONL plus"
  echo "median/p95 engine latency and peak resident memory. System Data and"
  echo "health probes are excluded so every run has the same storage scope."
  echo "Optional output directory retains scan JSON, timing, metadata, and summary."
  echo "Build the desired release binary first; an existing binary is never rebuilt."
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      RUNS="$2"
      shift 2
      ;;
    --output-dir)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      OUTPUT_DIR="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -* )
      usage >&2
      exit 2
      ;;
    *)
      [[ -z "$TARGET" ]] || { usage >&2; exit 2; }
      TARGET="$1"
      shift
      ;;
  esac
done

[[ "$RUNS" =~ ^[1-9][0-9]*$ ]] || { echo "--runs must be a positive integer" >&2; exit 2; }
[[ -n "$TARGET" && -d "$TARGET" ]] || { usage >&2; exit 2; }

if [[ ! -x "$BACKEND" ]]; then
  cargo build --release --manifest-path "$ROOT/Cargo.toml"
fi

if [[ -n "$OUTPUT_DIR" ]]; then
  mkdir "$OUTPUT_DIR"
  WORK_DIR="$(cd "$OUTPUT_DIR" && pwd -P)"
else
  WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/macclean-benchmark.XXXXXX")"
fi
STATE_DIR="$WORK_DIR/state"
RESULTS="$WORK_DIR/results.jsonl"
cleanup() {
  if [[ -z "$OUTPUT_DIR" ]]; then rm -rf "$WORK_DIR"; fi
}
trap cleanup EXIT

TARGET="$(cd "$TARGET" && pwd -P)"
mkdir -p "$STATE_DIR"
# Capture identity and conditions; repeated runs are not controlled cold-cache tests.
jq -n --arg target "$TARGET" --arg backend "$BACKEND" \
  --arg binary_sha256 "$(shasum -a 256 "$BACKEND" | awk '{print $1}')" \
  --arg git_revision "$(git -C "$ROOT" rev-parse HEAD)" \
  --arg git_status "$(git -C "$ROOT" status --porcelain)" \
  --arg machine "$(uname -sm)" --arg os "$(sw_vers -productVersion)" \
  --arg started_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  '{target:$target, backend:$backend, binary_sha256:$binary_sha256,
    git_revision:$git_revision, working_tree_status:$git_status, machine:$machine,
    os_version:$os, started_at:$started_at,
    cache_conditions:"uncontrolled first run; subsequent runs normally warm",
    workload:"app-scan --deep --depth 1 --limit 100 --no-system-data --no-health",
    includes:"backend traversal, inventory construction, classification, JSON serialization and isolated cache write",
    excludes:"Swift decoding/indexing/rendering; UI memory; cleanup execution"}' > "$WORK_DIR/metadata.json"

for ((run = 1; run <= RUNS; run++)); do
  OUTPUT="$WORK_DIR/scan-$run.json"
  TIMING="$WORK_DIR/time-$run.txt"
  MACCLEAN_STATE_DIR="$STATE_DIR" python3 "$ROOT/scripts/measure-process.py" "$TIMING" -- "$BACKEND" \
    app-scan "$TARGET" --deep --depth 1 --limit 100 --no-system-data --no-health \
    >"$OUTPUT" 2>"$WORK_DIR/stderr-$run.txt"

  ENGINE_MS="$(jq -er '.root_scan.elapsed_ms' "$OUTPUT")"
  ENTRIES="$(jq -er '.root_scan.metrics.entries_seen // 0' "$OUTPUT")"
  ACCOUNTING_VERSION="$(jq -er '.root_scan.metrics.accounting_version // 0' "$OUTPUT")"
  ALLOCATED="$(jq -er '.root_scan.tree.size_bytes' "$OUTPUT")"
  LOGICAL="$(jq -r '.root_scan.tree.logical_size_bytes // null' "$OUTPUT")"
  HARD_LINKS="$(jq -r '.root_scan.metrics.hard_links_deduplicated // null' "$OUTPUT")"
  ERRORS="$(jq -er '.root_scan.metrics.metadata_errors' "$OUTPUT")"
  PARTIAL="$(jq -r '.root_scan.partial' "$OUTPUT")"
  REAL_SECONDS="$(jq -er '.real_seconds' "$TIMING")"
  PEAK_RSS="$(jq -er '.peak_rss_bytes' "$TIMING")"

  jq -cn \
    --argjson run "$run" \
    --argjson engine_ms "$ENGINE_MS" \
    --argjson real_seconds "$REAL_SECONDS" \
    --argjson peak_rss_bytes "$PEAK_RSS" \
    --argjson entries_seen "$ENTRIES" \
    --argjson partial "$PARTIAL" \
    --argjson accounting_version "$ACCOUNTING_VERSION" \
    --argjson allocated_bytes "$ALLOCATED" --argjson logical_bytes "$LOGICAL" \
    --argjson hard_links_deduplicated "$HARD_LINKS" --argjson metadata_errors "$ERRORS" \
    '{run: $run, engine_ms: $engine_ms, real_seconds: $real_seconds,
      peak_rss_bytes: $peak_rss_bytes, entries_seen: $entries_seen,
      partial: $partial, accounting_version:$accounting_version,
      allocated_bytes:$allocated_bytes, logical_bytes:$logical_bytes,
      hard_links_deduplicated:$hard_links_deduplicated, metadata_errors:$metadata_errors}' | tee -a "$RESULTS"
done

jq -s --arg target "$TARGET" '
  def percentile(field; fraction):
    (map(. [field]) | sort) as $values
    | $values[(((length * fraction) | ceil) - 1)];
  {
    target: $target,
    runs: length,
    entries_seen: (map(.entries_seen) | max),
    any_partial: any(.[]; .partial),
    accounting_versions: (map(.accounting_version) | unique),
    allocated_bytes: (map(.allocated_bytes) | unique),
    logical_bytes: (map(.logical_bytes) | unique),
    entry_counts: (map(.entries_seen) | unique),
    coverage_errors: (map(.metadata_errors) | unique),
    warning: "First run cache state uncontrolled; backend-only RSS; no UI measurement. Compare equivalent coverage/accounting.",
    engine_ms: {
      median: percentile("engine_ms"; 0.50),
      p95: percentile("engine_ms"; 0.95)
    },
    real_seconds: {
      median: percentile("real_seconds"; 0.50),
      p95: percentile("real_seconds"; 0.95)
    },
    peak_rss_bytes: (map(.peak_rss_bytes) | map(select(. != null)) | max // null)
  }
' "$RESULTS" | tee "$WORK_DIR/summary.json"
