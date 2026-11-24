#!/usr/bin/env python3
"""Synthetic Claude Code CLI for background sessions (`claude --bg`, `claude agents --json`, `claude stop`).
No real Claude Code invocation, credentials, or network.
"""
import json
import os
import sys


def log_call(argv):
    log_path = os.environ.get("CLAUDE_BG_CALLS_FILE")
    if log_path:
        with open(log_path, "a", encoding="utf-8") as f:
            f.write(json.dumps(argv) + "\n")


def main():
    argv = sys.argv[1:]
    log_call(argv)

    if not argv:
        sys.stderr.write("missing subcommand\n")
        sys.exit(1)

    if argv[0] == "--bg":
        if os.environ.get("CLAUDE_BG_UNTRUSTED") == "1":
            sys.stderr.write(
                "Workspace not trusted. Run `claude` in /tmp/work once and accept the trust prompt, then retry.\n"
            )
            sys.exit(1)
        start_says = os.environ.get(
            "CLAUDE_BG_START_SAYS",
            "Starting background service…\nbackgrounded · ad39859a\n  claude agents  list sessions\n",
        )
        code = int(os.environ.get("CLAUDE_BG_START_CODE", "0"))
        sys.stdout.write(start_says)
        sys.exit(code)

    if argv[0] == "agents":
        if "--json" not in argv[1:]:
            sys.stderr.write(f"expected --json in agents args: {argv!r}\n")
            sys.exit(2)
        agents_file = os.environ.get("CLAUDE_BG_AGENTS_FILE")
        if agents_file and os.path.exists(agents_file):
            with open(agents_file, "r", encoding="utf-8") as f:
                sys.stdout.write(f.read())
            sys.exit(0)
        raw = os.environ.get("CLAUDE_BG_AGENTS_JSON", "[]")
        sys.stdout.write(raw)
        sys.exit(0)

    if argv[0] == "stop":
        session_id = argv[1] if len(argv) > 1 else ""
        sys.stdout.write(f"stopped {session_id}\n")
        sys.exit(0)

    sys.stderr.write(f"unexpected args: {argv!r}\n")
    sys.exit(1)


if __name__ == "__main__":
    main()
