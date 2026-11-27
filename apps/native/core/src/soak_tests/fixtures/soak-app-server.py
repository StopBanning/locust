#!/usr/bin/env python3
"""A fast synthetic Codex app-server for the soak tests (no Codex, no credentials, no network).

Shapes from scripts/fixtures/m1-app-server.py, without its 0.4 s pause or its policy asserts, so hundreds of turns fit in
a test. Scenarios (argv[1]):
  quick         answers "Soak answer." at once (start or follow-up)
  long          a turn written over ~2 s: 200 deltas and 20 command rows, each a few ms apart (killed mid-turn, row 3)
  burst         SOAK_RECORDS (default 50,000) numbered deltas as fast as it can, then the answer and the end (row 2)
SOAK_PID_FILE, when set, is told this process's pid.
"""
import json, os, sys, time

if os.environ.get("SOAK_PID_FILE"):
    with open(os.environ["SOAK_PID_FILE"], "w") as f:
        f.write(str(os.getpid()))
scenario = sys.argv[1] if len(sys.argv) > 1 else "quick"
out = sys.stdout


def send(v):
    out.write(json.dumps(dict(jsonrpc="2.0", **v)) + "\n")
    out.flush()


def notify(method, **params):
    send(dict(method=method, params=dict(threadId="thread_soak", **params)))


def item(done, **fields):
    notify("item/completed" if done else "item/started", item=fields)


for line in sys.stdin:
    msg = json.loads(line)
    method, p = msg.get("method"), msg.get("params", {})
    if method == "initialize":
        send(dict(id=msg["id"], result={}))
    elif method == "initialized":
        pass
    elif method == "config/read":
        send(dict(id=msg["id"], result={"config": {}}))
    elif method in ("thread/start", "thread/resume", "thread/fork"):
        send(dict(id=msg["id"], result={"thread": {"id": "thread_soak"}}))
    elif method == "turn/start":
        send(dict(id=msg["id"], result={"turn": {"id": "turn_soak"}}))
        notify("turn/started", turn={"id": "turn_soak"})
        if scenario == "long":
            for i in range(200):
                notify("item/agentMessage/delta", itemId="answer", delta="w%03d " % i)
                if i % 10 == 9:
                    k = "cmd%d" % i
                    item(False, id=k, type="commandExecution", command="echo %d" % i, aggregatedOutput="", exitCode=None)
                    item(True, id=k, type="commandExecution", status="completed", command="echo %d" % i,
                         aggregatedOutput=str(i), exitCode=0)
                time.sleep(0.01)
            text = "".join("w%03d " % i for i in range(200))
        elif scenario == "burst":
            n = int(os.environ.get("SOAK_RECORDS", "50000"))
            for i in range(n):
                out.write('{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"thread_soak",'
                          '"itemId":"answer","delta":"<%d>"}}\n' % i)
            out.flush()
            text = "Burst complete."
        else:
            text = "Soak answer."
        item(True, id="answer", type="agentMessage", text=text)
        notify("thread/tokenUsage/updated", tokenUsage={"total": {"inputTokens": 12, "outputTokens": 4},
                                                         "last": {"inputTokens": 12, "outputTokens": 4}})
        notify("turn/completed", turn={"id": "turn_soak", "status": "completed"})
    elif method == "turn/interrupt":
        send(dict(id=msg["id"], result={}))
        notify("turn/completed", turn={"id": "turn_soak", "status": "interrupted"})
    elif "id" in msg and method:
        send(dict(id=msg["id"], error={"code": -32601, "message": "not in the soak fake: " + str(method)}))
