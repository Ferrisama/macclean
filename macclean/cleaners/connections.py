from collections import defaultdict
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import run_cmd

console = Console()


def _parse_connections() -> dict[str, list[dict]]:
    out, code = run_cmd(["lsof", "-i", "-n", "-P", "+c", "0"], timeout=15)
    if code != 0:
        return {}
    by_proc: dict[str, list[dict]] = defaultdict(list)
    for line in out.splitlines()[1:]:
        parts = line.split()
        if len(parts) < 9:
            continue
        proc = parts[0]
        pid = parts[1]
        proto = parts[7] if len(parts) > 7 else ""
        addr = parts[8] if len(parts) > 8 else ""
        state = parts[9] if len(parts) > 9 else ""
        # Only include TCP/UDP with actual addresses
        if not addr or addr == "*:*":
            continue
        by_proc[proc].append({"pid": pid, "proto": proto, "addr": addr, "state": state})
    return by_proc


@click.command()
@click.option("--proc", default=None, help="Filter by process name")
@click.pass_context
def cmd(ctx, proc: str | None):
    """Show active network connections grouped by process."""
    by_proc = _parse_connections()
    if not by_proc:
        console.print("[green]No active network connections found.[/]")
        return

    if proc:
        by_proc = {k: v for k, v in by_proc.items() if proc.lower() in k.lower()}
        if not by_proc:
            console.print(f"[yellow]No connections for process matching '{proc}'[/]")
            return

    total = 0
    for process, conns in sorted(by_proc.items(), key=lambda x: len(x[1]), reverse=True):
        if not conns:
            continue
        table = Table(show_header=True, show_lines=False, box=None, padding=(0, 1))
        table.add_column("Proto", style="dim")
        table.add_column("Address")
        table.add_column("State", style="dim")

        pid = conns[0]["pid"]
        seen = set()
        for c in conns:
            key = (c["proto"], c["addr"])
            if key in seen:
                continue
            seen.add(key)
            state = c["state"]
            state_color = "green" if state == "(ESTABLISHED)" else "yellow" if state == "(LISTEN)" else "dim"
            table.add_row(c["proto"], c["addr"], f"[{state_color}]{state}[/{state_color}]")
            total += 1

        console.print(Panel(table, title=f"[bold cyan]{process}[/] [dim](pid {pid})[/]"))

    console.print(f"  [dim]{total} unique connection(s) across {len(by_proc)} process(es)[/]")
