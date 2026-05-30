import os
import subprocess
import click
from rich.console import Console
from rich.panel import Panel
from rich.table import Table
from rich.rule import Rule

console = Console()

# Ordered list of (key, display_name, module_name) for all cleaners
_CLEANERS = [
    ("trash",        "Trash",               "trash"),
    ("crash",        "Crash Reports",       "crash_reports"),
    ("quicklook",    "QuickLook Cache",     "quicklook"),
    ("memory",       "Inactive Memory",     "memory"),
    ("spotlight",    "Spotlight Reindex",   "spotlight"),
    ("system",       "System Caches & Logs","system"),
    ("timemachine",  "Time Machine Snaps",  "timemachine"),
    ("browser",      "Browser Caches",      "browser"),
    ("stremio",      "Stremio Cache",       "stremio"),
    ("apps",         "Ghost App Files",     "apps"),
    ("xcode",        "Xcode Data",          "xcode"),
    ("ios",          "iOS Backups",         "ios_backups"),
    ("fonts",        "Duplicate Fonts",     "fonts"),
    ("brew",         "Homebrew",            "brew"),
    ("docker",       "Docker",              "docker"),
    ("python",       "Python Versions",     "python_versions"),
    ("node",         "Node.js Caches",      "node"),
    ("pip",          "pip Cache",           "pip_cache"),
    ("cargo",        "Cargo Cache",         "cargo"),
    ("gradle",       "Gradle Cache",        "gradle"),
    ("maven",        "Maven Repository",    "maven"),
    ("go",           "Go Module Cache",     "go_cache"),
    ("zsh",          "ZSH History",         "zsh"),
]


def _notify(title: str, message: str) -> None:
    script = f'display notification "{message}" with title "{title}"'
    subprocess.run(["osascript", "-e", script], capture_output=True)


def _import_cleaner(module_name: str):
    import importlib
    return importlib.import_module(f"macclean.cleaners.{module_name}")


def _run_cleaner(module, name: str, dry_run: bool, yes: bool) -> int:
    """Run one cleaner, return bytes cleaned (0 if unknown)."""
    try:
        result = module.analyze()
        before = result.total_bytes
        module.clean(result, dry_run=dry_run, yes=yes)
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
        _interactive_menu(dry_run=dry_run, yes=yes)


def _interactive_menu(dry_run: bool, yes: bool) -> None:
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
        "[dim]Space to select · Enter to run · Ctrl+C to quit[/]",
        expand=False,
    ))

    choices = [
        questionary.Choice(display_name, value=module_name)
        for _, display_name, module_name in _CLEANERS
    ]

    selected = questionary.checkbox(
        "What would you like to clean?",
        choices=choices,
        style=style,
    ).ask()

    if not selected:
        console.print("[dim]Nothing selected.[/]")
        return

    summary: list[tuple[str, int]] = []

    for module_name in selected:
        display = next(d for _, d, m in _CLEANERS if m == module_name)
        console.print(Rule(f"[bold]{display}[/]"))
        module = _import_cleaner(module_name)
        cleaned = _run_cleaner(module, display, dry_run=dry_run, yes=yes)
        summary.append((display, cleaned))

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
@click.pass_context
def _all_cmd(ctx, use_select: bool):
    """Run every cleaner in sequence, confirming each step."""
    dry_run = ctx.obj["dry_run"]
    yes = ctx.obj["yes"]

    if use_select:
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

    summary: list[tuple[str, int]] = []

    for display, module_name in runners:
        console.rule(f"[bold]{display}[/]")
        module = _import_cleaner(module_name)
        cleaned = _run_cleaner(module, display, dry_run=dry_run, yes=yes)
        summary.append((display, cleaned))

    _print_summary(summary, dry_run)

    if not dry_run:
        total = sum(b for _, b in summary)
        from macclean.core.utils import format_size
        _notify("macclean", f"Done — {format_size(total)} cleaned")


def _register_commands():
    from macclean.cleaners import (
        trash, crash_reports, browser, node, pip_cache, cargo,
        gradle, maven, go_cache, brew, docker, quicklook, memory,
        spotlight, system, timemachine, zsh, python_versions,
        apps, xcode, ios_backups, fonts, stremio,
        largest, dupes, security, ports, privacy, agents, login_items, wifi, connections,
    )
    from macclean.core import disk
    from macclean.cleaners import health

    main.add_command(disk.analyze_cmd, "analyze")
    main.add_command(disk.status_cmd, "status")
    main.add_command(health.cmd, "health")
    # Cleaners
    main.add_command(trash.cmd, "trash")
    main.add_command(crash_reports.cmd, "crash-reports")
    main.add_command(browser.cmd, "browser")
    main.add_command(node.cmd, "node")
    main.add_command(pip_cache.cmd, "pip")
    main.add_command(cargo.cmd, "cargo")
    main.add_command(gradle.cmd, "gradle")
    main.add_command(maven.cmd, "maven")
    main.add_command(go_cache.cmd, "go")
    main.add_command(brew.cmd, "brew")
    main.add_command(docker.cmd, "docker")
    main.add_command(quicklook.cmd, "quicklook")
    main.add_command(memory.cmd, "memory")
    main.add_command(spotlight.cmd, "spotlight")
    main.add_command(system.cmd, "system")
    main.add_command(timemachine.cmd, "timemachine")
    main.add_command(zsh.cmd, "zsh")
    main.add_command(python_versions.cmd, "python")
    main.add_command(apps.cmd, "apps")
    main.add_command(xcode.cmd, "xcode")
    main.add_command(ios_backups.cmd, "ios-backups")
    main.add_command(fonts.cmd, "fonts")
    main.add_command(stremio.cmd, "stremio")
    # Disk intelligence
    main.add_command(largest.cmd, "largest")
    main.add_command(dupes.cmd, "dupes")
    # Security & privacy
    main.add_command(security.cmd, "security")
    main.add_command(ports.cmd, "ports")
    main.add_command(privacy.cmd, "privacy")
    # Startup & background
    main.add_command(agents.cmd, "agents")
    main.add_command(login_items.cmd, "login-items")
    # Network
    main.add_command(wifi.cmd, "wifi")
    main.add_command(connections.cmd, "connections")
    main.add_command(_all_cmd, "all")


_register_commands()
