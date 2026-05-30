import copy
from pathlib import Path

_DEFAULT_CONFIG_PATH = Path.home() / ".maccleanrc"

_DEFAULTS = {
    "defaults": {"dry_run": False},
    "skip": {"cleaners": [], "paths": []},
    "profiles": {
        "light": {"cleaners": ["trash", "crash_reports", "browser", "quicklook", "zsh"]},
        "dev": {"cleaners": [
            "trash", "crash_reports", "brew", "docker", "gradle", "cargo",
            "node", "pip_cache", "python_versions", "xcode", "zsh", "projects",
        ]},
        "deep": {"cleaners": []},
    },
}


def _load_toml(path: Path) -> dict:
    try:
        import tomllib  # Python 3.11+
    except ImportError:
        import tomli as tomllib  # type: ignore
    with open(path, "rb") as f:
        return tomllib.load(f)


def load_config(config_path: Path = _DEFAULT_CONFIG_PATH) -> dict:
    cfg = copy.deepcopy(_DEFAULTS)
    if not config_path.exists():
        return cfg
    try:
        user = _load_toml(config_path)
        for section, values in user.items():
            if section not in cfg:
                cfg[section] = {}
            if isinstance(values, dict):
                cfg[section].update(values)
    except Exception:
        pass
    return cfg


def get_profile_cleaners(
    profile: str,
    config_path: Path = _DEFAULT_CONFIG_PATH,
) -> list[str] | None:
    cfg = load_config(config_path)
    profiles = cfg.get("profiles", {})
    if profile not in profiles:
        return None
    cleaners = profiles[profile].get("cleaners", [])
    return cleaners if cleaners else None
