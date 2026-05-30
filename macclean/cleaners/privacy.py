import sqlite3
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel

console = Console()

_TCC_SERVICES = {
    "kTCCServiceCamera": "Camera",
    "kTCCServiceMicrophone": "Microphone",
    "kTCCServiceScreenCapture": "Screen Recording",
    "kTCCServiceLocation": "Location",
    "kTCCServiceAddressBook": "Contacts",
    "kTCCServiceCalendar": "Calendar",
    "kTCCServicePhotos": "Photos",
    "kTCCServiceSystemPolicyAllFiles": "Full Disk Access",
    "kTCCServiceAccessibility": "Accessibility",
    "kTCCServiceReminders": "Reminders",
    "kTCCServiceLiverpool": "Location Services",
    "kTCCServiceUbiquity": "iCloud",
    "kTCCServiceShareKit": "Sharing",
}

_AUTH = {0: "[red]Denied[/]", 1: "[dim]Unknown[/]", 2: "[green]Allowed[/]", 3: "[yellow]Limited[/]"}


def _query_tcc(db_path: Path) -> list[tuple[str, str, str]]:
    rows = []
    if not db_path.exists():
        return rows
    try:
        conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
        cur = conn.execute("SELECT service, client, auth_value FROM access ORDER BY service, client")
        for service, client, auth_val in cur.fetchall():
            svc = _TCC_SERVICES.get(service, service.replace("kTCCService", ""))
            auth = _AUTH.get(auth_val, str(auth_val))
            rows.append((svc, client, auth))
        conn.close()
    except Exception as e:
        rows.append(("Error", str(e), ""))
    return rows


@click.command()
@click.pass_context
def cmd(ctx):
    """Show app privacy permissions (camera, mic, screen recording, location, etc.)."""
    home = Path.home()

    for label, db_path in [
        ("User Permissions", home / "Library" / "Application Support" / "com.apple.TCC" / "TCC.db"),
        ("System Permissions", Path("/Library/Application Support/com.apple.TCC/TCC.db")),
    ]:
        rows = _query_tcc(db_path)
        if not rows:
            console.print(f"[dim]{label}: not readable (run with sudo for system DB)[/]")
            continue

        table = Table(show_header=True, show_lines=True)
        table.add_column("Permission")
        table.add_column("App / Bundle ID")
        table.add_column("Status")
        for svc, client, auth in rows:
            table.add_row(svc, client, auth)

        console.print(Panel(table, title=f"[bold cyan]{label}[/]"))
