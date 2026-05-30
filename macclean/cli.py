import click
from rich.console import Console

console = Console()


@click.group()
@click.option("--dry-run", is_flag=True, default=False, help="Analyze only, no deletion")
@click.option("--yes", "-y", is_flag=True, default=False, help="Skip confirmation prompts")
@click.option("--json", "as_json", is_flag=True, default=False, help="Output as JSON")
@click.pass_context
def main(ctx, dry_run, yes, as_json):
    """macclean — Mac system maintenance CLI."""
    ctx.ensure_object(dict)
    ctx.obj["dry_run"] = dry_run
    ctx.obj["yes"] = yes
    ctx.obj["as_json"] = as_json


def _register_commands():
    from macclean.cleaners import (
        trash, crash_reports, browser, node, pip_cache, cargo,
        gradle, maven, go_cache, brew, docker, quicklook, memory,
        spotlight, system, timemachine, zsh, python_versions,
        apps, xcode, ios_backups, fonts, stremio,
    )
    from macclean.cleaners import (
        largest, dupes, security, ports, privacy, agents, login_items, wifi, connections,
    )
    from macclean.core import disk

    main.add_command(disk.analyze_cmd, "analyze")
    main.add_command(disk.status_cmd, "status")
    # Cleaning commands
    main.add_command(trash.cmd, "trash")
    main.add_command(crash_reports.cmd, "crash-reports")
    main.add_command(browser.cmd, "browser")
    main.add_command(node.cmd, "node")
    main.add_command(pip_cache.cmd, "pip")
    main.add_command(cargo.cmd, "cargo")
    main.add_command(gradle.cmd, "gradle")
    main.add_command(maven.cmd, "maven")
    main.add_command(go_cache.cmd, "go")
    main.add_command(brew.cmd, "brew")
    main.add_command(docker.cmd, "docker")
    main.add_command(quicklook.cmd, "quicklook")
    main.add_command(memory.cmd, "memory")
    main.add_command(spotlight.cmd, "spotlight")
    main.add_command(system.cmd, "system")
    main.add_command(timemachine.cmd, "timemachine")
    main.add_command(zsh.cmd, "zsh")
    main.add_command(python_versions.cmd, "python")
    main.add_command(apps.cmd, "apps")
    main.add_command(xcode.cmd, "xcode")
    main.add_command(ios_backups.cmd, "ios-backups")
    main.add_command(fonts.cmd, "fonts")
    main.add_command(stremio.cmd, "stremio")
    # Disk intelligence
    main.add_command(largest.cmd, "largest")
    main.add_command(dupes.cmd, "dupes")
    # Security & privacy
    main.add_command(security.cmd, "security")
    main.add_command(ports.cmd, "ports")
    main.add_command(privacy.cmd, "privacy")
    # Startup & background
    main.add_command(agents.cmd, "agents")
    main.add_command(login_items.cmd, "login-items")
    # Network
    main.add_command(wifi.cmd, "wifi")
    main.add_command(connections.cmd, "connections")
    main.add_command(_all_cmd, "all")


@click.command("all")
@click.pass_context
def _all_cmd(ctx):
    """Run every cleaner in sequence, confirming each step."""
    from macclean.cleaners import (
        trash, crash_reports, browser, node, pip_cache, cargo,
        gradle, maven, go_cache, brew, docker, quicklook, memory,
        spotlight, system, timemachine, zsh, python_versions,
        apps, xcode, ios_backups, fonts, stremio,
    )
    dry_run = ctx.obj["dry_run"]
    yes = ctx.obj["yes"]

    runners = [
        trash, crash_reports, quicklook, memory, spotlight,
        system, timemachine, browser, stremio, apps, xcode, ios_backups, fonts,
        brew, docker, python_versions, node, pip_cache, cargo,
        gradle, maven, go_cache, zsh,
    ]

    for module in runners:
        name = module.__name__.split(".")[-1].replace("_", "-")
        console.rule(f"[bold]{name}[/]")
        try:
            result = module.analyze()
            module.clean(result, dry_run=dry_run, yes=yes)
        except SystemExit:
            console.print(f"[yellow]Skipped {name} (needs sudo)[/]")
        except Exception as e:
            console.print(f"[red]Error in {name}:[/] {e}")


_register_commands()
