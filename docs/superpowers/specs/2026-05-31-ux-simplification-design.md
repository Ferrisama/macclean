# macclean UX Simplification Design

**Date:** 2026-05-31  
**Status:** Approved

---

## Problem

macclean has 40+ commands. The current interactive menu (`macclean` with no args) dumps all 25 cleaners as a flat uncategorized checkbox list. New and returning users have no clear starting point and no sense of what's safe vs thorough.

---

## Goal

Reduce cognitive load at the entry point without removing any existing commands. A user should be able to open macclean and act in under 5 seconds without reading docs.

---

## Design

### 1. New Interactive Menu (two-level navigation)

`macclean` (no args) opens a **single-select** menu — not a checkbox. Two groups separated by a divider:

```
  ⚡ Quick Clean   — trash, browser, crash reports (safe, ~2 min)
  🔧 Dev Clean     — brew, docker, node/pip/cargo, xcode, projects
  🧹 Deep Clean    — everything, confirm each step
  ──────────────────────────────────────────────────────────────
  → Clean
  → Analyze
  → Security
  → Manage
```

- Selecting a **preset** runs its bundle immediately (no second screen).
- Selecting a **category** opens a second screen — a checkbox of that category's commands only. Runs selected commands when confirmed.

### 2. Category Mapping

| Category | Module names |
|---|---|
| **Clean** | trash, system, browser, docker, brew, xcode, node, pip_cache, cargo, gradle, maven, go_cache, zsh, stremio, timemachine, crash_reports, ios_backups, fonts, memory, quicklook, spotlight, python_versions, projects, installers |
| **Analyze** | health, analyze, status, largest, dupes, outdated, wifi |
| **Security** | security, privacy, ports, connections, agents, login_items |
| **Manage** | uninstall, update, quit_apps, touchid |

### 3. Smart Preset Bundles

| Preset | Commands | Intent |
|---|---|---|
| **Quick Clean** | trash, browser, crash_reports | Safe daily clean, no dev tools, ~2 min |
| **Dev Clean** | brew, docker, node, pip_cache, cargo, gradle, maven, go_cache, xcode, projects, zsh | Developer cache flush |
| **Deep Clean** | all cleaners, confirm each step | Full clean, same as `macclean all` |

### 4. New Top-level CLI Commands

Three new commands added to `_TOOLS`:

```bash
macclean quick   # runs Quick Clean preset
macclean dev     # runs Dev Clean preset
macclean deep    # runs all cleaners (alias for macclean all)
```

These are usable without ever opening the interactive menu — good for scripts and muscle memory.

---

## Implementation Scope

### Files to change

- `macclean/cli.py` — add `_CATEGORIES` dict, `_PRESETS` dict, rewrite `_interactive_menu()`, add 3 new commands
- No other files change — all existing commands, cleaners, and tools remain untouched

### `_CATEGORIES` structure (in cli.py)

```python
_CATEGORIES = {
    "clean":    ("Clean",    ["trash", "system", "browser", ...]),
    "analyze":  ("Analyze",  ["health", "analyze", "status", ...]),
    "security": ("Security", ["security", "privacy", "ports", ...]),
    "manage":   ("Manage",   ["uninstall", "update", "quit_apps", "touchid"]),
}

_PRESETS = {
    "quick": ("⚡ Quick Clean", "trash, browser, crash reports — safe, ~2 min",
              ["trash", "browser", "crash_reports"]),
    "dev":   ("🔧 Dev Clean",   "brew, docker, node/pip/cargo, xcode, projects",
              ["brew", "docker", "node", "pip_cache", "cargo", "gradle", "maven", "go_cache", "xcode", "projects", "zsh"]),
    "deep":  ("🧹 Deep Clean",  "everything, confirm each step",
              None),  # None = all cleaners
}
```

### Menu flow

```
_interactive_menu()
  → questionary.select() with presets + category entries
  → if preset selected: run preset modules directly (all are cleaners)
  → if category selected: questionary.checkbox() with category's commands
                          → run selected commands
```

### Cleaner vs Tool distinction

Commands in `_CLEANERS` use the `analyze() + clean()` interface and are run via `_run_cleaner()`.  
Commands in `_TOOLS` are Click commands with their own UI — invoked directly via `ctx.invoke(module.cmd)`.

Category entries must declare which type each command is so the runner uses the right path:

```python
_CATEGORIES = {
    "clean":    ("Clean",    [("trash", "cleaner"), ("browser", "cleaner"), ...]),
    "analyze":  ("Analyze",  [("health", "tool"), ("analyze", "tool"), ...]),
    "security": ("Security", [("security", "tool"), ("privacy", "tool"), ...]),
    "manage":   ("Manage",   [("uninstall", "tool"), ("quit_apps", "tool"), ...]),
}
```

Presets only include cleaners, so they always use `_run_cleaner()`.

---

## Non-goals

- No changes to individual command behavior
- No removal of any existing commands
- No new dependencies
- `macclean all`, `macclean --profile`, `macclean all --select` all continue to work unchanged
