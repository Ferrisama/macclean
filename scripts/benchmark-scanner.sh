#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND="${MACCLEAN_BENCH_BACKEND:-$ROOT/target/release/macclean}"
RUNS=5
TARGET=""

usage() {
  echo "Usage: scripts/benchmark-scanner.sh [--runs N] <directory>"
  echo
  echo "Runs the same complete fast scan repeatedly and reports raw JSONL plus"
  echo "median/p95 engine latency and peak resident memory. System Data and"
  echo "health probes are excluded so every run has the same storage scope."
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs)
      [[ $# -ge 2 ]] || { usage >&2; exit 2; }
      RUNS="$2"
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

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/macclean-benchmark.XXXXXX")"
STATE_DIR="$WORK_DIR/state"
RESULTS="$WORK_DIR/results.jsonl"
cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

TARGET="$(cd "$TARGET" && pwd -P)"
mkdir -p "$STATE_DIR"

TIME_FLAGS=(-lp)
TIME_PROBE="$WORK_DIR/time-probe.txt"
if ! /usr/bin/time "${TIME_FLAGS[@]}" true >/dev/null 2>"$TIME_PROBE"; then
  # Restricted shells can deny the sysctl used by macOS `time -l`. Latency is
  # still measurable there; run this script normally to include peak RSS.
  TIME_FLAGS=(-p)
fi

for ((run = 1; run <= RUNS; run++)); do
  OUTPUT="$WORK_DIR/scan-$run.json"
  TIMING="$WORK_DIR/time-$run.txt"
  /usr/bin/time "${TIME_FLAGS[@]}" env MACCLEAN_STATE_DIR="$STATE_DIR" "$BACKEND" \
    app-scan "$TARGET" --depth 1 --limit 100 --no-system-data --no-health \
    >"$OUTPUT" 2>"$TIMING"

  ENGINE_MS="$(jq -er '.root_scan.elapsed_ms' "$OUTPUT")"
  ENTRIES="$(jq -er '.root_scan.metrics.entries_seen // 0' "$OUTPUT")"
  PARTIAL="$(jq -r '.root_scan.partial' "$OUTPUT")"
  REAL_SECONDS="$(awk '$1 == "real" { print $2; exit } $2 == "real" { print $1; exit }' "$TIMING")"
  PEAK_RSS="$(awk '/maximum resident set size/ { print $1; exit }' "$TIMING")"
  [[ -n "$REAL_SECONDS" ]] || {
    echo "Unable to parse /usr/bin/time output:" >&2
    sed -n '1,40p' "$TIMING" >&2
    exit 1
  }
  PEAK_RSS="${PEAK_RSS:-null}"

  jq -cn \
    --argjson run "$run" \
    --argjson engine_ms "$ENGINE_MS" \
    --argjson real_seconds "$REAL_SECONDS" \
    --argjson peak_rss_bytes "$PEAK_RSS" \
    --argjson entries_seen "$ENTRIES" \
    --argjson partial "$PARTIAL" \
    '{run: $run, engine_ms: $engine_ms, real_seconds: $real_seconds,
      peak_rss_bytes: $peak_rss_bytes, entries_seen: $entries_seen,
      partial: $partial}' | tee -a "$RESULTS"
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
' "$RESULTS"
