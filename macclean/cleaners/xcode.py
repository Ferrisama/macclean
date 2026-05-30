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
    xcode_root = home / "Library" / "Developer" / "Xcode"
    sim_root = home / "Library" / "Developer" / "CoreSimulator"

    targets = [
        ("DerivedData", xcode_root / "DerivedData"),
        ("Archives", xcode_root / "Archives"),
        ("iOS Device Support", xcode_root / "iOS DeviceSupport"),
        ("watchOS Device Support", xcode_root / "watchOS DeviceSupport"),
        ("tvOS Device Support", xcode_root / "tvOS DeviceSupport"),
        ("visionOS Device Support", xcode_root / "visionOS DeviceSupport"),
        ("Simulator Devices", sim_root / "Devices"),
        ("Simulator Runtimes Cache", sim_root / "Caches" / "dyld"),
    ]

    for label, path in targets:
        if path.exists():
            result.items.append(CleanItem(label=label, path=path, size_bytes=dir_size(path)))

    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No Xcode data found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Item")
    table.add_column("Size", justify="right")
    for item in result.items:
        color = "red" if item.size_bytes > 5 * 1024**3 else "yellow" if item.size_bytes > 500 * 1024**2 else "white"
        table.add_row(item.label, f"[{color}]{format_size(item.size_bytes)}[/{color}]")

    console.print(Panel(table, title="[bold cyan]Xcode[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")
    console.print("  [dim]DerivedData and Device Support are safe to delete — Xcode rebuilds them.[/]")

    if dry_run:
        return
    if not yes and not confirm("Clear selected Xcode data?"):
        return

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
    """Clear Xcode DerivedData, archives, device support files, and simulators."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
