#!/usr/bin/env python3
"""Which of an Electron test file's cases have a Rust test, matched by name.

    python3 scripts/case-map.py FILE...            one line per case: + named in Rust, - not named
    python3 scripts/case-map.py --ledger STATUS    X of Y for every ledger row with that status
    python3 scripts/case-map.py --write            rewrite parity/cases.tsv and the count at the front of each partial note
    python3 scripts/case-map.py --check            fail when either is stale, or a partial note says nothing past its count

The port's rule is one Rust test per Electron case, named for the case's own words ("keeps both messages in a running
turn of that length" -> keeps_both_messages_in_a_running_turn_of_that_length). A case counts as named when:
- a Rust `fn` in this app has its name, starts with it, or it starts with the Rust name (names cut short);
- a Rust test says `// case: <the case's words>` (one line per case), for a test that covers several cases or could not
  take the case's name;
- one of the tests the ledger row names shares most of the case's words (half or more, each test used once).

It reads names only: a match says a test was written for the case, not that the test is right.
"""

import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
ROOT = HERE.parent.parent
LEDGER = HERE / "parity" / "tests.tsv"

CASE_RE = re.compile(r"""\b(?:it|test)(?:\.(?:each|skip|only|todo)\b[^(]*\([^)]*\))?\s*\(\s*(['"`])((?:\\.|(?!\1).)*)\1""", re.S)
FN_RE = re.compile(r"\bfn\s+([a-z0-9_]+)\s*\(")
CASE_NOTE_RE = re.compile(r"//\s*case:\s*(.+)")
MIN_PREFIX = 24


def words(title):
    title = re.sub(r"\$\{[^}]*\}", " ", title)
    title = title.replace("’", "").replace("'", "")
    return "_".join(re.findall(r"[a-z0-9]+", title.lower()))


def cases(path):
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return None
    return [re.sub(r"\\(.)", r"\1", m.group(2)) for m in CASE_RE.finditer(text)]


def rust_names():
    names, noted = set(), set()
    for top in ("core/src", "src", "tests"):
        for f in (HERE / top).rglob("*.rs") if (HERE / top).exists() else []:
            text = f.read_text(encoding="utf-8", errors="replace")
            names.update(FN_RE.findall(text))
            noted.update(words(m.group(1)) for m in CASE_NOTE_RE.finditer(text))
    return names, noted


def carried(title, names, noted):
    key = words(title)
    if not key:
        return True
    if key in names or key in noted:
        return True
    if len(key) >= MIN_PREFIX and any(n.startswith(key) for n in names):
        return True
    return any(len(n) >= MIN_PREFIX and key.startswith(n) for n in names)


STOP = set("a an the is it its of to and in on for when as by or not be with at that this".split())


def tokens(name):
    return {w for w in name.split("_") if w and w not in STOP}


def row_tests(rust_cell, fns_by_file):
    found = set()
    for part in re.split(r"[,\s;|]+", rust_cell):
        if part.endswith(".rs"):
            path = part if part.startswith("apps/") else "apps/native/" + part
            found |= fns_by_file.get(path, set())
        elif re.fullmatch(r"[a-z0-9_:]+", part):
            found.add(part.split("::")[-1])
    return found


def file_map(rel, names, noted, own=frozenset()):
    found = cases(ROOT / rel)
    if found is None:
        return None
    out, used = [], set()
    for title in found:
        if carried(title, names, noted):
            out.append((title, True))
            continue
        key, best, score = tokens(words(title)), None, 0.0
        for name in sorted(own - used):
            t = tokens(name)
            if key and t:
                j = len(key & t) / len(key | t)
                if j > score:
                    best, score = name, j
        if score >= 0.5:
            used.add(best)
        out.append((title, score >= 0.5))
    return out


def fns_by_file():
    out = {}
    for top in ("core/src", "src", "tests"):
        base = HERE / top
        if base.exists():
            for f in base.rglob("*.rs"):
                out[str(f.relative_to(ROOT))] = set(FN_RE.findall(f.read_text(encoding="utf-8", errors="replace")))
    return out


CASES = HERE / "parity" / "cases.tsv"
# The public guard's shapes (_tools/public-export.mjs SCAN_PATTERNS): a case's own words may hold a made-up key or
# address on purpose; the generated lists say what it was, not the text.
SHAPES = [
    (re.compile(r"(?<![A-Za-z0-9])sk-[A-Za-z0-9_-]{8,}"), "[key-shaped text]"),
    (re.compile(r"(?<![A-Za-z0-9])ghp_[A-Za-z0-9]{8,}"), "[key-shaped text]"),
    (re.compile(r"(?<![A-Za-z0-9])AKIA[A-Z0-9]{8,}"), "[key-shaped text]"),
    (re.compile(r"-----BEGIN [A-Z ]+-----"), "[key-shaped text]"),
    (re.compile(r"(?<![A-Za-z0-9])xox[baprs]-[A-Za-z0-9-]{8,}"), "[key-shaped text]"),
    (re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"), "[an address]"),
    (re.compile(r"(?<!\d)(?:\+\d{1,3}[\s.-])?(?:\(\d{3}\)|\d{3})[\s.-]\d{3}[\s.-]\d{4}(?!\d)"), "[a number]"),
    (re.compile("".join(["h", "i", "s", "b", "o"]), re.I), "[a name]"),
]


def shown(title):
    title = title.replace("\t", " ").replace("\n", " ")
    for shape, said in SHAPES:
        title = shape.sub(said, title)
    return title
COUNT_RE = re.compile(r"^cases \d+ of \d+ named(?:; not named: .*?)? ‖ ")


def count_text(got):
    named = sum(c for _, c in got)
    left = [shown(t) for t, c in got if not c]
    text = f"cases {named} of {len(got)} named"
    if left:
        text += "; not named: " + " | ".join(left)
    return text + " ‖ "


def build():
    names, noted = rust_names()
    by_file = fns_by_file()
    lines = LEDGER.read_text(encoding="utf-8").split("\n")
    case_rows, out = ["file\tcase\tnamed"], []
    for i, line in enumerate(lines):
        f = line.split("\t")
        if i == 0 or len(f) != 5 or f[2] not in ("carried", "partial"):
            out.append(line)
            continue
        got = file_map(f[0], names, noted, frozenset(row_tests(f[3], by_file)))
        if got is None:
            out.append(line)
            continue
        for t, c in got:
            case_rows.append(f"{f[0]}\t{shown(t)}\t{'yes' if c else 'no'}")
        rest = COUNT_RE.sub("", f[4])
        if f[2] == "partial":
            f[4] = count_text(got) + rest
        else:
            f[4] = rest
        out.append("\t".join(f))
    return "\n".join(out), "\n".join(case_rows) + "\n"


def write_or_check(check):
    ledger, case_text = build()
    if not check:
        LEDGER.write_text(ledger, encoding="utf-8")
        CASES.write_text(case_text, encoding="utf-8")
        named = case_text.count("\tyes\n")
        print(f"{named} of {named + case_text.count(chr(9) + 'no' + chr(10))} cases in carried and partial files are named in Rust.")
        return 0
    bad = []
    if ledger != LEDGER.read_text(encoding="utf-8"):
        bad.append("the counts at the front of the partial notes are stale: run case-map.py --write")
    if not CASES.exists() or case_text != CASES.read_text(encoding="utf-8"):
        bad.append("parity/cases.tsv is stale: run case-map.py --write")
    for line in LEDGER.read_text(encoding="utf-8").splitlines()[1:]:
        f = line.split("\t")
        if len(f) == 5 and f[2] == "partial" and len(COUNT_RE.sub("", f[4]).strip()) < 12:
            bad.append(f"{f[0]}: a partial note must say what is missing past its count")
    for b in bad:
        print(b)
    return 1 if bad else 0


def main():
    args = sys.argv[1:]
    if args[:1] in (["--write"], ["--check"]):
        sys.exit(write_or_check(args[0] == "--check"))
    names, noted = rust_names()
    if args[:1] == ["--ledger"]:
        want = args[1]
        for line in LEDGER.read_text(encoding="utf-8").splitlines()[1:]:
            f = line.split("\t")
            if len(f) == 5 and f[2] == want:
                got = file_map(f[0], names, noted)
                if got is None:
                    print(f"?\t{f[0]}")
                else:
                    print(f"{sum(c for _, c in got)}/{len(got)}\t{f[0]}")
        return
    for rel in args:
        got = file_map(rel, names, noted) or []
        print(f"{rel}: {sum(c for _, c in got)} of {len(got)}")
        for t, c in got:
            print(f"  {'+' if c else '-'} {t}")


if __name__ == "__main__":
    main()
