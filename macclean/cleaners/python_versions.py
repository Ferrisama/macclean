import shutil
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size, run_cmd

console = Console()


def _get_pyenv_root(home: Path) -> Path:
    out, code = run_cmd(["pyenv", "root"])
    if code == 0 and out.strip():
        return Path(out.strip())
    return home / ".pyenv"


def _get_pyenv_versions(pyenv_root: Path) -> list[str]:
    versions_dir = pyenv_root / "versions"
    if not versions_dir.exists():
        return []
    return [d.name for d in versions_dir.iterdir() if d.is_dir()]


def _get_active_version(home: Path) -> str:
    out, code = run_cmd(["pyenv", "version-name"])
    if code == 0:
        return out.strip()
    version_file = home / ".pyenv" / "version"
    if version_file.exists():
        return version_file.read_text().strip()
    return ""


def _has_virtualenvs(version: str, pyenv_root: Path) -> bool:
    envs_dir = pyenv_root / "versions" / version / "envs"
    return envs_dir.exists() and any(envs_dir.iterdir())


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()

    if not shutil.which("pyenv"):
        return result

    pyenv_root = _get_pyenv_root(home)
    active = _get_active_version(home)
    versions = _get_pyenv_versions(pyenv_root)

    for version in versions:
        version_path = pyenv_root / "versions" / version
        size = dir_size(version_path)
        has_envs = _has_virtualenvs(version, pyenv_root)
        is_active = version == active
        removable = not is_active and not has_envs
        label = version
        if is_active:
            label += " [active]"
        if has_envs:
            label += " [has virtualenvs]"
        result.items.append(CleanItem(
            label=label,
            path=version_path,
            size_bytes=size,
            removable=removable,
        ))

    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not shutil.which("pyenv"):
        console.print("[yellow]pyenv not found — skipping.[/]")
        return

    removable = [i for i in result.items if i.removable]

    table = Table(show_header=True, show_lines=True)
    table.add_column("Python Version")
    table.add_column("Size", justify="right")
    table.add_column("Action")
    for item in result.items:
        action = "[red]Remove[/]" if item.removable else "[dim]Keep[/]"
        table.add_row(item.label, format_size(item.size_bytes), action)

    console.print(Panel(table, title="[bold cyan]Python Versions (pyenv)[/]"))

    if not removable:
        console.print("[green]No unused Python versions found.[/]")
        return

    console.print(f"  Removable: {len(removable)} version(s), [bold]{format_size(sum(i.size_bytes for i in removable))}[/]")

    if dry_run:
        return
    if not yes and not confirm(f"Uninstall {len(removable)} unused Python version(s)?"):
        return

    for item in removable:
        version = item.path.name
        out, code = run_cmd(["pyenv", "uninstall", "-f", version], timeout=60)
        if code == 0:
            console.print(f"  [green]+[/] Removed Python {version}")
        else:
            console.print(f"  [yellow]![/] {version}: {out}")


@click.command()
@click.pass_context
def cmd(ctx):
    """Remove unused pyenv Python versions."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
