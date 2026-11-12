# Locust in Rust: the plan

## Goal

The Rust app becomes Locust. It covers the whole TypeScript app one to one, engine included, with no Electron or Node
underneath, and it does better wherever it can. When it's done, it replaces the Electron app.

## Why

Measured on 9 Nov 2026 (README, Benchmark; to be measured again with the same conversations in both):

- Idle memory: about 80 MiB, against 224 to 409 MiB for Electron.
- Idle CPU: about 0 %.
- Size: a 27 MiB binary then, 41 MiB on 11 Nov, against a 320 MiB app bundle.
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
- Progress is reported in three marks per row (built, alone, live) from `scripts/recount.py`, and as "milestone
  reached", not as "round N shipped".
- Every change gets synthetic-data tests. Live checks run headless, or behind other windows without focus.
- **The behavior is ported along with the features** (Colin, 9 Nov). Each round lists the TypeScript test files for its
  area (the `*.test.ts(x)` beside the code it ports, about 1,280 in all). Each of those tests either gets a matching
  Rust test or a line in the round's notes saying why it doesn't apply. A checklist row is only done when its tests
  are carried over.
- Colin switches only when Rust has earned it: Rust passes the same checks on the daily path, then he uses it for a
  day with Electron kept as a fallback.

## Risks

- **Runtime protocols drift.** Codex app-server and Claude Code change their wire formats. Mitigation: port the TS
  adapters' handling exactly, and keep their test fixtures as Rust tests.
- **The UI toolkit (Rune) has gaps.** Text editing, web views and a terminal grid may need work. Mitigation: confirm
  each Rune API before using it, and list any gap in the round notes.
- **Ledger compatibility.** Mitigation: rule 3 and a both-ways compatibility test in every engine round.
- **Size.** About 205k lines of TypeScript. Mitigation: the checklist order, and cutting nothing silently.

## Order of work from 11 Nov 2026

A review of the whole app and the port on 11 Nov found the engine broad but shallow, and the scoreboard counting rows
rather than behaviour. The milestones above stay; the work towards them now runs in this order:

| Phase | What | Done when |
| --- | --- | --- |
| A. A true scoreboard | Documents that match the code; three marks per row; a ledger of every TypeScript test file; CI for the port; a kept copy of Rune; the engine split from the window | `scripts/recount.py --check` and `scripts/test-ledger.py --check` pass in CI |
| B. Carry a day | Checks generated from the TypeScript (every agent's command, every event stream, ledger views); the door served by Rust; hand-over and carry-on; every agent in every mode with the command guard as a hook; models and limits; notifications and the folder picker; then the native engine by default | Colin works a day in it with Electron quit |
| C. Look like Locust | rune-kit's components, Locust's colours, type, icons, logo, agent marks and faces; picture pairs against Electron | Each main screen matches its Electron picture |
| D. The rest of the map | The remaining rows, with overlapping features simplified in Electron first | No row missing |
| E. Ship | Windows and Linux, the updater, moving the data folder, the switch | M7 |

Known gaps the review found, now in the rows: OpenCode's command guard is still the JavaScript plugin; Claude Code has
no skill or reference folders, forks or connectors; there is no notification on the native
engine. The door is served by Rust since 12 Nov (`notes/B1-NOTES.md`), and a turn can be carried on to another
agent (`notes/B2-NOTES.md`). Claude Code's modes and follow-ups and Codex's plan tool were fixed
on 11 Nov, and the guard became a hook inside Claude Code, Codex and Copilot (`notes/B3-NOTES.md`).

## Status

- **11 Nov 2026:** the scoreboard is generated from `parity/rows.tsv`. Every call into Electron's door is pinned by a
  test (`src/door_calls.rs`). The round notes are in `notes/`.
  - The engine is its own crate, `core/` (`locust-core`, 40 modules), with no dependency on Rune; CI checks that. The
    window crate keeps drawing. Tests: 407 in the engine, 180 in the window, all passing. Release rebuild after
    touching one window file: 28.1 s before, 23.6 to 26.0 s after; after touching one engine file: 27.6 s before,
    31.2 s after (both crates rebuild).
  - **Checks from the TypeScript (B0).** `scripts/command-grid.mjs` runs Electron's own command builders over 385
    cases (7 agents, 5 modes, 11 option sets) into `parity/commands.json`; a Rust test builds each case the way the
    engine does and lists the ones that differ in `parity/commands-not-yet.txt`. First count: 146 match, 239 not yet
    (every Claude Code and Codex case, as the review predicted). `scripts/event-goldens.mjs` reads 37 recorded agent
    streams with Electron's readers into `parity/events.json`; Rust's readers match none of them yet
    (`parity/events-not-yet.txt`). Both lists may only shrink. Re-run either script with `--electron` on a newer
    checkout (the grid also has `--compare`) to see what Electron changed.
  - **Codex and Claude Code (B3, first part).** Codex switches its plan tool on as Electron does, and Claude Code's
    command line is a port of Electron's builder: every mode with its own tools, the permission bridge outside Auto,
    `--resume` on a follow-up, and Locust's command guard as a hook served by this binary. The grid went from 146 to
    227 of 385: Codex 55 of 55, Claude Code 26 of 55. Live tests on the real CLIs passed, including a resumed Claude
    session. Tests: 419 in the engine, 180 in the window.
  - **The guard inside the agents.** Codex gets it in its thread config (only when no other Codex hook would load) and
    Copilot as a plugin folder, as Electron does. Live, each of Claude Code, Codex and Copilot in Auto had a push from a
    fix copy refused by the hook. Grid: 264 of 385. Tests: 421 in the engine, 180 in the window.
  - **Codex's modes.** Accept edits had run read-only on the native engine; it writes in the folder now, and the
    person's own MCP servers are off in Ask and Plan only, as in Electron. Tests: 422 in the engine.
  - **The kit (C1).** `src/look/` reads every token from Electron's `tokens.css` at build time and maps it onto
    rune-kit; Locust's controls are drawn to their `shell.css` rules, and `--scene kit` / `kit-light` show them all.
    Figtree and Geist Mono are vendored as TrueType (`notes/C1-NOTES.md`).
  - **Icons and marks (C2).** `scripts/marks.py` writes Electron's 55 icons, 10 agent marks and the Locust mark and
    wordmark into `src/look/icons.rs` (CI checks they match). The thread's toolbar is icon buttons with tooltips, and
    the sidebar's head draws the mark and wordmark (`notes/C2-NOTES.md`).
  - **The shell, first part (C3).** The message box, the thread's head, your bubble, the approval card, the sidebar's
    rows and Home are on the kit, compared with Electron's frames in both themes (`notes/C3-NOTES.md`).
  - **Faces (C4).** Every teammate shows its bot, as stills exported from Electron's own TeammateBot (440 bots), chosen
    by a port of avatar.ts that matches Electron for 100 ids (`notes/C4-NOTES.md`).
  - **Stream readers.** All 37 recorded agent streams now read exactly as Electron reads them (was 0): evidence keeps
    the sanitized record, counts and numbers as Electron writes them; Claude Code's helper, background and status
    records; a full port of Antigravity's transcript reader (`notes/B3-NOTES.md`).
  - **The door (B1, first part).** The Rust app serves its own door: the local server, Electron's tools and rules for
    mode and folder, and a stdio bridge (`locust-native --door-bridge`). Live through the bridge, an Auto turn in a
    named folder, a follow-up, a stop and a refused folder; Electron's reader accepted every ledger. Native ledgers now
    record the mode, sandbox and folder a turn really ran in, and Auto runs only where Settings allow it
    (`notes/B1-NOTES.md`).
  - **Hand-over and carry-on (B2, first part).** A reply on another agent, and a turn whose plan ran out or whose
    provider or process failed, start the new agent cold with Electron's brief and checkpoint; the door has
    `carry_on`. Rust matches Electron's code on eight generated cases byte for byte, and a live carry-on from a stopped
    Codex turn to OpenCode's free model finished from the brief alone (`notes/B2-NOTES.md`).

- **M0:** written and pushed on 9 Nov 2026, waiting for Colin's approval.
- **M1:** reached on 9 Nov 2026, except the run with Electron fully quit, which needs Colin.
  - Two real Codex turns ran through the Rust engine into a scratch data folder, each streaming and exiting 0.
  - Electron's TypeScript reader accepted both ledgers with no issues.
  - Codec drafted it and Cursor reviewed it. The review found an `O_NOFOLLOW` race and a stream deadlock, and both
    are fixed and tested.
- **M2:** built on 9 Nov 2026, with the real exit test pending.
  - The window starts, continues and stops Codex conversations through the Rust engine. The status line shows
    "Engine: native".
  - Teammates are read from Electron's `teammates.json` without writing to it. Recent and transcripts merge both
    folders.
  - Every send rescans both folders for the newest turn first.
  - Ledger files are created by walking the path one directory at a time with `O_NOFOLLOW`, and `~/.codex` is
    refused.
  - Sage built it, Codec reviewed it, and all five review findings are fixed. 112 tests pass.
  - Real check, first try: turn 1 ran and its continuation's resume was accepted. Then Codex's usage limit ended the
    turn, and the limit was recorded the way Electron records it.
  - Real check, 9 Nov evening: `real_codex_continue` passed. The continuation resumed the same Codex thread and
    remembered a word from turn 1, chained by `continuesFrom`, in 9 s.
  - Still to do: use the window with Electron quit.
- **M3:** reached on 9 Nov 2026, except the window check with Electron quit.
  - Claude Code runs through the Rust engine (`runtime/claude.rs`), ported from `claude-commands.ts` and
    `claude-events.ts`. A real turn ran and Electron's reader accepted its 38 events.
  - Approvals for Claude Code and Codex: a card in the window in Electron's words, with keys 1/2/3 and "Needs you" in
    the status line. Claude asks through the same permission-tool bridge Electron uses.
  - Real checks `real_claude_approval_deny_then_allow` and `real_codex_approval_deny_then_allow` both passed. Each
    runtime asked to create a file, a denial left no file, an approval made it, each answer was recorded once with no
    retry, a late second answer was refused, and Electron's folder was unchanged byte for byte.
  - Cursor built it, and Codec reviewed and fixed it. 144 tests pass.
