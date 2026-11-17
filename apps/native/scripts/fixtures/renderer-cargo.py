#!/usr/bin/env python3
"""Run gentle-cargo with the documented cross-worktree mtime fix inside its lock.

Touching before waiting is insufficient: another tree may build newer shared
artifacts during the wait. Keep the standard lock, target, timeout and environment.
"""
from pathlib import Path
import runpy
import subprocess
import sys

native = Path(__file__).resolve().parents[2]
original_run = subprocess.run


def rebuild_this_tree(command, *args, **kwargs):
    if command[:3] == ["nice", "-n", "10"]:
        for source_root in (native / "core/src", native / "src"):
            for source in source_root.rglob("*.rs"):
                source.touch()
    return original_run(command, *args, **kwargs)


subprocess.run = rebuild_this_tree
sys.argv[0] = str(native / "scripts/gentle-cargo.py")
runpy.run_path(sys.argv[0], run_name="__main__")
