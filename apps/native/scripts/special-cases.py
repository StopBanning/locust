#!/usr/bin/env python3
"""The special cases in Electron's code for a port job, listed before the job starts.

Agents port an Electron feature's main path well and miss the branch that only one agent, platform or path takes.
On 12 Nov 2026 two such misses reached review: OpenCode 2's fresh-session side question (`opencodeAcp`), and a
restore that assumed no engine process. This lists every line in the named Electron files that branches on an agent,
a protocol, a platform, a cloud or remote run, or a named kind, with the function it sits in, so a brief can say:
port each one, or say in the notes why it does not apply. It also lists the TypeScript tests that import the files.

    python3 scripts/special-cases.py apps/desktop/src/main/codex-mission.ts --near side
    python3 scripts/special-cases.py apps/desktop/src/main/profile-backup.ts apps/desktop/src/main/retention.ts

Paths are inside Electron's checkout (`--root`, default ~/work/locust). `--near WORD` keeps only cases within 40 lines
of a line naming WORD, for a big file where one feature is a small part.
"""
import argparse
import os
import re
import sys

AGENTS = r"claude|codex|cursor|copilot|antigravity|opencode|kilo|muse|gemini|acp|agy"
PATTERNS = [
    ("agent", re.compile(rf"""(===|!==|==|!=)\s*['"]({AGENTS})['"]|['"]({AGENTS})['"]\s*(===|!==)|case\s+['"]({AGENTS})['"]""", re.I)),
    ("protocol", re.compile(r"\b\w*(Acp|acp|AppServer|appServer|Serve\b|serve\b|v2|V2)\w*\b\s*(&&|\|\||\?|\)|===|!==)")),
    ("platform", re.compile(r"process\.platform|platform\s*===|['\"](win32|darwin|linux)['\"]")),
    ("remote", re.compile(r"\b(cloud|remote|phone|desk|relay)\w*\s*(!==|===)\s*undefined|\bcloud\b\s*(&&|\|\|)")),
    ("kind", re.compile(r"""\.(kind|reason|mode|status|type)\s*(===|!==)\s*['"][\w-]+['"]""")),
    ("version", re.compile(r"\b(version|Version)\w*\s*(<|>|<=|>=)|compareVersions|semver|atLeast\(")),
]
FUNC = re.compile(r"^\s*(export\s+)?(async\s+)?function\s+(\w+)|^\s*(const|let)\s+(\w+)\s*=\s*(async\s*)?\(|^\s*(async\s+)?(\w+)\s*\([^)]*\)\s*(:\s*[^={]+)?\{\s*$")


KEYWORDS = {"if", "for", "while", "switch", "catch", "return", "else", "do", "try", "with"}


def function_at(lines, index):
    for j in range(index, -1, -1):
        m = FUNC.match(lines[j])
        if m:
            name = next(g for g in (m.group(3), m.group(5), m.group(8)) if g)
            if name not in KEYWORDS:
                return name
    return "(top level)"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="+")
    ap.add_argument("--root", default=os.path.expanduser("~/work/locust"))
    ap.add_argument("--near")
    a = ap.parse_args()
    total = 0
    for rel in a.files:
        path = os.path.join(a.root, rel)
        try:
            lines = open(path, encoding="utf-8").read().split("\n")
        except OSError as e:
            print(f"{rel}: {e}", file=sys.stderr)
            continue
        near = None
        if a.near:
            hits = [i for i, l in enumerate(lines) if a.near.lower() in l.lower()]
            near = lambda i: any(abs(i - h) <= 40 for h in hits)
        found = []
        for i, line in enumerate(lines):
            code = line.split("//")[0]
            if code.strip().startswith(("*", "/*", "import ")):
                continue
            kinds = [name for name, rx in PATTERNS if rx.search(code)]
            if kinds and (near is None or near(i)):
                found.append((i, kinds, line.strip()))
        print(f"\n## {rel}: {len(found)} special cases")
        for i, kinds, text in found:
            print(f"- `{rel}:{i + 1}` ({', '.join(kinds)}, in `{function_at(lines, i)}`): `{text[:160]}`")
        total += len(found)
        stem = os.path.splitext(os.path.basename(rel))[0]
        tests = []
        for base, _, names in os.walk(os.path.join(a.root, "apps")):
            if "node_modules" in base or "/native" in base:
                continue
            for n in names:
                if re.search(r"\.test\.tsx?$", n):
                    p = os.path.join(base, n)
                    try:
                        if re.search(rf"from ['\"][./\w-]*/{re.escape(stem)}(\.js)?['\"]", open(p, encoding="utf-8").read()):
                            tests.append(os.path.relpath(p, a.root))
                    except OSError:
                        pass
        if tests:
            print(f"\nTests that import {stem}: " + ", ".join(f"`{t}`" for t in sorted(tests)))
    print(f"\n{total} in all. A brief lists them and says: port each one, or say in your notes why it does not apply.")


main()
