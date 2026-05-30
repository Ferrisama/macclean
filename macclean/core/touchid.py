from pathlib import Path

_PAM_SUDO = Path("/etc/pam.d/sudo_local")
_PAM_LINE = "auth       sufficient     pam_tid.so\n"


def is_touchid_enabled() -> bool:
    if not _PAM_SUDO.exists():
        return False
    return "pam_tid.so" in _PAM_SUDO.read_text()


def enable_touchid() -> tuple[bool, str]:
    if is_touchid_enabled():
        return True, "Touch ID already enabled for sudo."

    template = Path("/etc/pam.d/sudo_local.template")
    if template.exists():
        content = template.read_text()
    else:
        content = "# sudo_local: local config file which survives system update and for Touch ID\n"

    content = _PAM_LINE + content
    try:
        _PAM_SUDO.write_text(content)
        return True, f"Touch ID enabled. Written to {_PAM_SUDO}"
    except PermissionError:
        return False, "Permission denied. Run: sudo macclean touchid"
