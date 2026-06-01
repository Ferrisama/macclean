import os
import subprocess
import click
from rich.console import Console
from rich.panel import Panel
from rich.table import Table
from rich.rule import Rule

console = Console()

# Single source of truth: (cli_key, display_name, module_name)
# cli_key  = the `macclean <cli_key>` command name
# module_name = module inside macclean/cleaners/
_CLEANERS = [
    ("trash",          "Trash",                "trash"),
    ("crash-reports",  "Crash Reports",        "crash_reports"),
    ("quicklook",      "QuickLook Cache",      "quicklook"),
    ("memory",         "Inactive Memory",      "memory"),
    ("spotlight",      "Spotlight Reindex",    "spotlight"),
    ("system",         "System Caches & Logs", "system"),
    ("timemachine",    "Time Machine Snaps",   "timemachine"),
    ("browser",        "Browser Caches",       "browser"),
    ("stremio",        "Stremio Cache",        "stremio"),
    ("apps",           "Ghost App Files",      "apps"),
    ("xcode",          "Xcode Data",           "xcode"),
    ("ios-backups",    "iOS Backups",          "ios_backups"),
    ("fonts",          "Duplicate Fonts",      "fonts"),
    ("brew",           "Homebrew",             "brew"),
    ("docker",         "Docker",               "docker"),
    ("python",         "Python Versions",      "python_versions"),
    ("node",           "Node.js Caches",       "node"),
    ("pip",            "pip Cache",            "pip_cache"),
    ("cargo",          "Cargo Cache",          "cargo"),
    ("gradle",         "Gradle Cache",         "gradle"),
    ("maven",          "Maven Repository",     "maven"),
    ("go",             "Go Module Cache",      "go_cache"),
    ("zsh",            "ZSH History",          "zsh"),
    ("projects",       "Project Artifacts",    "projects"),
    ("installers",     "Installer Files",      "installers"),
]

# Non-cleaner tools: (cli_key, module_path, attr)
_TOOLS = [
    ("analyze",      "macclean.core.disk",           "analyze_cmd"),
    ("status",       "macclean.core.disk",           "status_cmd"),
    ("health",       "macclean.cleaners.health",     "cmd"),
    ("largest",      "macclean.cleaners.largest",    "cmd"),
    ("dupes",        "macclean.cleaners.dupes",      "cmd"),
    ("security",     "macclean.cleaners.security",   "cmd"),
    ("ports",        "macclean.cleaners.ports",      "cmd"),
    ("privacy",      "macclean.cleaners.privacy",    "cmd"),
    ("agents",       "macclean.cleaners.agents",     "cmd"),
    ("login-items",  "macclean.cleaners.login_items","cmd"),
    ("wifi",         "macclean.cleaners.wifi",       "cmd"),
    ("connections",  "macclean.cleaners.connections","cmd"),
    ("uninstall",    "macclean.cleaners.uninstall",  "cmd"),
    ("outdated",     "macclean.cleaners.outdated",   "cmd"),
    ("update",       "macclean.cleaners.update",     "cmd"),
    ("quit-apps",    "macclean.cleaners.quit_apps",  "cmd"),
]

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
        ("uninstall",    "Uninstall App",              "tool"),
        ("update",       "Update Packages",            "tool"),
        ("quit-apps",    "Quit Apps",                  "tool"),
        ("quit-apps",    "Configure Quit-Apps List",   "tool", {"do_configure": True}),
    ]),
}

# Preset definitions: key → (label, description, [module_names] | None)
# module_names match _CLEANERS entries — all presets run cleaners only
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


def _notify(title: str, message: str) -> None:
    script = f'display notification "{message}" with title "{title}"'
    subprocess.run(["osascript", "-e", script], capture_output=True)


def _import_cleaner(module_name: str):
    import importlib
    return importlib.import_module(f"macclean.cleaners.{module_name}")


def _run_cleaner(module, name: str, dry_run: bool, yes: bool) -> int:
    """Run one cleaner, return bytes cleaned (0 if unknown)."""
    from macclean.core.log import append_log
    try:
        result = module.analyze()
        before = result.total_bytes
        module.clean(result, dry_run=dry_run, yes=yes)
        append_log(name, before, dry_run=dry_run)
        return before if not dry_run else 0
    except SystemExit:
        console.print(f"[yellow]Skipped {name} (needs sudo)[/]")
        return 0
    except Exception as e:
        console.print(f"[red]Error in {name}:[/] {e}")
        return 0


@click.group(invoke_without_command=True)
@click.option("--dry-run", is_flag=True, default=False, help="Analyze only, no deletion")
@click.option("--yes", "-y", is_flag=True, default=False, help="Skip confirmation prompts")
@click.option("--json", "as_json", is_flag=True, default=False, help="Output as JSON")
@click.pass_context
def main(ctx, dry_run, yes, as_json):
    """macclean — Mac system maintenance CLI.

    Run without arguments for interactive mode.
    """
    ctx.ensure_object(dict)
    ctx.obj["dry_run"] = dry_run
    ctx.obj["yes"] = yes
    ctx.obj["as_json"] = as_json

    if ctx.invoked_subcommand is None:
        _interactive_menu(ctx, dry_run=dry_run, yes=yes)


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
        top_choices.append(questionary.Choice(f"{label}  {desc}", value=("preset", key)))
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
        questionary.Choice(entry[1], value=entry)
        for entry in items
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
    for entry in selected:
        cli_key, display, kind = entry[0], entry[1], entry[2]
        kwargs = entry[3] if len(entry) > 3 else {}
        console.print(Rule(f"[bold]{display}[/]"))
        if kind == "cleaner":
            module_name = _CLEANER_BY_KEY[cli_key]
            module = _import_cleaner(module_name)
            cleaned = _run_cleaner(module, display, dry_run=dry_run, yes=yes)
            summary.append((display, cleaned))
        else:
            mod_path, attr = _TOOL_BY_KEY[cli_key]
            tool_module = importlib.import_module(mod_path)
            ctx.invoke(getattr(tool_module, attr), **kwargs)

    if summary:
        _print_summary(summary, dry_run)
        if not dry_run:
            total = sum(b for _, b in summary)
            from macclean.core.utils import format_size
            _notify("macclean", f"Done — {format_size(total)} cleaned")


def _print_summary(summary: list[tuple[str, int]], dry_run: bool) -> None:
    from macclean.core.utils import format_size

    if not summary:
        return

    console.print()
    table = Table(show_header=True, show_lines=False, box=None, padding=(0, 2))
    table.add_column("Cleaner", style="bold")
    table.add_column("Recovered", justify="right")

    total = 0
    for name, size in summary:
        color = "green" if size > 0 else "dim"
        table.add_row(name, f"[{color}]{format_size(size)}[/{color}]")
        total += size

    console.print(Panel(
        table,
        title="[bold cyan]Session Summary[/]",
        subtitle=f"[bold]Total: {format_size(total)}[/]" if not dry_run else "[dim]dry-run — nothing deleted[/]",
    ))


@click.command("all")
@click.option("--select", "use_select", is_flag=True, default=False,
              help="Choose cleaners interactively before running")
@click.option("--profile", default=None,
              help="Run a preset profile: light, dev, deep")
@click.pass_context
def _all_cmd(ctx, use_select: bool, profile: str | None):
    """Run every cleaner in sequence, confirming each step."""
    from macclean.core.config import get_profile_cleaners
    dry_run = ctx.obj["dry_run"]
    yes = ctx.obj["yes"]

    if profile:
        profile_modules = get_profile_cleaners(profile)
        if profile_modules is None:
            runners = [(d, m) for _, d, m in _CLEANERS]
        else:
            runners = [(d, m) for _, d, m in _CLEANERS if m in profile_modules]
        console.print(f"[dim]Profile: [bold]{profile}[/] — {len(runners)} cleaners[/]")
    elif use_select:
        import questionary
        choices = [
            questionary.Choice(display_name, value=module_name, checked=True)
            for _, display_name, module_name in _CLEANERS
        ]
        selected_modules = questionary.checkbox(
            "Select cleaners to run (all checked by default):",
            choices=choices,
        ).ask()
        if not selected_modules:
            console.print("[dim]Nothing selected.[/]")
            return
        runners = [(d, m) for _, d, m in _CLEANERS if m in selected_modules]
    else:
        runners = [(d, m) for _, d, m in _CLEANERS]

    from concurrent.futures import ThreadPoolExecutor, as_completed

    # Phase 1: analyze all in parallel
    console.print("[dim]Analyzing...[/]")
    analysis: dict[str, tuple[str, object]] = {}

    with ThreadPoolExecutor(max_workers=8) as executor:
        future_map = {
            executor.submit(_import_cleaner(m).analyze): (d, m)
            for d, m in runners
        }
        for future in as_completed(future_map):
            display, module_name = future_map[future]
            try:
                analysis[module_name] = (display, future.result())
            except Exception as e:
                console.print(f"[red]Error analyzing {display}:[/] {e}")
                analysis[module_name] = (display, None)

    # Phase 2: clean sequentially (requires user interaction)
    summary: list[tuple[str, int]] = []
    from macclean.core.log import append_log
    for display, module_name in runners:
        if module_name not in analysis:
            continue
        disp, result = analysis[module_name]
        if result is None:
            continue
        console.rule(f"[bold]{disp}[/]")
        module = _import_cleaner(module_name)
        try:
            before = result.total_bytes
            module.clean(result, dry_run=dry_run, yes=yes)
            append_log(disp, before, dry_run=dry_run)
            summary.append((disp, before if not dry_run else 0))
        except SystemExit:
            console.print(f"[yellow]Skipped {disp} (needs sudo)[/]")
        except Exception as e:
            console.print(f"[red]Error in {disp}:[/] {e}")

    _print_summary(summary, dry_run)

    if not dry_run:
        total = sum(b for _, b in summary)
        from macclean.core.utils import format_size
        _notify("macclean", f"Done — {format_size(total)} cleaned")


@click.command("log")
@click.option("--limit", default=50, show_default=True, help="Number of recent entries to show")
def _log_cmd(limit: int):
    """Show macclean operation history."""
    from macclean.core.log import read_log
    from macclean.core.utils import format_size

    records = read_log(limit=limit)
    if not records:
        console.print("[dim]No history yet. Run some cleaners first.[/]")
        return

    table = Table(show_header=True, show_lines=False)
    table.add_column("Time", style="dim")
    table.add_column("Cleaner", style="bold")
    table.add_column("Recovered", justify="right")
    table.add_column("Mode")

    total = 0
    for r in reversed(records):
        ts = r.get("timestamp", "")[:16].replace("T", " ")
        size = r.get("bytes_cleaned", 0)
        mode = "[dim]dry-run[/]" if r.get("dry_run") else "[green]cleaned[/]"
        table.add_row(ts, r.get("cleaner", "?"), format_size(size), mode)
        if not r.get("dry_run"):
            total += size

    console.print(Panel(table, title="[bold cyan]macclean History[/]",
                        subtitle=f"Total cleaned: {format_size(total)}"))


@click.command("touchid")
def _touchid_cmd():
    """Enable Touch ID authentication for sudo (no more password prompts)."""
    from macclean.core.touchid import is_touchid_enabled, enable_touchid

    if is_touchid_enabled():
        console.print(Panel("[green]✓ Touch ID is already enabled for sudo.[/]",
                            title="[bold cyan]Touch ID[/]"))
        return

    console.print(Panel(
        "This will add [bold]pam_tid.so[/] to [dim]/etc/pam.d/sudo_local[/].\n"
        "After this, sudo commands will prompt for Touch ID instead of password.",
        title="[bold cyan]Enable Touch ID for sudo[/]",
    ))
    from macclean.core.utils import confirm
    if not confirm("Enable Touch ID for sudo?"):
        return

    ok, msg = enable_touchid()
    if ok:
        console.print(f"[green]✓[/] {msg}")
        console.print("  [dim]Open a new terminal to use Touch ID with sudo.[/]")
    else:
        console.print(f"[red]✗[/] {msg}")


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


def _register_commands():
    import importlib

    # Register all cleaners from _CLEANERS (single source of truth)
    for cli_key, _, module_name in _CLEANERS:
        module = importlib.import_module(f"macclean.cleaners.{module_name}")
        main.add_command(module.cmd, cli_key)

    # Register tools from _TOOLS
    for cli_key, module_path, attr in _TOOLS:
        module = importlib.import_module(module_path)
        main.add_command(getattr(module, attr), cli_key)

    # Built-in commands
    main.add_command(_all_cmd, "all")
    main.add_command(_log_cmd, "log")
    main.add_command(_touchid_cmd, "touchid")
    main.add_command(_quick_cmd, "quick")
    main.add_command(_dev_cmd, "dev")
    main.add_command(_deep_cmd, "deep")


_register_commands()
