# Releasing macclean

This project ships a single Rust binary. The release path is:

1. validate locally
2. build reproducible tarballs
3. optionally sign/notarize the macOS binary
4. tag and publish GitHub release assets
5. update the Homebrew formula

## Local validation

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

## Build release artifacts

Build for the current host target:

```bash
scripts/build-release.sh
```

Build both macOS targets when the toolchains are installed:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
scripts/build-release.sh aarch64-apple-darwin x86_64-apple-darwin
```

Artifacts are written to `dist/`:

```text
macclean-vX.Y.Z-aarch64-apple-darwin.tar.gz
macclean-vX.Y.Z-aarch64-apple-darwin.tar.gz.sha256
macclean-vX.Y.Z-x86_64-apple-darwin.tar.gz
macclean-vX.Y.Z-x86_64-apple-darwin.tar.gz.sha256
```

## Sign and notarize

The SwiftUI bundle build uses `MACCLEAN_SIGN_IDENTITY` when set, otherwise it
uses `MacClean Local Development` when that identity is installed. It signs the
embedded Rust backend first and the outer app last. Run
`scripts/setup-local-signing.sh` once to keep Full Disk Access attached to local
development rebuilds. That self-signed identity is strictly for local use and
cannot replace Developer ID signing or notarization.

Prerequisites:

- Apple Developer Program membership
- Developer ID Application certificate installed in Keychain
- app-specific password for notarization

Required environment:

```bash
export MACCLEAN_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)"
export APPLE_ID="you@example.com"
export APPLE_TEAM_ID="TEAMID"
export APPLE_APP_PASSWORD="xxxx-xxxx-xxxx-xxxx"
```

Run:

```bash
scripts/notarize-release.sh aarch64-apple-darwin
scripts/notarize-release.sh x86_64-apple-darwin
```

The script signs the CLI binary with hardened runtime, submits it through
`xcrun notarytool`, and writes the final tarball plus checksum to `dist/`.

## Publish GitHub release

1. Bump `version` in `Cargo.toml`.
2. Commit the release prep.
3. Tag:

```bash
git tag vX.Y.Z
git push origin master --tags
```

The `Release` workflow builds macOS arm64 and Intel tarballs, generates
`.sha256` files, and attaches them to the GitHub Release.

## Update Homebrew formula

After the tag exists on GitHub:

```bash
scripts/update-homebrew-formula.sh X.Y.Z
```

Then test locally:

```bash
brew install --build-from-source Formula/macclean.rb
macclean --help
brew test Formula/macclean.rb
```

Copy `Formula/macclean.rb` into the `homebrew-macclean` tap repository and push
that change.
