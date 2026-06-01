import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size, run_cmd, require_sudo

console = Console()

_SYSTEM_CACHE_DIRS = [
    ("User Caches", "Library/Caches"),
    ("User Logs", "Library/Logs"),
]

_ROOT_CACHE_DIRS = [
    Path("/Library/Caches"),
    Path("/private/var/log"),
    Path("/private/tmp"),
]


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()
    for label, rel in _SYSTEM_CACHE_DIRS:
        path = home / rel
        if path.exists():
            result.items.append(CleanItem(label=label, path=path, size_bytes=dir_size(path)))
    for path in _ROOT_CACHE_DIRS:
        if path.exists():
            result.items.append(CleanItem(label=str(path), path=path, size_bytes=dir_size(path)))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    table = Table(show_header=True, show_lines=True)
    table.add_column("Location")
    table.add_column("Size", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes))

    console.print(Panel(table, title="[bold cyan]System Caches & Logs[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")

    if dry_run:
        return
    if not yes and not confirm("Clear system caches and logs?"):
        return

    require_sudo()

    for item in result.items:
        try:
            for child in item.path.iterdir():
                if child.is_dir() and not child.is_symlink():
                    shutil.rmtree(child, ignore_errors=True)
                elif not child.is_symlink():
                    child.unlink(missing_ok=True)
            console.print(f"  [green]+[/] Cleared {item.label}")
        except Exception as e:
            console.print(f"  [yellow]![/] {item.label}: {e}")

    out, code = run_cmd(["/usr/sbin/periodic", "daily", "weekly", "monthly"], timeout=300)
    if code == 0:
        console.print("  [green]+[/] macOS periodic maintenance scripts ran")
    else:
        console.print(f"  [yellow]![/] periodic scripts: {out[:200]}")

    run_cmd(["dscacheutil", "-flushcache"])
    run_cmd(["killall", "-HUP", "mDNSResponder"])
    console.print("  [green]+[/] DNS cache flushed")


@click.command()
@click.pass_context
def cmd(ctx):
    """Clear system caches, logs, tmp, and run macOS maintenance scripts."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
