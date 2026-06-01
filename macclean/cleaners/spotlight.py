from pathlib import Path
import click
from rich.console import Console
from rich.panel import Panel
from macclean.core.utils import AnalysisResult, confirm, run_cmd, require_sudo

console = Console()


def analyze(home: Path | None = None) -> AnalysisResult:
    return AnalysisResult()


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    console.print(Panel("[bold cyan]Spotlight Reindex[/]"))
    console.print("  This will reindex the entire drive (Spotlight unavailable for ~10 min).")

    if dry_run:
        return
    if not yes and not confirm("Reindex Spotlight?"):
        return

    require_sudo()
    out, code = run_cmd(["mdutil", "-E", "/"], timeout=30)
    if code == 0:
        console.print("  [green]+[/] Spotlight reindex started")
    else:
        console.print(f"  [yellow]![/] mdutil error: {out}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Reindex Spotlight (sudo mdutil -E /)."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
