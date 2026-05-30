import plistlib
import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size

console = Console()


def _read_backup_name(backup_dir: Path) -> str:
    info_plist = backup_dir / "Info.plist"
    if info_plist.exists():
        try:
            with open(info_plist, "rb") as f:
                data = plistlib.load(f)
            device = data.get("Device Name", "")
            product = data.get("Product Type", "")
            date = data.get("Last Backup Date", "")
            date_str = str(date)[:10] if date else "unknown date"
            return f"{device} ({product}) — {date_str}"
        except Exception:
            pass
    return backup_dir.name


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()
    backup_root = home / "Library" / "Application Support" / "MobileSync" / "Backup"
    if not backup_root.exists():
        return result

    for backup_dir in backup_root.iterdir():
        if not backup_dir.is_dir():
            continue
        name = _read_backup_name(backup_dir)
        size = dir_size(backup_dir)
        result.items.append(CleanItem(label=name, path=backup_dir, size_bytes=size))

    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]No iOS backups found.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Device")
    table.add_column("Size", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes))

    console.print(Panel(table, title="[bold cyan]iOS/iPadOS Backups[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")
    console.print("  [dim]Warning: deleting removes the only local backup for that device.[/]")

    if dry_run:
        return
    if not yes and not confirm("Delete all listed backups?"):
        return

    for item in result.items:
        try:
            shutil.rmtree(item.path, ignore_errors=True)
            console.print(f"  [green]✓[/] Deleted backup: {item.label}")
        except Exception as e:
            console.print(f"  [yellow]⚠[/] {item.label}: {e}")


@click.command()
@click.pass_context
def cmd(ctx):
    """List and delete iPhone/iPad local backups."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
