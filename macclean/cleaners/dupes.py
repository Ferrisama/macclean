import os
import hashlib
from collections import defaultdict
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from rich.progress import Progress, SpinnerColumn, TextColumn, BarColumn, TaskProgressColumn
from macclean.core.utils import format_size, confirm

console = Console()

DEFAULT_MIN_MB = 10


def _hash_file(path: Path, block_size: int = 65536) -> str | None:
    h = hashlib.sha256()
    try:
        with open(path, "rb") as f:
            while chunk := f.read(block_size):
                h.update(chunk)
        return h.hexdigest()
    except OSError:
        return None


def _find_dupes(root: Path, min_bytes: int) -> list[list[Path]]:
    # Group by size first (cheap filter)
    by_size: dict[int, list[Path]] = defaultdict(list)
    for dirpath, dirs, files in os.walk(root, followlinks=False):
        dirs[:] = [d for d in dirs if not d.startswith(".") or dirpath == str(root)]
        for fname in files:
            fpath = Path(dirpath) / fname
            try:
                size = fpath.stat().st_size
                if size >= min_bytes:
                    by_size[size].append(fpath)
            except OSError:
                pass

    candidates = [paths for paths in by_size.values() if len(paths) > 1]

    # Hash candidates
    by_hash: dict[str, list[Path]] = defaultdict(list)
    total = sum(len(g) for g in candidates)

    with Progress(SpinnerColumn(), TextColumn("[cyan]Hashing {task.completed}/{task.total} files..."),
                  BarColumn(), TaskProgressColumn(), console=console) as p:
        task = p.add_task("hashing", total=total)
        for group in candidates:
            for fpath in group:
                digest = _hash_file(fpath)
                if digest:
                    by_hash[digest].append(fpath)
                p.advance(task)

    return [paths for paths in by_hash.values() if len(paths) > 1]


@click.command()
@click.option("--min", "min_mb", default=DEFAULT_MIN_MB, show_default=True, help="Minimum file size in MB")
@click.option("--path", "scan_path", default=None, help="Directory to scan (default: home)")
@click.pass_context
def cmd(ctx, min_mb: int, scan_path: str | None):
    """Find duplicate files by content hash."""
    root = Path(scan_path) if scan_path else Path.home()
    min_bytes = min_mb * 1024 * 1024

    console.print(f"[dim]Scanning {root} for duplicates ≥ {min_mb} MB...[/]")
    groups = _find_dupes(root, min_bytes)

    if not groups:
        console.print("[green]No duplicate files found.[/]")
        return

    # Sort groups by wasted space descending
    groups.sort(key=lambda g: g[0].stat().st_size * (len(g) - 1), reverse=True)

    table = Table(show_header=True, show_lines=True)
    table.add_column("Duplicate Files")
    table.add_column("Size", justify="right")
    table.add_column("Copies", justify="right")
    table.add_column("Wasted", justify="right", style="bold red")

    total_wasted = 0
    for group in groups:
        try:
            size = group[0].stat().st_size
        except OSError:
            continue
        wasted = size * (len(group) - 1)
        total_wasted += wasted
        first = True
        for fpath in group:
            label = str(fpath).replace(str(Path.home()), "~")
            if first:
                table.add_row(label, format_size(size), str(len(group)), format_size(wasted))
                first = False
            else:
                table.add_row(f"  [dim]↳ {label}[/]", "", "", "")

    console.print(Panel(table, title="[bold cyan]Duplicate Files[/]"))
    console.print(f"  Total wasted: [bold red]{format_size(total_wasted)}[/]")
    console.print("  [dim]Review paths above — delete copies manually or with rm.[/]")
