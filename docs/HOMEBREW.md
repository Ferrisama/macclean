# Installing macclean

## Option 1: Homebrew tap (recommended once a release is tagged)

```bash
brew tap Ferrisama/macclean
brew install macclean
```

## Option 2: From source

```bash
git clone https://github.com/Ferrisama/macclean
cd macclean
cargo build --release
cp target/release/macclean /usr/local/bin/
```

## Option 3: Prebuilt binary

Download the `.tar.gz` for your Mac's architecture (`aarch64-apple-darwin` for
Apple Silicon, `x86_64-apple-darwin` for Intel) from the release's GitHub
Releases page, then:

```bash
tar -xzf macclean-*-apple-darwin.tar.gz
sudo mv macclean /usr/local/bin/
```

---

## Maintainer: publishing a new release

1. Bump `version` in `Cargo.toml`.
2. Commit and tag: `git tag v0.4.0 && git push --tags`.
3. The `Release` GitHub Actions workflow builds `aarch64-apple-darwin` and
   `x86_64-apple-darwin` binaries and attaches them to a GitHub Release
   automatically on tag push.
4. Update `Formula/macclean.rb`:
   - Update `url` to the new tag's source tarball
     (`https://github.com/Ferrisama/macclean/archive/refs/tags/vX.Y.Z.tar.gz`)
   - Update `sha256`: `curl -sL <url> | shasum -a 256`

## Setting up the Homebrew tap (one-time)

1. Create a GitHub repo named `homebrew-macclean`.
2. Copy `Formula/macclean.rb` into it.
3. Users can then run `brew tap Ferrisama/macclean && brew install macclean`.
