# Electron main-process tests carried into Rust

Lane: `~/work/locust-wt/maxlane`, branch `exec/claude-tests`, cut from native-engine at 6cd55444. Each batch takes about
five `apps/desktop/src/main/*.test.ts` files. For each case whose behaviour Rust has, there is one Rust test with
Electron's values and words, and a break check where the named test must fail. Rows are in `parity/tests.tsv`.

Break checks ran through `~/work/scratch/maxlane/mutall.py`. It is a copy of `~/work/scratch/mut6.py` that prints
every failing test name, not just the first three. That way "caught by the named test" can be read off the output.

## Batch 1: handoff, mode, handover note, route at start, remembered effort

| File | Row | Rust tests |
| --- | --- | --- |
| `handoff.test.ts` | carried (8/8) | `core/src/handoff_brief_tests.rs` |
| `a-handoff-keeps-its-mode.test.ts` | partial (1/3) | `core/src/mission_mode_tests.rs` |
| `a-handoff-without-a-note-says-so.test.ts` | carried (4/4) | `core/src/handover_note_tests.rs` |
| `route-at-start.test.ts` | partial (16/17) | `core/src/route_at_start_tests.rs` |
| `route-remembers-effort.test.ts` | waiting | none |

Bugs found and fixed:
- **The brief had no `kept`.** Electron's `composeHandoffPrompt` returns the sections it carried, in reading order and
  without the person's next words. The preview under the box reads that list. `carry_on::Briefing` now has `kept`.
- **The saved route dropped the request's account.** When a start matched the teammate's saved route, Rust took the
  saved route's account even when that was none. Electron sets the saved account only when there is one, so the
  request's own account stays. Fixed in `route_at_start`.

Found and not fixed (outside these files):
- `compose_handoff_prompt_with` leaves out the sections the person chose to drop, but it does not add Electron's
  `choiceNotice` line, "(The person chose to leave out ...)". Electron also takes that line's length out of the
  budget. This belongs to `a-person-can-leave-out-part-of-a-brief.test.ts`.

Why some rows are not carried:
- Mode: Electron's `parsedMissionMode` reads a missing or malformed mode as Ask. Rust has no lenient reader. Its
  request paths take a typed mode, and the door refuses one it does not know. The third case checks Electron's
  `index.ts` source text, so it has no Rust counterpart.
- Route at start: "returns no permission mode" holds by type, because `StartRoute` has no mode field. No test was
  written, so the row is partial.
- Remembered effort: Rust never records the route a start used. There is no `rememberRoute`, and a teammate's route
  changes only through the people panel. The teammate-store reader (`native::route_of`) also leaves effort out of the
  `Route` it builds. The window reads effort straight from the JSON instead (`native.rs` job.effort).

Checked for M26 and found to match:
- A carried-on turn reads the stopped turn's recorded mode as it is. Electron does this (`carry-on.ts` `modeOf`, sent
  straight to `codexMissions.start`, without `parsedMissionMode`), and so does Rust (`native::carry`). An earlier
  version of this note said they differed. That came from reading only the IPC payload path, and it was wrong.
- In Rust, a mode `Mode::parse` does not know becomes None, and the turn falls back to the teammate's saved mode.
  This applies to a carried turn and to a relay request (`native.rs`). Whether Electron's relay start does the same
  was not checked. The engine wire (`engine_process::origin_from`) and the door refuse an unknown mode.

Break checks (all caught by the named test): `kept` including next; a blank next counted as an instruction; next
not charged against the budget (refuses); newest-that-fit taking the oldest; the clip notice's words; stopped partway
with work in flight; the clean-finish wording; the request's account; the override for another teammate; the saved
effort; `modelChoice` ignored; the conversation's own route; the pick key; Kilo's Auto Free; an explicit pick
ignored; a share to anyone counted as a note; any share counted as a note to the owner; the no-note row's words; the
board brief's words; `plan` read as Ask.
A follow-up made one break for each of the 9 tests the first round never saw fail. All 9 were caught: next words
moved up front; the picker override ignored (both override tests); for nobody, the request's effort dropped; a
seeded account default promoted to a pick; the default runtime; the saved identity's effort change; a new chat not
on the saved model; home's pick key.

## Batch 2: model lists, free routes, models of your own

| File | Row | Rust tests |
| --- | --- | --- |
| `a-model-list-missing-one-is-asked-again.test.ts` | carried (2/2) | `core/src/model_lists_tests.rs` |
| `free-routes.test.ts` | partial (3/6) | `core/src/model_lists_tests.rs` |
| `a-model-of-your-own-is-unpriced-not-free.test.ts` | partial (2/3) | `core/src/model_lists_tests.rs` |
| `a-model-of-your-own-is-priced-as-listed.test.ts` | waiting | none |

Bug found and fixed:
- **An incomplete model list was kept for ten minutes.** This is Colin's 2026-10-02 report: "only account default is
  popping up for opencode". `models::local` kept whatever the sweep returned. Now `models::kept_catalog` keeps a
  catalog only when `lists_its_models_but_listed_none` is false. That function is Electron's rule: an OpenCode,
  Cursor or Claude Code that is ready and listed no models (for Claude Code, no aliases either). `read_listed_checked`
  counts an agent as ready when it is installed. An OpenCode 2 is not counted, because its list comes from an ACP
  session this sweep does not read. Electron's other branch (Antigravity readiness `unknown`, a `check-pending`
  diagnostic) is not carried, because Rust's sweep has no such states.

Not built, so not carried:
- `freeRoutesOnly`: holding a launch with `--remote-debugging-port`, `LOCUST_SPEND` or `LOCUST_FREE_ONLY` to free routes.
- `spendByTeammate` counting `unpricedRuns`: `spend::spent_by` adds up dollars only.
- Listed prices (`listed-prices.ts`): the OpenCode `models.json` price list, and own models told their price.

How "ready" is decided: Rust has no discovery readiness here, so `read_listed_checked` counts an agent as ready when
it is installed (`find_named`). Two edges differ from Electron's `readiness === 'ready'`:
- An installed but signed-out Cursor or Claude Code that lists nothing keeps the catalog from being kept. Every
  `models::local()` then reads again. That costs one sweep per call, but `local()` runs only for a Cursor start
  (`native.rs`) and the door's model list. It does not run on a poll.
- An `opencode --version` that times out counts as "not a 1.x". Its empty list is then kept.

`model-catalog.test.ts` (33 cases) was left for a batch of its own.

Break checks (all caught by the named test): an incomplete catalog kept; Copilot counted as listing models; OpenCode's
free rule loosened to any "free"; Kilo's `:free` suffix dropped; an own model on any runtime; unpriced runs counted
toward the limit. No break was made for the OpenCode 1.18.27 list on its own. Its six ids go through the same rule
as the first free-route case.

## Batch 3: worktrees, an npm lock, git without a shell

| File | Row | Rust tests |
| --- | --- | --- |
| `a-half-made-worktree-is-finished-not-trusted.test.ts` | partial (2/3) | `core/src/worktree_checkout_tests.rs` |
| `a-worktree-survives-only-a-finished-checkout.test.ts` | partial (6 tests) | `core/src/worktree_checkout_tests.rs` |
| `a-locked-file-is-not-a-permission-problem.test.ts` | carried (3/3) | `core/src/install_lock_tests.rs` |
| `a-planted-git-is-never-run.test.ts` | waiting | none |
| `git-reaches-a-run-with-no-shell.test.ts` | waiting | none |

No bugs were found. The worktree tests use real git and real `post-checkout` hooks with Electron's hook scripts, and
Rust behaved as Electron does in every case written.

Not carried, and why:
- Electron's worktree manager takes an injected `runGit` and a diagnostic `note` callback. Rust's `Manager` has
  neither. So these four cases have no Rust test: "no git at all for a finished tree", "an unreadable HEAD after a
  nonzero add", "a timeout is not a finished hook failure", and "a failing note callback". Neither does the note
  assertion in the first hook case. Rust drops the hook's stderr when it accepts the tree.
- `NoDefaultCurrentDirectoryInExePath` is Windows-only, and Rust does not set it.
- The `locust-git-reads` server (git for Ask and Plan runs with no shell) is not built.

Break checks, run through `~/work/scratch/maxlane/mutsafe.py` (all caught by the named test): `index.lock` left;
`reset --hard` instead of `reset`; an admin folder outside the repository accepted; the recorded branch not checked;
an `initializing` lock not checked; a usable tree refused; any failed hook accepted; the add writing
`core.longpaths`; the longpaths flag off Windows; a lock read off Windows; EPERM-on-rename not a lock; EACCES not a
folder problem.

**Why `mutsafe.py`.** One mut6 run was cut off by its outer `timeout 590` while another lane held the build lock.
That killed the Python but not its cargo child, and the break was left applied in `worktrees.rs`. I found it at
once, stopped my own orphaned run (the `sol` lane's suite was left alone) and checked the file out clean.
`mutsafe.py` restores the file on SIGTERM and SIGINT, and runs cargo in its own process group. It kills that group
at its own limit, so it needs no outer `timeout`.
`worktree_checkout_tests` is registered only for Unix (`cfg(all(test, unix))`), because it makes hooks executable
with `PermissionsExt`. `worktrees_tests.rs` does the same without the guard, so CI's Windows `cargo check
--all-targets` already stops there.
Two tests outside these files failed once while a break was applied, and passed in every full run:
`native::tests::relay_roundtrip_starts_recipient_records_peer_links_remembers_hub_and_returns_to_origin_thread` (batch
1, with the room_store note break) and
`door::tests::the_door_is_off_until_turned_on_and_answers_only_its_current_token`, with the longpaths break applied.
