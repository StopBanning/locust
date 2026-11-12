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

## Next

The Rust first frame ranges from 408 ms to 1.4 s now that setup reads real data. Setup work that can wait belongs after
the first frame.
