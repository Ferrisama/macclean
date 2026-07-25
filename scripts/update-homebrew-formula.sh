#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)}"
TAG="v${VERSION#v}"
URL="https://github.com/Ferrisama/macclean/archive/refs/tags/$TAG.tar.gz"
FORMULA="$ROOT/Formula/macclean.rb"

if [[ -z "$VERSION" ]]; then
  echo "Usage: $0 <version-or-tag>" >&2
  exit 1
fi

echo "Fetching source tarball checksum for $TAG"
sha="$(curl -fsSL "$URL" | shasum -a 256 | awk '{print $1}')"

tmp="$(mktemp)"
sed \
  -e "s#url \"https://github.com/Ferrisama/macclean/archive/refs/tags/v[^\"]*\.tar\.gz\"#url \"$URL\"#" \
  -e "s#sha256 \"[^\"]*\"#sha256 \"$sha\"#" \
  "$FORMULA" > "$tmp"
mv "$tmp" "$FORMULA"

echo "Updated $FORMULA"
echo "  tag: $TAG"
echo "  sha256: $sha"
