import json
from datetime import datetime, timezone
from pathlib import Path

_DEFAULT_LOG = Path.home() / ".macclean.log"


def append_log(
    cleaner: str,
    bytes_cleaned: int,
    dry_run: bool,
    log_path: Path = _DEFAULT_LOG,
) -> None:
    record = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "cleaner": cleaner,
        "bytes_cleaned": bytes_cleaned,
        "dry_run": dry_run,
    }
    with open(log_path, "a") as f:
        f.write(json.dumps(record) + "\n")


def read_log(log_path: Path = _DEFAULT_LOG, limit: int = 100) -> list[dict]:
    if not log_path.exists():
        return []
    records = []
    for line in log_path.read_text().splitlines():
        line = line.strip()
        if line:
            try:
                records.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return records[-limit:]
