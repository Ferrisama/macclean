import os
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

    user_trash = home / ".Trash"
    if user_trash.exists():
        size = dir_size(user_trash)
        if size > 0:
            result.items.append(CleanItem(label="~/.Trash", path=user_trash, size_bytes=size))

    uid = os.getuid()
    volumes = Path("/Volumes")
    if volumes.exists():
        for vol in volumes.iterdir():
            t = vol / ".Trashes" / str(uid)
            if t.exists():
                size = dir_size(t)
                if size > 0:
                    result.items.append(CleanItem(
                        label=f"/Volumes/{vol.name}/.Trashes",
                        path=t,
                        size_bytes=size,
                    ))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]Trash is already empty.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Location")
    table.add_column("Size", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes))

    console.print(Panel(table, title="[bold cyan]Trash[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")

    if dry_run:
        return
    if not yes and not confirm("Empty all trash?"):
        return

    for item in result.items:
        try:
            for child in item.path.iterdir():
                if child.is_dir(follow_symlinks=False):
                    shutil.rmtree(child, ignore_errors=True)
                else:
                    child.unlink(missing_ok=True)
            console.print(f"  [green]✓[/] Cleared {item.label}")
        except Exception as e:
            console.print(f"  [yellow]⚠[/] {item.label}: {e}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Empty Trash across all volumes."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
