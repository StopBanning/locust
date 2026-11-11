#!/usr/bin/env python3
"""The test ledger: one line in parity/tests.tsv for every TypeScript test file in the Electron app.

    python3 scripts/test-ledger.py --check [--electron PATH]   fail when a test file has no line
    python3 scripts/test-ledger.py --add   [--electron PATH]   add a 'waiting' line for each new test file

PATH is an Electron checkout (default: $LOCUST_ELECTRON_CHECKOUT, else the repository this script is in). Test files
are *.test.ts, *.test.tsx and *.test.mjs under apps/ and packages/, leaving out node_modules, dist, out and this app.

Columns: file, round (the notes file that names it), status, rust (the Rust tests that carry it), note.
Status: carried (every assertion has a Rust test), partial (some do), does-not-apply (the note says why), waiting
(not ported yet). A file is carried only where its note plainly says so.
"""

import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
LEDGER = HERE / "parity" / "tests.tsv"
FIELDS = ["file", "round", "status", "rust", "note"]
STATUSES = ("carried", "partial", "does-not-apply", "waiting")
SKIP = {"node_modules", "dist", "out"}
SUFFIXES = (".test.ts", ".test.tsx", ".test.mjs")


def electron_root():
    args = sys.argv[1:]
    if "--electron" in args:
        return Path(args[args.index("--electron") + 1]).resolve()
    env = os.environ.get("LOCUST_ELECTRON_CHECKOUT")
    return Path(env).resolve() if env else HERE.parent.parent


def test_files(root):
    found = []
    for top in ("apps", "packages"):
        for dirpath, dirs, files in os.walk(root / top):
            rel = Path(dirpath).relative_to(root)
            dirs[:] = sorted(d for d in dirs if d not in SKIP and (rel / d).as_posix() != "apps/native")
            found += [(rel / f).as_posix() for f in files if f.endswith(SUFFIXES)]
    return sorted(found)


def read_ledger():
    lines = LEDGER.read_text(encoding="utf-8").splitlines()
    if lines[0].split("\t") != FIELDS:
        sys.exit(f"{LEDGER}: the header must be {FIELDS}")
    rows, bad = {}, []
    for n, line in enumerate(lines[1:], start=2):
        cells = line.split("\t")
        if len(cells) != len(FIELDS):
            bad.append(f"tests.tsv:{n}: {len(cells)} fields, expected {len(FIELDS)}")
            continue
        row = dict(zip(FIELDS, cells))
        if row["status"] not in STATUSES:
            bad.append(f"tests.tsv:{n}: status must be one of {', '.join(STATUSES)}")
        if row["status"] == "does-not-apply" and not row["note"]:
            bad.append(f"tests.tsv:{n}: say why {row['file']} does not apply")
        if row["status"] == "carried" and not row["rust"]:
            bad.append(f"tests.tsv:{n}: name the Rust tests that carry {row['file']}")
        if row["file"] in rows:
            bad.append(f"tests.tsv:{n}: {row['file']} is listed twice")
        rows[row["file"]] = row
    return rows, bad


def main():
    root = electron_root()
    files = test_files(root)
    if not files:
        sys.exit(f"no TypeScript test files under {root}")
    rows, bad = read_ledger()
    new = [f for f in files if f not in rows]
    gone = [f for f in rows if f not in set(files)]
    counts = {}
    for r in rows.values():
        counts[r["status"]] = counts.get(r["status"], 0) + 1
    print(f"{len(files)} TypeScript test files in {root}; {len(rows)} lines: "
          + ", ".join(f"{counts.get(s, 0)} {s}" for s in STATUSES) + ".")
    for f in gone:
        print(f"not in this checkout (kept): {f}")
    if "--add" in sys.argv[1:]:
        for f in new:
            rows[f] = dict(file=f, round="", status="waiting", rust="", note="")
        out = ["\t".join(FIELDS)] + ["\t".join(rows[f][k] for k in FIELDS) for f in sorted(rows)]
        LEDGER.write_text("\n".join(out) + "\n", encoding="utf-8")
        print(f"added {len(new)} waiting lines")
        new = []
    for f in new:
        print(f"no line for: {f}")
    for p in bad:
        print("problem:", p)
    return 1 if new or bad else 0


if __name__ == "__main__":
    sys.exit(main())
