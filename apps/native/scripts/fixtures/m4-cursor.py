#!/usr/bin/env python3
"""Synthetic cursor-agent: replay a captured stream-json fixture on stdout."""
import pathlib
import sys
import time

name = sys.argv[1] if len(sys.argv) > 1 else "read-only-partial"
# Drain stdin (prompt) so the host's close is observed.
try:
    sys.stdin.read()
except Exception:
    pass
root = pathlib.Path(__file__).resolve().parent / "cursor"
path = root / f"{name}.jsonl"
if not path.is_file():
    path = root / "read-only-partial.jsonl"
for line in path.read_text(encoding="utf-8").splitlines():
    if not line:
        continue
    sys.stdout.write(line + "\n")
    sys.stdout.flush()
    time.sleep(0.001)
sys.exit(0)
