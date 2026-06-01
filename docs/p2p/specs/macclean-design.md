# macclean — Mac System Maintenance CLI

**Date:** 2026-05-30  
**Status:** Approved  
**Language:** Python 3.10+  
**Install:** `pipx install .` → global `macclean` command

---

## Overview

`macclean` is a Python CLI for on-demand macOS system maintenance. It scans, previews, and cleans across 20 domains — from Homebrew and Docker to Xcode derived data and APFS Time Machine snapshots. Every destructive operation shows a confirmation table first. Requires `sudo macclean <cmd>` upfront; the tool exits cleanly if elevation is missing for commands that need it.

---

## Architecture

### Project Layout

```
cleanup/
├── macclean/
│   ├── cli.py                    # click entry point, registers all subcommands
│   ├── core/
│   │   ├── utils.py              # sudo check, size formatting, confirm prompt
│   │   └── disk.py               # disk usage analysis helpers
│   └── cleaners/
│       ├── brew.py
│       ├── docker.py
│       ├── python_versions.py
│       ├── system.py
│       ├── quicklook.py
│       ├── zsh.py
│       ├── apps.py
│       ├── xcode.py
│       ├── timemachine.py
│       ├── ios_backups.py
│       ├── browser.py
│       ├── fonts.py
│       ├── trash.py
│       ├── crash_reports.py
│       ├── node.py
│       ├── pip_cache.py
│       ├── cargo.py
│       ├── gradle.py
│       ├── maven.py
│       └── go_cache.py
├── pyproject.toml
└── README.md
```

### Cleaner Contract

Every cleaner module exposes two functions:

```python
def analyze() -> AnalysisResult:
    """Scan and return what was found with sizes. No side effects."""

def clean(result: AnalysisResult, dry_run: bool = False) -> None:
    """Print confirmation table, prompt user, then delete if confirmed."""
```

`AnalysisResult` is a dataclass: `items: list[CleanItem]`, `total_bytes: int`. `CleanItem` holds `label`, `path`, `size_bytes`, `removable: bool`.

### Core Utilities (`core/utils.py`)

- `require_sudo()` — checks `os.geteuid() == 0`; prints re-run hint and calls `sys.exit(1)` if not root
- `format_size(bytes)` → human-readable string (KB / MB / GB)
- `confirm(prompt, default=False)` → `bool` — rich-styled `[y/N]` prompt
- `run_cmd(args, capture=True)` → `(stdout, returncode)` — thin subprocess wrapper

---

## Subcommands

### Analysis

| Command | Sudo needed | Description |
|---|---|---|
| `macclean analyze` | no | Disk breakdown by major directories; highlights biggest consumers |
| `macclean status` | no | Live refreshing view of disk free space and top space consumers (updates every 5s via `rich.live`) |

### System

| Command | Sudo needed | Description |
|---|---|---|
| `macclean system` | yes | Clears `~/Library/Caches`, `/Library/Caches`, `~/Library/Logs`, `/private/var/log`, `/private/tmp`; runs `sudo periodic daily weekly monthly`; flushes DNS via `dscacheutil -flushcache` |
| `macclean timemachine` | yes | Lists APFS local snapshots via `tmutil listlocalsnapshots /`; deletes selected via `tmutil deletelocalsnapshots <date>` |
| `macclean spotlight` | yes | Reindexes Spotlight via `sudo mdutil -E /` (maintenance op, no deletion) |
| `macclean memory` | yes | Flushes inactive memory via `sudo purge` (maintenance op) |
| `macclean quicklook` | no | Kills QuickLook server (`qlmanage -r`), rebuilds cache (`qlmanage -r cache`), removes stale `com.apple.QuickLookDaemon` cache dirs |
| `macclean trash` | no | Empties Trash across all volumes (`~/.Trash` + `/Volumes/*/.Trashes/<uid>`) |
| `macclean crash-reports` | yes | Removes `~/Library/Logs/DiagnosticReports` and `/Library/Logs/DiagnosticReports` |

### Apps & Developer

| Command | Sudo needed | Description |
|---|---|---|
| `macclean apps` | no | Finds orphaned bundle-ID folders in `~/Library/Application Support`, `~/Library/Preferences`, `~/Library/Containers` with no matching `.app` in `/Applications` or `~/Applications` |
| `macclean xcode` | no | Clears `~/Library/Developer/Xcode/DerivedData`, old Archives, per-version iOS/watchOS/tvOS Device Support files, unused CoreSimulator device sets |
| `macclean ios-backups` | no | Lists iPhone/iPad backups in `~/Library/Application Support/MobileSync/Backup`; shows device name, date, size; prompts per backup |
| `macclean browser` | no | Clears cache dirs for Safari (`~/Library/Caches/com.apple.Safari`), Chrome (`~/Library/Caches/Google/Chrome`), Firefox (`~/Library/Caches/Firefox`) |
| `macclean fonts` | no | Scans `/Library/Fonts`, `~/Library/Fonts`, `/System/Library/Fonts`; reports duplicate font filenames across locations; does not auto-delete system fonts |

### Package Managers

| Command | Sudo needed | Description |
|---|---|---|
| `macclean brew` | no | `brew cleanup --prune=all`, `brew autoremove`; clears `$(brew --cache)` |
| `macclean docker` | no | `docker image prune -af`, `docker container prune -f`, `docker volume prune -f`, `docker builder prune -af` |
| `macclean python` | no | Lists pyenv versions; cross-checks `~/.pyenv/versions/*/envs/` for dependent virtualenvs; offers `pyenv uninstall` for unused versions |
| `macclean node` | no | Clears `~/.npm`, `~/.yarn/cache`, `~/.cache/pnpm` |
| `macclean pip` | no | Runs `pip cache purge` for each pyenv Python version found |
| `macclean cargo` | no | Clears `~/.cargo/registry/cache` and `~/.cargo/registry/src` (source tarballs; index is kept) |
| `macclean gradle` | no | Clears `~/.gradle/caches` |
| `macclean maven` | no | Clears `~/.m2/repository` |
| `macclean go` | no | Runs `go clean -modcache` if `go` is on PATH |

### Shell

| Command | Sudo needed | Description |
|---|---|---|
| `macclean zsh` | no | Deduplicates `~/.zsh_history` preserving order; removes `~/.zcompcache/*` and `~/.zcompdump*` |

### Composite

| Command | Description |
|---|---|
| `macclean all` | Runs every cleaner's full analyze→confirm→clean cycle in sequence. Order: analysis → system → apps → package managers → shell. Each cleaner confirms independently; skipping one does not abort the rest. |

---

## Global Flags

Applied to every subcommand via click's `@click.pass_context`:

| Flag | Effect |
|---|---|
| `--dry-run` | Analyze only; print table but skip confirmation prompt and deletion |
| `--yes` / `-y` | Skip confirmation prompt; delete immediately after analysis |
| `--json` | Output analysis result as JSON (for scripting) |

---

## Display

Uses `rich` throughout:

- **Panel** header per cleaner with name and emoji icon
- **Table** showing items, counts, and sizes before confirmation
- **Color coding:** green = safe/small, yellow = review, red = large (>500MB)
- **Progress bar** during slow operations (xcode DerivedData walk, ios-backups sizing)
- **Live display** for `macclean status` (refreshes every 5 seconds)

---

## Installation

```toml
# pyproject.toml
[project]
name = "macclean"
version = "0.1.0"
requires-python = ">=3.10"
dependencies = ["click>=8.0", "rich>=13.0", "psutil>=5.9"]

[project.scripts]
macclean = "macclean.cli:main"
```

Install: `pipx install .`  
Use: `sudo macclean <cmd>`

---

## Error Handling

- Missing tool (e.g. `docker` not running, `go` not on PATH): cleaner prints a yellow warning and skips gracefully — does not abort `macclean all`
- Permission errors on individual files during walk: logged and skipped, not fatal
- Sudo missing when required: print re-run hint, exit 1 immediately
- All subprocess calls use `timeout=60` to prevent hangs

---

## Out of Scope (v1)

- Scheduling / launchd automation
- GUI or TUI interface
- Language pack removal (Monolingual-style) — risks breaking apps
- Universal binary slimming — too risky without testing
- Sleep image / swapfile manipulation — OS-managed, unsafe to touch
