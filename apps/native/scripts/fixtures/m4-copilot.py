#!/usr/bin/env python3
import pathlib, sys, time
name = sys.argv[1] if len(sys.argv) > 1 else "plain-reply"
root = pathlib.Path(__file__).resolve().parent / "copilot"
path = root / f"{name}.jsonl"
if not path.is_file():
    path = root / "plain-reply.jsonl"
for line in path.read_text(encoding="utf-8").splitlines():
    if not line: continue
    sys.stdout.write(line + "\n"); sys.stdout.flush(); time.sleep(0.001)
sys.exit(0)
