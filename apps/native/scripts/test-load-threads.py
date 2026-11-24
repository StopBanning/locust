#!/usr/bin/env python3
"""Tests for load_threads.py. The expected counts are Evener's own cases (loadawareworkers_test.go) and literals."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from load_threads import spare_threads  # noqa: E402


class SpareThreads(unittest.TestCase):
    def test_a_whole_load_takes_that_many_cores(self):
        self.assertEqual(spare_threads(16, 12.0, 4), 4)

    def test_a_partly_busy_core_costs_a_whole_thread(self):
        self.assertEqual(spare_threads(16, 12.1, 3), 3)
        self.assertEqual(spare_threads(4, 1.2, 0), 2)

    def test_a_machine_busier_than_its_cores_still_runs_one(self):
        self.assertEqual(spare_threads(16, 43.27, 4), 1)

    def test_an_idle_machine_is_held_to_the_cap(self):
        self.assertEqual(spare_threads(12, 0.0, 8), 8)

    def test_no_cap_means_every_core(self):
        self.assertEqual(spare_threads(12, 0.0, 0), 12)

    def test_an_unreadable_load_uses_the_cap_never_a_guess(self):
        self.assertEqual(spare_threads(12, None, 6), 6)
        self.assertEqual(spare_threads(12, None, 0), 12)


if __name__ == "__main__":
    unittest.main()
