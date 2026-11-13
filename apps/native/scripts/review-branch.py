#!/usr/bin/env python3
"""The mechanical half of reviewing an agent's port branch, in one command.

    python3 scripts/review-branch.py ~/work/locust-wt/hydrogen [--base exec/native-engine] [--keep-out PATH ...]

What it checks, and why each is here (12 Nov 2026, the reviews of Hydrogen's side chat and backup jobs):
- what the branch changed: only `apps/native/`, and none of the `--keep-out` paths another agent is working in;
- added lines that need a person's look: home folders, `gh`, the network, an exit that tests could reach, an ignored
  test, an email-shaped literal without `public-guard: allow`;
- the whole suite and the release build, through `gentle-cargo.py`, with the counts read from cargo itself, never
  from the agent's report (a report claimed 238 window tests passed; two failed);
- each new window test run alone, then the new ones together three times: shared state shows up only one way or the
  other;
- `recount.py --check`.

It does not do the other half: reading Electron's source for the feature, and breaking each fix once to see a test
fail. It ends by listing the functions the branch added or changed, as the places to break.
"""
import argparse
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
GENTLE = os.path.join(HERE, "gentle-cargo.py")
RISKY = [
    ("a real home folder", re.compile(r"/Users/(?!locust/work/scratch)|home_dir\(\)|\bdirs::home")),
    ("the gh command", re.compile(r"""Command::new\(\s*"gh"\s*\)""")),
    ("the network", re.compile(r"TcpStream::connect|reqwest|ureq::|https?://(?!localhost|127\.0\.0\.1|github\.com/someone|example\.)")),
    ("an exit tests could reach", re.compile(r"process::exit\(")),
    ("an ignored test", re.compile(r"#\[ignore")),
]
EMAIL = re.compile(r"[\w.+-]+@[\w-]+\.[\w.]+")


def run(cmd, cwd, timeout=None):
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout)
    return p.returncode, p.stdout + p.stderr


def counts(output):
    passed = sum(int(m) for m in re.findall(r"test result: \w+\. (\d+) passed", output))
    failed = sum(int(m) for m in re.findall(r"(\d+) failed;", output))
    names = re.findall(r"^test (\S+) \.\.\. FAILED", output, re.M)
    return passed, failed, names


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("worktree")
    ap.add_argument("--base", default="exec/native-engine")
    ap.add_argument("--keep-out", nargs="*", default=[])
    ap.add_argument("--repeat", type=int, default=3)
    a = ap.parse_args()
    root = os.path.abspath(os.path.expanduser(a.worktree))
    native = os.path.join(root, "apps/native")
    problems = []

    _, base = run(["git", "merge-base", "HEAD", a.base], root)
    base = base.strip()
    _, names = run(["git", "diff", "--name-status", base], root)
    _, untracked = run(["git", "ls-files", "--others", "--exclude-standard"], root)
    print(f"# Review of {root} against {a.base}\n\n## What changed\n")
    for line in names.strip().splitlines():
        print(f"- {line}")
        path = line.split("\t")[-1]
        if not path.startswith("apps/native/"):
            problems.append(f"changes a file outside apps/native/: {path}")
        for keep in a.keep_out:
            if path.startswith(keep):
                problems.append(f"changes {path}, which another agent is working in")
    for path in untracked.split():
        if not path.startswith("HANDOFF"):
            problems.append(f"leaves an uncommitted file: {path}")

    _, diff = run(["git", "diff", "-U0", base, "--", "apps/native"], root)
    current = None
    new_tests, touched = [], set()
    pending_test = False
    print("\n## Added lines worth a look\n")
    flagged = 0
    for line in diff.splitlines():
        if line.startswith("+++ "):
            current = line[6:]
            continue
        if line.startswith("@@"):
            m = re.search(r"@@.*@@\s*(?:.*\bfn\s+(\w+))?", line)
            if m and m.group(1):
                touched.add((current, m.group(1)))
            continue
        if not line.startswith("+") or line.startswith("+++"):
            continue
        text = line[1:]
        if "#[test]" in text:
            pending_test = True
        fn = re.search(r"\bfn\s+(\w+)", text)
        if fn:
            if pending_test:
                new_tests.append((current, fn.group(1)))
                pending_test = False
            elif "test" not in (current or ""):
                touched.add((current, fn.group(1)))
        if not (current or "").endswith(".rs"):
            continue
        for what, rx in RISKY:
            if rx.search(text) and "public-guard: allow" not in text:
                print(f"- {what}: `{current}`: `{text.strip()[:140]}`")
                flagged += 1
        if EMAIL.search(text) and "public-guard: allow" not in text and not text.strip().startswith("//"):
            print(f"- an email-shaped literal (the public guard refuses it): `{current}`: `{text.strip()[:140]}`")
            flagged += 1
    if flagged == 0:
        print("- none")

    print(f"\n## New tests ({len(new_tests)})\n")
    for f, n in new_tests:
        print(f"- `{n}` in `{f}`")

    print("\n## The whole suite\n")
    code, out = run([sys.executable, GENTLE, "--timeout", "590", "test", "--release", "--workspace"], native)
    passed, failed, failing = counts(out)
    print(f"- {passed} passed, {failed} failed" + (f": {', '.join(failing)}" if failing else ""))
    if code != 0 or failed:
        problems.append(f"the suite failed ({failed} failed)")

    code, out = run([sys.executable, GENTLE, "build", "--release"], native)
    warnings = len(re.findall(r"^warning", out, re.M))
    print(f"- release build: {'passes' if code == 0 else 'FAILS'}, {warnings} warnings")
    if code != 0 or warnings:
        problems.append("the release build failed or warned")

    window = [n for f, n in new_tests if f and f.endswith("window_drive_tests.rs")]
    if window:
        print("\n## New window tests, alone and together\n")
        for n in window:
            _, out = run([sys.executable, GENTLE, "--timeout", "300", "test", "--release", "-p", "locust-native", "--", f"window_drive_tests::{n}", "--exact"], native)
            p, f, _ = counts(out)
            print(f"- `{n}` alone: {'passes' if f == 0 and p >= 1 else 'FAILS'}")
            if f or p == 0:
                problems.append(f"{n} fails alone")
        # Together: every new window test in one run, a few times.
        for i in range(a.repeat):
            _, out = run([sys.executable, GENTLE, "--timeout", "300", "test", "--release", "-p", "locust-native", "--", *window], native)
            p, f, failing = counts(out)
            print(f"- together, run {i + 1}: {p} passed, {f} failed" + (f": {', '.join(failing)}" if failing else ""))
            if f:
                problems.append(f"new window tests fail together (run {i + 1})")

    code, out = run([sys.executable, os.path.join(native, "scripts/recount.py"), "--check"], native)
    print(f"\n## PARITY bookkeeping\n\n- recount --check: {'clean' if code == 0 else 'DRIFTED'}")
    if code != 0:
        problems.append("PARITY tables drifted from rows.tsv")

    print("\n## Break each of these once (by hand) and see a test fail\n")
    for f, n in sorted(touched):
        if f and "test" not in f:
            print(f"- `{n}` in `{f}`")

    print("\n## Verdict\n")
    if problems:
        for p in problems:
            print(f"- PROBLEM: {p}")
    else:
        print("- the mechanical checks pass; now read Electron's source for the feature and do the breaks")
    sys.exit(1 if problems else 0)


main()
