import os
from pathlib import Path
import pytest
from macclean.cleaners.trash import analyze as trash_analyze
from macclean.cleaners.crash_reports import analyze as crash_analyze


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
