import time
import click
import psutil
from pathlib import Path
from rich.console import Console
from rich.table import Table
from rich.live import Live
from macclean.core.utils import format_size, dir_size

console = Console()

_DEFAULT_DIRS = [
    ("Home (~/Library/Caches)", Path.home() / "Library" / "Caches"),
    ("Home (~/Library/Logs)", Path.home() / "Library" / "Logs"),
    ("Home (~/Library/Developer)", Path.home() / "Library" / "Developer"),
    ("pyenv versions", Path.home() / ".pyenv" / "versions"),
    ("npm cache", Path.home() / ".npm"),
    ("cargo registry", Path.home() / ".cargo" / "registry"),
    ("Go pkg cache", Path.home() / "go" / "pkg"),
    ("Gradle caches", Path.home() / ".gradle" / "caches"),
    ("Maven repo", Path.home() / ".m2" / "repository"),
    ("Trash", Path.home() / ".Trash"),
    ("Crash Reports", Path.home() / "Library" / "Logs" / "DiagnosticReports"),
]


def scan_dirs(dirs: list[tuple[str, Path]]) -> list[tuple[str, Path, int]]:
    results = []
    for label, path in dirs:
        if path.exists():
            results.append((label, path, dir_size(path)))
    return results


def _make_disk_table() -> Table:
    disk = psutil.disk_usage("/")
    table = Table(title="Live Disk Status", show_lines=True)
    table.add_column("Metric")
    table.add_column("Value", justify="right")
    table.add_row("Total", format_size(disk.total))
    table.add_row("Used", format_size(disk.used))
    table.add_row("Free", format_size(disk.free))
    table.add_row("Used %", f"{disk.percent:.1f}%")
    return table


@click.command("analyze")
@click.pass_context
def analyze_cmd(ctx):
    """Show disk usage breakdown by major directories."""
    from rich.panel import Panel
    console.print(Panel("[bold cyan]Disk Usage Analysis[/]"))
    results = scan_dirs(_DEFAULT_DIRS)
    results.sort(key=lambda x: x[2], reverse=True)

    table = Table(show_header=True, show_lines=True)
    table.add_column("Directory")
    table.add_column("Size", justify="right", style="bold")

    for label, path, size in results:
        color = "red" if size > 1024**3 else "yellow" if size > 500 * 1024**2 else "green"
        table.add_row(label, f"[{color}]{format_size(size)}[/{color}]")

    console.print(table)


@click.command("status")
def status_cmd():
    """Live-refresh disk free space (Ctrl+C to exit)."""
    console.print("[dim]Watching disk status — Ctrl+C to exit[/]")
    with Live(_make_disk_table(), refresh_per_second=0.5, console=console) as live:
        while True:
            time.sleep(5)
            live.update(_make_disk_table())
