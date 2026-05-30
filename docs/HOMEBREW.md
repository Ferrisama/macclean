# Installing macclean

## Option 1: pipx (recommended — always latest)

```bash
pipx install macclean
```

## Option 2: Homebrew tap

```bash
brew tap Ferrisama/macclean
brew install macclean
```

## Option 3: From source

```bash
git clone https://github.com/Ferrisama/macclean
cd macclean
pipx install .
```

---

## Maintainer: publishing a new release

1. Bump `version` in `pyproject.toml`
2. Commit and tag: `git tag v0.2.0 && git push --tags`
3. GitHub Actions publishes to PyPI automatically on tag push
4. Update `Formula/macclean.rb` in `homebrew-macclean` repo:
   - Update `url` to new PyPI tarball URL
   - Update `sha256`: `curl -sL <url> | shasum -a 256`

## Setting up the Homebrew tap (one-time)

1. Create a GitHub repo named `homebrew-macclean`
2. Copy `Formula/macclean.rb` into it
3. Users can then run `brew tap Ferrisama/macclean && brew install macclean`
