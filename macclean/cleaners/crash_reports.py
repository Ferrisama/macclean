import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size, require_sudo

console = Console()


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()
    dirs = [
        home / "Library" / "Logs" / "DiagnosticReports",
        Path("/Library/Logs/DiagnosticReports"),
    ]
    for path in dirs:
        if path.exists():
            result.items.append(CleanItem(label=str(path), path=path, size_bytes=dir_size(path)))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No crash reports found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Directory")
    table.add_column("Size", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes))

    console.print(Panel(table, title="[bold cyan]Crash Reports[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")

    if dry_run:
        return
    if not yes and not confirm("Remove all crash reports?"):
        return

    require_sudo()
    for item in result.items:
        try:
            shutil.rmtree(item.path, ignore_errors=True)
            item.path.mkdir(parents=True, exist_ok=True)
            console.print(f"  [green]✓[/] Cleared {item.label}")
        except Exception as e:
            console.print(f"  [yellow]⚠[/] {item.label}: {e}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Remove crash reports and diagnostic logs."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
