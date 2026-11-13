# Routines that start on an arriving file, work in a copy, and recover, in Rust (13 Nov 2026)

Three rows of area 9, on `exec/native-routines` (from `exec/native-engine`): the arriving-file trigger, copy-mode
changes awaiting Keep or Discard, and retry-until-check, hand-off steps and wait-for-OK gates. Before this round the
native app ran a routine's first step only and held it for review; nothing advanced a run, counted it or recovered it.
All three rows sit on one port of Electron's runner, so the runner came first.

## What was built

1. **The runner** (`core/src/routine_runner.rs`, from `main/routine-runner.ts`, ported function by function):
   - `run`, `onRunEnded`, `reconcile`, `recover` and `tick`, behind one lock, as Electron's `serial` queue.
   - Everything durable goes through a `Routines` trait (the store), everything else through a `Host` trait (Electron's
     `RoutineRunnerOptions`). A host method left at its default is an option Electron's runner was not given.
   - Each later step follows up the one before once it COMPLETED. A failed, stopped, interrupted or questioning step
     ends the run there. A checker step counts only if it approves.
   - A step marked Wait for my OK first holds before it starts. A busy next teammate holds too. A standing goal runs
     the folder's check and sends the same teammate back to fix it, at most the set number of times.
   - A copy run's step runs in the routine's copy, and its changes are recorded in the same write that counts the run.
   - Only completion counts a run. A scheduled start that is refused is held off for an hour and said once.
2. **The words a hand-off is given** (`core/src/hand_off.rs`, from `shared/hand-off.ts` and `shared/step-budget.ts`):
   `CHECK_RULE`, `STEP_MARK`, the hand-off prompt, `stepWordsOf`, `verdictOf` and the step budget. Lengths are counted
   in UTF-16 units, as JavaScript counts them.
3. **The folder watcher** (`core/src/routine_files.rs`, from `main/routine-file-watch.ts`): the baseline, the 20-second
   settle, hidden and partial names skipped, six runs an hour, one run per batch, and a teammate's own files held and
   let go.
4. **The routine's copy** (`core/src/routine_copy.rs`, from `main/routine-copy.ts` and the named-copy half of
   `main/compare-copies.ts`): the folder as it is, without dependencies and build output, refused when too big. A
   sha256 manifest lets Keep know what changed and refuse over the person's own edits. Dependencies are linked, and
   unlinked before a copy is removed.
5. **The store's run writes** (`core/src/routines/progress.rs`, from `main/routine-store.ts`):
   - `recordRun`, `saveProgress`, `clearProgress`, `keepSchedule`, `abandon`, `settleStaged` and `recordMiss`. Each one
     compares attempt ids.
   - The run fields of `parsedRoutine` and `parsedExecution`. A receipt that does not read becomes a visible hold. A
     hand-off list, goal, waiting copy or last failure that does not read is dropped, never the routine.
   - `routines::save` now keeps hand-offs, copy mode, the goal and a new mode, with Electron's refusals.
6. **The host in the app** (`core/src/routine_host.rs`):
   - A step's turn goes through `engine_api::api().send`, with `started_by: {kind: 'routine', routineId, step}`.
   - Endings, answers and decision blocks are read from the turn's ledger (`door_tools::latest_final`, one new
     accessor). Whether a turn is still live is asked of the engine.
   - What a turn wrote is read from Undo's checkpoint records in the native data folder, so the watcher hears of it
     even when the engine is another process.
7. **Decisions** (`core/src/routine_recovery.rs`): `isRoutineRecoveryRequest`, `decideRoutineRecovery`, and the Keep or
   Discard handler from `index.ts` (`ROUTINE_SETTLE_CHANNEL`), with its sentences.
8. **The window** (`src/ui/routines_panel.rs`):
   - Run goes through the runner. The schedule, the watched folders and the ends of followed steps are looked at off
     the window's thread: once a minute, every 10 seconds and every second.
   - The card shows the run's last notice, and Electron's two cards:
     - `RoutineRecovery`: "Waiting for your OK" with Go ahead and Stop here, or "Waiting for your review" with Continue,
       Keep and Abandon behind the review box.
     - `RoutineChanges`: Keep, Discard (asks "Discard for good?") and Open the copy.
   - The editor gained On a new file (with its folder), who takes each step, Checker: must approve, Wait for my OK
     first, Where it works, and Keep going until the check passes.

## Judgement calls

- **One folder per teammate, not per window.** Electron's runner has one `workspaceId` and one `folderNow`: the window's
  folder. A native teammate works in its own folder, so the host answers both per routine, from the routine's
  teammate. Every step of a chain runs in that folder (or the copy), as every step in Electron runs in the window's
  folder, even when it is handed to a teammate whose own folder is elsewhere.
- **A native `send` error counts as a preflight refusal.** The agent's process is started on the turn's own thread, and
  a failure there is written in the ledger. So an `Err` proves nothing ran, and a first step's receipt is cleared.
  The spend-limit sentence maps to `SPEND_LIMIT_REACHED`. Everything else is `POLICY_REFUSED`, which is on Electron's
  preflight list.
- **Copies live in `<native data>/routine-copies`.** Electron keeps them in `~/.locust/routines`, but the native app
  writes only its own data folder. I added `routine-copies` to `profile_backup::LEFT_OUT` with its reason:
  `every_file_the_native_app_keeps_in_its_profile_is_classified` caught it. Electron never backs these up either.
  `profile_backup.rs` is beside Hydrogen's work but not on the do-not-edit list; the change is one line.
- **The window looks on a clock it is given.** The watcher's clock is the host's `now()`, which a test moves on with
  `skew`. The window's look intervals sit in `Cadence`, which tests set to zero. No test sleeps through a settle or
  a schedule.
- **The p11 startup hold stays.** `record_misses` still holds a dispatching or running receipt at startup. The runner's
  reconcile then re-decides that hold, because it is not `settledAtDispatch`. A step whose turn finished meanwhile
  therefore ends up continuable, as in Electron.
- **The routines row's text** (`Routine editor, schedules ...`, not one of the three) said there was no scheduler,
  completion accounting or recovery cards. I corrected its detail; its mark stays partial.

## Electron's special cases (`scripts/special-cases.py`, 21 lines)

| Line | What it does | Here |
| --- | --- | --- |
| runner 206 `fillInputs`: folder inputs must come from the folder picker | refuses a folder value nobody picked | **Not reachable.** Native has no input form (its own row), so `fill_inputs` refuses every routine that declares inputs before anything starts. No value, folder or otherwise, can reach a run. The picker check comes with the input form. Test: `a_routine_that_asks_for_inputs_is_refused_before_anything_starts`. |
| 279 `hold` skips a write that changes nothing | no reload loop | Ported. `repeated_reconciliation_does_not_create_an_event_reload_feedback_loop`. |
| 344 `reconcile` skips abandoned and followed runs | | Ported. |
| 354 `settledAtDispatch` skipped by `reconcile` | a refusal's reason is not re-decided | Ported. Checker refusals, wait-for-OK and busy holds stay through a new runner's reconcile (hand-off and wait-for-OK tests). |
| 443, 484 a later step too long, or with no runnable route, holds the prior attempt unless abandoned | | Ported in `start_step`. |
| 499, 507 `abandoned`: a fresh attempt id and start time | an abandoned run counts as no run | Ported (`fresh` in `start_step`). |
| 512 `saveProgress` expects no attempt after an abandoned one | | Ported. The store refuses otherwise (`store_refuses_overwriting_an_active_attempt...`). |
| 534 only a first step of a fresh attempt is cleared on a preflight refusal | | Ported. `refuses_what_it_cannot_honestly_run` checks no receipt is left. |
| 594 `recover`: only the held run's own attempt and step | a stale request is refused | Ported, in Electron's sentence. `refuses_stale_and_duplicate_decisions...` and `an_old_step_acknowledgement_cannot_abandon...`. |
| 642, 915, 968 an abandoned execution does not block Run, the tick, or the blocked notice | | Ported. |
| 923, 949 `arrivals.ready` and `arrivals.fired`: once per batch, told when it fired | | Ported. The runner test checks `fired`; the window test checks one run per file. |
| 924, 925, 932, 946 the `hook` kind | a private link starts a routine | **Left out with the link hook row.** A `hook` schedule is never due here; its Ask-only and batch paths are not built. |
| file-watch 81 `samePath` ignores case on Windows only | | Ported as written: case-insensitive under `cfg!(windows)`, exact elsewhere. That includes macOS, whose disks usually ignore case; Electron compares exactly there too. Both sides of the comparison come from the same folder strings, so they agree. |

## Which TypeScript tests came over

| File | Status | Where |
| --- | --- | --- |
| `a-file-arriving-starts-its-routine.test.ts` | carried | `routine_files.rs` (the 7 watcher cases, folder validity among them, plus the arrival note), `routine_runner_tests.rs::file_arriving`. The `index.ts` source pin is replaced by `routine_host.rs::a_turn_that_wrote_a_file_into_the_watched_folder_does_not_start_its_routine`. |
| `a-routine-works-in-a-copy.test.ts` | carried | `routine_copy.rs` (4 cases, plus too big), `routine_runner_tests.rs::works_in_a_copy` (3, plus abandon takes the copy), parsed `staged` in `the_stores_reads_drop_what_does_not_read...` |
| `a-routine-keeps-going-until-its-check-passes.test.ts` | carried | `routine_runner_tests.rs::until_check` (5), `valid_goal` and parsed goal and last failure in the recovery file |
| `a-routine-hands-off-to-the-next-teammate.test.ts` | carried | `routine_runner_tests.rs::hands_off` (11), `hand_off.rs` (6, plus the step budget), `routines/tests.rs` (4 store cases, including the mode edit) |
| `a-routine-step-waits-for-your-ok.test.ts` | partial | Runner (3) and store (1) carried. Not carried: the routine file as Markdown (the import/export row), the source pins for import, starters, team card and MCP listing, and the phone alert (no phone in native). |
| `a-paused-routine-waits-to-be-resumed.test.ts` | partial | Runner (3) and `scheduleBase` carried; p11 had the store's pause and resume. Not carried: the tray's "next routine" line (no tray line in native) and `setPaused` with odd values (native's `pause` takes a bool). |
| `routine-recovery.test.ts` | carried | `routine_runner_recovery_tests.rs` (18, on the native store) and `routine_recovery.rs` (request and workspace validation; native has no IPC sender to check) |
| `routine-runner.test.ts` | partial | All but W7 carried (`routine_runner_tests.rs::routine_runner_ts`, 17). W7's three input cases wait for the input form. |

Window tests (`src/ui/window_drive_tests.rs`, each also passes alone):

- `a_file_arriving_in_a_watched_folder_runs_its_routine_once`;
- `a_step_that_waits_for_your_ok_shows_its_card_and_go_ahead_runs_the_next_step`;
- `a_copy_runs_changes_are_kept_into_the_folder_or_discarded`.

`routine_host.rs` also runs two steps end to end through the engine and the fake Codex: one conversation, counted once.

## What was left out

- **Private routine link hook** (`routine-hooks.ts`). It needs the encrypted relay, which the Rust app does not have. A
  `hook` schedule saves and shows, and never starts.
- **Wait-for-OK alerts.** Electron's `asksFirst` raises a desktop attention and a phone alert. Here the card and the
  run's notice say it, and the status line shows the notice. There is no OS notification, phone or chat app.
- **Routine inputs** (W7, and the folder-picker rule above), with the input-form row.
- **Who takes a step's role line, and template chains,** with the templates row.
- **Open the copy** opens Finder with `open`. No test presses it: it would open a window.
- **Not checked:** a live session with a real agent. Everything here ran on the fake agents in `scripts/fixtures/`.

## Review (13 Nov)

- `scripts/review-branch.py` passed: 975 tests, a clean release build, and the three window tests alone and together.
- Break checks:
  - Firing a batch without clearing it fails `runs_at_most_six_times_an_hour...`.
  - The "abandoned starts fresh" rule (`start_step`, from `routine-runner.ts` 499 and 507) could be removed with all
    103 routine tests still passing. `a_run_after_an_abandoned_one_is_a_fresh_attempt` now pins it, and it fails
    without the rule.

