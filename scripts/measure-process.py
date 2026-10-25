#!/usr/bin/env python3
"""Measure one child process with wait4; stdout/stderr remain caller-controlled."""
import json
import os
from pathlib import Path
import sys
import time

if len(sys.argv) < 4 or sys.argv[2] != "--":
    sys.exit("usage: measure-process.py TIMING_JSON -- EXECUTABLE [ARGS...]")
started = time.perf_counter_ns()
pid = os.posix_spawnp(sys.argv[3], sys.argv[3:], os.environ)
while True:
    try:
        _, status, usage = os.wait4(pid, 0)
        break
    except InterruptedError:
        continue
returncode = os.waitstatus_to_exitcode(status)
Path(sys.argv[1]).write_text(json.dumps({
    "real_seconds": (time.perf_counter_ns() - started) / 1e9,
    "peak_rss_bytes": int(usage.ru_maxrss) * (1 if sys.platform == "darwin" else 1024),
    "exit_code": returncode,
    "measurement": "wait4 child resource usage; macOS RSS bytes, Linux KiB converted to bytes"
}) + "\n")
sys.exit(returncode if returncode >= 0 else 128 - returncode)
