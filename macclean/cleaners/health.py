import psutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from rich.columns import Columns
from macclean.core.utils import format_size, run_cmd, dir_size

console = Console()


def _disk_info() -> Table:
    disk = psutil.disk_usage("/")
    t = Table(show_header=False, box=None, padding=(0, 2))
    t.add_column("K", style="bold")
    t.add_column("V")
    pct = disk.percent
    bar_color = "red" if pct > 85 else "yellow" if pct > 70 else "green"
    t.add_row("Total",  format_size(disk.total))
    t.add_row("Used",   f"[{bar_color}]{format_size(disk.used)} ({pct:.0f}%)[/{bar_color}]")
    t.add_row("Free",   f"[green]{format_size(disk.free)}[/]")
    return t


def _memory_info() -> Table:
    vm = psutil.virtual_memory()
    t = Table(show_header=False, box=None, padding=(0, 2))
    t.add_column("K", style="bold")
    t.add_column("V")
    pct = vm.percent
    color = "red" if pct > 85 else "yellow" if pct > 70 else "green"
    t.add_row("Total",    format_size(vm.total))
    t.add_row("Used",     f"[{color}]{format_size(vm.used)} ({pct:.0f}%)[/{color}]")
    t.add_row("Free",     f"[green]{format_size(vm.available)}[/]")
    t.add_row("Inactive", f"[dim]{format_size(vm.inactive)}[/]")
    return t


def _cpu_info() -> Table:
    load = psutil.getloadavg()
    pct = psutil.cpu_percent(interval=0.5)
    count = psutil.cpu_count()
    t = Table(show_header=False, box=None, padding=(0, 2))
    t.add_column("K", style="bold")
    t.add_column("V")
    color = "red" if pct > 80 else "yellow" if pct > 50 else "green"
    t.add_row("Cores",   str(count))
    t.add_row("Usage",   f"[{color}]{pct:.0f}%[/{color}]")
    t.add_row("Load",    f"{load[0]:.2f}  {load[1]:.2f}  {load[2]:.2f}")
    return t


def _battery_info() -> Table | None:
    battery = psutil.sensors_battery()
    if not battery:
        return None
    t = Table(show_header=False, box=None, padding=(0, 2))
    t.add_column("K", style="bold")
    t.add_column("V")
    pct = battery.percent
    color = "red" if pct < 20 else "yellow" if pct < 40 else "green"
    t.add_row("Charge",  f"[{color}]{pct:.0f}%[/{color}]")
    t.add_row("Plugged", "[green]Yes[/]" if battery.power_plugged else "[yellow]No[/]")
    if not battery.power_plugged and battery.secsleft > 0:
        h, rem = divmod(battery.secsleft, 3600)
        t.add_row("Remaining", f"{int(h)}h {int(rem//60)}m")
    return t


def _security_summary() -> Table:
    from macclean.cleaners.security import _filevault_status, _firewall_status, _sip_status
    t = Table(show_header=False, box=None, padding=(0, 2))
    t.add_column("K", style="bold")
    t.add_column("V")
    fv, _ = _filevault_status()
    fw, _ = _firewall_status()
    sip, _ = _sip_status()
    t.add_row("FileVault", "[green]On[/]" if fv else "[red]OFF[/]")
    t.add_row("Firewall",  "[green]On[/]" if fw else "[red]OFF[/]")
    t.add_row("SIP",       "[green]On[/]" if sip else "[red]OFF[/]")
    return t


def _top_dirs() -> Table:
    home = Path.home()
    targets = [
        ("Gradle",   home / ".gradle" / "caches"),
        ("Xcode Dev",home / "Library" / "Developer"),
        ("Containers",home / "Library" / "Containers"),
        ("App Support",home / "Library" / "Application Support"),
        ("Caches",   home / "Library" / "Caches"),
        ("pyenv",    home / ".pyenv" / "versions"),
        ("npm",      home / ".npm"),
        ("cargo",    home / ".cargo" / "registry"),
    ]
    results = [(label, dir_size(p)) for label, p in targets if p.exists()]
    results.sort(key=lambda x: x[1], reverse=True)

    t = Table(show_header=False, box=None, padding=(0, 2))
    t.add_column("K", style="bold")
    t.add_column("V", justify="right")
    for label, size in results[:6]:
        color = "red" if size > 5 * 1024**3 else "yellow" if size > 1024**3 else "dim"
        t.add_row(label, f"[{color}]{format_size(size)}[/{color}]")
    return t


@click.command()
@click.pass_context
def cmd(ctx):
    """One-page system health snapshot."""
    console.print(Panel("[bold cyan]macclean health[/]  — system snapshot", expand=False))

    # Row 1: Disk + Memory + CPU
    panels = [
        Panel(_disk_info(),   title="[bold]Disk[/]",   expand=True),
        Panel(_memory_info(), title="[bold]Memory[/]", expand=True),
        Panel(_cpu_info(),    title="[bold]CPU[/]",    expand=True),
    ]
    bat = _battery_info()
    if bat:
        panels.append(Panel(bat, title="[bold]Battery[/]", expand=True))
    console.print(Columns(panels, equal=True))

    # Row 2: Security + Top dirs
    console.print(Columns([
        Panel(_security_summary(), title="[bold]Security[/]", expand=True),
        Panel(_top_dirs(),         title="[bold]Top Space Users[/]", expand=True),
    ], equal=True))
