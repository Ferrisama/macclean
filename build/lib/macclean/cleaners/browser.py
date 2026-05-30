import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size

console = Console()

_BROWSER_CACHE_DIRS = [
    ("Safari", "Library/Caches/com.apple.Safari"),
    ("Chrome", "Library/Caches/Google/Chrome"),
    ("Firefox", "Library/Caches/Firefox"),
    ("Edge", "Library/Caches/Microsoft Edge"),
    ("Brave", "Library/Caches/BraveSoftware"),
]


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()
    for label, rel in _BROWSER_CACHE_DIRS:
        path = home / rel
        if path.exists():
            result.items.append(CleanItem(label=label, path=path, size_bytes=dir_size(path)))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No browser caches found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Browser")
    table.add_column("Cache Size", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes))

    console.print(Panel(table, title="[bold cyan]Browser Caches[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")

    if dry_run:
        return
    if not yes and not confirm("Clear all browser caches?"):
        return

    for item in result.items:
        try:
            shutil.rmtree(item.path, ignore_errors=True)
            console.print(f"  [green]✓[/] Cleared {item.label}")
        except Exception as e:
            console.print(f"  [yellow]⚠[/] {item.label}: {e}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Clear Safari, Chrome, Firefox, and other browser caches."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
