import plistlib
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel

console = Console()

_AGENT_DIRS = [
    ("User LaunchAgents", Path.home() / "Library" / "LaunchAgents"),
    ("System LaunchAgents", Path("/Library/LaunchAgents")),
    ("System LaunchDaemons", Path("/Library/LaunchDaemons")),
]


def _parse_plist(path: Path) -> dict:
    try:
        with open(path, "rb") as f:
            return plistlib.load(f)
    except Exception:
        return {}


def _check_agent(plist_path: Path) -> tuple[str, str, str, str]:
    data = _parse_plist(plist_path)
    label = data.get("Label", plist_path.stem)
    program = data.get("Program") or (data.get("ProgramArguments") or [""])[0]
    program = str(program)

    # Check if program binary exists
    from pathlib import Path as P
    prog_path = P(program.split()[0]) if program else None
    if program and prog_path and not prog_path.exists():
        status = "[red]Binary missing[/]"
    elif not program:
        status = "[yellow]? No program set[/]"
    else:
        status = "[green]OK[/]"

    disabled = data.get("Disabled", False)
    if disabled:
        status = "[dim]Disabled[/]"

    return label, program[:60] or "—", status, plist_path.name


@click.command()
@click.pass_context
def cmd(ctx):
    """List LaunchAgents and LaunchDaemons, flagging broken or missing binaries."""
    for section_label, directory in _AGENT_DIRS:
        if not directory.exists():
            continue

        plists = sorted(directory.glob("*.plist"))
        if not plists:
            continue

        table = Table(show_header=True, show_lines=True)
        table.add_column("Label")
        table.add_column("Program")
        table.add_column("Status")

        broken = 0
        for plist_path in plists:
            label, program, status, _ = _check_agent(plist_path)
            table.add_row(label, program, status)
            if "missing" in status or "?" in status:
                broken += 1

        title = f"[bold cyan]{section_label}[/] ({len(plists)} items"
        title += f", [red]{broken} broken[/])" if broken else ")"
        console.print(Panel(table, title=title))
