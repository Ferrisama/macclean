from collections import defaultdict
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import run_cmd

console = Console()


def _parse_lsof() -> list[dict]:
    out, code = run_cmd(["lsof", "-i", "-n", "-P", "-sTCP:LISTEN"], timeout=15)
    if code != 0:
        return []
    entries = []
    for line in out.splitlines()[1:]:
        parts = line.split()
        if len(parts) < 9:
            continue
        proc = parts[0]
        pid = parts[1]
        addr = parts[8] if len(parts) > 8 else ""
        proto = parts[7] if len(parts) > 7 else ""
        entries.append({"proc": proc, "pid": pid, "addr": addr, "proto": proto})
    return entries


@click.command()
@click.pass_context
def cmd(ctx):
    """Show all open listening ports and the process behind each."""
    entries = _parse_lsof()
    if not entries:
        console.print("[green]No listening ports found (or lsof unavailable).[/]")
        return

    # Deduplicate by (proc, addr)
    seen = set()
    unique = []
    for e in entries:
        key = (e["proc"], e["addr"])
        if key not in seen:
            seen.add(key)
            unique.append(e)

    unique.sort(key=lambda x: (x["proc"].lower(), x["addr"]))

    table = Table(show_header=True, show_lines=True)
    table.add_column("Process")
    table.add_column("PID", justify="right")
    table.add_column("Address / Port")
    table.add_column("Proto")

    for e in unique:
        table.add_row(e["proc"], e["pid"], e["addr"], e["proto"])

    console.print(Panel(table, title="[bold cyan]Open Listening Ports[/]"))
    console.print(f"  [dim]{len(unique)} listening socket(s)[/]")
