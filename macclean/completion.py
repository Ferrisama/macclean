import os
import subprocess
import sys
from pathlib import Path


_ZSH_SNIPPET = '''
# macclean shell completion
eval "$(_MACCLEAN_COMPLETE=zsh_source macclean)"
'''

_BASH_SNIPPET = '''
# macclean shell completion
eval "$(_MACCLEAN_COMPLETE=bash_source macclean)"
'''


def install():
    """Install shell tab completion for macclean."""
    shell = Path(os.environ.get("SHELL", "")).name

    if shell == "zsh":
        rc = Path.home() / ".zshrc"
        snippet = _ZSH_SNIPPET
    elif shell == "bash":
        rc = Path.home() / ".bashrc"
        snippet = _BASH_SNIPPET
    else:
        print(f"Shell '{shell}' not supported. Add this manually to your shell rc:")
        print(_ZSH_SNIPPET)
        sys.exit(1)

    content = rc.read_text() if rc.exists() else ""
    if "_MACCLEAN_COMPLETE" in content:
        print(f"Completion already installed in {rc}")
        return

    with open(rc, "a") as f:
        f.write(snippet)
    print(f"✓ Completion installed in {rc}")
    print(f"  Restart your shell or run: source {rc}")
