import os
from pathlib import Path
import pytest
from unittest.mock import patch
from macclean.cleaners.trash import analyze as trash_analyze
from macclean.cleaners.crash_reports import analyze as crash_analyze
from macclean.cleaners.browser import analyze as browser_analyze
from macclean.cleaners.node import analyze as node_analyze
from macclean.cleaners.cargo import analyze as cargo_analyze
from macclean.cleaners.brew import analyze as brew_analyze
from macclean.cleaners.docker import analyze as docker_analyze


def test_trash_analyze_finds_files(tmp_path):
    trash = tmp_path / ".Trash"
    trash.mkdir()
    (trash / "old.txt").write_bytes(b"x" * 500)
    result = trash_analyze(home=tmp_path)
    assert result.total_bytes >= 500
    assert any("Trash" in item.label for item in result.items)


def test_trash_analyze_empty_trash(tmp_path):
    trash = tmp_path / ".Trash"
    trash.mkdir()
    result = trash_analyze(home=tmp_path)
    assert result.total_bytes == 0


def test_crash_analyze_finds_dir(tmp_path):
    reports = tmp_path / "Library" / "Logs" / "DiagnosticReports"
    reports.mkdir(parents=True)
    (reports / "crash.log").write_bytes(b"x" * 1000)
    result = crash_analyze(home=tmp_path)
    assert result.total_bytes >= 1000


def test_browser_analyze_finds_safari_cache(tmp_path):
    safari = tmp_path / "Library" / "Caches" / "com.apple.Safari"
    safari.mkdir(parents=True)
    (safari / "cache.db").write_bytes(b"x" * 2000)
    result = browser_analyze(home=tmp_path)
    assert any("Safari" in item.label for item in result.items)
    assert result.total_bytes >= 2000


def test_node_analyze_finds_npm(tmp_path):
    npm = tmp_path / ".npm"
    npm.mkdir()
    (npm / "package.json").write_bytes(b"x" * 100)
    result = node_analyze(home=tmp_path)
    assert any("npm" in item.label for item in result.items)


def test_cargo_analyze_finds_registry(tmp_path):
    reg = tmp_path / ".cargo" / "registry" / "cache"
    reg.mkdir(parents=True)
    (reg / "crate.tar.gz").write_bytes(b"x" * 5000)
    result = cargo_analyze(home=tmp_path)
    assert result.total_bytes >= 5000


def test_brew_analyze_skips_when_not_installed():
    import shutil
    with patch("macclean.cleaners.brew.shutil.which", return_value=None):
        result = brew_analyze()
    assert result.total_bytes == 0


def test_docker_analyze_skips_when_not_running():
    with patch("macclean.cleaners.docker.run_cmd", return_value=("Cannot connect to daemon", 1)):
        result = docker_analyze()
    assert result.total_bytes == 0


# ZSH tests
from macclean.cleaners.zsh import analyze as zsh_analyze, _dedup_lines


def test_dedup_lines_preserves_order():
    lines = ["a", "b", "a", "c", "b", "d"]
    assert _dedup_lines(lines) == ["a", "b", "c", "d"]


def test_dedup_lines_empty():
    assert _dedup_lines([]) == []


def test_zsh_analyze_finds_history(tmp_path):
    history = tmp_path / ".zsh_history"
    history.write_text(": 1000:0;ls\n: 1001:0;ls\n: 1002:0;pwd\n")
    result = zsh_analyze(home=tmp_path)
    assert any("history" in item.label.lower() for item in result.items)


# Python versions tests
from macclean.cleaners.python_versions import _get_pyenv_versions, _has_virtualenvs


def test_get_pyenv_versions_empty(tmp_path):
    versions_dir = tmp_path / ".pyenv" / "versions"
    versions_dir.mkdir(parents=True)
    versions = _get_pyenv_versions(pyenv_root=tmp_path / ".pyenv")
    assert versions == []


def test_get_pyenv_versions_lists_dirs(tmp_path):
    versions_dir = tmp_path / ".pyenv" / "versions"
    (versions_dir / "3.10.12").mkdir(parents=True)
    (versions_dir / "3.11.0").mkdir(parents=True)
    versions = _get_pyenv_versions(pyenv_root=tmp_path / ".pyenv")
    assert "3.10.12" in versions
    assert "3.11.0" in versions


def test_has_virtualenvs_true(tmp_path):
    envs = tmp_path / ".pyenv" / "versions" / "3.10.12" / "envs"
    envs.mkdir(parents=True)
    (envs / "myenv").mkdir()
    assert _has_virtualenvs("3.10.12", pyenv_root=tmp_path / ".pyenv") is True


def test_has_virtualenvs_false(tmp_path):
    (tmp_path / ".pyenv" / "versions" / "3.10.12").mkdir(parents=True)
    assert _has_virtualenvs("3.10.12", pyenv_root=tmp_path / ".pyenv") is False


# Apps, Xcode, iOS Backups, Fonts tests
from macclean.cleaners.apps import analyze as apps_analyze, _get_installed_bundle_ids
from macclean.cleaners.xcode import analyze as xcode_analyze
from macclean.cleaners.ios_backups import analyze as ios_analyze
from macclean.cleaners.fonts import analyze as fonts_analyze, _find_duplicates


def test_get_installed_bundle_ids_empty(tmp_path):
    ids = _get_installed_bundle_ids(apps_dirs=[tmp_path])
    assert ids == set()


def test_apps_analyze_finds_orphans(tmp_path):
    support = tmp_path / "Library" / "Application Support" / "com.example.GoneApp"
    support.mkdir(parents=True)
    (support / "data.db").write_bytes(b"x" * 100)
    result = apps_analyze(home=tmp_path, apps_dirs=[])
    assert any("GoneApp" in item.label for item in result.items)


def test_xcode_analyze_finds_derived_data(tmp_path):
    dd = tmp_path / "Library" / "Developer" / "Xcode" / "DerivedData" / "MyApp-abc123"
    dd.mkdir(parents=True)
    (dd / "artifact").write_bytes(b"x" * 10000)
    result = xcode_analyze(home=tmp_path)
    assert any("DerivedData" in item.label for item in result.items)
    assert result.total_bytes >= 10000


def test_ios_analyze_finds_backups(tmp_path):
    backup_dir = tmp_path / "Library" / "Application Support" / "MobileSync" / "Backup" / "abc123"
    backup_dir.mkdir(parents=True)
    (backup_dir / "Info.plist").write_bytes(b"x" * 500)
    result = ios_analyze(home=tmp_path)
    assert len(result.items) >= 1


def test_find_duplicates_detects_same_name(tmp_path):
    dir_a = tmp_path / "FontsA"
    dir_b = tmp_path / "FontsB"
    dir_a.mkdir(); dir_b.mkdir()
    (dir_a / "Arial.ttf").write_bytes(b"x" * 100)
    (dir_b / "Arial.ttf").write_bytes(b"x" * 100)
    dupes = _find_duplicates([dir_a, dir_b])
    assert "Arial.ttf" in dupes


def test_find_duplicates_no_dupes(tmp_path):
    d = tmp_path / "Fonts"
    d.mkdir()
    (d / "Helvetica.ttf").write_bytes(b"x" * 100)
    dupes = _find_duplicates([d])
    assert dupes == {}
