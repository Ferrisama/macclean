import sqlite3
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import run_cmd

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
}


def _check(label: str, good: bool, detail: str = "") -> tuple[str, str, str]:
    status = "[green]OK[/]" if good else "[red]OFF[/]"
    return label, status, detail


def _filevault_status() -> tuple[bool, str]:
    out, _ = run_cmd(["fdesetup", "status"])
    on = "FileVault is On" in out
    return on, out.split("\n")[0].strip()


def _firewall_status() -> tuple[bool, str]:
    out, code = run_cmd([
        "/usr/libexec/ApplicationFirewall/socketfilterfw", "--getglobalstate"
    ])
    on = "enabled" in out.lower()
    return on, "Enabled" if on else "Disabled"


def _sip_status() -> tuple[bool, str]:
    out, _ = run_cmd(["csrutil", "status"])
    on = "enabled" in out.lower()
    return on, out.split("\n")[0].strip()


def _gatekeeper_status() -> tuple[bool, str]:
    out, _ = run_cmd(["spctl", "--status"])
    on = "enabled" in out.lower() or "assessments enabled" in out.lower()
    return on, "Enabled" if on else "Disabled"


def _read_tcc_permissions(home: Path) -> list[tuple[str, str, str]]:
    tcc_db = home / "Library" / "Application Support" / "com.apple.TCC" / "TCC.db"
    rows = []
    if not tcc_db.exists():
        return rows
    try:
        conn = sqlite3.connect(str(tcc_db))
        cur = conn.execute(
            "SELECT service, client, auth_value FROM access WHERE auth_value = 2"
        )
        for service, client, _ in cur.fetchall():
            svc_name = _TCC_SERVICES.get(service, service)
            rows.append((svc_name, client, "Allowed"))
        conn.close()
    except Exception:
        pass
    return sorted(rows)


@click.command()
@click.pass_context
def cmd(ctx):
    """Security health check: FileVault, Firewall, SIP, Gatekeeper, app permissions."""
    home = Path.home()

    # System security checks
    checks = []
    fv_ok, fv_detail = _filevault_status()
    checks.append(_check("FileVault (disk encryption)", fv_ok, fv_detail))
    fw_ok, fw_detail = _firewall_status()
    checks.append(_check("Firewall", fw_ok, fw_detail))
    sip_ok, sip_detail = _sip_status()
    checks.append(_check("System Integrity Protection (SIP)", sip_ok, sip_detail))
    gk_ok, gk_detail = _gatekeeper_status()
    checks.append(_check("Gatekeeper", gk_ok, gk_detail))

    sec_table = Table(show_header=True, show_lines=True)
    sec_table.add_column("Check")
    sec_table.add_column("Status")
    sec_table.add_column("Detail")
    for row in checks:
        sec_table.add_row(*row)
    console.print(Panel(sec_table, title="[bold cyan]System Security[/]"))

    # App permissions from TCC
    perms = _read_tcc_permissions(home)
    if perms:
        perm_table = Table(show_header=True, show_lines=True)
        perm_table.add_column("Permission")
        perm_table.add_column("App / Bundle ID")
        perm_table.add_column("Status")
        for svc, client, status in perms:
            perm_table.add_row(svc, client, f"[green]{status}[/]")
        console.print(Panel(perm_table, title="[bold cyan]App Permissions (TCC)[/]"))
    else:
        console.print("[dim]TCC database not readable without Full Disk Access or sudo.[/]")
