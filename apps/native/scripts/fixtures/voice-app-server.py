"""Fake Codex realtime app-server: local JSON streams only."""
import json
import os
from pathlib import Path
import select
import sys

root = Path(sys.argv[1])
case = sys.argv[2]
stdin = open(0, "rb", buffering=0)
transcript = root / "wire"
notifications = root / "notifications"

def send(**message):
    print(json.dumps({"jsonrpc": "2.0", **message}), flush=True)

def notify(short, **params):
    send(method=f"thread/realtime/{short}", params={"threadId": "thread_1", **params})

def receive(message):
    with transcript.open("a") as log:
        log.write(json.dumps(message) + "\n")
    method = message.get("method")
    if "id" not in message or method is None:
        return
    result = {}
    if method == "initialize" and case == "no-initialize":
        return
    if method == "account/read":
        result = {"account": {"type": "apiKey" if case == "key" else "chatgpt"}}
    if method == "config/read":
        if case == "no-config":
            send(id=message["id"], error={"code": -32000, "message": "no config"})
            return
        result = {"config": {"mcp_servers": {"robinhood": {}, "remote_notes": {"url": "https://notes.example.test"}}}}
    if method == "thread/start":
        result = {"thread": {"id": "thread_1"}}
    if method == "thread/realtime/start":
        if case == "service-error":
            notify("error", message="realtime conversation requires API key auth")
        else:
            notify("sdp", sdp="v=0\r\nanswer")
            if case == "closed-on-start":
                notify("closed", reason="transport")
    if method == "thread/realtime/stop" and case == "no-stop":
        return
    send(id=message["id"], result=result)

with (root / "pids").open("a") as pids:
    pids.write(f"{os.getpid()}\n")
buffer = b""
offset = 0
while True:
    if select.select([stdin], [], [], 0.01)[0]:
        chunk = stdin.read(65536)
        if not chunk:
            break
        buffer += chunk
        while b"\n" in buffer:
            line, buffer = buffer.split(b"\n", 1)
            receive(json.loads(line))
    with notifications.open("rb") as offered:
        offered.seek(offset)
        for line in offered:
            if not line.endswith(b"\n"):
                break
            send(**json.loads(line))
            offset += len(line)
