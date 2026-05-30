from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from macclean.core.utils import CleanItem, AnalysisResult, format_size, confirm, dir_size

console = Console()


def _dedup_lines(lines: list[str]) -> list[str]:
    seen: set[str] = set()
    result = []
    for line in lines:
        if line not in seen:
            seen.add(line)
            result.append(line)
    return result


def analyze(home: Path | None = None) -> AnalysisResult:
    home = home or Path.home()
    result = AnalysisResult()

    history = home / ".zsh_history"
    if history.exists():
        size = history.stat().st_size
        lines = history.read_text(errors="replace").splitlines()
        deduped = _dedup_lines(lines)
        saved = size - len("\n".join(deduped).encode())
        if saved > 0:
            result.items.append(CleanItem(
                label=f"~/.zsh_history ({len(lines) - len(deduped)} duplicate entries)",
                path=history,
                size_bytes=max(saved, 0),
            ))

    compcache = home / ".zcompcache"
    if compcache.exists():
        result.items.append(CleanItem(
            label="~/.zcompcache (completion cache)",
            path=compcache,
            size_bytes=dir_size(compcache),
        ))

    for dump in home.glob(".zcompdump*"):
        result.items.append(CleanItem(
            label=f"~/{dump.name} (completion dump)",
            path=dump,
            size_bytes=dump.stat().st_size,
        ))

    return result


def clean(result: AnalysisResult, dry_run: bool = False, yes: bool = False) -> None:
    if not result.items:
        console.print("[green]ZSH files are already clean.[/]")
        return

    table = Table(show_header=True, show_lines=True)
    table.add_column("Item")
    table.add_column("Recoverable", justify="right")
    for item in result.items:
        table.add_row(item.label, format_size(item.size_bytes))

    console.print(Panel(table, title="[bold cyan]ZSH Cleanup[/]"))
    console.print(f"  Total recoverable: [bold]{format_size(result.total_bytes)}[/]")

    if dry_run:
        return
    if not yes and not confirm("Clean ZSH history and caches?"):
        return

    import shutil
    home = Path.home()
    history = home / ".zsh_history"
    if history.exists():
        lines = history.read_text(errors="replace").splitlines()
        deduped = _dedup_lines(lines)
        history.write_text("\n".join(deduped) + "\n")
        console.print(f"  [green]✓[/] History deduped ({len(lines) - len(deduped)} entries removed)")

    compcache = home / ".zcompcache"
    if compcache.exists():
        shutil.rmtree(compcache, ignore_errors=True)
        console.print("  [green]✓[/] Removed ~/.zcompcache")

    for dump in home.glob(".zcompdump*"):
        dump.unlink(missing_ok=True)
    console.print("  [green]✓[/] Removed zcompdump files (zsh rebuilds on next open)")


@click.command()
@click.pass_context
def cmd(ctx):
    """Deduplicate zsh history and clean completion caches."""
    result = analyze()
    clean(result, dry_run=ctx.obj["dry_run"], yes=ctx.obj["yes"])
