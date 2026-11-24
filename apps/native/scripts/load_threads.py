#!/usr/bin/env python3
"""How many test threads this machine can spare right now: cores minus the rounded-up 1-minute load, kept between 1
and a cap. The formula is Evener's load_aware_workers (prime-radiant-inc/evener, MIT), written again here.

    python3 scripts/load_threads.py            # prints the count for this machine now

It reads the load once, at the start of a run. Two runs that start together read the same load and can still stack;
gentle-cargo.py's slots are what keep runs apart. An unreadable load is not an idle machine: the cap is used.
"""
import math
import os

def spare_threads(cores, load1, cap):
    """cap <= 0 means "cores". load1 None (unreadable) gives the cap."""
    if cap <= 0:
        cap = cores
    if load1 is None:
        return max(1, cap)
    return max(1, min(cap, cores - math.ceil(load1)))

def here(cap=None):
    """For this machine now. The cap comes from LOCUST_TEST_THREAD_CAP, else the core count."""
    cores = os.cpu_count() or 1
    if cap is None:
        try:
            cap = int(os.environ.get("LOCUST_TEST_THREAD_CAP", "0"))
        except ValueError:
            cap = 0
    try:
        load1 = os.getloadavg()[0]
    except OSError:
        load1 = None
    return spare_threads(cores, load1, cap), cores, load1

if __name__ == "__main__":
    n, cores, load1 = here()
    print(f"{n} (cores {cores}, load {load1 if load1 is not None else 'unknown'})")
