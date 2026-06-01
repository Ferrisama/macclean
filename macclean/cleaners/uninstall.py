import plistlib
import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size

console = Console()

_TRACE_DIRS = [
    ("~/Library/Application Support", "Library/Application Support"),
    ("~/Library/Caches",              "Library/Caches"),
    ("~/Library/Containers",          "Library/Containers"),
    ("~/Library/Logs",                "Library/Logs"),
    ("~/Library/Preferences",         "Library/Preferences"),
    ("~/Library/Application Scripts", "Library/Application Scripts"),
    ("~/Library/Saved Application State", "Library/Saved Application State"),
    ("~/Library/HTTPStorages",        "Library/HTTPStorages"),
    ("~/Library/WebKit",              "Library/WebKit"),
    ("~/Library/LaunchAgents",        "Library/LaunchAgents"),
]

_SYSTEM_TRACE_DIRS = [
    Path("/Library/Application Support"),
    Path("/Library/Caches"),
    Path("/Library/LaunchAgents"),
    Path("/Library/LaunchDaemons"),
]


def _find_bundle_id(app_path: Path) -> str | None:
    plist = app_path / "Contents" / "Info.plist"
    if not plist.exists():
        return None
    try:
        with open(plist, "rb") as f:
            data = plistlib.load(f)
        return data.get("CFBundleIdentifier")
    except Exception:
        return None


def _find_app_traces(bundle_id: str, home: Path) -> list[tuple[str, Path, int]]:
    traces = []
    name_lower = bundle_id.split(".")[-1].lower()

    for label, rel in _TRACE_DIRS:
        search_dir = home / rel
        if not search_dir.exists():
            continue
        for entry in search_dir.iterdir():
            n = entry.name.lower()
            if bundle_id.lower() in n or name_lower in n:
                size = dir_size(entry) if entry.is_dir() else entry.stat().st_size
                traces.append((label, entry, size))

    for search_dir in _SYSTEM_TRACE_DIRS:
        if not search_dir.exists():
            continue
        try:
            for entry in search_dir.iterdir():
                n = entry.name.lower()
                if bundle_id.lower() in n or name_lower in n:
                    size = dir_size(entry) if entry.is_dir() else entry.stat().st_size
                    traces.append((str(search_dir), entry, size))
        except PermissionError:
            pass

    return traces


def _find_app(name: str) -> Path | None:
    for apps_dir in [Path("/Applications"), Path.home() / "Applications"]:
        for variant in [name, name.title(), name.upper(), name.lower()]:
            candidate = apps_dir / f"{variant}.app"
            if candidate.exists():
                return candidate
    return None


@click.command()
@click.argument("app_name")
@click.pass_context
def cmd(ctx, app_name: str):
    """Uninstall an app and all its associated files."""
    dry_run = ctx.obj["dry_run"]
    yes = ctx.obj["yes"]
    home = Path.home()

    app_path = _find_app(app_name)
    if not app_path:
        console.print(f"[yellow]App '{app_name}' not found in /Applications or ~/Applications.[/]")
        console.print("  [dim]Tip: use the exact app name as it appears in Finder.[/]")
        return

    bundle_id = _find_bundle_id(app_path)
    console.print(f"  Found: [bold]{app_path.name}[/]")
    if bundle_id:
        console.print(f"  Bundle ID: [dim]{bundle_id}[/]")

    traces = _find_app_traces(bundle_id or app_name, home) if bundle_id else []

    table = Table(show_header=True, show_lines=True)
    table.add_column("Location")
    table.add_column("Path")
    table.add_column("Size", justify="right")

    app_size = dir_size(app_path)
    table.add_row("[bold].app bundle[/]", str(app_path), format_size(app_size))

    total = app_size
    for label, path, size in traces:
        color = "red" if size > 100 * 1024**2 else "yellow" if size > 10 * 1024**2 else "white"
        table.add_row(label, str(path).replace(str(home), "~"),
                      f"[{color}]{format_size(size)}[/{color}]")
        total += size

    console.print(Panel(table, title=f"[bold cyan]Uninstall: {app_path.stem}[/]"))
    console.print(f"  Total to remove: [bold red]{format_size(total)}[/]  ({1 + len(traces)} items)")

    if dry_run:
        return
    if not yes and not confirm(f"Permanently delete {app_path.stem} and all its files?", default=False):
        return

    shutil.rmtree(app_path, ignore_errors=True)
    console.print(f"  [green]+[/] Removed {app_path}")

    for _, path, _ in traces:
        try:
            if path.is_dir():
                shutil.rmtree(path, ignore_errors=True)
            else:
                path.unlink(missing_ok=True)
            console.print(f"  [green]+[/] Removed {str(path).replace(str(home), '~')}")
        except Exception as e:
            console.print(f"  [yellow]![/] {path}: {e}")
