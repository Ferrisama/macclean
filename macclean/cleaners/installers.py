from datetime import datetime
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm

console = Console()

_INSTALLER_EXTS = {".dmg", ".pkg", ".zip"}
_SCAN_DIRS = ["Downloads", "Desktop"]
_MIN_SIZE = 5 * 1024 * 1024  # 5 MB


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()
    for dir_name in _SCAN_DIRS:
        scan_dir = home / dir_name
        if not scan_dir.exists():
            continue
        for f in scan_dir.iterdir():
            if not f.is_file():
                continue
            name_lower = f.name.lower()
            is_installer = (
                f.suffix.lower() in _INSTALLER_EXTS or
                name_lower.endswith(".tar.gz") or
                name_lower.endswith(".tar.bz2")
            )
            if not is_installer:
                continue
            size = f.stat().st_size
            if size < _MIN_SIZE:
                continue
            age_days = (datetime.now() - datetime.fromtimestamp(f.stat().st_mtime)).days
            result.items.append(CleanItem(
                label=f"{f.name} ({dir_name}, {age_days}d old)",
                path=f,
                size_bytes=size,
            ))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No installer files found.[/]")
        return

    sorted_items = sorted(result.items, key=lambda x: x.size_bytes, reverse=True)

    table = Table(show_header=True, show_lines=True)
    table.add_column("File")
    table.add_column("Size", justify="right")
    for item in sorted_items:
        color = "red" if item.size_bytes > 1024**3 else "yellow" if item.size_bytes > 200*1024**2 else "white"
        table.add_row(item.label, f"[{color}]{format_size(item.size_bytes)}[/{color}]")

    console.print(Panel(table, title="[bold cyan]Installer Files[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")

    if dry_run:
        return
    if not yes and not confirm("Delete all listed installer files?"):
        return

    for item in sorted_items:
        try:
            item.path.unlink()
            console.print(f"  [green]✓[/] Deleted {item.path.name}")
        except Exception as e:
            console.print(f"  [yellow]⚠[/] {item.path.name}: {e}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Find and remove .dmg, .pkg, .zip installers in Downloads and Desktop."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
