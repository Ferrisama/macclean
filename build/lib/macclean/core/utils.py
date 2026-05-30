import os
import sys
import subprocess
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class CleanItem:
    label: str
    path: Path
    size_bytes: int
    removable: bool = True


@dataclass
class AnalysisResult:
    items: list[CleanItem] = field(default_factory=list)

    @property
    def total_bytes(self) -> int:
        return sum(i.size_bytes for i in self.items if i.removable)


def require_sudo() -> None:
    if os.geteuid() != 0:
        from rich.console import Console
        cmd = "sudo macclean " + " ".join(sys.argv[1:])
        Console().print(f"[yellow]This command needs root. Re-run with:[/] {cmd}")
        sys.exit(1)


def format_size(n: int) -> str:
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if n < 1024:
            return f"{n:.1f} {unit}"
        n /= 1024
    return f"{n:.1f} PB"


def confirm(prompt: str, default: bool = False) -> bool:
    from rich.console import Console
    hint = "[Y/n]" if default else "[y/N]"
    Console().print(f"{prompt} {hint} ", end="")
    ans = input().strip().lower()
    if not ans:
        return default
    return ans in ("y", "yes")


def run_cmd(args: list[str], timeout: int = 60) -> tuple[str, int]:
    try:
        r = subprocess.run(args, capture_output=True, text=True, timeout=timeout)
        return (r.stdout + r.stderr).strip(), r.returncode
    except subprocess.TimeoutExpired:
        return f"Timed out after {timeout}s", 1
    except FileNotFoundError:
        return f"Command not found: {args[0]}", 1


def run_as_user(args: list[str], timeout: int = 60) -> tuple[str, int]:
    """Run a command as the original (non-root) user, even when invoked via sudo."""
    sudo_user = os.environ.get("SUDO_USER")
    if sudo_user and os.geteuid() == 0:
        args = ["sudo", "-u", sudo_user] + args
    return run_cmd(args, timeout=timeout)


def dir_size(path: Path) -> int:
    total = 0
    try:
        for entry in os.scandir(path):
            if entry.is_dir(follow_symlinks=False):
                total += dir_size(Path(entry.path))
            elif entry.is_file(follow_symlinks=False):
                try:
                    total += entry.stat().st_size
                except OSError:
                    pass
    except (PermissionError, FileNotFoundError, OSError):
        pass
    return total
