# How the port is checked

A rewrite that claims to match the app it replaces needs a referee. Tests written by the person who wrote the port only
check that person's reading of the original. So in this port, the referee is **Electron's own TypeScript**. The real
functions run over the same inputs, and the Rust has to give the same answers.

Then a person uses the real window. That catches what no test was written for.

This page covers both, and what each one missed.

## 1. Electron's code is the answer key

Scripts in `scripts/` run Electron's own functions from this repository: its command builders, stream readers,
carry-on code, model catalog and avatar rule. They write what Electron answers into `parity/`. A Rust test builds or
reads the same thing and compares. Each answer file records the Electron commit it came from.

| What is checked | Electron's code that answers | Cases | Rust today |
| --- | --- | ---: | --- |
| **The command each agent is started with**: every agent, in every mode, with each option a turn can carry (model, effort, resume, fork, browser, guard, skills, reference folders, connectors) | `runtime-adapters` command builders, called as the mission host calls them (`scripts/command-grid.mjs`) | 385 | 358 match |
| **How each agent's output is read**: recorded streams from Claude Code, Codex, OpenCode, Cursor, Copilot, Antigravity and Muse | `runtime-adapters` readers (`scripts/event-goldens.mjs`) | 37 | 37 match |
| **The conversation files Rust writes** | `packages/mission-store`, the reader Electron opens history with (`LOCUST_TEST_TS_READER`) | every ledger the engine tests write | accepted |
| **Carrying a conversation on** to another agent: why a turn can be carried on, the refusal, the checkpoint, the hand-over brief | `mission-store` and `main/handoff.ts` (`scripts/handoff-goldens.mjs`) | a set of stopped turns | match |
| **The model list**, from each agent's real list output captured on this Mac | the parsers and `model-catalog.ts` (`scripts/model-goldens.mjs`) | 5 agents | match |
| **Which modes an agent offers** | `status.ts` mode facts, held to the command grid | 385 | match |
| **A teammate's face** | `shared/avatar.ts` (`scripts/face-goldens.mjs`) | 100 ids | match |
| **Home's lists and the usage figures** | Electron's reducers and helpers over the same made-up rows (`home-oracle.mjs`, `usage-oracle.mjs`) | | match |

`python3 scripts/recount.py` prints the live totals.

**The lists only shrink.** Each case where Rust still differs is written down with its first difference
(`parity/commands-not-yet.txt`, `parity/events-not-yet.txt`). The test fails on a new difference. It also fails when a
case gets fixed but is left on the list. So the count can't drift silently in either direction.

**When Electron changes, the Rust finds out.** Run the scripts against a newer Electron and the answer files change.
The Rust tests then fail exactly where the two apps now disagree. `command-grid.mjs --compare` prints what moved.

### What it caught

On its first run (11 Nov 2026), 146 of the 385 commands matched, and **none** of the 37 streams were read the same
way. Every Codex and every Claude Code case differed. The port's own tests had passed through all of that. A few of
the differences:

- **Codex's plan tool.** Electron turns it on from Codex 0.153.0 and later, which means reading the CLI's version the
  way Electron does.
- **Claude Code's permission bridge.** It belongs in every mode except Auto. The port had it in Approve each only.
- **The command guard** has to run inside Claude Code in every mode, Auto included.
- **Antigravity** makes no start event of its own.
- **OpenCode** names evidence by its own record types.
- **Claude Code's system records**: helper tasks, refusals, compaction, billing.

None of these is visible in a demo. Each would have made a Rust turn behave differently from the same turn in Electron.

## 2. The bookkeeping

- **[PARITY.md](PARITY.md)** lists every feature as a row with three marks: built, alone and live. The rows live in
  `parity/rows.tsv`, and the tables are generated from them. `recount.py --check` fails when the tables drift.
- **`parity/tests.tsv`** tracks every TypeScript test file (1,309 in all). Each is marked as carried over, partial,
  waiting or does not apply.
- **`src/door_calls.rs`** pins every place the window still calls into Electron, by file. A new call fails its test.
- **The suite**: 678 tests pass (472 in the engine, 206 in the window). Another 20 need a real agent and are skipped
  unless asked for.
- **The `notes/` folder** has a notes file for each round: what was built, which TypeScript tests came over, what was
  left, and how it was checked.

## 3. What the real window caught

On 12 Nov the app ran as `Locust Native.app` with Electron quit, the Rust engine running every turn, for one live hour.
Sending, a follow-up, Stop, the door and an approval all worked. The hour also found eight things the tests had not
([notes/LIVE1-NOTES.md](notes/LIVE1-NOTES.md)):

| Found | Why no test saw it |
| --- | --- |
| The recent list named the agent ("Codex") where Electron names the teammate ("Codec"). So every rail card said "Nothing yet" and no count badge ever showed. | The tests' data folders had no team file, so there was no teammate to name. |
| A turn started through the door never appeared in the sidebar, the rail or the Board, so its approval card could not be found. | Tests started turns through the window's own send, which refreshes the list. |
| The release build was broken while every test passed. | Tests turn on a test-only feature. Three helpers the engine called existed only under it. |
| Quitting left turns marked as running. | No test quit the app. |
| The folder chip said "No folder" while the turn ran in Electron's open folder. | No test looked at the chip when no folder had been chosen. |
| Stopping a turn read as an error, then "Stopped". | No test looked at what the thread shows for a stopped turn. |
| Pressing a face on the Board kept the Board open. | No test drove the window. |
| Board rows printed raw state words ("· needs-you"). | No test drove the window. |

All eight came down to the same thing. The tests checked the engine with data shaped by whoever wrote the test, and
never what a person sees.

### Closing the loop

Each finding became a test that drives the real window code with no window at all. `rune::Runtime` builds the view
headless, and the tests click by key, paste into the message box, and run the window's own poll
([notes/WINDOW-TESTS-NOTES.md](notes/WINDOW-TESTS-NOTES.md)). There are 13 now. Three of them were run once with their
fix taken out, and they failed the way the bug did. CI now runs the release build **before** the tests, so a build the
tests hide fails in the first minutes.

These tests replaced the live sessions. They need no screen and no person, and they run on every push.

## 4. What is not checked yet

- **27 command cases**, listed with their first difference. They need connectors you can tick, forks, and a case
  Electron's host never builds.
- **7 two-way transcripts** (Codex's app-server, the Agent Client Protocol agents). These are conversations rather than
  streams, so the stream check doesn't fit them.
- **The system's own windows**: the folder dialog, a notification banner, and Quick Ask's global shortcut. Each one's
  decision logic is tested. None has been seen on screen.
- **Typing through accessibility.** The macOS accessibility library Rune uses (AccessKit 0.27.2) drops "insert at the
  caret", so dictation tools that type that way don't reach the message box. Keystrokes, paste and macOS Dictation
  work.
- **Speed.** Memory and CPU at rest are measured fairly ([README](README.md#benchmark)). The first frame and CPU while a
  reply streams are not.
