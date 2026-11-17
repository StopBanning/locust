#!/usr/bin/env python3
"""Synthetic locust-apple-model: reads the one JSON request on stdin, as the Swift helper does, and answers in its lines.

argv[1] picks the answer:
  echo         the prompt it was sent, back, in two deltas, then done
  replace      a delta, then a replace that rewrites it, then done
  error:KIND   an error line of that kind (unavailable takes a reason after a second colon), exit 1
  silent       exits 3 having said nothing
  slow         one delta, then waits to be stopped
If LOCUST_FAKE_APPLE_SAW is set, the prompt it received is written to that file (appended, one JSON line each).
"""
import json, os, sys, time

mode = sys.argv[1] if len(sys.argv) > 1 else "echo"
request = json.loads(sys.stdin.read() or "{}")
prompt = request.get("prompt", "")
saw = os.environ.get("LOCUST_FAKE_APPLE_SAW")
if saw:
    with open(saw, "a") as f:
        f.write(json.dumps(prompt) + "\n")

def say(**line):
    sys.stdout.write(json.dumps(line) + "\n")
    sys.stdout.flush()

if mode == "echo":
    answer = "You said: " + prompt[-60:]
    half = len(answer) // 2
    say(type="delta", text=answer[:half])
    say(type="delta", text=answer[half:])
    say(type="done", text=answer)
elif mode == "replace":
    say(type="delta", text="Red, blue")
    say(type="replace", text="Red, yellow, blue.")
    say(type="done", text="Red, yellow, blue.")
elif mode.startswith("error:"):
    parts = mode.split(":")
    line = {"type": "error", "kind": parts[1], "message": "synthetic"}
    if len(parts) > 2:
        line["reason"] = parts[2]
    say(**line)
    sys.exit(1)
elif mode == "silent":
    sys.exit(3)
elif mode == "slow":
    say(type="delta", text="Thinking about it")
    time.sleep(60)
