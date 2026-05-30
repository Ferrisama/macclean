import os
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from rich.progress import Progress, SpinnerColumn, TextColumn
from macclean.core.utils import format_size

console = Console()

DEFAULT_MIN_MB = 100
DEFAULT_LIMIT = 30


def _scan(root: Path, min_bytes: int, limit: int) -> list[tuple[Path, int]]:
    results = []
    try:
        for dirpath, dirs, files in os.walk(root, followlinks=False):
            dirs[:] = [d for d in dirs if not d.startswith(".") or dirpath == str(root)]
            for fname in files:
                fpath = Path(dirpath) / fname
                try:
                    size = fpath.stat().st_size
                    if size >= min_bytes:
                        results.append((fpath, size))
                except OSError:
                    pass
    except (PermissionError, OSError):
        pass
    results.sort(key=lambda x: x[1], reverse=True)
    return results[:limit]


@click.command()
@click.option("--min", "min_mb", default=DEFAULT_MIN_MB, show_default=True, help="Minimum file size in MB")
@click.option("--limit", default=DEFAULT_LIMIT, show_default=True, help="Max results to show")
@click.option("--path", "scan_path", default=None, help="Directory to scan (default: home)")
@click.pass_context
def cmd(ctx, min_mb: int, limit: int, scan_path: str | None):
    """Find the largest files on disk."""
    root = Path(scan_path) if scan_path else Path.home()
    min_bytes = min_mb * 1024 * 1024

    with Progress(SpinnerColumn(), TextColumn("[cyan]Scanning {task.description}..."), console=console) as p:
        p.add_task(str(root))
        results = _scan(root, min_bytes, limit)

    if not results:
        console.print(f"[green]No files larger than {min_mb} MB found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("File", no_wrap=False)
    table.add_column("Size", justify="right", style="bold")

    for fpath, size in results:
        color = "red" if size > 1024**3 else "yellow" if size > 500 * 1024**2 else "white"
        try:
            label = str(fpath).replace(str(Path.home()), "~")
        except Exception:
            label = str(fpath)
        table.add_row(label, f"[{color}]{format_size(size)}[/{color}]")

    console.print(Panel(table, title=f"[bold cyan]Largest Files in {root}[/]"))
    console.print(f"  Showing top {len(results)} files ≥ {min_mb} MB")
