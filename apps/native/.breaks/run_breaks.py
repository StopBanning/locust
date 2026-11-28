#!/usr/bin/env python3
"""Break checks for one piece of work. Run from apps/native:  python3 .breaks/run_breaks.py

Reads .breaks/entries.txt (each entry: path|||old ==> new; old may span lines; an entry starts where a line begins
with core/ or src/). First runs each needed crate's tests clean (the baseline). Then, for each entry: puts the new
text in place of the old (which must appear exactly once), runs the tests, and puts the file back.
Writes .breaks/results.txt (one line per entry) and .breaks/logs/NN.log (the end of each test run).
A test that already fails in the baseline never counts as a catch. Nothing here edits anything but the one file under
test, and it is always restored, also when the script is interrupted.
"""
import os, re, subprocess, sys, pathlib, time

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
LOGS = HERE / "logs"; LOGS.mkdir(exist_ok=True)
LIMIT = int(os.environ.get("BREAK_TIMEOUT", "5400"))
FAILED = re.compile(r"^test (\S+) \.\.\. FAILED", re.M)

def cargo(pkg, filters=None):
    cmd = ["cargo", "test", "-p", pkg, "--no-fail-fast"] + (["--"] + filters if filters else [])
    try:
        p = subprocess.run(cmd, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=LIMIT)
        return p.returncode, p.stdout
    except subprocess.TimeoutExpired as e:
        return None, (e.stdout or b"").decode("utf-8", "replace") if isinstance(e.stdout, bytes) else (e.stdout or "")

def built(out):
    return "error[" not in out and "could not compile" not in out

def stem(path):
    s = pathlib.Path(path).stem
    return s[:-6] if s.endswith("_tests") else s

def main():
    text = (HERE / "entries.txt").read_text()
    entries = [e for e in re.split(r"\n(?=core/|src/)", text.strip("\n")) if e.strip()]
    parsed = []
    for e in entries:
        path, rest = e.split("|||", 1)
        old, new = rest.split(" ==> ", 1) if " ==> " in rest else rest.split(" ==>", 1)
        parsed.append((path, old, new))
    crates = ["locust-core"] if any(p.startswith("core/") for p, _, _ in parsed) else []
    crates.append("locust-native")
    results = open(HERE / "results.txt", "w")
    def say(line):
        print(line, flush=True); results.write(line + "\n"); results.flush()
    base = {}
    for pkg in crates:
        t = time.time(); code, out = cargo(pkg)
        (LOGS / f"baseline-{pkg}.log").write_text(out[-20000:])
        if code is None or not built(out):
            say(f"BASELINE {pkg} DID NOT BUILD OR FINISH: stop"); return 3
        base[pkg] = set(FAILED.findall(out))
        ran = re.findall(r"^test result: .*", out, re.M)
        say(f"BASELINE {pkg}: {len(base[pkg])} already failing; {' | '.join(ran)[-300:]} ({int(time.time()-t)}s)")
        for name in sorted(base[pkg]): say(f"  ALREADY FAILING {pkg} {name}")
    for n, (path, old, new) in enumerate(parsed, 1):
        f = ROOT / path; label = old.strip().splitlines()[0][:70] if old.strip() else "(empty)"
        if not f.exists():
            say(f"{n:02d} MISSING  {path} (no such file)"); continue
        orig = f.read_text(); count = orig.count(old)
        if count != 1:
            say(f"{n:02d} MISSING  {path} old text appears {count} times :: {label}"); continue
        order = ["locust-core", "locust-native"] if path.startswith("core/") else ["locust-native"]
        status, why, log = "SURVIVED", "", ""
        try:
            f.write_text(orig.replace(old, new, 1))
            for pkg in order:
                for filters in ([stem(path)], None):
                    code, out = cargo(pkg, filters); log += f"\n===== {pkg} {filters or 'whole package'} exit {code}\n" + out[-6000:]
                    if code is None:
                        status, why = "TIMEOUT", f"{pkg} {filters or 'whole'}"; break
                    if not built(out):
                        err = next((l for l in out.splitlines() if l.startswith("error")), "")
                        status, why = "BUILD", err[:200]; break
                    new_fails = [t for t in FAILED.findall(out) if t not in base[pkg]]
                    if new_fails:
                        status, why = "CAUGHT", f"{pkg}: " + ", ".join(new_fails[:5]) + (f" (+{len(new_fails)-5} more)" if len(new_fails) > 5 else ""); break
                if status != "SURVIVED": break
        finally:
            f.write_text(orig)
        (LOGS / f"{n:02d}.log").write_text(log)
        say(f"{n:02d} {status:8} {path} :: {label}" + (f" -> {why}" if why else ""))
    dirty = subprocess.run(["git", "status", "--short", "--", "core", "src"], cwd=ROOT, stdout=subprocess.PIPE, text=True).stdout.strip()
    say("TREE " + ("clean" if not dirty else "DIRTY: " + dirty.replace("\n", "; ")))
    return 0

if __name__ == "__main__":
    sys.exit(main())
