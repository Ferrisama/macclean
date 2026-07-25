#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
DIST="$ROOT/dist"

if [[ -z "$VERSION" ]]; then
  echo "Could not read version from Cargo.toml" >&2
  exit 1
fi

mkdir -p "$DIST"

targets=("$@")
if [[ ${#targets[@]} -eq 0 ]]; then
  targets=("$(rustc -vV | sed -n 's/^host: //p')")
fi

for target in "${targets[@]}"; do
  echo "==> Building macclean v$VERSION for $target"
  cargo build --release --target "$target" --manifest-path "$ROOT/Cargo.toml"

  artifact_dir="$DIST/macclean-v$VERSION-$target"
  rm -rf "$artifact_dir"
  mkdir -p "$artifact_dir"
  cp "$ROOT/target/$target/release/macclean" "$artifact_dir/macclean"
  cp "$ROOT/README.md" "$artifact_dir/README.md"
  cp "$ROOT/LICENSE" "$artifact_dir/LICENSE"

  if command -v strip >/dev/null 2>&1; then
    strip "$artifact_dir/macclean" 2>/dev/null || true
  fi

  tarball="$DIST/macclean-v$VERSION-$target.tar.gz"
  tar -C "$DIST" -czf "$tarball" "macclean-v$VERSION-$target"
  shasum -a 256 "$tarball" > "$tarball.sha256"
  echo "Wrote $tarball"
  echo "Wrote $tarball.sha256"
done
