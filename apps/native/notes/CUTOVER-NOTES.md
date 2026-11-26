# Cutover 1: the Rust app gets ready to be Colin's Locust on his Mac (25 Nov 2026)

From Big Brother's review of 25 Nov (`docs/REVIEW-big-brother-port-2026-11-25.md`, findings 2 and 3 and the cutover
checklist). Colin decided on 25 Nov: his data is re-seeded into a fresh Rust folder on switch day (Electron's folder is
left untouched, so Electron stays the way back), and the version keeps Electron's numbering. None of these rows is a
TypeScript test row, so they are recorded here and not in `parity/tests.tsv`.

Break runs: `~/work/scratch/mut6.py`, each list and its output kept side by side in `~/work/scratch/evidence/cutover-1/`.

## Row 1: the native engine by default

**What changed.** `core/src/native.rs` `forced()` now says native unless `LOCUST_NATIVE_ENGINE` is exactly `0`
(unset, empty, `1` or anything else all mean native); the reading is a private pure function, `native_by_default`.
`choose` is unchanged in shape and stays pure: with `forced` true it never asks Electron's door. So a double-clicked
app, which has no environment, runs on its own engine even while Electron is open and its door answers.
`LOCUST_NATIVE_ENGINE=0` is the one way back to Electron's engine (the door while it answers, native only when it
cannot be reached, as before).

**Call sites** (both read `forced()`, so both follow): `src/main.rs:844` (`choose_engine`, the window) and
`src/quick.rs:328` (`reach`, Quick Ask).

**Tests.**
- `core/src/native/tests.rs` `native_is_the_engine_unless_locust_native_engine_is_zero`: unset, `1`, empty, `yes`,
  `00` give native; only `0` gives the door.
- `core/src/native/tests.rs` `engine_choice_forced_door_down_door_up_and_door_refusal`: comments updated for the new
  default; the cases are the same.
- `src/engine_default_tests.rs` `the_window_runs_on_its_own_engine_while_electrons_door_is_up`: a made-up door (a
  connection file pointing at a local listener that counts connections) is up; `choose_engine` in a child copy of the
  test binary, with `LOCUST_MCP_CONNECTION` set on the child alone, chooses native and never asks the door; with
  `LOCUST_NATIVE_ENGINE=0` the door is asked. Before this change the default asked the door.

**Breaks.** 5 of 5 caught by name.
- `row1-core.entries` / `row1-core.out` (locust-core): 2 of 2, both by `native_is_the_engine_unless_locust_native_engine_is_zero`
  (the old `== "1"` reading; `is_none()`).
- `row1-native.entries` / `row1-native.out` (locust-native): 3 of 3, all by
  `engine_default_tests::the_window_runs_on_its_own_engine_while_electrons_door_is_up`: the call site in `choose_engine`
  bypassed (`native::forced()` replaced by `false`), `forced` ignoring the variable, and the old reading.

**Not changed (not this lane's files).** These still describe the old default: the comment above `choose_engine`
(`src/main.rs:840`), `src/quick.rs:323`, `README.md:23` and `PARITY.md:11`.
