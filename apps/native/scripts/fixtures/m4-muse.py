#!/usr/bin/env python3
"""Synthetic Muse Code: replay echo-provider-run.jsonl (BOM + CRLF stripped like the process runner)."""
import pathlib, sys, time
name = sys.argv[1] if len(sys.argv) > 1 else "echo-provider-run"
root = pathlib.Path(__file__).resolve().parent / "muse"
path = root / f"{name}.jsonl"
if not path.is_file():
    path = root / "echo-provider-run.jsonl"
raw = path.read_bytes()
if raw.startswith(b"\xef\xbb\xbf"):
    raw = raw[3:]
text = raw.decode("utf-8")
for line in text.splitlines():
    line = line.rstrip("\r")
    if not line:
        continue
    sys.stdout.write(line + "\n")
    sys.stdout.flush()
    time.sleep(0.001)
sys.exit(0)
