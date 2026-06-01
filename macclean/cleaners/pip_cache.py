from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size, run_as_user

console = Console()


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()
    dirs = [
        ("pip cache (user)", home / "Library" / "Caches" / "pip"),
        ("pip cache (XDG)", home / ".cache" / "pip"),
    ]
    for label, path in dirs:
        if path.exists():
            result.items.append(CleanItem(label=label, path=path, size_bytes=dir_size(path)))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No pip caches found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Cache")
    table.add_column("Size", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes))

    console.print(Panel(table, title="[bold cyan]pip Cache[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")

    if dry_run:
        return
    if not yes and not confirm("Clear pip cache?"):
        return

    out, _ = run_as_user(["pip", "cache", "purge"])
    console.print(f"  [green]+[/] {out or 'pip cache cleared'}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Clear Python pip download cache."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
