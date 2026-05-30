import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size

console = Console()


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()
    cargo_root = home / ".cargo"
    dirs = [
        ("registry cache", cargo_root / "registry" / "cache"),
        ("registry src", cargo_root / "registry" / "src"),
        ("git checkouts", cargo_root / "git" / "checkouts"),
    ]
    for label, path in dirs:
        if path.exists():
            result.items.append(CleanItem(label=label, path=path, size_bytes=dir_size(path)))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No Cargo caches found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Cache")
    table.add_column("Size", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes))

    console.print(Panel(table, title="[bold cyan]Cargo (Rust) Cache[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")

    if dry_run:
        return
    if not yes and not confirm("Clear Cargo caches?"):
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
    """Clear Rust Cargo registry and source caches."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
