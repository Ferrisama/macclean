from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, run_cmd, require_sudo

console = Console()


def analyze(home: Path | None = None) -> AnalysisResult:
    result = AnalysisResult()
    out, code = run_cmd(["tmutil", "listlocalsnapshots", "/"])
    if code != 0:
        return result
    for line in out.splitlines():
        line = line.strip()
        if line.startswith("com.apple.TimeMachine"):
            result.items.append(CleanItem(
                label=line,
                path=Path("/"),
                size_bytes=0,
                removable=True,
            ))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No local Time Machine snapshots found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Snapshot")
    for item in result.items:
        table.add_row(item.label)

    console.print(Panel(table, title="[bold cyan]Time Machine Local Snapshots[/]"))
    console.print("  [dim]Note: snapshot sizes not reported by tmutil — all shown are safe to delete.[/]")
    console.print(f"  {len(result.items)} snapshot(s) found")

    if dry_run:
        return
    if not yes and not confirm(f"Delete all {len(result.items)} local snapshots?"):
        return

    require_sudo()
    for item in result.items:
        parts = item.label.split(".")
        date_part = next((p for p in parts if p[:4].isdigit()), None)
        if not date_part:
            console.print(f"  [yellow]⚠[/] Could not parse date from: {item.label}")
            continue
        out, code = run_cmd(["tmutil", "deletelocalsnapshots", date_part])
        if code == 0:
            console.print(f"  [green]✓[/] Deleted snapshot {date_part}")
        else:
            console.print(f"  [yellow]⚠[/] {item.label}: {out}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Delete local Time Machine APFS snapshots (frees 'System Data' space)."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
