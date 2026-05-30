from pathlib import Path
from macclean.core.disk import scan_dirs


def test_scan_dirs_returns_sizes(tmp_path):
    (tmp_path / "a.txt").write_bytes(b"x" * 1024)
    results = scan_dirs([("Test Dir", tmp_path)])
    assert len(results) == 1
    label, path, size = results[0]
    assert label == "Test Dir"
    assert size >= 1024


def test_scan_dirs_skips_missing():
    results = scan_dirs([("Missing", Path("/nonexistent/xyz"))])
    assert results == []
