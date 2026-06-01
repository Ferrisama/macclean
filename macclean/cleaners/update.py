import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.panel import Panel
from macclean.core.utils import run_as_user

console = Console()


@click.command()
@click.option("--brew/--no-brew", default=True, show_default=True)
@click.option("--pip/--no-pip", "do_pip", default=True, show_default=True)
@click.option("--npm/--no-npm", default=True, show_default=True)
@click.pass_context
def cmd(ctx, brew: bool, do_pip: bool, npm: bool):
    """Upgrade all outdated packages across Homebrew, pip, and npm."""
    console.print(Panel("[bold cyan]Package Updates[/]"))

    if brew and shutil.which("brew"):
        console.print("\n[bold]Homebrew[/]")
        out, code = run_as_user(["brew", "upgrade"], timeout=300)
        if code == 0:
            console.print("  [green]+[/] brew upgrade complete")
        else:
            console.print(f"  [yellow]![/] brew upgrade: {out[:300]}")

    if do_pip:
        console.print("\n[bold]pip[/]")
        out, _ = run_as_user(["pip", "list", "--outdated", "--format=freeze"])
        pkgs = [line.split("==")[0] for line in out.splitlines() if "==" in line]
        if pkgs:
            _, code = run_as_user(["pip", "install", "--upgrade"] + pkgs, timeout=120)
            if code == 0:
                console.print(f"  [green]+[/] Upgraded {len(pkgs)} pip package(s)")
            else:
                console.print(f"  [yellow]![/] pip upgrade failed")
        else:
            console.print("  [green]+[/] All pip packages up to date")

    if npm and shutil.which("npm"):
        console.print("\n[bold]npm[/]")
        out, code = run_as_user(["npm", "update", "-g"], timeout=120)
        if code == 0:
            console.print("  [green]+[/] npm global packages updated")
        else:
            console.print(f"  [yellow]![/] npm update: {out[:200]}")
