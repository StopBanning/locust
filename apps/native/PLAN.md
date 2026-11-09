# Locust in Rust: the plan

## Goal

The Rust app becomes Locust. It covers the whole TypeScript app one to one, engine included, with no Electron or Node
underneath, and it does better wherever it can. When it's done, it replaces the Electron app.

## Why

Measured on 9 Nov 2026 (README, Benchmark):

- Idle memory: about 80 MiB, against 224 to 409 MiB for Electron.
- Idle CPU: about 0 %.
- Size: a 27 MiB binary, against a 320 MiB app bundle.
- First frame: about 0.2 s.

## Rules while both apps exist

1. Electron is never broken. Rust work stays in `apps/native` on `exec/native`, the Electron code is not touched, and
   nothing merges to main without Colin's OK.
2. Rust writes only to its own data folder (`LOCUST_NATIVE_DATA`, default `<Electron data folder>-native`) and only
   reads Electron's. It points at the real folder only at cutover.
3. The file formats are shared. Rust writes ledgers in exactly the schema Electron writes, so each app can read the
   other's history. Compatibility tests prove it both ways.
4. The product rules in AGENTS.md apply unchanged:
   - Runtime, model, provider route and fallback are separate concepts.
   - Every switch goes into the append-only event stream.
   - Hidden reasoning is never kept.
   - An irreversible action is never retried.
   - Subscription credentials are never touched.

## Milestones

Each milestone has an exit test. A milestone is done when its test passes, not when its code is written.

| # | Milestone | Exit test |
| - | --- | --- |
| M0 | This plan, and the checklist in PARITY.md | Colin approves it |
| M1 | **Walking skeleton**: the Rust engine runs a real Codex turn and writes its own ledger | With Electron quit, `locust-native --run codex "<prompt>"` streams a reply. The ledger it writes opens in the native thread view and passes a schema check against Electron's reader |
| M2 | Skeleton in the window | The native window starts and continues a Codex conversation with no Electron running, then stops the turn |
| M3 | Claude Code plus approvals | A Claude Code turn asks for approval in the window, and allowing or denying it works, with Electron quit |
| M4 | Daily driver | Teammates, every runtime Locust supports, routing and carry-on, the composer, and the full thread view. Colin uses Rust for a day's work with Electron kept as a fallback |
| M5 | Dev tools | Changes and diffs, git and PRs, terminal, browser, routines and compare |
| M6 | Everything else | Settings, usage, calls and voice, cloud and remote, documents, onboarding, updates and pets. PARITY.md has no row left missing |
| M7 | Cutover | Rust reads and writes the real data folder, ships as Locust, and Electron is retired |

## How the work runs

- Codec (Codex) drafts each round in a copy folder. Argon (Antigravity/Gemini) reviews it. Claude tests, fixes and
  pushes. Each round closes named PARITY.md rows and ticks them off.
- Riskiest first: the engine (M1 to M3) comes before more screens.
- Progress is reported as "rows done of 153" and "milestone reached", not as "round N shipped".
- Every change gets synthetic-data tests. Live checks run headless, or behind other windows without focus.

## Risks

- **Runtime protocols drift.** Codex app-server and Claude Code change their wire formats. Mitigation: port the TS
  adapters' handling exactly, and keep their test fixtures as Rust tests.
- **The UI toolkit (Rune) has gaps.** Text editing, web views and a terminal grid may need work. Mitigation: confirm
  each Rune API before using it, and list any gap in the round notes.
- **Ledger compatibility.** Mitigation: rule 3 and a both-ways compatibility test in every engine round.
- **Size.** About 205k lines of TypeScript. Mitigation: the checklist order, and cutting nothing silently.
