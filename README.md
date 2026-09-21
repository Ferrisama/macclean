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
macclean doctor        # verify permissions, tools, Trash, and release readiness
macclean system-data   # explain why System Data/storage is large
macclean system-data --json
macclean system-data --path ~/Library --depth 2 --limit 8 --deep
macclean scan ~/Library --json --depth 1
macclean app-scan ~ --depth 2 --limit 20 --no-health
macclean history       # cleanup sessions, receipt paths, restore availability
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

Running `macclean` with no arguments opens a full-screen dashboard with five tabs. `Tab`/`Shift+Tab` (or click) switches between them; `Ctrl+C` quits from anywhere.

| Tab | What it does |
|---|---|
| **Dashboard** | Live disk/memory gauges, CPU/load/battery, FileVault/Firewall/SIP status, biggest space users in home. `r` to refresh. |
| **System Data** | Fast visual bucket map for System Data with category shares, sizes, partial-scan warnings, and cleanup actions. `r` to rescan. |
| **Clean** | The highest-value cache categories with sizes, checkboxes, and `1`/`2`/`3` for Quick/Dev/Deep presets. `Space` toggles, `Enter` runs the selected ones. |
| **Uninstall** | Search installed apps, review every associated file (settings, caches, containers) before confirming. |
| **Explore** | Bounded drill-down disk usage browser with parent percentages, partial markers, and Trash-backed delete. `Enter` opens, `Backspace`/`u` goes up, `d` trashes. |

Analyzing, scanning, and building an uninstall plan all run in the background, so switching tabs or typing is never blocked waiting on a scan to finish.

`macclean` shows each cleanup item's type, risk, and reason before removal. User-data-adjacent cleanups such as app leftovers, project artifacts, installers, iOS backups, duplicate fonts, Xcode data, and ZSH completion files move items to the macOS Trash and are recorded in cleanup history with JSON receipts under `~/Library/Application Support/macclean/receipts/`. On macOS, every new Trash-backed record stores the exact destination returned by Foundation, so Restore never guesses by filename and refuses collisions. Cache/log cleaners delete immediately and permanently where moving cache contents to Trash would not free space until Trash is emptied.

## Native macOS app

The repository also includes a native SwiftUI beta focused on trustworthy
storage exploration and recoverable cleanup. It currently provides:

- Fast and Deep scans with streamed progress, cancellation, cached startup,
  partial-coverage reporting, and safety classification.
- A responsive glass-style storage map with proportional rounded tiles,
  hover/selection feedback, Finder actions, and compact/large-window layouts.
- Reversible directory navigation with Back, Forward, Parent, scan-root,
  clickable breadcrumb, and failed-navigation rollback controls.
- Cleanup recipes and scan candidates that are reviewed before selected paths
  move to Trash.
- Content-verified duplicate groups with explicit keeper selection, immediate
  content/identity revalidation, Trash receipts, History, and restore.
- Exact recorded Trash destinations, per-path outcomes, collision-safe restore,
  and warnings that distinguish moved bytes from reclaimed disk space.
- Full Disk Access status and System Settings guidance without requesting access
  during normal scans.
- A searchable installed-app list and read-only standard/deep uninstall-plan
  preview. Destructive uninstall execution remains a follow-up beta slice.

Build a self-contained bundle with:

```bash
./scripts/build-swiftui-app.sh
open dist/MacClean.app
```

Development builds use `MacClean Local Development` when that identity is
installed; otherwise they fall back to ad-hoc signing. Because an ad-hoc app's
identity changes whenever its executable changes, macOS may request Full Disk
Access again after each rebuild. Install the repository's trusted local
development identity once, then rebuild and grant access once:

```bash
./scripts/setup-local-signing.sh
./scripts/build-swiftui-app.sh
```

Set `MACCLEAN_SIGN_IDENTITY` to an Apple Development or Developer ID Application
identity to override the local identity. The local certificate is for this Mac
only and is not suitable for distributing the app to other users.

Fast Scan sizes the selected folder's immediate children for a useful overview. Deep Scan explicitly traverses the requested depth. Both report incomplete coverage instead of presenting partial totals as complete.

### Beta verification

```bash
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
(cd MacCleanApp && swift test)
./scripts/smoke-swiftui-app.sh
./scripts/verify-beta-upgrade.sh
./scripts/benchmark-scanner.sh --runs 5 /path/to/same-scope-fixture
```

The packaged smoke test uses temporary fixtures to exercise scan, reviewed
cleanup, duplicate keeper preservation, Trash receipts, History, and restore.
The upgrade verifier rebuilds the signed app and checks that its designated
requirement and isolated persisted state remain compatible.

Current beta gaps are rendered SwiftUI automation across window sizes,
post-result background deep refinement, destructive uninstall execution through
the reviewed recovery pipeline, Intel/universal bundle testing, and public
Developer ID signing/notarization.

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
| `macclean doctor` | Permission/tool readiness checks for Full Disk Access, Trash, Homebrew, Docker, Xcode, Time Machine, and signing |
| `macclean scan <path>` | Chart-ready folder tree scan (`--json`, `--depth`, `--limit`, `--deep`) |
| `macclean app-scan <path>` | GUI-ready JSON contract with folder tree, largest items, safety totals, cleanup candidates, System Data, and optional health snapshot |
| `macclean largest` | Biggest files on disk (`--min-mb 500`) |
| `macclean dupes` | Duplicate files by content hash (`--min 10`) |
| `macclean system-data` | Categorized System Data estimate and storage tree (`--json`, `--path`, `--depth`, `--limit`, `--deep`) |
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
| `macclean history` | Show recent Trash-backed cleanup sessions, receipt paths, and restore availability |
| `macclean restore [session]` | Restore the latest or named Trash-backed cleanup session with per-item restore results |
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
macclean scan ~/Library --json --depth 1 --limit 12
macclean system-data --json
macclean -n plan apply downloads
macclean profile create-project dev-safe --path ~/code --only node,python --exclude ~/code/client --older-than-days 30
```

Storage scans run in fast mode by default as a top-level overview and may mark results as `partial`. Add `--deep` for a slower, depth-aware traversal. Latest chart-ready scan JSON is cached under `~/Library/Application Support/macclean/scans/`.

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

Maintainer release, signing, notarization, and Homebrew formula steps are in
[`docs/RELEASE.md`](docs/RELEASE.md).

---

## License

MIT




.
