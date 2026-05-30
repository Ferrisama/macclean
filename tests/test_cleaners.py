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
