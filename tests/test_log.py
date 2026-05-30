import json
from pathlib import Path
from macclean.core.log import append_log, read_log


def test_append_log_creates_file(tmp_path):
    log_path = tmp_path / "macclean.log"
    append_log("gradle", 21474836480, dry_run=False, log_path=log_path)
    assert log_path.exists()
    record = json.loads(log_path.read_text().strip())
    assert record["cleaner"] == "gradle"
    assert record["bytes_cleaned"] == 21474836480
    assert record["dry_run"] is False
    assert "timestamp" in record


def test_append_log_appends(tmp_path):
    log_path = tmp_path / "macclean.log"
    append_log("brew", 1000, dry_run=False, log_path=log_path)
    append_log("docker", 2000, dry_run=False, log_path=log_path)
    lines = [l for l in log_path.read_text().splitlines() if l.strip()]
    assert len(lines) == 2


def test_read_log_returns_records(tmp_path):
    log_path = tmp_path / "macclean.log"
    append_log("zsh", 500, dry_run=True, log_path=log_path)
    records = read_log(log_path=log_path)
    assert len(records) == 1
    assert records[0]["cleaner"] == "zsh"


def test_read_log_missing_file(tmp_path):
    records = read_log(log_path=tmp_path / "nonexistent.log")
    assert records == []
