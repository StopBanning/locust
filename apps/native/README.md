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
`LOCUST_NATIVE_ENGINE=1`, it uses its own. A few things still need Electron's door and say so in the window: choosing
a model for one turn, the model list, carrying a conversation on with another agent, the hand-over brief, Quick Ask
and `--ask`.

The Rust app never writes Electron's data. It reads Electron's folder and writes its own (`LOCUST_NATIVE_DATA`, by
default `<Electron's data folder>-native`).

## What is behind the engine

- Conversations: the full thread, streaming Markdown, tool steps, plans, reasoning, find, search, the beside view.
- The composer: drafts, @ files and teammates, slash commands, attachments, the queue.
- Approvals: the card in Electron's words, saved rules, the command guard's reach and plain explanation.
- Teammates, memory, About you; Changes with commit and pull request; a terminal; a browser panel with teammate tools;
  routines (partly); Settings and usage; Home, the Board and what's new; PDF, Word, PowerPoint, sheets, math and
  Mermaid.
- **Quick Ask**: `locust-native --quick` waits in the background; Option+Space (or `LOCUST_QUICK_CHORD`) opens a small
  panel over any app.

Partial and missing pieces are listed row by row in PARITY.md.

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
cargo test --release
```

## Benchmark

`scripts/bench.sh [out-dir]` measures the native window against the Electron app that is already running. It never
launches, quits or signals Electron. The native window runs with `--bench`, which opens it behind other windows
without taking focus and uses synthetic data only.

First run, 9 Nov 2026, Apple M6, macOS 27.2, Electron 0.870.0 (native figures are medians of three runs):

| metric | native | Electron |
| --- | ---: | ---: |
| Idle memory (median RSS) | 80–84 MiB | 224–409 MiB across two runs |
| Idle CPU (interval) | 0.00 % | 0.03 % |
| First frame | 154–209 ms | not measured (never relaunched) |
| Size | 27 MiB binary then; 41 MiB on 11 Nov | 320 MiB app bundle |

This run is not a fair memory comparison yet: Electron held real conversations and the native window synthetic ones.
It will be measured again with the same conversations open in both.

## Notes

Each round of the port has a notes file in [notes/](notes): what was built, which TypeScript tests it carried over,
what was left, and how it was checked. Rune is pre-release, so the app pins one commit of it.
