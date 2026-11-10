#!/usr/bin/env python3
"""Synthetic Claude Code stream-json. Shapes from packages/runtime-adapters/test/claude-events.test.ts.
No Claude invocation, credentials, or network. Asserts Ask-mode argv from createClaudePrintCommand.
"""
import json, os, signal, sys, time

if os.environ.get("M3_PID_FILE"):
    with open(os.environ["M3_PID_FILE"], "w") as f:
        f.write(str(os.getpid()))

scenario = os.environ.get("M3_SCENARIO") or (sys.argv[1] if len(sys.argv) > 1 else "success")
PROMPT = "Synthetic prompt"

# createClaudePrintCommand Ask (read-only) argv checks.
argv = sys.argv[1:]
if os.environ.get("CLAUDE_ARGV_FILE"):
    with open(os.environ["CLAUDE_ARGV_FILE"]) as f:
        flags = [line.rstrip("\n") for line in f]
else:
    flags = list(argv)
KNOWN = ("success", "failure", "lost", "interrupt", "limit", "tools", "thinking", "approval", "approval-allow", "approval-deny")
if flags and flags[0] in KNOWN:
    scenario = flags[0]
    flags = flags[1:]
# Scenario can also come from the environment (CLI smoke shim).
scenario = os.environ.get("M3_SCENARIO", scenario)
scenario = os.environ.get("CLAUDE_SCENARIO", scenario)

def send(v):
    print(json.dumps(v), flush=True)

# Validate Ask-mode flags when present (shim passes them).
if flags:
    joined = " ".join(flags)
    assert "--restricted" in flags, flags
    assert "--print" in flags, flags
    assert "--input-format" in flags and flags[flags.index("--input-format") + 1] == "stream-json"
    assert "--output-format" in flags and flags[flags.index("--output-format") + 1] == "stream-json"
    assert "--permission-mode" in flags and flags[flags.index("--permission-mode") + 1] == "manual"
    assert "bypassPermissions" not in joined
    assert "--include-partial-messages" in flags
    tools = flags[flags.index("--tools") + 1]
    assert "Read" in tools and "Glob" in tools and "Grep" in tools
    assert "Edit" not in tools and "Write" not in tools and "Bash" not in tools

# Read the user turn (stream-json input stays open).
line = sys.stdin.readline()
assert line, "expected user turn"
msg = json.loads(line)
assert msg["type"] == "user" and msg["message"]["content"] == PROMPT, msg

SESSION = "33b21513-a4fb-478d-ad84-21cb7ef30571"
send({
    "type": "system", "subtype": "init", "session_id": SESSION,
    "cwd": "/tmp", "tools": ["Read", "Glob", "Grep"], "mcp_servers": [{"name": "Gmail"}],
    "model": "synthetic-model", "permissionMode": "default",
    "slash_commands": ["verify"], "claude_code_version": "2.1.252",
    "powershell_path": "/bin/false", "agents": ["Explore"], "skills": [],
})

if scenario == "lost":
    sys.exit(0)

if scenario in ("approval", "approval-allow", "approval-deny"):
    expected = "allow" if scenario == "approval-allow" else "deny"
    send({"type": "control_request", "request_id": "req_1", "request": {
        "subtype": "can_use_tool", "tool_name": "Bash", "input": {"command": "touch hello.txt"},
    }})
    import select
    if not select.select([sys.stdin], [], [], 5.0)[0]:
        sys.stderr.write(f"approval: timed out waiting for {expected}\n")
        sys.exit(2)
    raw = sys.stdin.readline()
    if not raw:
        sys.stderr.write(f"approval: stdin closed before {expected}\n")
        sys.exit(2)
    reply = json.loads(raw)
    assert reply["type"] == "control_response", reply
    assert reply["response"]["response"]["behavior"] == expected, reply
    send({"type": "result", "subtype": "success", "is_error": False, "result": "ok"})
    try:
        sys.stdin.read()
    except Exception:
        pass
    sys.exit(0)

if scenario == "limit":
    send({
        "type": "rate_limit_event",
        "rate_limit_info": {
            "status": "allowed_warning",
            "rateLimitType": "seven_day",
            "utilization": 0.79,
            "resetsAt": 1788764400,
        },
    })

if scenario == "thinking":
    send({"type": "stream_event", "event": {"type": "message_start", "message": {"id": "msg_t"}}})
    send({"type": "stream_event", "event": {"type": "content_block_start", "index": 0, "content_block": {"type": "thinking"}}})
    send({"type": "stream_event", "event": {"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "RAW_REASONING_MUST_NOT_SURVIVE"}}})
    send({"type": "stream_event", "event": {"type": "content_block_stop", "index": 0}})
    send({"type": "stream_event", "event": {"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "Hello "}}})
    time.sleep(0.3)
    send({"type": "stream_event", "event": {"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "world"}}})
    send({"type": "assistant", "message": {"id": "msg_t", "content": [{"type": "text", "text": "Hello world"}]}})
    send({"type": "result", "subtype": "success", "is_error": False, "result": "Hello world",
          "usage": {"input_tokens": 10, "output_tokens": 4}})
    # Wait until stdin closes (native closes on result) or interrupt.
    try:
        sys.stdin.read()
    except Exception:
        pass
    sys.exit(0)

if scenario == "tools":
    send({"type": "stream_event", "event": {"type": "message_start", "message": {"id": "msg_1"}}})
    send({"type": "stream_event", "event": {
        "type": "content_block_start", "index": 0,
        "content_block": {"type": "tool_use", "id": "toolu_1", "name": "Read"},
    }})
    send({"type": "assistant", "message": {"id": "msg_1", "content": [
        {"type": "tool_use", "id": "toolu_1", "name": "Read", "input": {"file_path": "fixture.txt"}}
    ]}})
    send({"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "toolu_1", "content": "ok", "is_error": False}
    ]}})
    send({"type": "stream_event", "event": {"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "Hello world"}}})
    send({"type": "assistant", "message": {"id": "msg_1", "content": [{"type": "text", "text": "Hello world"}]}})
    send({"type": "result", "subtype": "success", "is_error": False, "result": "Hello world"})
    try:
        sys.stdin.read()
    except Exception:
        pass
    sys.exit(0)

if scenario == "failure":
    send({"type": "result", "subtype": "error_max_turns", "is_error": True, "result": "too many turns"})
    try:
        sys.stdin.read()
    except Exception:
        pass
    sys.exit(1)

if scenario == "interrupt":
    # Hold until SIGINT (native stop).
    signal.signal(signal.SIGINT, lambda *_: sys.exit(130))
    while True:
        time.sleep(0.2)

# success (default)
send({"type": "stream_event", "event": {"type": "message_start", "message": {"id": "msg_a"}}})
send({"type": "stream_event", "event": {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello "}}})
time.sleep(0.35)
send({"type": "stream_event", "event": {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "world"}}})
send({"type": "assistant", "message": {"id": "msg_a", "content": [{"type": "text", "text": "Hello world"}]}})
send({"type": "result", "subtype": "success", "is_error": False, "result": "Hello world",
      "usage": {"input_tokens": 12, "output_tokens": 4},
      "modelUsage": {"claude-sonnet-5": {"canonicalModel": "claude-sonnet-5", "contextWindow": 1000000}}})
try:
    sys.stdin.read()
except Exception:
    pass
sys.exit(0)
