# macclean

> Mac system maintenance CLI — clean, analyze, secure, monitor.

40+ commands covering every category of Mac waste: caches, dev artifacts, Docker, Xcode, ghost app files, Time Machine snapshots, ZSH history, duplicate files, and more. Plus security checks, network inspection, and live system monitoring.

```
pip install macclean   # coming soon
pipx install .         # from source now
```

---

## Quick start

```bash
macclean               # interactive menu — pick what to run
macclean health        # one-page system snapshot
macclean analyze       # disk usage by directory
macclean all           # run every cleaner, confirm each step
macclean all --profile light   # quick daily clean
macclean all --profile dev     # developer-focused clean
```

---

## Commands

### Cleaning

| Command | What it removes |
|---|---|
| `macclean trash` | Trash across all volumes |
| `macclean system` | User + system caches, logs, tmp |
| `macclean browser` | Safari, Chrome, Firefox, Brave caches |
| `macclean xcode` | DerivedData, simulators, device support |
| `macclean docker` | Unused images, volumes, containers, build cache |
| `macclean gradle` | Gradle build cache |
| `macclean cargo` | Rust registry cache |
| `macclean node` | npm, yarn, pnpm caches |
| `macclean brew` | Homebrew download cache + autoremove |
| `macclean stremio` | Stremio video stream cache |
| `macclean apps` | Ghost files from uninstalled apps |
| `macclean zsh` | ZSH history duplicates + completion cache |
| `macclean timemachine` | Local APFS snapshots (frees "System Data") |
| `macclean crash-reports` | Crash logs and diagnostic reports |
| `macclean projects` | `node_modules`, `.venv`, `build/`, `dist/` |
| `macclean installers` | `.dmg`, `.pkg`, `.zip` in Downloads/Desktop |
| `macclean ios-backups` | iPhone/iPad local backups |
| `macclean pip` | Python pip download cache |
| `macclean maven` | Maven local repository |
| `macclean go` | Go module cache |
| `macclean fonts` | Duplicate fonts in ~/Library/Fonts |
| `macclean memory` | Flush inactive memory (`sudo purge`) |
| `macclean quicklook` | Rebuild QuickLook server and cache |
| `macclean spotlight` | Reindex Spotlight |
| `macclean python` | Unused pyenv Python versions |

### Analysis

| Command | What it shows |
|---|---|
| `macclean health` | CPU, memory, disk, battery, security at a glance |
| `macclean analyze` | Disk usage breakdown by major directory |
| `macclean status` | Live-refresh disk stats |
| `macclean largest` | Biggest files on disk (`--min 500` for ≥500 MB) |
| `macclean dupes` | Duplicate files by content hash |
| `macclean outdated` | Outdated brew/pip/npm packages |

### Security & Privacy

| Command | What it shows |
|---|---|
| `macclean security` | FileVault, Firewall, SIP, Gatekeeper status |
| `macclean privacy` | App permissions (camera, mic, screen recording) |
| `macclean ports` | Open listening ports by process |
| `macclean connections` | Active network connections by process |

### System & Apps

| Command | What it does |
|---|---|
| `macclean uninstall <App>` | Remove app + all 16 associated Library locations |
| `macclean agents` | List LaunchAgents/Daemons, flag broken ones |
| `macclean login-items` | Show startup apps |
| `macclean update` | Upgrade brew + pip + npm packages |
| `macclean wifi` | Wi-Fi signal, channel, DNS |
| `macclean log` | History of everything cleaned |
| `macclean touchid` | Enable Touch ID for sudo |

---

## Global flags

```bash
macclean --dry-run all        # preview without deleting
macclean --yes all            # skip all confirmations
macclean all --profile light  # light, dev, or deep profile
macclean all --select         # interactive checklist before running
macclean-complete             # install tab completion (zsh/bash)
```

---

## Config (`~/.maccleanrc`)

```toml
[defaults]
dry_run = false

[profiles.light]
cleaners = ["trash", "browser", "crash_reports", "zsh"]

[profiles.dev]
cleaners = ["brew", "docker", "gradle", "cargo", "node", "xcode", "zsh"]
```

---

## Install

```bash
# From source (now)
git clone https://github.com/Ferrisama/macclean
cd macclean
pipx install .

# Homebrew tap (after PyPI publish)
brew tap Ferrisama/macclean
brew install macclean
```

---

## License

MIT
