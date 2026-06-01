import re
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size

console = Console()

_BUNDLE_ID_RE = re.compile(r"^[a-zA-Z0-9\-]+\.[a-zA-Z0-9\-\.]+$")

_SCAN_DIRS = [
    "Library/Application Support",
    "Library/Containers",
    "Library/Preferences",
]

_KEEP_PREFIXES = {"com.apple.", "com.google.", "io.iterm2", "com.microsoft."}


def _get_installed_bundle_ids(apps_dirs: list[Path]) -> set[str]:
    import plistlib
    ids: set[str] = set()
    for apps_dir in apps_dirs:
        if not apps_dir.exists():
            continue
        for app in apps_dir.glob("*.app"):
            plist = app / "Contents" / "Info.plist"
            if plist.exists():
                try:
                    with open(plist, "rb") as f:
                        data = plistlib.load(f)
                    bid = data.get("CFBundleIdentifier", "")
                    if bid:
                        ids.add(bid)
                except Exception:
                    pass
    return ids


def analyze(home: Path | None = None, apps_dirs: list[Path] | None = None) -> AnalysisResult:
    home = home or Path.home()
    if apps_dirs is None:
        apps_dirs = [Path("/Applications"), home / "Applications"]

    installed = _get_installed_bundle_ids(apps_dirs)
    result = AnalysisResult()

    for rel in _SCAN_DIRS:
        scan_dir = home / rel
        if not scan_dir.exists():
            continue
        for entry in scan_dir.iterdir():
            if not entry.is_dir():
                continue
            name = entry.name
            if not _BUNDLE_ID_RE.match(name):
                continue
            if any(name.startswith(p) for p in _KEEP_PREFIXES):
                continue
            if name not in installed:
                result.items.append(CleanItem(
                    label=f"{name} (in {rel})",
                    path=entry,
                    size_bytes=dir_size(entry),
                ))

    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No orphaned app files found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Bundle / Location")
    table.add_column("Size", justify="right")
    for item in sorted(result.items, key=lambda x: x.size_bytes, reverse=True):
        color = "red" if item.size_bytes > 100 * 1024**2 else "yellow" if item.size_bytes > 10 * 1024**2 else "white"
        table.add_row(item.label, f"[{color}]{format_size(item.size_bytes)}[/{color}]")

    console.print(Panel(table, title="[bold cyan]Ghost App Files[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")
    console.print("  [dim]Warning: some folders may contain cloud-synced data. Review before confirming.[/]")

    if dry_run:
        return
    if not yes and not confirm("Delete all ghost app folders?"):
        return

    import shutil
    for item in result.items:
        try:
            shutil.rmtree(item.path, ignore_errors=True)
            console.print(f"  [green]+[/] Removed {item.label}")
        except Exception as e:
            console.print(f"  [yellow]![/] {item.label}: {e}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Find and remove leftover files from uninstalled apps."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
