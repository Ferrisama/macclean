"""
Quit all running apps except a saved keep list.
First run: interactive selector to build the list.
Subsequent runs: instant quit, no prompts (unless --configure).
"""
import json
import subprocess
from pathlib import Path

import click
from rich.console import Console
from rich.panel import Panel
from rich.table import Table

console = Console()

_CONFIG_PATH = Path.home() / ".macclean_quit_apps.json"

# Always stays open — Finder cannot be quit
_ALWAYS_KEEP = {"Finder"}

# Pre-checked on first-run setup
_DEFAULT_KEEP = {
    "Finder",
    "Google Chrome", "Safari", "Firefox", "Brave Browser", "Arc",
    "Visual Studio Code", "Cursor",
    "iTerm2", "Terminal", "Warp",
}


def _get_running_apps() -> list[str]:
    result = subprocess.run(
        ["osascript", "-e",
         "tell application \"System Events\" to get name of "
         "every process where background only is false"],
        capture_output=True, text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return []
    return sorted(a.strip() for a in result.stdout.strip().split(",") if a.strip())


def _load_keep_list() -> list[str] | None:
    if not _CONFIG_PATH.exists():
        return None
    try:
        data = json.loads(_CONFIG_PATH.read_text())
        return data.get("keep")
    except Exception:
        return None


def _save_keep_list(keep: list[str]) -> None:
    _CONFIG_PATH.write_text(json.dumps({"keep": sorted(keep)}, indent=2))


def _configure(running_apps: list[str]) -> list[str]:
    """Interactive selector — returns the keep list."""
    import questionary
    from questionary import Style

    saved = set(_load_keep_list() or _DEFAULT_KEEP)

    style = Style([
        ("qmark",       "fg:#00d7ff bold"),
        ("question",    "bold"),
        ("pointer",     "fg:#00d7ff bold"),
        ("highlighted", "fg:#00d7ff bold"),
        ("selected",    "fg:#afffff"),
        ("instruction", "fg:#777777"),
    ])

    choices = [
        questionary.Choice(
            app,
            value=app,
            checked=(app in saved or app in _ALWAYS_KEEP),
        )
        for app in running_apps
    ]

    console.print(Panel(
        "[bold]Select the apps you want to [green]keep open[/green].[/]\n"
        "[dim]Everything else will be quit. This choice is saved — you only do this once.[/]",
        title="[bold cyan]quit-apps — Setup[/]",
        expand=False,
    ))

    selected = questionary.checkbox(
        "Keep these apps open:",
        choices=choices,
        style=style,
    ).ask()

    if selected is None:
        return list(saved | _ALWAYS_KEEP)

    keep = list(set(selected) | _ALWAYS_KEEP)
    _save_keep_list(keep)
    console.print(f"  [green]+[/] Saved keep list ({len(keep)} apps) to [dim]{_CONFIG_PATH}[/]")
    return keep


def _quit_app(app: str) -> bool:
    result = subprocess.run(
        ["osascript", "-e", f'tell application "{app}" to quit'],
        capture_output=True,
    )
    return result.returncode == 0


@click.command("quit-apps")
@click.option("--configure", "do_configure", is_flag=True,
              help="Reconfigure which apps to keep open")
@click.pass_context
def cmd(ctx, do_configure: bool):
    """Quit all running apps except your saved keep list.

    First run opens an interactive selector to build your keep list.
    After that, running this command instantly quits everything else.
    Use --configure to update your keep list anytime.
    """
    dry_run = ctx.obj.get("dry_run", False)
    yes = ctx.obj.get("yes", False)

    running = _get_running_apps()
    if not running:
        console.print("[red]Could not read running apps. Check System Events permissions.[/]")
        return

    saved_keep = _load_keep_list()

    if do_configure or saved_keep is None:
        if saved_keep is None:
            console.print("[dim]First run — let's set up your keep list.[/]\n")
        keep = set(_configure(running))
    else:
        keep = set(saved_keep) | _ALWAYS_KEEP

    to_quit = [app for app in running if app not in keep]

    if not to_quit:
        console.print("[green]Nothing to quit — every running app is in your keep list.[/]")
        if saved_keep is not None and not do_configure:
            console.print(f"  [dim]Run [bold]macclean quit-apps --configure[/] to update your list.[/]")
        return

    table = Table(show_header=False, show_lines=False, box=None, padding=(0, 2))
    table.add_column()
    table.add_column()
    for app in to_quit:
        table.add_row("[red]-[/]", app)

    console.print(Panel(
        table,
        title="[bold cyan]Apps to Quit[/]",
        subtitle=f"[dim]{len(to_quit)} apps · {len(keep)} kept[/]",
    ))

    if dry_run:
        console.print("[dim]Dry run — nothing was quit.[/]")
        return

    if not yes:
        from macclean.core.utils import confirm
        if not confirm(f"Quit {len(to_quit)} app{'s' if len(to_quit) != 1 else ''}?"):
            return

    failed = []
    for app in to_quit:
        ok = _quit_app(app)
        status = "[green]+[/]" if ok else "[yellow]![/]"
        console.print(f"  {status} {app}")
        if not ok:
            failed.append(app)

    console.print()
    console.print(f"[bold]Quit {len(to_quit) - len(failed)}/{len(to_quit)} apps.[/]")
    if failed:
        console.print(f"  [dim]Couldn't quit: {', '.join(failed)} (may have unsaved changes)[/]")
