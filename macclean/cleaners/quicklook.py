from pathlib import Path
import click
from rich.console import Console
from rich.panel import Panel
from macclean.core.utils import AnalysisResult, CleanItem, format_size, confirm, dir_size, run_cmd

console = Console()

_QL_CACHE_DIRS = [
    "com.apple.QuickLookDaemon",
    "com.apple.quicklook.ThumbnailsAgent",
]


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()
    caches = home / "Library" / "Caches"
    for name in _QL_CACHE_DIRS:
        path = caches / name
        if path.exists():
            result.items.append(CleanItem(label=name, path=path, size_bytes=dir_size(path)))
    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    console.print(Panel("[bold cyan]QuickLook Cache Rebuild[/]"))
    total = format_size(result.total_bytes) if result.total_bytes else "0 B"
    console.print(f"  Stale QL cache: [bold]{total}[/]")

    if dry_run:
        return
    if not yes and not confirm("Kill and rebuild QuickLook server?"):
        return

    for args, label in [
        (["qlmanage", "-r"], "Kill QL server"),
        (["qlmanage", "-r", "cache"], "Rebuild QL cache"),
    ]:
        out, code = run_cmd(args)
        if code == 0 or "reset" in out.lower():
            console.print(f"  [green]+[/] {label}")
        else:
            console.print(f"  [yellow]![/] {label}: {out}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Kill and rebuild the QuickLook server and cache."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
