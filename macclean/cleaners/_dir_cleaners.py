"""
Factory for simple directory-based cleaners that scan known paths,
show a size table, and delete on confirmation.
"""
import shutil
from dataclasses import dataclass, field
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size, run_as_user

console = Console()


@dataclass
class DirCleanerConfig:
    name: str
    title: str
    dirs: list[tuple[str, str]]      # [(label, path_relative_to_home)]
    note: str = ""
    delete_cmd: list[str] = field(default_factory=list)  # if set, run instead of rmtree


def make_dir_cleaner(config: DirCleanerConfig):
    """Return (analyze, clean, cmd) for a directory-based cleaner."""

    def analyze(home: Path | None = None) -> AnalysisResult:
        home = home or Path.home()
        result = AnalysisResult()
        for label, rel in config.dirs:
            path = home / rel
            if path.exists():
                result.items.append(CleanItem(label=label, path=path, size_bytes=dir_size(path)))
        return result

    def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
        if not result.items:
            console.print(f"[green]No {config.name} found.[/]")
            return

        table = Table(show_header=True, show_lines=True)
        table.add_column("Cache")
        table.add_column("Size", justify="right")
        for item in result.items:
            table.add_row(item.label, format_size(item.size_bytes))

        console.print(Panel(table, title=f"[bold cyan]{config.title}[/]"))
        console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")
        if config.note:
            console.print(f"  [dim]{config.note}[/]")

        if dry_run:
            return
        if not yes and not confirm(f"Clear {config.name}?"):
            return

        if config.delete_cmd:
            out, code = run_as_user(config.delete_cmd)
            if code == 0:
                console.print(f"  [green]✓[/] {config.name} cleared")
            else:
                console.print(f"  [yellow]⚠[/] {out}")
        else:
            for item in result.items:
                try:
                    shutil.rmtree(item.path, ignore_errors=True)
                    console.print(f"  [green]✓[/] Cleared {item.label}")
                except Exception as e:
                    console.print(f"  [yellow]⚠[/] {item.label}: {e}")

    @click.command()
    @click.pass_context
    def cmd(ctx):
        result = analyze()
        clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])

    cmd.__doc__ = f"Clear {config.name}."
    return analyze, clean, cmd
