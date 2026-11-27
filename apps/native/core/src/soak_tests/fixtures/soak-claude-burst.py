#!/usr/bin/env python3
"""Synthetic Claude Code stream-json for the soak's burst row: SOAK_RECORDS (default 50,000) numbered text deltas as fast
as it can, then the answer and the result. No Claude, no credentials, no network."""
import json, os, sys

line = sys.stdin.readline()
assert line, "expected the user turn"
out = sys.stdout
def send(v):
    out.write(json.dumps(v) + "\n")
send({"type": "system", "subtype": "init", "session_id": "33b21513-a4fb-478d-ad84-21cb7ef30571", "cwd": "/tmp",
      "tools": ["Read"], "mcp_servers": [], "model": "synthetic-model", "permissionMode": "default"})
n = int(os.environ.get("SOAK_RECORDS", "50000"))
for i in range(n):
    out.write('{"type":"stream_event","event":{"type":"content_block_delta","index":0,'
              '"delta":{"type":"text_delta","text":"<%d>"}}}\n' % i)
send({"type": "assistant", "message": {"content": [{"type": "text", "text": "Burst complete."}]}})
send({"type": "result", "subtype": "success", "is_error": False, "result": "Burst complete."})
out.flush()
# Stream-json input stays open until Locust closes it.
sys.stdin.read()
