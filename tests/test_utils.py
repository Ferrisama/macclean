from pathlib import Path
from macclean.core.utils import format_size, dir_size, CleanItem, AnalysisResult


def test_format_size_bytes():
    assert format_size(512) == "512.0 B"


def test_format_size_kb():
    assert format_size(2048) == "2.0 KB"


def test_format_size_mb():
    assert format_size(1024 * 1024) == "1.0 MB"


def test_format_size_gb():
    assert format_size(1024 ** 3) == "1.0 GB"


def test_dir_size_counts_files(tmp_path):
    (tmp_path / "a.txt").write_bytes(b"x" * 100)
    (tmp_path / "sub").mkdir()
    (tmp_path / "sub" / "b.txt").write_bytes(b"x" * 200)
    assert dir_size(tmp_path) == 300


def test_dir_size_missing_path():
    assert dir_size(Path("/nonexistent/path/xyz")) == 0


def test_analysis_result_total():
    r = AnalysisResult()
    r.items.append(CleanItem(label="a", path=Path("/a"), size_bytes=100))
    r.items.append(CleanItem(label="b", path=Path("/b"), size_bytes=200, removable=False))
    assert r.total_bytes == 100
