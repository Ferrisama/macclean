from pathlib import Path
from macclean.core.config import load_config, get_profile_cleaners


def test_load_config_missing_returns_defaults(tmp_path):
    cfg = load_config(config_path=tmp_path / ".maccleanrc")
    assert cfg["defaults"]["dry_run"] is False


def test_load_config_reads_toml(tmp_path):
    rc = tmp_path / ".maccleanrc"
    rc.write_text('[defaults]\ndry_run = true\n')
    cfg = load_config(config_path=rc)
    assert cfg["defaults"]["dry_run"] is True


def test_get_profile_cleaners_light(tmp_path):
    rc = tmp_path / ".maccleanrc"
    rc.write_text('[profiles.light]\ncleaners = ["trash", "zsh"]\n')
    cleaners = get_profile_cleaners("light", config_path=rc)
    assert cleaners == ["trash", "zsh"]


def test_get_profile_cleaners_unknown_returns_none(tmp_path):
    cleaners = get_profile_cleaners("nonexistent", config_path=tmp_path / ".maccleanrc")
    assert cleaners is None
