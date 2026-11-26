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

## Row 2: bursts (`bursts.rs`)

A fake agent prints numbered records (`<17>`) as fast as it can while the engine's reader is held, once for each reader
with a queue: records.rs's shared process-runner bound (2,048; driven through Claude Code, `soak-claude-burst.py`),
Codex's app-server (4,096; `soak-app-server.py burst`), ACP (20,000; `soak-acp-burst.py`) and OpenCode 1's server
(20,000; `soak-opencode-serve-burst.py`, a loopback HTTP server). Readers: **Held** reads nothing for 2 s after the start,
then as fast as it can; **Slow** sleeps 1 ms after every 100 events from the start. What a run kept is the distinct
numbers seen in its events. The rule: the run completes with all of 0..n, or it fails with a whole prefix 0..k (k at most
the bound when held) and Electron's sentence (packages/runtime-adapters: claude-events.ts, codex-app-server-run.ts,
acp-run.ts, opencode-serve-run.ts). Never hangs: an ending within 60 s of the reader starting to read.

No run hung, no run dropped a record silently (every kept set was a whole prefix, every completed run had all of them),
and every run past its bound stopped. Long form `bursts_long`: 48 runs (4 readers x held/slow x 50,000 and half the
bound x 3 rounds), 39 pass, 9 find the faults below. Typical numbers (round 0):

| reader | n | held | slow |
|---|---:|---|---|
| claude (records.rs, 2,048) | 50,000 | failed, 2,047 kept, Electron's words, 200 ms | failed, 2,167 kept, Electron's words, 285 ms |
| claude | 1,024 | completed, all kept, 135 ms | completed, all kept, 138 ms |
| codex (4,096) | 50,000 | failed, 0 kept, Electron's words, 8 ms | failed, 999 kept, Electron's words, 45 ms |
| codex | 2,048 | completed, all kept, 39 ms | completed, all kept, 33 ms |
| acp (20,000) | 50,000 | failed, 0 kept, **"ACP notification queue overflow; run stopped"** | failed, ~1,070 kept, **same words** |
| acp | 10,000 | completed, all kept, **12.7 s** | completed, all kept, **12.9 s** |
| opencode serve (20,000) | 50,000 | failed, 0 kept, **"OpenCode output overflow"** | failed, 0 kept, Electron's words, 158 ms |
| opencode serve | 10,000 | completed, all kept, **11.6 s** | completed, all kept, **11.9 s** |

(ms: time inside the engine's `next_event`. Codex and OpenCode drop what was queued when they stop, as Electron's
`lose` does; that is said, not silent. A slow reader can keep more than the bound, since it empties the queue as it fills.)

### Fault 2a: ACP stops at its bound without Electron's sentence

`burst_acp_short` (ignored, failing). Rust says "ACP notification queue overflow; run stopped" (runtime/acp.rs:293);
Electron's acp-run.ts says "The agent sent output faster than Locust could record it, so the run was stopped rather than
leave a gap in its record. Sending it again usually works." Every run past 20,000, held or slow, 6 of 6.

### Fault 2b: OpenCode 1 stops at its bound without Electron's sentence, when the engine was busy

`burst_opencode_serve_short` (ignored, failing). Held (the engine busy while the burst arrives), Rust says "OpenCode
output overflow" (runtime/opencode_serve.rs:975, `next_event` sees the overflow flag first); slow, the same burst ends
with Electron's sentence. One overflow, two wordings, depending on who notices first. 3 of 3 held runs.

### Fault 2c: ACP and OpenCode 1 read a long answer in time that grows with its square

`bursts_scale_long` (ignored, failing). Held reader, a burst under the bound, milliseconds in the engine's `next_event`:

| records | 1,250 | 2,500 | 5,000 | 10,000 | 19,000 |
|---|---:|---:|---:|---:|---:|
| acp | 182 | 723 | 3,118 | 12,174 | **45,895** |
| opencode serve | 187 | 715 | 2,880 | 11,911 | **45,542** |

Doubling the records quadruples the time: 0.15 ms a record at 1,250, 2.4 ms at 19,000 (16x). Claude Code (128 to 1,946:
0.25 to 0.11 ms a record) and Codex (256 to 3,892: 0.012 to 0.016) stay flat. The likely cause, read in the code: each
chunk appends to the message held so far and then passes the WHOLE held text through `events::text` (runtime/events.rs:154,
which runs `redact_secrets` over all of it and counts it): runtime/acp_events.rs:236 `*held = text(held, 131072)` and
runtime/opencode_serve.rs:685 `held.text = super::events::text(&held.text, 262144)`. A 19,000-chunk answer, a long but
ordinary one when an agent streams a token or two a chunk, costs ~46 s of engine time in a debug build (release will be
several times faster, still quadratic); the turn shows nothing new meanwhile. It stays under the 60 s hang line here, so
it is reported as a fault of its own, not as a hang.

Seen in the code, not run (other users of the records.rs bound, beside the four rows asked): Copilot says "Copilot
notification queue overflow (2048 records); terminating run" (runtime/copilot.rs:229) and Antigravity "Antigravity
notification queue overflow" (runtime/antigravity.rs:245), where Electron's copilot-events.ts and agy-events.ts say
"... sent output faster than Locust could record it ...". Muse keeps Electron's words.
