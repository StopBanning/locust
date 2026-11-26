# SOAK 1: a day of use in an hour

Cloud lane "Soak 1", 26 Nov 2026, on `cloud/soak-1` (cut from `exec/native-engine`). The engine (locust-core) is run
through a day's worth of use with fake agents only, to find leaks, hangs and corruption before Colin uses the Rust app
daily. This lane writes tests and notes; it fixes no engine code.

The tests are in `core/src/soak_tests/`, one file a row. Each has a short form in the normal suite and a long form marked
`#[ignore]`:

```bash
cargo test -p locust-core --lib soak_tests:: -- --include-ignored --nocapture --test-threads=1
```

The fake agents are in `core/src/soak_tests/fixtures/` (the shared ones in `scripts/fixtures` pause or assert per
scenario; these are fast and numbered). No test reaches the network or runs a real agent. Rows whose numbers are the
process's own (memory, descriptors) or that kill the engine run this same test binary again as a child process on one
named `#[ignore]` test that does nothing unless `SOAK_CHILD` is set (`soak_tests/mod.rs`, `child`).

Machine: the cloud container, Linux 6.18, 4 CPUs, 15 GiB, debug build (`cargo test`).

## Row 1: many turns (`many_turns.rs`)

500 turns in one engine with the soak fake Codex app-server (`soak-app-server.py quick`): 100 conversations, each a
start and four follow-ups. A reading after every tenth turn, once the turn's slot is released and its agent has exited.

"Keeps growing", in numbers, comparing the early reading (turn 50 of 500; turn 10 of 60 in the short form) with the last:

- descriptors, threads: more than 3 above the early reading (one leaked a turn would be +450);
- children: any agent still running after its turn ended, or more unreaped than at the early reading;
- resident memory: more than 16 KiB a turn on average AND more than 4 MiB in all. A turn's ledger is ~3 KiB on disk, so
  16 KiB a turn is a turn's records kept several times over; the 4 MiB floor is allocator drift.

Long form, `many_turns_long` (69.2 s):

| after turn | RSS KiB | fds | threads | children alive | zombies | ms since turn 1 |
|---:|---:|---:|---:|---:|---:|---:|
| 0 (before) | 22,552 | 4 | 2 | 0 | 0 | 0 |
| 10 | 61,388 | 4 | 2 | 0 | 0 | 1,092 |
| 50 | 61,692 | 4 | 2 | 0 | 0 | 5,038 |
| 100 | 61,872 | 4 | 2 | 0 | 0 | 9,893 |
| 200 | 62,612 | 4 | 2 | 0 | 0 | 21,658 |
| 300 | 63,664 | 4 | 2 | 0 | 0 | 35,168 |
| 400 | 64,000 | 4 | 2 | 0 | 0 | 51,481 |
| 500 | 65,948 | 4 | 2 | 0 | 0 | 69,238 |

Turn 50 to 500: RSS +4,256 KiB (+9.5 KiB a turn), descriptors 4 to 4, threads 2 to 2, no child alive or unreaped after
any reading. All 500 ledgers end `run.completed`. Short form (60 turns): +444 KiB (+8.9 KiB a turn), 4 to 4, 2 to 2, 0, 0.

Passes. Two things to watch, neither a fault by the rule above:

- **Memory rises ~9.5 KiB a turn and has not flattened by turn 500.** It tracks the number of ledger files (one a turn),
  which fits the history cache (`history_cache.rs`) keeping one `CachedFile` a ledger, as Electron's mission history does.
  At that rate a year of 50 turns a day is ~170 MiB. Not a leak of turns, but unbounded in the size of the history.
- **Each turn gets slower as the history grows.** Milliseconds a turn, per 50 turns: 97, 110, 125, 131, 139, 166, 160,
  169, 186 (turns 50-100 through 450-500): about +0.2 ms a turn for every ledger already in the folder, so something
  in a turn's start or end reads every ledger. At 10,000 ledgers that is ~2 s a turn of the engine's own work. Not
  chased here (tests and notes only); row 5 measures listing and reading directly.
