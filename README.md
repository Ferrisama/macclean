# macclean

> Mac system maintenance CLI — clean, analyze, secure, monitor.

40+ commands covering every category of Mac waste: caches, dev artifacts, Docker, Xcode, ghost app files, Time Machine snapshots, ZSH history, duplicate files, and more. Plus security checks, network inspection, and system monitoring.

Single self-contained binary. No Python, no runtime, no dependencies.

```bash
cargo install --git https://github.com/Ferrisama/macclean
```

---

## Quick start

```bash
macclean               # interactive menu
macclean quick         # trash + browser + crash reports
macclean dev           # brew + docker + node/pip/cargo + xcode + projects + zsh
macclean deep          # everything
macclean health        # one-page system snapshot
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
| `macclean brew` | Homebrew download cache + autoremove |
| `macclean node` | npm, yarn, pnpm caches |
| `macclean pip` | Python pip download cache |
| `macclean cargo` | Rust registry cache |
| `macclean gradle` | Gradle build cache |
| `macclean maven` | Maven local repository |
| `macclean go` | Go module cache |
| `macclean python` | Unused pyenv Python versions |
| `macclean zsh` | ZSH history duplicates + completion cache |
| `macclean stremio` | Stremio video stream cache |
| `macclean apps` | Ghost files from uninstalled apps |
| `macclean timemachine` | Local APFS snapshots (frees "System Data") |
| `macclean crash-reports` | Crash logs and diagnostic reports |
| `macclean projects` | `node_modules`, `.venv`, `build/`, `dist/` |
| `macclean installers` | `.dmg`, `.pkg`, `.zip` in Downloads/Desktop |
| `macclean ios-backups` | iPhone/iPad local backups |
| `macclean fonts` | Duplicate fonts in ~/Library/Fonts |
| `macclean memory` | Flush inactive memory (`sudo purge`) |
| `macclean quicklook` | Rebuild QuickLook server and cache |
| `macclean spotlight` | Reindex Spotlight |

### Analysis

| Command | What it shows |
|---|---|
| `macclean health` | CPU, memory, disk, battery, security at a glance |
| `macclean largest` | Biggest files on disk (`--min-mb 500`) |
| `macclean dupes` | Duplicate files by content hash (`--min 10`) |
| `macclean outdated` | Outdated brew/pip/npm packages |
| `macclean wifi` | Wi-Fi signal, channel, DNS |

### Security & Privacy

| Command | What it shows |
|---|---|
| `macclean security` | FileVault, Firewall, SIP, Gatekeeper status |
| `macclean privacy` | App permissions (camera, mic, screen recording) |
| `macclean ports` | Open listening ports by process |
| `macclean connections` | Active network connections by process |
| `macclean agents` | List LaunchAgents/Daemons, flag broken ones |
| `macclean login-items` | Show startup apps |

### System & Apps

| Command | What it does |
|---|---|
| `macclean uninstall <App>` | Remove app + all associated Library locations |
| `macclean update` | Upgrade brew + pip + npm packages |
| `macclean quit-apps` | Quit configured apps before sleep/travel |

---

## Global flags

```bash
macclean --dry-run trash    # preview without deleting
macclean --yes deep         # skip all confirmations
```

---

## Install

```bash
# From source
git clone https://github.com/Ferrisama/macclean
cd macclean
cargo build --release
cp target/release/macclean /usr/local/bin/

# Via Homebrew tap (after release tag)
brew tap Ferrisama/macclean
brew install macclean
```

---

## License

MIT
