from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size, run_cmd

console = Console()


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()
    path = home / "go" / "pkg"
    if path.exists():
        result.items.append(CleanItem(label="Go module cache (~/go/pkg)", path=path, size_bytes=dir_size(path)))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No Go module cache found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Cache")
    table.add_column("Size", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes))

    console.print(Panel(table, title="[bold cyan]Go Module Cache[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")

    if dry_run:
        return
    if not yes and not confirm("Clear Go module cache?"):
        return

    out, code = run_cmd(["go", "clean", "-modcache"])
    if code == 0:
        console.print("  [green]✓[/] Go module cache cleared")
    else:
        console.print(f"  [yellow]⚠[/] go not on PATH or error: {out}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Clear Go module cache."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
