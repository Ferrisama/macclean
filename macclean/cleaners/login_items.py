from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import run_cmd

console = Console()


def _get_login_items_osascript() -> list[tuple[str, str]]:
    script = 'tell application "System Events" to get the properties of every login item'
    out, code = run_cmd(["osascript", "-e", script], timeout=10)
    if code != 0 or not out.strip():
        return []
    items = []
    for entry in out.split("},"):
        name, path = "", ""
        for part in entry.split(","):
            part = part.strip()
            if "name:" in part:
                name = part.split("name:")[-1].strip()
            elif "path:" in part:
                path = part.split("path:")[-1].strip()
        if name:
            items.append((name, path))
    return items


def _get_login_items_sfltool() -> list[tuple[str, str]]:
    out, code = run_cmd(["sfltool", "dumpbtm"], timeout=10)
    if code != 0:
        return []
    items = []
    name, url = "", ""
    for line in out.splitlines():
        line = line.strip()
        if "name =" in line:
            name = line.split("name =")[-1].strip().strip('"')
        elif "url =" in line:
            url = line.split("url =")[-1].strip().strip('"').replace("file://", "")
            if name:
                items.append((name, url))
                name, url = "", ""
    return items


@click.command("login-items")
@click.pass_context
def cmd(ctx):
    """Show apps that launch at login."""
    items = _get_login_items_sfltool()
    if not items:
        items = _get_login_items_osascript()

    if not items:
        console.print("[dim]No login items found (or permission denied).[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("App Name")
    table.add_column("Path")
    table.add_column("Exists?")

    for name, path in sorted(items, key=lambda x: x[0].lower()):
        exists = "[green]Yes[/]" if path and Path(path).exists() else "[red]No[/]" if path else "[dim]—[/]"
        table.add_row(name, path or "—", exists)

    console.print(Panel(table, title=f"[bold cyan]Login Items ({len(items)})[/]"))
    console.print("  [dim]To remove an item: System Settings → General → Login Items[/]")
