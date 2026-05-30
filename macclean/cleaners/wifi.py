import re
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import run_cmd

console = Console()

_AIRPORT = "/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport"


def _airport_info() -> dict[str, str]:
    out, code = run_cmd([_AIRPORT, "-I"], timeout=10)
    if code != 0:
        return {}
    info = {}
    for line in out.splitlines():
        if ":" in line:
            key, _, val = line.partition(":")
            info[key.strip()] = val.strip()
    return info


def _get_dns() -> list[str]:
    out, _ = run_cmd(["scutil", "--dns"], timeout=5)
    servers = []
    for line in out.splitlines():
        m = re.search(r"nameserver\[\d+\]\s*:\s*(\S+)", line)
        if m:
            s = m.group(1)
            if s not in servers:
                servers.append(s)
    return servers[:4]


def _get_interface() -> str:
    out, _ = run_cmd(["route", "get", "default"], timeout=5)
    for line in out.splitlines():
        if "interface:" in line:
            return line.split("interface:")[-1].strip()
    return "en0"


@click.command()
@click.pass_context
def cmd(ctx):
    """Show current Wi-Fi network, signal, channel, and DNS."""
    info = _airport_info()
    iface = _get_interface()
    dns = _get_dns()

    table = Table(show_header=False, show_lines=True, box=None, padding=(0, 2))
    table.add_column("Key", style="bold")
    table.add_column("Value")

    ssid = info.get("SSID", "Not connected")
    rssi = info.get("agrCtlRSSI", "")
    noise = info.get("agrCtlNoise", "")
    channel = info.get("channel", "")
    bssid = info.get("BSSID", "")
    tx_rate = info.get("lastTxRate", "")
    phy = info.get("op mode", "")
    country = info.get("countryCode", "")

    # Signal quality
    if rssi:
        dbm = int(rssi)
        if dbm >= -50:
            sig = f"[green]{rssi} dBm (Excellent)[/]"
        elif dbm >= -70:
            sig = f"[yellow]{rssi} dBm (Good)[/]"
        else:
            sig = f"[red]{rssi} dBm (Weak)[/]"
    else:
        sig = "—"

    rows = [
        ("SSID", ssid),
        ("Interface", iface),
        ("Signal", sig),
        ("Noise", f"{noise} dBm" if noise else "—"),
        ("Channel", channel or "—"),
        ("BSSID", bssid or "—"),
        ("TX Rate", f"{tx_rate} Mbps" if tx_rate else "—"),
        ("Mode", phy or "—"),
        ("Country", country or "—"),
        ("DNS Servers", ", ".join(dns) if dns else "—"),
    ]

    for key, val in rows:
        table.add_row(key, val)

    console.print(Panel(table, title="[bold cyan]Wi-Fi Status[/]"))
