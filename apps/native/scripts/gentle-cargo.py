#!/usr/bin/env python3
"""Cargo, one at a time on this Mac, and gently.

Several agents build this port at once, each in its own folder, sharing one target folder. Cargo already serializes
compiling there, but not running the tests: a second agent's test binaries ran while a third compiled, and the load
reached 8-10 (Colin's own Locust froze once). This wrapper holds one lock for the whole cargo run, compile and tests,
so only one runs at a time; the others wait and say so.

    python3 scripts/gentle-cargo.py nextest run --workspace     # every day (cargo-nextest: both test binaries at once)
    python3 scripts/gentle-cargo.py test -p locust-core         # every break check (debug: fast rebuilds)
    python3 scripts/gentle-cargo.py test --release --workspace  # once before a merge is pushed
    python3 scripts/gentle-cargo.py build --release

It sets what every run here needs unless already set: the shared target folder, four jobs, the TypeScript reader the
ledger tests use, and nice 10. A timeout in seconds can come first: `gentle-cargo.py --timeout 590 test ...`.
"""
import fcntl
import os
import subprocess
import sys
import time

HOME = os.path.expanduser("~")
TARGET = os.environ.setdefault("CARGO_TARGET_DIR", f"{HOME}/work/scratch/native-target")
os.environ.setdefault("CARGO_BUILD_JOBS", "4")
os.environ.setdefault("LOCUST_TEST_TS_READER", f"{HOME}/work/locust/packages/mission-store/dist/index.js")
os.environ["PATH"] = f"{HOME}/.cargo/bin:" + os.environ.get("PATH", "")
# A reader path that is not there fails ~50 ledger tests with "TypeScript reader rejected" (16 Nov: a path copied from
# another worktree). Say so before building instead.
if not os.path.isfile(os.environ["LOCUST_TEST_TS_READER"]):
    sys.exit(f"gentle-cargo: LOCUST_TEST_TS_READER is {os.environ['LOCUST_TEST_TS_READER']}, which is not a file. "
             "Leave it unset to use the one built from ~/work/locust, or build packages/mission-store there.")

args = sys.argv[1:]
timeout = None
if args[:1] == ["--timeout"]:
    timeout = int(args[1])
    args = args[2:]
if not args:
    sys.exit(__doc__)

os.makedirs(TARGET, exist_ok=True)
lock_path = os.path.join(TARGET, ".gentle-cargo.lock")
with open(lock_path, "a+") as lock:
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        lock.seek(0)
        holder = lock.read().strip() or "another run"
        print(f"gentle-cargo: waiting for {holder} to finish", file=sys.stderr, flush=True)
        started = time.time()
        fcntl.flock(lock, fcntl.LOCK_EX)
        print(f"gentle-cargo: waited {int(time.time() - started)} s", file=sys.stderr, flush=True)
    lock.seek(0)
    lock.truncate()
    lock.write(f"cargo {' '.join(args)} in {os.getcwd()} (pid {os.getpid()})\n")
    lock.flush()
    # The lanes share one target folder, and Cargo keeps one copy of locust-core there for every checkout: a build in
    # another worktree on another commit leaves its copy, and this one then fails with errors about functions that are
    # right there in this tree (16 Nov: three wasted runs). When the last build here was another checkout, the two port
    # packages are cleaned first, inside the lock, so Cargo builds them from this tree.
    here = os.path.realpath(os.getcwd())
    last_path = os.path.join(TARGET, ".gentle-cargo.last")
    try:
        with open(last_path) as f:
            last = f.read().strip()
        stamp = os.path.getmtime(last_path)
    except OSError:
        last, stamp = "", 0.0
    # Another lane's wrapper may be older and record nothing, so also look at the libraries themselves: a locust-core
    # newer than this checkout's last build was built by someone else.
    import glob
    # Cargo 1.100 keeps each package's build in <profile>/build/<package>/<hash>/out; the hash is the same in every
    # checkout, which is the whole trouble.
    outputs = glob.glob(os.path.join(TARGET, "*", "build", "locust-core", "*", "out")) + glob.glob(os.path.join(TARGET, "*", "build", "locust-native", "*", "out"))
    newest = max((os.path.getmtime(f) for f in outputs), default=0.0)
    if last != here or newest > stamp + 1:
        # Touching this tree's files is not enough: Cargo's record of the shared unit names the other checkout's files,
        # sees them unchanged and keeps its copy. Clean the two port packages so they are built from this tree.
        print("gentle-cargo: another checkout built here last; rebuilding locust-core and locust-native from this one", file=sys.stderr, flush=True)
        profile = ["--release"] if "--release" in args else []
        subprocess.run(["nice", "-n", "10", "cargo", "clean", "-q", "-p", "locust-core", "-p", "locust-native", *profile])
    try:
        result = subprocess.run(["nice", "-n", "10", "cargo", *args], timeout=timeout)
        code = result.returncode
    except subprocess.TimeoutExpired:
        print(f"gentle-cargo: cargo {' '.join(args)} ran past {timeout} s and was stopped", file=sys.stderr)
        code = 124
    with open(last_path, "w") as f:
        f.write(here + "\n")
    lock.seek(0)
    lock.truncate()
sys.exit(code)
