#!/usr/bin/env python3
"""A scripted ACP agent for the soak's burst row: SOAK_RECORDS (default 50,000) numbered message chunks for its own
session as fast as it can, then the answer to the prompt. Shapes from scripts/fixtures/m4-acp.py."""
import json, os, sys
out = sys.stdout
def send(v):
    out.write(json.dumps(dict(jsonrpc="2.0", **v)) + "\n")
    out.flush()
def answer(v, result):
    send(dict(id=v["id"], result=result))
for line in sys.stdin:
    v = json.loads(line)
    method = v.get("method")
    if method == "initialize":
        answer(v, dict(protocolVersion=1, agentCapabilities=dict(loadSession=True)))
    elif method in ("session/new", "session/load"):
        answer(v, dict(sessionId="s1"))
    elif method == "session/set_config_option":
        answer(v, dict(configOptions=[]))
    elif method == "session/prompt":
        n = int(os.environ.get("SOAK_RECORDS", "50000"))
        for i in range(n):
            out.write('{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":'
                      '{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"<%d>"}}}}\n' % i)
        out.flush()
        answer(v, dict(stopReason="end_turn"))
    elif method == "session/cancel":
        pass
    elif "id" in v and method:
        send(dict(id=v["id"], error={"code": -32601, "message": "not in the soak fake"}))
