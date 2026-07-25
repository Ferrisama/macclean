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
macclean               # interactive dashboard (TUI)
macclean ask "why is system data huge"
macclean ask "clean old node projects"
macclean quick         # trash + browser + crash reports
macclean dev           # brew + docker + node/pip/cargo + xcode + projects + zsh
macclean deep          # everything
macclean health        # one-page system snapshot
macclean system-data   # explain why System Data/storage is large
macclean system-data --path ~/Library --depth 2 --limit 8
macclean history       # recent Trash-backed cleanup sessions
macclean restore       # restore the latest Trash-backed cleanup session
macclean projects --path ~/code --only node --older-than-days 30
macclean uninstall --path /Applications/Foo.app --deep
macclean largest --path ~/Downloads --trash
macclean dupes --path ~/Pictures --trash --keep newest
macclean plan create downloads ~/Downloads/old.dmg ~/Downloads/old.pkg
macclean plan apply downloads --yes
macclean profile create-project dev-safe --path ~/code --only node,rust --older-than-days 30
macclean profile run dev-safe --yes
```

Every command above also works non-interactively for scripting -- the dashboard is purely what launches when you run `macclean` with no subcommand.

---

## Dashboard

Running `macclean` with no arguments opens a full-screen dashboard with four tabs. `Tab`/`Shift+Tab` (or click) switches between them; `Ctrl+C` quits from anywhere.

| Tab | What it does |
|---|---|
| **Dashboard** | Live disk/memory gauges, CPU/load/battery, FileVault/Firewall/SIP status, biggest space users in home. `r` to refresh. |
| **Clean** | The highest-value cache categories with sizes, checkboxes, and `1`/`2`/`3` for Quick/Dev/Deep presets. `Space` toggles, `Enter` runs the selected ones. |
| **Uninstall** | Search installed apps, review every associated file (settings, caches, containers) before confirming. |
| **Explore** | Drill-down disk usage browser -- `Enter` opens a folder, `Backspace`/`u` goes up, `d` trashes the selected item on the spot. |

Analyzing, scanning, and building an uninstall plan all run in the background, so switching tabs or typing is never blocked waiting on a scan to finish.

`macclean` shows each cleanup item's type, risk, and reason before removal. User-data-adjacent cleanups such as app leftovers, project artifacts, installers, iOS backups, duplicate fonts, Xcode data, and ZSH completion files move items to the macOS Trash and are recorded in cleanup history. Cache/log cleaners delete immediately and permanently where moving cache contents to Trash would not free space until Trash is emptied.

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
| `macclean android` | Android SDK/build caches |
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
| `macclean system-data` | Categorized System Data estimate and storage tree (`--path`, `--depth`, `--limit`) |
| `macclean ask "<request>"` | Offline natural-language command router; shows the planned command before running |
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
| `macclean uninstall <App>` | Move app + associated Library locations to Trash |
| `macclean uninstall --path /Applications/Foo.app --deep` | Deep app uninstall scan including helpers, receipts, group containers, launch items |
| `macclean update` | Upgrade brew + pip + npm packages |
| `macclean quit-apps` | Quit configured apps before sleep/travel |
| `macclean history` | Show recent Trash-backed cleanup records |
| `macclean restore [session]` | Restore the latest or named Trash-backed cleanup session when items still exist in Trash |
| `macclean plan create <name> <paths...>` | Save exact preselected paths as a fast reusable Trash-backed plan |
| `macclean plan apply <name>` | Validate and apply a saved plan |
| `macclean profile create-project <name>` | Save reusable project-cleanup rules |
| `macclean profile run <name>` | Re-scan and run a saved project cleanup profile |

---

## Global flags

```bash
macclean --dry-run trash    # preview without deleting
macclean --yes deep         # skip all confirmations
macclean -n projects --path ~/code --only node,python --exclude ~/code/client --older-than-days 30
macclean -n uninstall --bundle-id com.example.App --deep
macclean dupes --path ~/Downloads --trash --keep shortest
macclean -n plan apply downloads
macclean profile create-project dev-safe --path ~/code --only node,python --exclude ~/code/client --older-than-days 30
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
