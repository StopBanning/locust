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

**2a and 2b fixed (Soak fixes 1):** ACP says Electron's sentence (`acp::OVERFLOW_MESSAGE`, acp-run.ts `lose`) and still
marks the output limit; OpenCode 1 ends on `opencode_events::OUTPUT_LIMIT_MESSAGE` whoever notices the overflow first,
with the server's tail kept in the process's stderr, not the message. `burst_acp_short` and `burst_opencode_serve_short`
run in the suite.

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

**2c fixed (Soak fixes 1):** both now hold a growing answer as Electron's acp-events.ts `heldText` does
(`runtime/held_text.rs`): each chunk is appended and measured on its own, the whole cut to both ends only once it passes
four ends, and scrubbed and bounded when it is written (the message's close; for OpenCode 1 the normalizer, per delta
and at the finished part). Held reader, same machine: 1,250 / 19,000 records, ms a record, acp 0.024 / 0.014, opencode
serve 0.017 / 0.005 (19,000 ACP chunks: ~270 ms in the engine, was ~45,900). `bursts_scale_long` runs in the suite (~40 s).

Seen in the code, not run (other users of the records.rs bound, beside the four rows asked): Copilot says "Copilot
notification queue overflow (2048 records); terminating run" (runtime/copilot.rs:229) and Antigravity "Antigravity
notification queue overflow" (runtime/antigravity.rs:245), where Electron's copilot-events.ts and agy-events.ts say
"... sent output faster than Locust could record it ...". Muse keeps Electron's words.

## Row 3: killed mid-turn (`killed_mid_turn.rs`)

The engine is this test binary in a child process, on one turn of the soak fake's `long` scenario (200 deltas and 20
command rows over ~2 s; a whole turn's ledger is 245 lines). The parent waits for the ledger to reach its share of those
lines, from line 1 (the header) to line 244, SIGKILLs the engine, then reads the folder as the next start would: the
file through `history_check` (allowed: read whole, cut-off last line `truncated-tail`, or read with zero turns when killed
before any reply), the turn in `list_recent` and in the conversation's reader (must not be running), the agent the engine
had started (must be gone), and a follow-up sent in the conversation.

Long form `killed_mid_turn_long`, 20 kills (lines reached when killed / lines in the file after):

- 1/1: read with zero turns (killed right after the header, before the agent said anything);
- 13/14, 26/26, 39/44, 52/56, 64/68, 77/80, 90/92, 103/104, 116/116, 128/128, 141/146, 154/158, 167/170, 180/182,
  192/194, 205/206, 218/218, 231/236, 244/244: read whole.

So 19 read whole, 1 read with zero turns, 0 cut off, 0 unreadable or damaged. The ledger is written a whole line at a
time (often a few lines a write, hence "file has" above the target), so a kill never left half a line here. Every
time, the list and the reader show the turn not running (the killed engine's pid is gone: `native::orphaned`). The
agent was gone 5 to 26 ms after the engine died (it loses its pipes). Short form: 4 kills (1, 82, 163, 244), same.

### Fault 3: after a crash, a follow-up is refused as "still running" for 15 minutes

`killed_mid_turn_then_a_follow_up_is_taken` (ignored, failing). 19 of the 20 kills (all but the one after the turn's
last record): the next start shows the turn ended, but a follow-up typed in that conversation is refused with "A turn is
still running in that conversation. Wait for it or stop it first." `native::send_from` asks `native::prior_of`
(native.rs:1261), which reads the turn as running from its lifecycle and its file's age (`ledger::ACTIVE_WINDOW`, 15
minutes; native.rs:1332 `running: transcript.running && fresh`) and never asks `orphaned`, while `list_recent`
(native.rs:977) and the reader (ledger.rs:2545) do. Only an app's carry-on (`allow_cut_off`) gets past it. So after the
engine crashes, for 15 minutes the window says the turn has ended and refuses to continue it, in words that say it runs.

**Fixed (Soak fixes 1):** `prior_of` now reads the turn as running only while its process lives (`fresh && !orphaned`),
as the list and the reader do; the test runs in the suite, and `native::tests::a_follow_up_to_a_turn_whose_process_is_gone_is_not_refused_as_running`
checks it in a moment.

## Row 4: two at once (`two_at_once.rs`)

Each round: engine A (this binary in a child, a real `engine_process::Server::start` on a fresh native folder) takes
the folder; a second engine in this process and one in another process both try; A exits (quits on even rounds, is
SIGKILLed on odd ones, which leaves `engine.sock` behind); this process then takes the lock and lets it go; then two
children started at once race for it.

Long form `two_at_once_long`: 50 rounds in 41.4 s (~822 ms a round, most of it the race winner's 800 ms hold), 0 faults.
Every round: both second engines refused with exactly "Another Locust engine is already running."; the lock was free
at once after A quit (25 rounds) and after A was killed (25 rounds, socket file left behind each time, replaced by the
next engine, which then had its own socket); exactly one of the two racers took the folder and the other was refused in
the same words. Short form: 6 rounds, same.

## Row 5: a long conversation (`long_conversation.rs`)

300 follow-ups in one conversation (soak fake, `quick`), in a child engine. After every turn: `list_recent(50)` as the
window's list asks it; opening the conversation (a fresh `native::session` and its first `poll`, which follows all
the turns' ledgers); the window's reader kept open across turns polling once after the turn (tail) and once more with
nothing new (idle). Medians of each third of the turns, in ms (debug build):

| measure | turns 1-100 | 101-200 | 201-300 | last/first |
|---|---:|---:|---:|---:|
| list recent | 2.71 | 4.76 | 6.24 | **2.30x** (passes) |
| open the conversation | 31.70 | 114.91 | 199.41 | **6.29x** |
| open reader, after a turn | 4.30 | 25.90 | 58.81 | **13.68x** |
| open reader, nothing new | 3.31 | 24.57 | 59.39 | **17.94x** |
| the turn itself (send to slot released) | 94.28 | 120.39 | 141.01 | 1.50x |

Short form (60 follow-ups, thirds of 20): list 1.30 / 1.96 / 2.47 ms (1.90x), passes; open 7.25 / 20.65 / 30.83
(4.25x), tail 1.12 / 2.57 / 4.11 (3.67x), idle 0.47 / 1.60 / 3.32 (7.06x).

`long_conversation_list_short` runs in the suite (the list part). Every read gave back every prompt (turn + 1 user
items), not running, not failed.

### Fault 5: reading a long conversation slows down faster than its turns, even when nothing changed

`long_conversation_read_short` (ignored, failing) and `long_conversation_long` (ignored, failing). Opening the
conversation is 6.3x slower at turns 201-300 than at 1-100 (it reads every turn's ledger, so some growth is expected,
but the cost a turn also rises: 0.63 ms a turn at turn ~50, 0.80 at turn ~250). Worse, the reader the window keeps
open costs 59 ms a poll at turn ~250 **with nothing new to read** (3.3 ms at turn ~50, 18x): an idle poll redoes work
in proportion to the whole conversation instead of the bytes that arrived. The window polls an open conversation
continuously, so a 300-turn conversation left open keeps a core busy in a debug build (several times less in release,
still growing with every turn). Not chased into the code (tests and notes only); `ledger::Session::poll`
(ledger.rs:2396) is where to look: the per-poll rebuild of the snapshot over every reader in the chain.
