#!/usr/bin/env python3
"""Rebuild PARITY.md's tables from parity/rows.tsv and print the totals.

    python3 scripts/recount.py           rewrite PARITY.md, print totals
    python3 scripts/recount.py --check   fail if PARITY.md is out of date or a row breaks a rule

Each checklist row carries three marks:
  built  done / partial / missing: the code exists in the Rust app
  alone  yes / no: its path runs without Electron's door when the native engine runs ("-" when nothing is built)
  live   a date, or no: seen in the real window, or in a real-agent test, on that day

Rules the check enforces: a row not alone says why (usually Electron's door) and a row alone gives no such reason; a
missing row has no alone or live mark; a live mark is a date and says what was seen.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ROWS = ROOT / "parity" / "rows.tsv"
TESTS = ROOT / "parity" / "tests.tsv"
PARITY = ROOT / "PARITY.md"
FIELDS = ["area", "feature", "where", "built", "detail", "engine", "alone", "not_alone_because", "live", "seen"]


def read_rows():
    lines = ROWS.read_text(encoding="utf-8").splitlines()
    if lines[0].split("\t") != FIELDS:
        sys.exit(f"{ROWS}: the header must be {FIELDS}")
    rows = []
    for n, line in enumerate(lines[1:], start=2):
        cells = line.split("\t")
        if len(cells) != len(FIELDS):
            sys.exit(f"{ROWS}:{n}: {len(cells)} fields, expected {len(FIELDS)}")
        rows.append(dict(zip(FIELDS, cells), line=n))
    return rows


def problems(rows):
    out = []
    for r in rows:
        at = f"rows.tsv:{r['line']} ({r['feature']})"
        if r["built"] not in ("done", "partial", "missing"):
            out.append(f"{at}: built must be done, partial or missing")
        if r["engine"] not in ("yes", "no"):
            out.append(f"{at}: engine must be yes or no")
        if r["built"] == "missing":
            if r["alone"] != "-" or r["live"] != "no":
                out.append(f"{at}: a missing row is alone '-' and live 'no'")
            continue
        if r["alone"] not in ("yes", "no"):
            out.append(f"{at}: alone must be yes or no")
        if r["alone"] == "yes" and r["not_alone_because"]:
            out.append(f"{at}: a reason it needs Electron is given, so it cannot be alone")
        if r["alone"] == "no" and not r["not_alone_because"]:
            out.append(f"{at}: say why it is not alone")
        if r["live"] != "no":
            if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", r["live"]):
                out.append(f"{at}: live is a date (YYYY-MM-DD) or no")
            if not r["seen"]:
                out.append(f"{at}: a live mark says what was seen")
    return out


def cell(text):
    return text.replace("|", "\\|")


def area_table(rows):
    head = [
        "| Feature | Where in TS | Built | Alone | Live | Depends on engine |",
        "| :--- | :--- | :--- | :--- | :--- | :---: |",
    ]
    body = []
    for r in rows:
        built = f"**{r['built']}**" + (f" — {r['detail']}" if r["detail"] else "")
        alone = r["alone"] + (f" — {r['not_alone_because']}" if r["not_alone_because"] else "")
        live = r["live"] + (f" — {r['seen']}" if r["seen"] else "")
        body.append(
            f"| {cell(r['feature'])} | {r['where']} | {cell(built)} | {cell(alone)} | {cell(live)} | {r['engine']} |"
        )
    return "\n".join(head + body)


def totals(rows, names):
    per = {}
    for r in rows:
        a = per.setdefault(int(r["area"]), dict(done=0, partial=0, missing=0, alone=0, live=0))
        a[r["built"]] += 1
        a["alone"] += r["alone"] == "yes"
        a["live"] += r["live"] != "no"
    return per


def totals_table(per, names):
    lines = [
        "| # | Area | Done | Partial | Missing | Total | Alone | Live |",
        "| :--- | :--- | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    t = dict(done=0, partial=0, missing=0, alone=0, live=0)
    for n in sorted(per):
        a = per[n]
        for k in t:
            t[k] += a[k]
        total = a["done"] + a["partial"] + a["missing"]
        lines.append(
            f"| {n} | {names[n]} | {a['done']} | {a['partial']} | {a['missing']} | {total} | {a['alone']} | {a['live']} |"
        )
    total = t["done"] + t["partial"] + t["missing"]
    lines.append(
        f"| | **TOTAL** | **{t['done']}** | **{t['partial']}** | **{t['missing']}** | **{total}** | **{t['alone']}** | **{t['live']}** |"
    )
    return "\n".join(lines), t


TABLE = re.compile(r"(?:^\|.*\n?)+", re.M)


def rebuild(text, rows):
    names = {}
    for m in re.finditer(r"^### Area (\d+): (.+)$", text, re.M):
        names[int(m.group(1))] = m.group(2).strip()
    by_area = {}
    for r in rows:
        by_area.setdefault(int(r["area"]), []).append(r)
    if set(by_area) != set(names):
        sys.exit(f"PARITY.md has areas {sorted(names)}, rows.tsv has {sorted(by_area)}")
    for n in sorted(names):
        m = re.search(rf"^### Area {n}: .+$", text, re.M)
        t = TABLE.search(text, m.end())
        text = text[: t.start()] + area_table(by_area[n]) + "\n" + text[t.end():]
    per = totals(rows, names)
    table, t = totals_table(per, names)
    m = re.search(r"^## 3\. Totals$", text, re.M)
    first = TABLE.search(text, m.end())
    text = text[: first.start()] + table + "\n" + text[first.end():]
    return text, t


def test_totals():
    if not TESTS.exists():
        return None
    counts = {}
    for line in TESTS.read_text(encoding="utf-8").splitlines()[1:]:
        status = line.split("\t")[2]
        counts[status] = counts.get(status, 0) + 1
    return counts


def main():
    check = "--check" in sys.argv[1:]
    rows = read_rows()
    bad = problems(rows)
    text = PARITY.read_text(encoding="utf-8")
    new, t = rebuild(text, rows)
    built = t["done"] + t["partial"]
    print(
        f"{len(rows)} rows. Built: {t['done']} done, {t['partial']} partial, {t['missing']} missing. "
        f"Alone: {t['alone']} of the {built} built. Live: {t['live']}."
    )
    tests = test_totals()
    if tests is not None:
        print("TypeScript test files: " + ", ".join(f"{v} {k}" for k, v in sorted(tests.items())) + f" ({sum(tests.values())} in all).")
    for p in bad:
        print("problem:", p)
    if check:
        if new != text:
            print("PARITY.md is out of date: run python3 scripts/recount.py")
            return 1
        return 1 if bad else 0
    PARITY.write_text(new, encoding="utf-8")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
