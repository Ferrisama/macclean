# macclean UX Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat 25-item checkbox menu with a two-level menu (presets + category navigation) and add `quick`, `dev`, `deep` top-level commands.

**Architecture:** All changes live in `macclean/cli.py`. Add `_CATEGORIES` and `_PRESETS` dicts as the source of truth for groupings, rewrite `_interactive_menu()` into three focused helpers, and register three new preset commands.

**Tech Stack:** Python 3.10+, click, questionary, rich

---

## File Map

| File | Change |
|---|---|
| `macclean/cli.py` | Only file modified — add dicts, replace `_interactive_menu`, add 3 commands |

---

### Task 1: Add `_CATEGORIES` and `_PRESETS` dicts

**Files:**
- Modify: `macclean/cli.py` — insert after line 60 (closing `]` of `_TOOLS`)

- [ ] **Step 1: Insert lookup helpers and dicts after `_TOOLS`**

Open `macclean/cli.py`. After the closing `]` of `_TOOLS` (currently line 60), insert:

```python

# Lookup helpers built from the single-source-of-truth lists above
_CLEANER_BY_KEY = {k: m for k, _, m in _CLEANERS}
_TOOL_BY_KEY    = {k: (mod, attr) for k, mod, attr in _TOOLS}

# Category definitions: key → (display_label, [(cli_key, display_name, kind)])
# kind is "cleaner" (has analyze/clean interface) or "tool" (Click command with own UI)
_CATEGORIES: dict[str, tuple[str, list[tuple[str, str, str]]]] = {
    "clean": ("Clean", [
        ("trash",         "Trash",                "cleaner"),
        ("system",        "System Caches & Logs", "cleaner"),
        ("browser",       "Browser Caches",       "cleaner"),
        ("docker",        "Docker",               "cleaner"),
        ("brew",          "Homebrew",             "cleaner"),
        ("xcode",         "Xcode Data",           "cleaner"),
        ("node",          "Node.js Caches",       "cleaner"),
        ("pip",           "pip Cache",            "cleaner"),
        ("cargo",         "Cargo Cache",          "cleaner"),
        ("gradle",        "Gradle Cache",         "cleaner"),
        ("maven",         "Maven Repository",     "cleaner"),
        ("go",            "Go Module Cache",      "cleaner"),
        ("zsh",           "ZSH History",          "cleaner"),
        ("stremio",       "Stremio Cache",        "cleaner"),
        ("timemachine",   "Time Machine Snaps",   "cleaner"),
        ("crash-reports", "Crash Reports",        "cleaner"),
        ("ios-backups",   "iOS Backups",          "cleaner"),
        ("fonts",         "Duplicate Fonts",      "cleaner"),
        ("memory",        "Inactive Memory",      "cleaner"),
        ("quicklook",     "QuickLook Cache",      "cleaner"),
        ("spotlight",     "Spotlight Reindex",    "cleaner"),
        ("python",        "Python Versions",      "cleaner"),
        ("projects",      "Project Artifacts",    "cleaner"),
        ("installers",    "Installer Files",      "cleaner"),
    ]),
    "analyze": ("Analyze", [
        ("health",       "System Health",         "tool"),
        ("analyze",      "Disk Breakdown",        "tool"),
        ("status",       "Live Disk Stats",       "tool"),
        ("largest",      "Largest Files",         "tool"),
        ("dupes",        "Duplicate Files",       "tool"),
        ("outdated",     "Outdated Packages",     "tool"),
        ("wifi",         "Wi-Fi Info",            "tool"),
    ]),
    "security": ("Security", [
        ("security",     "Security Status",       "tool"),
        ("privacy",      "App Permissions",       "tool"),
        ("ports",        "Open Ports",            "tool"),
        ("connections",  "Active Connections",    "tool"),
        ("agents",       "Launch Agents",         "tool"),
        ("login-items",  "Login Items",           "tool"),
    ]),
    "manage": ("Manage", [
        ("uninstall",    "Uninstall App",         "tool"),
        ("update",       "Update Packages",       "tool"),
        ("quit-apps",    "Quit Apps",             "tool"),
    ]),
}

# Preset definitions: key → (label, description, [module_names] | None)
# module_names are entries in _CLEANER_BY_KEY (all presets run cleaners only)
# None means every cleaner in _CLEANERS order
_PRESETS: dict[str, tuple[str, str, list[str] | None]] = {
    "quick": (
        "⚡  Quick Clean",
        "trash, browser, crash reports — safe, ~2 min",
        ["trash", "browser", "crash_reports"],
    ),
    "dev": (
        "🔧  Dev Clean",
        "brew, docker, node/pip/cargo, xcode, projects, zsh",
        ["brew", "docker", "node", "pip_cache", "cargo", "gradle",
         "maven", "go_cache", "xcode", "projects", "zsh"],
    ),
    "deep": (
        "🧹  Deep Clean",
        "everything — confirm each step",
        None,
    ),
}
```

- [ ] **Step 2: Verify the file parses and dicts are accessible**

```bash
cd /Users/asmitghosh/Desktop/cleanup
python -c "
from macclean.cli import _CATEGORIES, _PRESETS, _CLEANER_BY_KEY, _TOOL_BY_KEY
print('categories:', len(_CATEGORIES))
print('presets:', len(_PRESETS))
print('cleaner keys:', len(_CLEANER_BY_KEY))
print('tool keys:', len(_TOOL_BY_KEY))
"
```

Expected:
```
categories: 4
presets: 3
cleaner keys: 25
tool keys: 15
```

- [ ] **Step 3: Commit**

```bash
cd /Users/asmitghosh/Desktop/cleanup
git add macclean/cli.py
git commit -m "feat: add _CATEGORIES, _PRESETS, and lookup dicts to cli"
```

---

### Task 2: Rewrite `_interactive_menu()` into three focused helpers

**Files:**
- Modify: `macclean/cli.py:95-160` (the `main` callback + `_interactive_menu`)

The old `_interactive_menu(dry_run, yes)` becomes three functions:
- `_interactive_menu(ctx, dry_run, yes)` — top-level select (presets + categories)
- `_run_preset_by_key(key, dry_run, yes)` — runs a preset's cleaners
- `_run_category_menu(ctx, key, dry_run, yes, style)` — category checkbox then run

- [ ] **Step 1: Update the `main` callback to pass `ctx` to `_interactive_menu`**

In `macclean/cli.py`, find the line:
```python
        _interactive_menu(dry_run=dry_run, yes=yes)
```
Replace it with:
```python
        _interactive_menu(ctx, dry_run=dry_run, yes=yes)
```

- [ ] **Step 2: Replace `_interactive_menu` (lines 109–160) with the three new functions**

Delete the entire old `_interactive_menu` function and replace with:

```python
def _interactive_menu(ctx, dry_run: bool, yes: bool) -> None:
    import questionary
    from questionary import Style

    style = Style([
        ("qmark",        "fg:#00d7ff bold"),
        ("question",     "bold"),
        ("answer",       "fg:#00d7ff bold"),
        ("pointer",      "fg:#00d7ff bold"),
        ("highlighted",  "fg:#00d7ff bold"),
        ("selected",     "fg:#afffff"),
        ("separator",    "fg:#555555"),
        ("instruction",  "fg:#777777"),
    ])

    console.print(Panel(
        "[bold cyan]macclean[/] — Mac system maintenance\n"
        "[dim]Arrow keys · Enter to select · Ctrl+C to quit[/]",
        expand=False,
    ))

    top_choices = []
    for key, (label, desc, _) in _PRESETS.items():
        top_choices.append(questionary.Choice(f"{label}  [dim]{desc}[/]", value=("preset", key)))
    top_choices.append(questionary.Separator())
    for key, (label, _items) in _CATEGORIES.items():
        top_choices.append(questionary.Choice(f"  {label} →", value=("category", key)))

    selection = questionary.select(
        "What would you like to do?",
        choices=top_choices,
        style=style,
        use_shortcuts=False,
    ).ask()

    if selection is None:
        console.print("[dim]Nothing selected.[/]")
        return

    kind, key = selection
    if kind == "preset":
        _run_preset_by_key(key, dry_run=dry_run, yes=yes)
    else:
        _run_category_menu(ctx, key, dry_run=dry_run, yes=yes, style=style)


def _run_preset_by_key(key: str, dry_run: bool, yes: bool) -> None:
    label, _desc, modules = _PRESETS[key]
    if modules is None:
        modules = [m for _, _, m in _CLEANERS]
    console.print(f"\n[bold cyan]{label}[/]\n")
    summary: list[tuple[str, int]] = []
    for module_name in modules:
        display = next((d for _, d, m in _CLEANERS if m == module_name), module_name)
        console.print(Rule(f"[bold]{display}[/]"))
        module = _import_cleaner(module_name)
        cleaned = _run_cleaner(module, display, dry_run=dry_run, yes=yes)
        summary.append((display, cleaned))
    _print_summary(summary, dry_run)
    if not dry_run:
        total = sum(b for _, b in summary)
        from macclean.core.utils import format_size
        _notify("macclean", f"Done — {format_size(total)} cleaned")


def _run_category_menu(ctx, category_key: str, dry_run: bool, yes: bool, style) -> None:
    import importlib
    import questionary
    label, items = _CATEGORIES[category_key]

    choices = [
        questionary.Choice(display, value=(cli_key, kind))
        for cli_key, display, kind in items
    ]
    selected = questionary.checkbox(
        f"{label} — select commands to run:",
        choices=choices,
        style=style,
    ).ask()

    if not selected:
        console.print("[dim]Nothing selected.[/]")
        return

    summary: list[tuple[str, int]] = []
    for cli_key, kind in selected:
        display = next(d for ck, d, _ in items if ck == cli_key)
        console.print(Rule(f"[bold]{display}[/]"))
        if kind == "cleaner":
            module_name = _CLEANER_BY_KEY[cli_key]
            module = _import_cleaner(module_name)
            cleaned = _run_cleaner(module, display, dry_run=dry_run, yes=yes)
            summary.append((display, cleaned))
        else:
            mod_path, attr = _TOOL_BY_KEY[cli_key]
            tool_module = importlib.import_module(mod_path)
            ctx.invoke(getattr(tool_module, attr))

    if summary:
        _print_summary(summary, dry_run)
        if not dry_run:
            total = sum(b for _, b in summary)
            from macclean.core.utils import format_size
            _notify("macclean", f"Done — {format_size(total)} cleaned")
```

- [ ] **Step 3: Verify the module imports cleanly**

```bash
cd /Users/asmitghosh/Desktop/cleanup
python -c "from macclean.cli import main, _run_preset_by_key, _run_category_menu; print('OK')"
```

Expected: `OK`

- [ ] **Step 4: Verify `--help` still works**

```bash
cd /Users/asmitghosh/Desktop/cleanup
python -c "
from macclean.cli import main
from click.testing import CliRunner
r = CliRunner().invoke(main, ['--help'])
assert r.exit_code == 0, r.output
print(r.output[:300])
"
```

Expected: Usage line with options, no traceback.

- [ ] **Step 5: Commit**

```bash
cd /Users/asmitghosh/Desktop/cleanup
git add macclean/cli.py
git commit -m "feat: two-level interactive menu with presets and categories"
```

---

### Task 3: Add `quick`, `dev`, `deep` top-level commands

**Files:**
- Modify: `macclean/cli.py` — insert before `_register_commands()` definition; update `_register_commands()` body

- [ ] **Step 1: Insert three command functions before `_register_commands()`**

In `macclean/cli.py`, directly before the `def _register_commands():` line, insert:

```python
@click.command("quick")
@click.pass_context
def _quick_cmd(ctx):
    """Quick clean: trash, browser, crash reports. Safe, ~2 min."""
    _run_preset_by_key("quick", dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])


@click.command("dev")
@click.pass_context
def _dev_cmd(ctx):
    """Dev clean: brew, docker, node/pip/cargo, xcode, projects, zsh."""
    _run_preset_by_key("dev", dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])


@click.command("deep")
@click.pass_context
def _deep_cmd(ctx):
    """Deep clean: every cleaner, confirm each step."""
    _run_preset_by_key("deep", dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])

```

- [ ] **Step 2: Register the three commands in `_register_commands()`**

In `_register_commands()`, after `main.add_command(_touchid_cmd, "touchid")`, add:

```python
    main.add_command(_quick_cmd, "quick")
    main.add_command(_dev_cmd, "dev")
    main.add_command(_deep_cmd, "deep")
```

- [ ] **Step 3: Verify all three appear in help and have correct docs**

```bash
cd /Users/asmitghosh/Desktop/cleanup
python -c "
from macclean.cli import main
from click.testing import CliRunner
r = CliRunner().invoke(main, ['--help'])
assert 'quick' in r.output, 'quick missing'
assert 'dev' in r.output, 'dev missing'
assert 'deep' in r.output, 'deep missing'
print('all three present')
print(r.output)
"
```

Expected: `all three present` followed by help output showing `quick`, `dev`, `deep`.

- [ ] **Step 4: Test each command's `--help`**

```bash
cd /Users/asmitghosh/Desktop/cleanup
python -c "
from macclean.cli import main
from click.testing import CliRunner
for cmd in ['quick', 'dev', 'deep']:
    r = CliRunner().invoke(main, [cmd, '--help'])
    assert r.exit_code == 0, f'{cmd} failed: {r.output}'
    print(f'{cmd}: OK — {r.output.splitlines()[2].strip()}')
"
```

Expected:
```
quick: OK — Quick clean: trash, browser, crash reports. Safe, ~2 min.
dev: OK — Dev clean: brew, docker, node/pip/cargo, xcode, projects, zsh.
deep: OK — Deep clean: every cleaner, confirm each step.
```

- [ ] **Step 5: Commit**

```bash
cd /Users/asmitghosh/Desktop/cleanup
git add macclean/cli.py
git commit -m "feat: add quick, dev, deep top-level preset commands"
```
