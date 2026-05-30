import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import run_as_user

console = Console()


def _brew_outdated() -> list[tuple[str, str]]:
    if not shutil.which("brew"):
        return []
    out, code = run_as_user(["brew", "outdated", "--verbose"])
    if code != 0:
        return []
    rows = []
    for line in out.splitlines():
        line = line.strip()
        if line:
            rows.append(("brew", line))
    return rows


def _pip_outdated() -> list[tuple[str, str]]:
    out, code = run_as_user(["pip", "list", "--outdated", "--format=columns"])
    if code != 0:
        return []
    rows = []
    for line in out.splitlines()[2:]:
        parts = line.split()
        if len(parts) >= 3:
            rows.append(("pip", f"{parts[0]}  {parts[1]} → {parts[2]}"))
    return rows


def _npm_outdated() -> list[tuple[str, str]]:
    if not shutil.which("npm"):
        return []
    out, code = run_as_user(["npm", "outdated", "-g"], timeout=20)
    if code not in (0, 1):
        return []
    rows = []
    for line in out.splitlines()[1:]:
        parts = line.split()
        if len(parts) >= 4:
            rows.append(("npm", f"{parts[0]}  {parts[1]} → {parts[3]}"))
    return rows


@click.command()
@click.pass_context
def cmd(ctx):
    """Show outdated packages across Homebrew, pip, and npm."""
    table = Table(show_header=True, show_lines=True)
    table.add_column("Manager", style="bold cyan")
    table.add_column("Package / Info")

    all_rows: list[tuple[str, str]] = []
    for rows_fn in [_brew_outdated, _pip_outdated, _npm_outdated]:
        for mgr, info in rows_fn():
            table.add_row(mgr, info)
            all_rows.append((mgr, info))

    if not all_rows:
        console.print("[green]All packages are up to date.[/]")
        return

    console.print(Panel(table, title=f"[bold cyan]Outdated Packages ({len(all_rows)})[/]"))
    console.print("  [dim]Run [bold]macclean update[/] to upgrade all.[/]")
