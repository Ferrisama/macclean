from collections import defaultdict
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm

console = Console()

_FONT_DIRS = [
    Path("/Library/Fonts"),
    Path("/System/Library/Fonts"),
]

_FONT_EXTS = {".ttf", ".otf", ".ttc", ".dfont"}


def _find_duplicates(dirs: list[Path]) -> dict[str, list[Path]]:
    seen: dict[str, list[Path]] = defaultdict(list)
    for d in dirs:
        if not d.exists():
            continue
        for f in d.iterdir():
            if f.suffix.lower() in _FONT_EXTS:
                seen[f.name].append(f)
    return {name: paths for name, paths in seen.items() if len(paths) > 1}


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    user_fonts = home / "Library" / "Fonts"
    dirs = [user_fonts] + _FONT_DIRS
    dupes = _find_duplicates(dirs)
    result = AnalysisResult()

    for name, paths in dupes.items():
        user_copies = [p for p in paths if str(p).startswith(str(home))]
        for path in user_copies:
            result.items.append(CleanItem(
                label=f"{name} (duplicate in {path.parent})",
                path=path,
                size_bytes=path.stat().st_size,
                removable=True,
            ))

    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No duplicate fonts found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Font")
    table.add_column("Size", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes))

    console.print(Panel(table, title="[bold cyan]Duplicate Fonts[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")
    console.print("  [dim]Only user-installed duplicates shown (~/Library/Fonts). System fonts are not touched.[/]")

    if dry_run:
        return
    if not yes and not confirm("Remove duplicate user fonts?"):
        return

    for item in result.items:
        try:
            item.path.unlink()
            console.print(f"  [green]+[/] Removed {item.label}")
        except Exception as e:
            console.print(f"  [yellow]![/] {item.label}: {e}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Detect and remove duplicate fonts in ~/Library/Fonts."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
