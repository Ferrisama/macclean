from pathlib import Path
import click
import psutil
from rich.console import Console
from rich.panel import Panel
from macclean.core.utils import AnalysisResult, format_size, confirm, run_cmd, require_sudo

console = Console()


def analyze(home: Path | None = None) -> AnalysisResult:
    return AnalysisResult()


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    console.print(Panel("[bold cyan]Memory — Flush Inactive[/]"))
    vm = psutil.virtual_memory()
    console.print(f"  RAM: [bold]{format_size(vm.total)}[/] total, [yellow]{format_size(vm.inactive)}[/] inactive")

    if dry_run:
        return
    if not yes and not confirm("Run sudo purge?"):
        return

    require_sudo()
    out, code = run_cmd(["purge"])
    if code == 0:
        console.print("  [green]+[/] Inactive memory flushed")
    else:
        console.print(f"  [yellow]![/] purge failed: {out}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Flush inactive memory (sudo purge)."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
