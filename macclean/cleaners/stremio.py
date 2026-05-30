import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size

console = Console()

_STREMIO_CACHE = Path.home() / "Library" / "Application Support" / "stremio-server" / "stremio-cache"


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()
    cache = home / "Library" / "Application Support" / "stremio-server" / "stremio-cache"
    if cache.exists():
        result.items.append(CleanItem(
            label="stremio-server video cache",
            path=cache,
            size_bytes=dir_size(cache),
        ))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No Stremio cache found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Cache")
    table.add_column("Size", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes))

    console.print(Panel(table, title="[bold cyan]Stremio Video Cache[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")
    console.print("  [dim]Stremio will re-cache streams on next use.[/]")

    if dry_run:
        return
    if not yes and not confirm("Clear Stremio video cache?"):
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
    """Clear Stremio video stream cache."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
