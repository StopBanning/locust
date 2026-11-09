# Locust Native (preview)

A native Locust window, written in Rust on [Rune](https://github.com/anomalyco/rune). It is being built **beside** the
Electron app, not in its place: the Electron app stays what you download until this one covers what you use every day.

It runs no agent of its own. It is a client of the Locust you already have running, through the same local door
Claude Code and Codex use (Settings → General → **Let your other AI apps use Locust**). So it shows your real
teammates, on the agents and models you picked, and every approval card still appears in Locust's own window.

## What works today

- Your team, with each teammate's agent and model, and who is busy.
- Ask a teammate and watch the answer arrive word by word. The window follows the conversation file Locust writes
  first (read-only), so it shows what Locust recorded, as Locust records it.
- Open any recent conversation in full: your messages, the answers as Markdown, a line per tool step, plans and notes.
- **Quick Ask**: `locust-native --quick` waits in the background; Option+Space (or `LOCUST_QUICK_CHORD`) opens a small
  panel over any app. Ask, Tab to switch teammate, and the answer streams in; Escape or click away to hide, and the
  turn keeps going, so the answer is there next time.

## Why native

Measured on a Mac mini (M-series), 8 Nov 2026:

| | Electron app | Native preview |
|---|---|---|
| App size | 320 MB | 21 MB (9 MB compressed) |
| Memory at rest | ~430 MB, all processes | ~92 MB, window open |
| CPU at rest | ~0% | 0% |
| First frame | — | 0.11 s including GPU start (headless) |

The waiting in Locust is mostly the agents and models, but a lighter window matters for teammates that stay on all
day, and it moves at 120 Hz.

## Try it (developers)

Locust must be running with **Let your other AI apps use Locust** on.

```bash
cargo run --release
```

Make a Mac app you can double-click (`Locust Native.app` in the target folder, Locust's own icon, not notarized):

```bash
sh scripts/bundle-mac.sh
```

Check it without opening a window:

```bash
cargo run --release -- --ask Scout "What changed today?"
cargo run --release -- --snap shot.png --scene replay
```

## Not yet

Approvals, settings, routines, the built-in browser and terminal live in the Electron app; the native window says
when a turn is waiting for you there. Rune is pre-release, so
this pins one commit of it.

## Benchmark

`scripts/bench.sh [out-dir]` measures the native window against the Electron app that is already running. It never launches, quits or signals Electron. The native window runs with `--bench`, which opens it behind other windows without taking focus and uses synthetic data only.

First run, 9 Nov 2026, Apple M6, macOS 27.2, Electron 0.870.0 (native figures are medians of three runs):

| metric | native | Electron |
| --- | ---: | ---: |
| Idle memory (median RSS) | 80–84 MiB | 224–409 MiB across two runs |
| Idle CPU (interval) | 0.00 % | 0.03 % |
| First frame | 154–209 ms | not measured (never relaunched) |
| Size | 27 MiB binary | 320 MiB app bundle |

Electron was holding real conversations and the native window synthetic ones, which favors native somewhat on memory. Electron's memory moves with what it has open. The report written next to the samples spells out every method and limit.
