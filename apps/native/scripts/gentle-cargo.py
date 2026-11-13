#!/usr/bin/env python3
"""Cargo, one at a time on this Mac, and gently.

Several agents build this port at once, each in its own folder, sharing one target folder. Cargo already serializes
compiling there, but not running the tests: a second agent's test binaries ran while a third compiled, and the load
reached 8-10 (Colin's own Locust froze once). This wrapper holds one lock for the whole cargo run, compile and tests,
so only one runs at a time; the others wait and say so.

    python3 scripts/gentle-cargo.py test --release --workspace
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
    try:
        result = subprocess.run(["nice", "-n", "10", "cargo", *args], timeout=timeout)
        code = result.returncode
    except subprocess.TimeoutExpired:
        print(f"gentle-cargo: cargo {' '.join(args)} ran past {timeout} s and was stopped", file=sys.stderr)
        code = 124
    lock.seek(0)
    lock.truncate()
sys.exit(code)
