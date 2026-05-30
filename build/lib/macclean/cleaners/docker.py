import re
import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, run_cmd

console = Console()


def _parse_reclaimed(text: str) -> int:
    m = re.search(r"([\d.]+)\s*(B|KB|MB|GB|TB)", text, re.IGNORECASE)
    if not m:
        return 0
    val = float(m.group(1))
    unit = m.group(2).upper()
    multipliers = {"B": 1, "KB": 1024, "MB": 1024**2, "GB": 1024**3, "TB": 1024**4}
    return int(val * multipliers.get(unit, 1))


def analyze(home: Path | None = None) -> AnalysisResult:
    result = AnalysisResult()
    if not shutil.which("docker"):
        return result

    _, code = run_cmd(["docker", "info"], timeout=10)
    if code != 0:
        return result

    checks = [
        (["docker", "image", "ls", "-q"], "Unused images"),
        (["docker", "ps", "-aq", "-f", "status=exited"], "Stopped containers"),
        (["docker", "volume", "ls", "-q"], "Unused volumes"),
    ]
    for args, label in checks:
        out, code = run_cmd(args)
        count = len([l for l in out.splitlines() if l.strip()])
        if count > 0:
            result.items.append(CleanItem(
                label=f"{label} ({count})",
                path=Path("/dev/null"),
                size_bytes=0,
                removable=True,
            ))

    result.items.append(CleanItem(
        label="Build cache + reclaimable",
        path=Path("/dev/null"),
        size_bytes=0,
        removable=True,
    ))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not shutil.which("docker"):
        console.print("[yellow]docker not found — skipping.[/]")
        return

    _, code = run_cmd(["docker", "info"], timeout=10)
    if code != 0:
        console.print("[yellow]Docker daemon not running — skipping.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Item")
    for item in result.items:
        table.add_row(item.label)

    console.print(Panel(table, title="[bold cyan]Docker[/]"))

    if dry_run:
        return
    if not yes and not confirm("Prune all Docker resources?"):
        return

    prune_cmds = [
        (["docker", "container", "prune", "-f"], "containers"),
        (["docker", "image", "prune", "-af"], "images"),
        (["docker", "volume", "prune", "-f"], "volumes"),
        (["docker", "builder", "prune", "-af"], "build cache"),
    ]
    for args, label in prune_cmds:
        out, code = run_cmd(args, timeout=120)
        if code == 0:
            reclaimed = _parse_reclaimed(out)
            suffix = f" ({format_size(reclaimed)} reclaimed)" if reclaimed else ""
            console.print(f"  [green]✓[/] Pruned {label}{suffix}")
        else:
            console.print(f"  [yellow]⚠[/] {label}: {out}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Prune Docker images, containers, volumes, and build cache."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
