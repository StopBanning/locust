# A fair measurement (12 Nov 2026)

The README's first numbers (9 Nov) compared Electron holding Colin's real conversations with the Rust window showing
made-up ones. The README itself said that wasn't fair. `scripts/bench-pair.mjs` measures both apps showing the same
conversation.

## How it runs

```bash
node apps/native/scripts/bench-pair.mjs --out ~/work/scratch/bench-pair --seconds 60
```

1. **Seed.** A throwaway Electron profile gets the polish drive's ten-turn conversation with Wren (a two-turn
   conversation with tool calls, a table and code), plus three shorter ones, written by Electron's own ledger writer.
2. **Electron.** Electron runs as every drive does (`_tools/drive-lib.mjs`): that profile, a 1100 × 780 window behind
   other windows and never focused. It opens Wren's conversation, waits 10 s, then measures a minute of idle.
3. **Rust.** The Rust app's bench window (`--bench`, below other windows, click-through) starts with `HOME` pointing at a
   copy of the same profile. So it reads exactly that ledger and none of the person's own. It opens the same
   conversation (`--bench-conversation mission_polish_second`, through the window's own open path), waits 10 s, then
   measures a minute of idle.
4. **Sampling.** Both are sampled the same way: every process in the app's tree, once a second. The result is the median
   and peak resident memory, and CPU over the whole minute.

Nothing touches the person's keyboard, mouse or screen, and no agent runs.

## What it does not measure

- **Electron's first frame.** The drive starts Electron its own way, so there is no fair launch-to-first-frame number.
  The Rust first frame is reported alone.
- **CPU while a reply streams.** No agent runs; this is the app at rest with a conversation open.
- **The person's real history.** Both apps read the same small profile on purpose.

## Results, 12 Nov 2026

Apple M6, macOS 27.2.1, Electron 0.872.0, 1100 × 780, three runs. Every run checked both apps: Electron's window was
captured showing the conversation, and the Rust bench wrote that it opened `mission_polish_second` with 7 items.

| run | Rust behind (median / peak / CPU) | Electron behind | Electron in front | Rust first frame |
| --- | --- | --- | --- | --- |
| 1 | 38 / 72 MiB / 0.03 % | 122 / 277 MiB / 0.36 % | 285 / 332 MiB / 5.04 % | 767 ms |
| 2 | 57 / 71 MiB / 0.03 % | 161 / 253 MiB / 0.34 % | 250 / 295 MiB / 8.77 % | 408 ms |
| 3 | 69 / 73 MiB / 0.03 % | 183 / 264 MiB / 0.23 % | 237 / 325 MiB / 5.26 % | 1,374 ms |

## Runs that did not count

These were thrown out:
- **The first run.** The Rust half never ran: the drive's `finish` needs an argument. It also left the throwaway Electron
  open, which was closed by hand. Colin's own Locust was untouched.
- **The installed app** (`/Applications/Locust.app`). It opened its big-update splash over the conversation, so it was
  not showing the same thing. Hence the built app from this checkout.
- **A run at 1100 wide from the start.** Electron folds to the rail there, and the drive's open step found no name. The
  conversation is now opened while the window is wide, then the window is made 1100.

## The first frame is not measured fairly here

The Rust first frame ranged from 408 ms to 1.4 s. It is not the app's setup: setup reads a few small files and loads the
team off the main thread. The same bench with made-up data and no setup ranged from 146 to 1,524 ms in the same
conditions. The bench window sits below other windows, and Rune does not draw a covered window until it looks again on a
back-off timer, so the first frame depends on what covers it. A clean figure needs an uncovered window, which these runs
never take.

## Results, 27 Nov 2026 (speed-1, every row, three runs each)

Same script, now counting the Rust engine (`--engine`, its own process since plan E3, found once through
`engine.lock`), so the Rust figures are two processes where 12 Nov's were one: that is why they read higher than 12
Nov's 38-69 MiB. Each row's release binary from `~/work/scratch/speed-1/bins/`, 1100 x 780, 60 s idle, load average
1.7-5.4 where logged (the reruns; other lanes compiling). Median of the three runs, range in brackets. Every valid run had Electron showing the
conversation and the Rust bench opening `mission_polish_second`.

| date | row (commit) | Rust behind: median MiB, CPU | Electron behind: median MiB, CPU | Electron in front |
| --- | --- | --- | --- | --- |
| 27 Nov | base (2339aa12) | 89 (80-90) MiB, 0.03 (0.03-0.07) % | 211 (78-231) MiB, 0.15 (0.12-0.21) % | 319 MiB, 2.6 % |
| 27 Nov | r1 (7f100e7d), 2 valid | 88.5 (88-89) MiB, 0.025 (0.02-0.03) % | 207 (181-233) MiB, 0.26 (0.16-0.36) % | 288 MiB, 2.45 % |
| 27 Nov | r2 (45240c7a) | 27 (15-83) MiB, 0.07 (0.02-0.10) % | 160 (89-216) MiB, 0.21 (0.20-0.33) % | 270 MiB, 3.22 % |
| 27 Nov | r3 (9e0ada3b, wip build) | 83 (31-88) MiB, 0.03 (0.03-0.03) % | 232 (89-236) MiB, 0.18 (0.16-0.23) % | 320 MiB, 2.53 % |
| 27 Nov | final (67099e62) | 63 (62-89) MiB, 0.03 (0.02-0.05) % | 216 (131-230) MiB, 0.20 (0.15-0.20) % | 317 MiB, 2.83 % |
| 27 Nov | bb1 (c3451d29) | 64 (37-89) MiB, 0.07 (0.03-0.07) % | 208 (201-251) MiB, 0.26 (0.18-0.34) % | 303 MiB, 3.7 % |
| 27 Nov | front (1371591d) | 89 (89-89) MiB, 0.06 (0.03-0.08) % | 195 (115-247) MiB, 0.18 (0.13-0.23) % | 323 MiB, 2.72 % |

Every row is under Electron behind on memory and CPU. At rest with this small profile the rows barely differ: the
speed-1 rows fixed costs that grow with real data (a polled engine, a closed browser, owners read every frame, faces
re-triangulated), which this seeded profile hardly has. Those are measured in SPEED-NOTES.md over the subject data.

**Big Brother change 1 (bb1), during a turn.** bench-pair runs no agent, so the turn's cost was measured apart
(`~/work/scratch/speed-1/turn-behind.sh`): a live Scout turn (OpenCode free route) while the subject window sat behind
the app in front, 30 s of the window's CPU, three runs each over the subject data. final (motion held without focus):
36.2 (35.4-41.7) % CPU, 4.2-5.0 frames/s (the streamed text alone). bb1 (orb, sweep and shimmer keep moving while the
window shows): 48.1 (46.3-59.4) %, 15.3-22.6 frames/s. So the change costs about 12 points of a core while a turn runs
in a visible background window, and nothing at rest. In 5 of 6 runs the turn ended inside the 30 s, so these are a
turn's last half-minute.

### Runs that did not count (27 Nov)

10 of the first 21 runs, rerun with the fixed script (failed folders kept in `~/work/scratch/speed-1/pair-failed/`):
- **8: the Rust window quit before its minute ended.** Asking `lsof` for the engine on every sample made each sample
  slower than its second under build load, so the bench's own `--bench-seconds` ran out mid-measure (one process
  counted, CPU negative), and the exit listener was attached after the window had gone, so the script hung. Now the
  engine is asked once, the window gets 120 s to spare and is closed once measured, and the result says `validRun`.
- **2: the Electron profile copy failed (ENOENT)**, racing Electron's own shutdown. Now the copy waits for Electron to
  exit and tries again if a file vanished.
