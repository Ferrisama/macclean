import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size, run_cmd, run_as_user

console = Console()


def analyze(home: Path | None = None) -> AnalysisResult:
    result = AnalysisResult()
    if not shutil.which("brew"):
        return result

    cache_out, code = run_as_user(["brew", "--cache"])
    if code != 0:
        return result
    cache_dir = Path(cache_out.strip())
    if cache_dir.exists():
        result.items.append(CleanItem(
            label="Homebrew download cache",
            path=cache_dir,
            size_bytes=dir_size(cache_dir),
        ))

    outdated_out, _ = run_as_user(["brew", "outdated", "--quiet"])
    outdated = [l for l in outdated_out.splitlines() if l.strip()]
    if outdated:
        result.items.append(CleanItem(
            label=f"Outdated formulae ({len(outdated)} packages)",
            path=Path("/dev/null"),
            size_bytes=0,
            removable=False,
        ))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not shutil.which("brew"):
        console.print("[yellow]brew not found — skipping.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Item")
    table.add_column("Size", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes) if item.size_bytes else "—")

    console.print(Panel(table, title="[bold cyan]Homebrew[/]"))
    console.print(f"  Cache to recover: [bold]{format_size(result.total_bytes)}[/]")

    if dry_run:
        return
    if not yes and not confirm("Run brew cleanup and autoremove?"):
        return

    for args, label in [
        (["brew", "cleanup", "--prune=all"], "brew cleanup"),
        (["brew", "autoremove"], "brew autoremove"),
    ]:
        out, code = run_as_user(args, timeout=120)
        if code == 0:
            console.print(f"  [green]✓[/] {label}")
        else:
            console.print(f"  [yellow]⚠[/] {label}: {out}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Clean Homebrew cache and remove orphaned packages."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
