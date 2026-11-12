# Locust Native (preview)

Locust, rewritten in Rust on [Rune](https://github.com/anomalyco/rune). It is being built **beside** the Electron app,
not in its place: the Electron app stays what you download until this one carries a whole working day.

The goal is the whole app, engine included, with no Electron or Node underneath ([PLAN.md](PLAN.md)). It is not there
yet. [PARITY.md](PARITY.md) lists every feature with three marks:

- **built**: the code exists in Rust (done, partial or missing);
- **alone**: it runs without Electron;
- **live**: it has been seen working in the real window, or in a test against a real agent, and when.

`python3 scripts/recount.py` prints the current totals.

## How it runs today

The Rust app has its own engine. It starts Codex, Claude Code, OpenCode, Kilo, Cursor, Copilot, Antigravity, Muse and
any Agent Client Protocol agent itself, writes its own conversation files in the format Electron writes, and asks you
for approvals in its own window.

When the Electron app is running with **Let your other AI apps use Locust** on, the window uses Electron's engine
through that local door instead, so the two never run the same teammate twice. With Electron quit, or with
`LOCUST_NATIVE_ENGINE=1`, it uses its own, and everything the Rust app has built runs there: the model list and a
model for one turn, carrying a conversation on with another agent, its own door for other apps, Quick Ask and
`--ask`.

The code is two crates. `core/` (`locust-core`) is the engine: agent drivers, ledgers, approvals and the command
guard, stores, routines, usage. It does not depend on the UI toolkit. The window (`src/`, `locust-native`) draws it.

The Rust app never writes Electron's data. It reads Electron's folder and writes its own (`LOCUST_NATIVE_DATA`, by
default `<Electron's data folder>-native`).

## What is built

- Conversations: the full thread, streaming Markdown, tool steps, plans, reasoning, find, search, the beside view.
- The composer: drafts, @ files and teammates, slash commands, attachments, the queue.
- Approvals: the card in Electron's words, saved rules, the command guard's reach and plain explanation.
- Teammates, memory, About you; Changes with commit and pull request; a terminal; a browser panel with teammate tools;
  routines (partly); Settings and usage; Home, the Board and what's new; PDF, Word, PowerPoint, sheets, math and
  Mermaid.
- **Quick Ask**: `locust-native --quick` waits in the background; Option+Space (or `LOCUST_QUICK_CHORD`) opens a small
  panel over any app.

Partial and missing pieces are listed row by row in PARITY.md.

## How it is checked

Electron's own TypeScript is the answer key. Scripts run Electron's command builders, stream readers, carry-on code,
model catalog and avatar rule, and the Rust has to give the same answers: today 358 of 385 agent commands and all 37
recorded streams, and every conversation file the Rust writes is opened by Electron's reader. What a live hour in the
real window caught that those tests did not, and the window tests that now guard it, are in [METHOD.md](METHOD.md).

## Try it (developers)

macOS only for now.

```bash
cargo run --release
```

Make a Mac app you can double-click (`Locust Native.app` in the target folder, Locust's own icon, not notarized):

```bash
sh scripts/bundle-mac.sh
```

Without opening a window:

```bash
cargo run --release -- --run codex "What changed today?"   # one turn on the Rust engine
cargo run --release -- --snap shot.png --scene home        # a frame, rendered headless
```

Tests. Electron's own ledger reader checks what Rust writes when `LOCUST_TEST_TS_READER` points at the built
`packages/mission-store/dist/index.js`:

```bash
cargo test --release --workspace
```

## Benchmark

`scripts/bench-pair.mjs` measures both apps showing **the same conversation** at the same size, the same way
([notes/BENCH-PAIR-NOTES.md](notes/BENCH-PAIR-NOTES.md)):

- **Electron:** the built app from this checkout, as every drive runs it. Its window has a throwaway profile, sits
  behind other windows and never takes focus.
- **Rust:** the bench window, below other windows and click-through. It reads a copy of that same profile and nothing
  else.

Both show Wren's two-turn conversation (tool calls, a table, code), and each run checks that both really did. Each app's
whole process tree is sampled once a second over a minute at rest. No agent runs.

12 Nov 2026, Apple M6, macOS 27.2.1, Electron 0.872.0, 1100 × 780. The ranges are across three runs:

| metric | Rust (behind other windows) | Electron (behind other windows) | Electron (told it is in front) |
| --- | ---: | ---: | ---: |
| Memory, median | 38–69 MiB | 122–183 MiB | 237–285 MiB |
| Memory, peak | 71–73 MiB | 253–277 MiB | 295–332 MiB |
| CPU at rest | 0.03 % | 0.23–0.36 % | 5.0–8.8 % |
| Processes | 1 | 4 | 4 |
| Size on disk | 44 MiB binary | 320 MiB app bundle | |

Read the pair that matches: both behind other windows.

**What this does not measure:**
- The Rust window in front. Its bench window stays behind, so that it never takes the screen.
- Electron's first frame. The drive starts it its own way.
- CPU while a reply streams.

**First frame.** It is not a fair figure in this setup. The bench window sits behind other windows, and a covered window
draws nothing until Rune looks again on a back-off timer. So the first frame depends on what covers it. The same bench
with made-up data and no setup at all ranged from 146 to 1,524 ms in the same conditions; the paired runs gave
408–1,374 ms. A clean first frame needs an uncovered window, and these runs never take the screen.

## Notes

Each round of the port has a notes file in [notes/](notes): what was built, which TypeScript tests it carried over,
what was left, and how it was checked. Rune is pre-release, so the app pins one commit of it.
