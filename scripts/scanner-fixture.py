#!/usr/bin/env python3
"""Create a deterministic, disposable scanner workload in a NEW directory."""
import argparse
import json
import os
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("directory", type=Path)
parser.add_argument("--files", type=int, default=10000)
args = parser.parse_args()
if args.files < 1:
    parser.error("--files must be positive")
root = args.directory.resolve()
root.mkdir(parents=True, exist_ok=False)
for branch in ("many-small", "wide", "deep", "links", "sparse", "empty"):
    (root / branch).mkdir()
for i in range(args.files):
    parent = root / "many-small" / f"project-{i % 100:03d}"
    parent.mkdir(exist_ok=True)
    (parent / f"file-{i:08d}").write_bytes(b"x" * (1 + i % 4096))
for i in range(min(args.files, 1000)):
    (root / "wide" / f"empty-{i:04d}").touch()
leaf = root / "deep"
for _ in range(100):
    leaf /= "d"
    leaf.mkdir()
(leaf / "leaf").write_bytes(b"deep")
original = root / "links" / "original"
original.write_bytes(b"linked" * 2048)
for i in range(10):
    os.link(original, root / "links" / f"alias-{i}")
(root / "links" / "directory-symlink").symlink_to(root / "many-small", target_is_directory=True)
(root / "links" / "broken-symlink").symlink_to(root / "missing")
with (root / "sparse" / "one-gib").open("wb") as stream:
    stream.truncate(1024 ** 3)
print(json.dumps({"fixture": str(root), "small_files": args.files,
                  "sparse_logical_bytes": 1024 ** 3, "hard_link_aliases": 10}))
