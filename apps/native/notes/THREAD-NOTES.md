# Conversation thread parity — Sol, 13 Nov 2026

Branch: `exec/native-thread`. Changes stay under `apps/native/`; no Scout source, handoff file, push, network, real agent, or on-screen window is part of this work.

## Sections carried

Each Electron case has its own named Rust test, using its command, path and sentence literals.

- `thread composition`: 15 cases in `core/src/thread_view_tests.rs`.
- `collapsed activity`: 7 cases in `core/src/activity_summary.rs`.
- `what a turn ran, and what came back`: 12 cases in `core/src/command_outcomes_tests.rs`.

The 34 previously carried cases remain. The missionView row stays **partial**, now recording 68 of 264 cases. No case was marked `does-not-apply`: the sections considered here apply to native.

## Behavior and evidence

- A stopped or provider-final message has no streaming caret: `shows_a_caret_only_while_text_is_genuinely_still_arriving`. The UI now uses the same predicate instead of just the provider-final flag.
- A cached projection of the latest turn's events supplies Starting, the named current step, Working after it closes, and Writing while text arrives, with the turn's start time throughout. The line goes away when the run ends or an approval waits. The composition tests name each of these transitions. The ledger caches the projection when events change, reusing its existing event vector; Session polling copies only that small result. Drawing reads it and the clock, never files, git, or a process. `the_ledger_caches_the_live_projection_when_events_change` verifies the cache. Live labels use the existing secret scrubber.
- Host relay/resume briefings stay out of the person's bubble, while preserving the ledger's turn boundary and metadata indexing. `never_draws_a_turn_the_host_briefed_as_the_persons_own_words` pins relay with/without a peer message, resume, ordinary and routine prompts. Native's existing attachment/tag/routine transformations are not replaced by this small origin predicate; their remaining missionView cases are unported.
- Peer navigation uses a received link, never a posted one or an empty message id: `finds_the_run_a_peer_message_was_delivered_into_so_the_exchange_can_be_opened`. The existing peer index now shares that lookup.
- Provider limits are distinct items. A temporary-rate-limit notice keeps the turn running and its live line visible, rather than offering carry-on as though it ended. A quota limit suppresses its duplicate error diagnostic and run-level failure; a temporary warning does not count as an already-shown failure. `surfaces_provider_limits_and_diagnostics_as_their_own_items` and `says_a_quota_failure_once_not_as_a_card_and_again_as_a_red_line` pin the behavior.
- A completed turn with neither reply nor work gets Electron's exact silent-turn sentence: `says_so_when_a_turn_finished_and_wrote_nothing_back`. Tool work, plan/thought activity, a posted peer message and compaction prevent a false claim.
- The collapsed group counts files, commands, helpers and other calls; reasoning is not a call. Failed edits are calls rather than changed files. Multiline file names become separate unreported-change rows. `counts_edits_and_commands_separately_from_their_tool_events`, `counts_files_not_edit_calls_one_codex_file_change_can_name_several`, `draws_one_row_per_file_a_runtime_named_without_a_diff_never_one_row_named_after_the_tool`, and `singularizes_honestly` cover this. Shell edit detection reuses the existing export predicate: read-only sed stays a command; sed in place, tee with a file and apply_patch count as edits.
- Command outcomes name distinct command kinds (past shell hosts and leading cd), without grading successful work. Non-zero exits outrank unreported commands; a live command is not yet unreported. Refused/declined commands never ran. Each of the 12 command cases pins its exact line. The UI shows the command line even when a thought headlines the group, with amber on reported trouble.

Three additional headless Rune checks verify that the cached live line reaches the renderer, the relay briefing does not, and named files reach separate rows. The existing group test also pins the rendered command-name line. Three additional core checks cover refused commands, suppression of a run failure already said by its quota card, and the ledger live-line cache. These supplement, rather than combine, the 34 case ports.

## Mutation checks

`scripts/fixtures/thread-mutation.py NAME` changes one expression or constant, runs the entire core suite through gentle-cargo under `timeout 590`, requires the named test to fail by assertion, restores the source in `finally`, captures the restored `git diff` in that control's log, and runs `git diff --check`. Controls now use the wrapper's shared target folder and shared build lock; the former `--target-dir target/thread-controls` override was removed in this finish pass. The first attempt was rejected: a cached neighboring core binary ran 1,536 passing tests without the new tests, so it could not qualify as mutation evidence. Logs are local ignored files `notes/thread-mutation-NAME.log`.

Verification results are recorded below after the runs finish. Five initial isolated-target controls completed in the ordinary debug profile. The host-prompt attempt was rejected because the existing `relay_prose_return_posts_unanswered_prose_back_to_asker` fixture hit its deadline, poisoned a mutex during cleanup, and aborted the harness before the intended assertion ran. Remaining controls use dev opt-level 1 with debug symbols off and four harness threads to bound fixture contention, after a clean core baseline at that profile; the required final workspace run still uses the ordinary release profile.

The initial full core run caught the silent-turn message being appended after the edit-check receipt. It now precedes that receipt; `edit_check::tests::a_reopened_conversation_keeps_its_check` proves the ordering. A later core run passed 1,567 tests (20 ignored). The first complete headless UI run exposed an old group-summary expectation and two presses outside the viewport after the thread grew. The summary expectation now uses Electron's words; the project-join and earlier-version checks explicitly scroll their existing controls into view before pressing, retaining their assertions. These test adjustments change no product navigation behavior.

## Left to carry

196 missionView cases remain, including the full activity change/diff card, live-line registers/connectors/plan aside, finished trace totals, conversation stitching, cancellation-summary case ports (native already has its own implementation), path identity/no-change claims, ended questions, peer exchanges, signal rail, and all other sections not named in the parity row. Existing implementations of some of these do not constitute carried Electron tests.

This batch does not claim full buildThread parity: native still uses its ledger items and adjacent activity groups. It does not add Electron's full step-group lifetime rules, turn footer/diffs, notice folding, or the remaining live-register details. The new file-name rows apply to native's existing no-diff representation; diff parsing remains in the later change-card work.

No real-agent run, visible app launch, screenshot, live approval interaction, or packaged app check was performed. Native's release workspace suite is the scoped gate requested by the handoff; the repository-wide Electron gate is outside this native-only task.

## Finish pass — 14 Nov 2026

The inherited batch passed the release workspace suite before its first commit: **1,568 core passed, 20 ignored; 393 headless window passed** (1,961 passed in total), with no compiler warnings. Commit `5f785c82` preserves that batch, with both handoff files excluded.

All **20 mutation controls** now have a completed whole-core run with the named assertion failure. The ten completed in this pass are `limit-item`, `temporary-limit`, `live-cache`, `edit-class`, `failed-edit`, `file-rows`, `nonzero`, `unreported`, `command-head`, and `command-kinds`. The other ten retain the previous pass's completed logs. The runner verifies failure before appending the restored diff, and every original expression was checked present after the final control. No mutation remains. Logs are ignored local evidence, not source files.

The shared-target development-profile baseline passed **1,568 tests, 20 ignored**, with no warnings. Controls retain opt-level 1, debug symbols off and four test threads. An early release-profile control was interrupted by its outer timeout; its exact one-line mutation was restored manually and the ledger diff checked empty. Its retry timed out inside the runner and restored through `finally`. Neither timeout counts as mutation evidence. The first development baseline also timed out after waiting for the shared lock and compiling dependencies; its complete retry is the green baseline reported above. Subsequent control invocations leave the timeout around Cargo inside the runner so restoration can finish.

No further missionView section was ported in this pass: shared-lock waits and rebuilding consumed the optional-section budget. The parity row remains **partial, 68/264 carried, 196 remaining**, with no does-not-apply cases. The activity change card, live registers and finished trace are still next in the brief's order.


## Next sections — 14 Nov 2026

The activity change-card section has 11 individual ports in `core/src/activity_entries_tests.rs`.
The projection now retains completion patches and their reporting runtime, parses one file row per
file, counts the recorded hunks rather than the runtime header, preserves a truncated single-file
header only beside that file, and opens the first small diff (new HTML pages open even when large).
Edits with no diff say `change not reported`; results past the cap say `result too large to keep`.
Ordinary row names suppress a repeated tool name. Nonzero command exits mark their row failed.
The renderer uses these rows and counts and reuses the existing pure diff renderer. A headless test
pins the displayed patch and counts. New HTML rows show their diff; native's existing page browser
remains a separate surface rather than Electron's embedded running-page frame.

Ten whole-core controls (`patch-rows`, `patch-counts`, `patch-reported`, `patch-large`, `patch-page`,
`patch-missing`, `patch-too-large`, `tool-name`, `patch-ledger`, `exit-result`) each failed their
named assertion and restored the original source. The runner now checks every original control at
startup, refusing a stale or unrestored mutation left by an interrupted process. The first focused
run passed 11 tests with no warnings. Restored baseline and final release results follow below.

Restored section baseline: **1,579 core passed, 20 ignored**; release headless activity check:
**6 passed** (including the new patch-row check). Both compiled without warnings.


`which register a live line is`: 11 individual cases in `thread_view_tests.rs`.
The cached live line now carries a connector's split tool name and server detail, and the
window names the tool/connector register and detail. The existing reasoning, turn, message,
ordinary tool, tool-close and initial registers match the Electron assertions unchanged.
`live-connector` and `live-detail` each failed their named assertion across the whole core suite
and restored. Focused thread suite: 28 passed. Restored whole core: **1,590 passed, 20 ignored**,
no warnings. No does-not-apply cases. Register changes still share the turn clock.


`the trace line for a finished turn (SURFACES-0.22)`: 9 individual ports in
`activity_trace_tests.rs`. Each ledger turn now carries its own trace footer, with duration,
reasoning time, command names/outcomes, other calls, helper outcomes and readable runtime notices.
Notices already said in a thread note are not repeated. Thinking is not a call; an open command
is running, and a silent helper is not unreported until the turn ends. Whole core: **1,599 passed,
20 ignored**, no warnings. Five whole-core controls (`trace-duration`, `trace-thought`,
`trace-running`, `trace-helper`, `trace-source`) failed their named assertions and restored.
Four ledger assertions now account for the new trace footer while retaining their original helper
and secret-scrubbing assertions. A new headless test checks the ledger-to-trace path and disclosure;
its run is deferred to the required final workspace suite. The redundant activity-only release
check was canceled while waiting for the shared lock. No does-not-apply cases.


`a conversation across turns`: 8 ports in `conversation_turns_tests.rs`, plus a synthetic
Session integration test in `ledger.rs`. The pure walk follows replies oldest first, stops on
missing/cyclic links, distinguishes a reply switch by the host's anchored instruction section,
and steps across a mid-run handoff without dropping the turns before it. Session assembly
uses that projection: a mid-run handoff keeps the original ask and both runs' work in one turn,
with a divider at the seam; a reply switch keeps a separate person message containing only the
new ask. The existing handoff divider draws both seams. Whole core: **1,608 passed, 20 ignored**,
no warnings. Four whole-core controls (`conversation-walk`, `conversation-reply`,
`conversation-midrun`, `conversation-draw`) failed their named assertions and restored.
No does-not-apply cases. Checkpoint counts beyond these fixtures remain the native divider's
existing behavior; this small pure switch projection does not add checkpoint recovery.

## Continued sections — 14 Nov 2026

`cancellation summary`: five individual ports in `cancellation_summary_tests.rs`, committed as
`882fd4b5`. A recorded planned count without reported plan states remains entirely never started;
settled tools cannot advance it. `separates_tool_outcomes_and_never_substitutes_them_for_unreported_plan_states`
fails without the fallback. Eleven cancellation-filtered tests passed. Both whole-core controls
(`cancellation-planned`, `cancellation-dedup`) failed their named assertions and restored. The first
attempt at the deduplication control failed to compile because removing insertion erased HashSet's
inferred type; it was rejected, corrected to retain insertion while allowing duplicates, and rerun.
No compile failure is counted as mutation evidence.

The path identity and no-change sections have eleven ports in `path_claim_tests.rs`: five for one
file spelled two ways, four for an edit-permitted run changing nothing, and two for the limits of
that claim. Commit `5bb21d9f`. File row identity includes the path relative to the known workspace,
status, and all hunks and rows; counts use those same deduplicated rows. Different hunks remain
separate real edits. Traces count distinct paths, retain their raw paths so the window can apply
the conversation's known folder, and derive edit permission from each turn's recorded sandbox,
including earlier turns. Unknown permissions make no no-change claim. A `host.shared_workspace`
notice withholds it, rather than turning lack of attribution into evidence that nothing changed.
The no-change segment is plain, not amber. The window uses the deduplicated rows and totals;
a new headless renderer check pins one row and one `+1 -0` total for two identical reports.

The eleven focused cases passed. A whole-core baseline passed **1,624 tests, 20 ignored**, without
compiler warnings (the log named `thread-cancellation-baseline.log` includes these new path cases;
it is not claimed as a cancellation-only snapshot). All five whole-core controls (`path-workspace`,
`path-restatement`, `path-hunks`, `no-change-permission`, `no-change-overlap`) failed their named
assertions, restored, and passed the whitespace check. All 48 registered original expressions
were checked present afterwards. The headless check is pending the final workspace run.

`a question the run ended on`: six individual ports in `ended_question_tests.rs`.
The decision renderer now calls the pure core projection for the newest ended turn's
last provider-final message; tag stripping remains shared with the reply renderer.
The six focused cases passed without compiler warnings. No does-not-apply cases.
Both whole-core controls (`ended-question-gate`, `ended-question-last`) failed their named
assertions and restored. The restored whole-core baseline passed 1,630 tests, 20 ignored,
with no compiler warnings. Shared-lock waits preceded these completed runs.

`both halves of a teammate exchange are drawn`: five one-to-one cases in
`peer_thread_tests.rs`. The pure projection retains all turns, groups by the other
party, sorts messages by time, and supplies distinct turn-scoped keys and before/after
placement. The window now draws received groups before the work and sent-only groups
after it, using that projection, without repeating them in a second Handed out card.
The five focused cases passed without warnings. Three whole-core controls
(`peer-turns`, `peer-placement`, `peer-key`) failed their named assertions and restored.
No does-not-apply cases.
The release workspace build completed with no warnings during the second release attempt;
its headless desktop tests were interrupted by the outer timeout. A full restored
workspace pass is still pending. Prior attempts spent 590 seconds waiting for the
shared lock, or exhausted the remaining time compiling after a long lock wait.


## Thread-5 continuation — 14 Nov 2026 (verification pending)

Read the original brief, thread-5 handoff, Electron cases and complete peer/rail functions.
The inherited peer source still contained the `peer-empty` mutation (`true &&`), despite
its handoff saying all controls were restored. Restored `message_count > 0 &&` and added
`peer-flat`, a whole-core control for multiline preview flattening. The seven case ports
and window wiring are prepared; **this continuation has not verified or committed them**.

The inherited signal-rail draft has eight one-to-one Electron cases. Corrected its helper
filter to use `parentItemId`, reused the shared shell-tool predicate, and replaced its UTC
clock slicing with native local-time formatting. Rail rows are projected on transcript
application and copied into inspector inputs: unlike Electron's render-time projection,
the native window need not consult the timezone database while drawing. Two supplemental
cases cover that cache and helper/refused-command behavior. These edits are **uncompiled
and unverified**, and no new signal-rail mutation is claimed. No does-not-apply decision.

Verification is blocked by the shared gentle-cargo lock. The focused peer attempt and the
first whole-core baseline each exited **124** after 590 seconds waiting for the lock;
neither reached compilation or ran a test. The third baseline also exited **124** after 590 seconds waiting for the lock, without
compiling or running tests (`notes/thread-reading-baseline-retry.log`). In total, all three
bounded attempts timed out before Cargo started. Whitespace checks passed. No real-agent run,
network call, visible window, push or branch switch was performed. No Scout source changed.
The first required commit, remaining sections, parity recount, full release workspace suite
and release build are still pending; no last-full-run counts are claimed for this continuation.

## Thread-6 continuation — 14 Nov 2026

The inherited exchange-reading and signal-rail source passed the full core baseline:
**1,652 passed, 20 ignored**, no compiler warnings (`thread-6-core.log`). The run
waited 673 seconds for the shared lock before compiling. The rail lives in
`signal_rail.rs`; its local clock strings are cached by the ledger and copied into
inspector inputs, avoiding render-time timezone lookups. This preserves the visible
sentences while doing the projection outside drawing. Seven exchange-reading cases
and eight rail cases remain one-to-one ports; two supplemental rail cases pin the
cache, helper-call filtering and refused-command sentence. No does-not-apply cases.

The fixture runner now uses the handoff's 1,800-second outer timeout and
1,780-second Cargo timeout. Thirteen exchange/rail controls are scheduled. The first
`peer-open` attempt waited 1,122 seconds, compiled and failed its named assertion,
but hit outer timeout 124 before the entire core suite finished. It restored the
source; all 66 registered original expressions were checked present afterwards.
That incomplete attempt is **not** counted as control evidence. The retry waited
1,164 seconds, then completed: **1,651 passed, one intended failure, 20 ignored**.
`peer-open` failed `leaves_a_long_back_and_forth_collapsed` and restored; the other
twelve controls are pending. The next control, `peer-empty`, is queued behind
another checkout's full native release suite. No later section is claimed carried.
The prepared assistant-text file was removed without registering it; the next
section remains `assistant text` (three cases), then the other sections in file order.

The second control (`peer-empty`) timed out at 1,800 seconds after waiting 1,469
seconds and compiling for the remainder, without running tests. Its `finally`
restored the file. The control loop stopped on that timeout: **1/13 completed**,
not a claim of a completed mutation pass. Commit `f0407404` records the inherited
core-verified batch; its staged source was checked to contain all 66 original
control expressions, leaving the active mutation only in the working tree.

The parity row now includes the previously committed cancellation (5), path
identity/no-change claims (11), ended questions (6), both exchange halves (5),
exchange reading (7), and signal rail (8). Total: **149 of 264 case ports**,
**115 remaining**, with exchange/rail controls explicitly incomplete. No new
Electron-only exemptions. Required release verification follows.


## Thread-7 continuation — 14 Nov 2026 (section verification pending)

Committed the inherited notes correction as `8633f4c9`; no handoff file is staged.
Started exactly one background native release suite as requested, with the
3,600-second outer timeout and 3,580-second Cargo timeout. `thread-7-native.log`
records a 701-second shared-lock wait, a warning-free release compilation in
7m 45s, and this final result:

`test result: FAILED. 396 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 175.95s`

The failure is
`window_drive_tests::review_changes_shows_a_teammates_branch_turn_by_turn_and_lands_it_as_one_commit`:
`timed out waiting for the landing, read again: status "Ready"`.
The test never reached its landing-result assertion. Its cause is not established;
no review/landing source or test was changed. The new cached start-label renderer
check passed in that run, but this does not verify the later clock-refresh edit.

Prepared, **uncommitted and not counted as carried**:

- Three individual `assistant text` cases in `core/src/assistant_text_tests.rs`,
  registered in `lib.rs`, exercising the real ledger's existing append, replace,
  final flag and per-item ordering with Electron's literals. No assistant fold
  behavior change is needed. Three whole-core mutation controls are prepared,
  not executed.
- Four individual `when a mission says it began` cases in `time_marker.rs`, plus
  a Session integration check and two headless window checks. A pure local start
  label preserves today's bare time, older days' date and older years' year.
  Session polling caches the original mission's timestamp and label even across
  a handoff; drawing uses the cached string. A minute-clock refresh updates an
  ended thread's date without reopening its ledger. This follows native's existing
  24-hour clock and English short-date convention; it does not add locale selection.
  The date section and later refresh are unverified; no mutation pass is claimed.

The initial assistant-text filtered run, its two retries, and the start-label
filtered run each exited **124** under `timeout 590` after waiting for the shared
lock. All four logs contain only the wait message: no compilation or tests ran.
The later assistant retries use the established opt-level-1/debug-symbols-off
profile and four test threads, matching the control runner's existing profile.
They did not get far enough to exercise it.

The parity row remains **partial: 149/264, 115 remaining**. No new exemption.
Exchange/rail controls remain incomplete at 1/13. Static whitespace and mutation
original-expression checks pass; these are not Rust compilation evidence.
Required final release build/workspace results are recorded below when complete.
No real agent, network request, visible window, Scout edit, branch switch or push.

`python3 scripts/recount.py` completed without changing the tables. The wrapper's
optional TypeScript reader defaults to another checkout; future checks in this run
explicitly use this workspace's `apps/native/dist/thread-7-ts-reader/index.js`.
That ignored local reader was generated from this checkout's `index.ts`,
`checkpoint.ts`, `fit.ts` and `workroom.ts` using Node 26's `stripTypeScriptTypes`,
then successfully imported. Node reported its experimental type-stripper warning;
no dependency install or source outside `apps/native/` was changed.

The required final `build --release` attempt exited **124** after 590 seconds
waiting for the shared lock (`thread-7-build.log`); it never compiled. No final-form
release-build pass is claimed. The final static check found all **69** registered
mutation originals present and no applied mutation; no new control was executed.

The final `test --release --workspace` attempt also exited **124** after 590
seconds waiting for the shared lock (`thread-7-workspace.log`), without compiling
or running a test. It used the workspace-local TypeScript reader above, but never
reached a reader check. There is **no final-form full-core or full-workspace pass**.
All six bounded filtered/build/workspace attempts ended before Cargo started.

No new section commit or coverage increase is claimed. The seven case ports and
their supporting source remain uncommitted drafts in this worktree, including the
clock refresh added after the completed native release run. The evidence notes
are committed separately. Next: acquire a usable build slot, run the three
assistant-text cases, check their three whole-core controls, and commit that
section first; then verify the four start-label cases, the original-turn cache,
both headless checks and their controls before committing that section. The
inherited twelve pending exchange/rail controls and a final release workspace
suite/build still need completion. Do not treat the initial 396/397 native result
as validation of the later edits.

## Thread-8 continuation — 14 Nov 2026 (core verification blocked)

The inherited assistant-text and start-label drafts remain uncommitted pending
verification. The start-label today's-time assertion now excludes any four-digit
year, matching Electron's assertion rather than excluding just 2026. Prepared
three whole-core start-label controls (day, year, original-turn cache) and two
headless window controls (display and ended-thread clock refresh). The fixture
runners use a 590-second outer Cargo bound, with restoration outside that bound.

Continued in file order past already-carried path/tool-name sections with five
changed-file-count cases (`changed_file_tests.rs`) and four route-switch typed
prompt cases (`typed_prompt_tests.rs`). Each uses Electron's exact values through
the existing native projection; no behavior change was needed. Three file-count
and two typed-prompt controls are prepared. These nine cases are drafts, not
verified ports. No additional does-not-apply exemptions or Scout edits.

The assistant-text, mission-start, headless-message, changed-file and typed-prompt
filtered attempts each exited 124 waiting for the shared lock, before Cargo
started. A combined core retry also exited 124 before compilation. None supplies
Rust test evidence. `git diff --check`, fixture Python syntax and checks that all
77 whole-core mutation original expressions are present passed. Recount completed
without table changes. The parity row stays partial at 149/264, 115 remaining;
the sixteen prepared Electron cases are not included. The inherited exchange/rail
control backlog remains incomplete. No source section has been committed yet.

Exactly one background release native suite was started; its final result and the
remaining verification results are recorded below after completion.

The background release run acquired the shared lock after 1,509 seconds and
compiled without reported warnings in 8m 38s. Its final summary (exit 101) is:

`test result: FAILED. 397 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 190.43s`

The sole failure is the handoff's owner-managed landing timeout:
`review_changes_shows_a_teammates_branch_turn_by_turn_and_lands_it_as_one_commit`,
`timed out waiting for the landing, read again: status "Ready"`. It was left
untouched and is not attributed to this work. Both final-form headless start-label
checks passed: cached label display and clock refresh without reopening an ended
ledger. This run does not execute the core crate's own tests, so it does not verify
the sixteen core case drafts. The separate core retry timed out as recorded below.

Final verification outcome: the post-release combined core retry also exited 124
waiting for the shared lock. The `assistant-append` whole-core control's inner
Cargo attempt exited 124 before compilation; the fixture correctly rejected that
as evidence (exit 1) and its `finally` restored the ledger and checked the diff.
No new mutation control completed. The other prepared controls, including the
two headless controls, were not run; the inherited exchange/rail backlog remains.

After restoration, one final combined core filter, `build --release --offline`,
and `test --release --offline --workspace` were each attempted under the required
590-second outer bound. **All three exited 124 before Cargo started**, with only
the shared-lock wait in their logs (`thread-8-core-final.log`, `thread-8-build.log`,
`thread-8-workspace.log`). No final full-core/workspace or application release-build
pass is claimed. The completed native release result above remains the last full
native suite result; it is not a full workspace result.

Reviewed the source diff and the added fixture/test files. Final checks confirm all
77 core and both window control originals are present, with no break left applied;
Python fixture syntax, recount and whitespace checks pass. The notes and parity
record are committed separately. All sixteen core case drafts and their supporting
source remain uncommitted and uncounted: **149/264 carried, 115 remaining**. No
handoff file is committed. Work stayed on `exec/native-thread`, based on `d099883b`,
under `apps/native/`; no Scout source, version, changelog, push, visible window or
real-agent run was touched.

Next: obtain a usable build slot, run the seventeen filtered core checks (sixteen
Electron cases plus original-turn cache), run the restored break controls, and
commit assistant text, start labels, changed-file counts and typed prompts one
section at a time. The start-label projection keeps native's existing English
short-date/24-hour convention and computes clocks outside drawing; the next nine
cases use existing folds rather than adding parallel implementations. No new
Electron-only exemption or durable architecture decision was introduced.


## Thread-9 continuation — 14 Nov 2026

One long release workspace run (`thread-9.log`, exit 0) acquired the shared lock
after 762 seconds and compiled in 6m 24s, with no reported warnings. Final results:
**1,669 core passed, 20 ignored; 398 headless window passed; zero failures; zero doc tests**.
The owner-managed landing test passed too. All sixteen inherited case drafts, the
original-turn cache test and both start-label headless checks passed by name.

Committed separately: assistant text (3), mission start labels (4), changed-file
counts (5), and typed prompts across route switches (4). Coverage is now 165/264,
99 remaining, still partial. No does-not-apply exemptions. Assistant/file-count/
prompt cases pin existing native behavior rather than adding another fold.
Start labels use the existing English short-date and 24-hour convention; the
original turn's date is cached while polling, and ended threads refresh on the
minute clock without disk reads while drawing. That is native's implementation
of Electron's date rule, rather than copying renderer-time parsing.

Prepared and registered eight next-section cases in file order: OpenCode's real
plan (2), a standalone plan (2), a peer-only answer (2), and completion-time tool
targets (2). Their existing native paths were inspected against Electron; no
production behavior change was needed. They are not counted until the second
combined release run finishes. Six corresponding break controls are prepared.
The core/window control fixtures now allow the same long shared-lock wait as the
handoff's workspace run instead of expiring in the queue after 590 seconds.
Controls are still individual, restore before exit, and reject a timeout or build
failure as evidence. All earlier control originals were checked before work.

The first run used the requested command verbatim (the wrapper's default external
TypeScript reader); the second explicitly selects this workspace's existing
`dist/thread-7-ts-reader/index.js`. No dependency install or reference-folder edit.
Final second-run, mutation and application-build results follow below.

The second combined release workspace run (`thread-9-next.log`, exit 0) waited
240 seconds, compiled in 3m 10s without reported warnings, and passed:
**1,677 core, 20 ignored; 398 headless window; zero failures; zero doc tests**.
All eight new cases passed by name in their final form. Each of their four
sections is now committed separately. Coverage is **173/264**, 91 remaining.
The next section in file order that is not already carried is **a runtime's
own helper** (2); later peer messages/reopening handoffs and the rest remain.
No additional does-not-apply exemption and no production change for these eight.

A sequential, individually restored control batch is now running, with a
one-hour budget checked only BETWEEN controls; it never interrupts a mutation.
It covers new section controls first, then the inherited exchange-reading/rail
backlog and the two headless start-label controls if the budget allows.

The `assistant-append` control was attempted first. Its named test and the
existing delta-semantics test failed, but a later relay test
(`relay_prose_return_posts_unanswered_prose_back_to_asker`) panicked while the
mutation was present; cleanup then hit a poisoned relay lock and aborted the
process (exit 250), so no full-suite result was produced. The fixture rejected
this incomplete run: **not counted as a completed control**. It restored the
source, confirmed byte-for-byte by `git diff --exit-code -- core/src src`.
The relay implementation and tests are outside this change and were left alone.
The remaining independent controls continue with restoration checked after each.

### Remaining case mapping after thread-9

These 91 cases are not counted as carried. Some share rules with existing native
tests, but that mapping has not been reviewed and no additional case is claimed.
No new Electron-only exemption was made. File order is preserved below.

| Remaining section | Cases |
| --- | ---: |
| a runtime's own helper | 2 |
| peer messages in the thread | 2 |
| reopening a handed-off mission | 4 |
| the running step | 1 |
| runtime notices in the thread | 4 |
| what a model alias resolved to | 3 |
| whether a finished run can be replied to | 3 |
| which words a turn shows | 4 |
| the routes this person has actually run | 2 |
| marking time in a long conversation | 3 |
| a run the host started for a teammate | 6 |
| what the decision card may say about the workspace | 4 |
| the waiting line’s clock | 2 |
| the host's disk observation of a path the runtime named | 4 |
| the model an alias turned out to mean (0.35.2) | 2 |
| Cursor reports files from inside its own copy of the project | 4 |
| an earlier turn says it changed nothing too | 2 |
| where a turn’s steps are drawn (0.491) | 1 |
| a turn sent with files attached | 4 |
| what a command printed | 5 |
| a shell row and its output | 3 |
| everything a teammate said in one turn | 4 |
| a plan does not claim nothing changed when something did | 2 |
| the group still growing at the end of a running turn (0.584) | 2 |
| a group the run has moved past (0.594) | 2 |
| a lone thought on its step line (0.594) | 1 |
| Cursor's scratch files (0.594) | 1 |
| a turn with no work still says what the runtime said (0.586) | 4 |
| the text a file row shows for a path (0.596) | 3 |
| a named file the host saw change but could not read (0.597) | 1 |
| the live line while nothing has arrived (0.602) | 2 |
| a Codex run, said as words | 1 |
| a tool the app has words for, as a row (0.610) | 1 |
| Locust’s git tool and OpenCode’s failed tool call, in words (0.818) | 2 |

### Completed control batch

Ten controls completed against the WHOLE core suite, each with its named assertion
failing, exit 101 and a final suite summary, followed by exact source restoration:
`assistant-replace`, `assistant-order`, `start-day`, `start-year`, `start-cache`,
`changed-count`, `changed-refused`, `changed-patch`, `typed-marker`, `typed-prior`.
Every restoration passed `git diff --exit-code -- core/src src` against the
committed positive baseline. `assistant-append` remains rejected as described above.

The budget expired between controls. Twenty were NOT run: `plan-content`,
`plan-state`, `plan-own`, `peer-only-reply`, `late-target`, `late-keep`, `peer-empty`,
`peer-flat`, `peer-snippet`, `rail-tool`, `rail-words`, `rail-order`, `rail-live`,
`rail-tone`, `rail-plan`, `rail-once`, `rail-helper`, `rail-cache`, and window
`draw`/`refresh`. The inherited exchange/rail backlog therefore stays 1/13.
Together with the rejected append run, 21 requested control verifications remain.
No incomplete run is counted as a successful control.

### Late-arriving plan ordering

Further review found an ordering gap not covered by the carried literals: native
kept a plan at its first arrival, while Electron's `buildThread` projects the plan
before the work and replies even if it arrived later. The earlier "no behavior
change needed" statement describes the eight exact case inputs only; it does not
hold for the function's full ordering rule. Added a separate native regression
check, `a_late_arriving_plan_precedes_the_turns_work_and_reply`, not an additional
Electron case and not added to the 173 count. Its pre-fix whole-core result and
the corrected projection's final release verification are recorded below.

The pre-fix whole-core ordering run (`thread-9-plan-order-red.log`) waited 256
seconds, compiled in 38.45s, then finished with exit 101: **1,677 passed, one
failed, 20 ignored**. The ONLY failure was the new named ordering check. This
establishes the missing behavior without accepting a launcher error or abort.

The fix moves the folded plan before the turn's other content, after any leading
person prompt, before qualification and drawing. It does not reorder or rewrite
events, timestamps or tool receipts, and performs no disk access. The rest of
the items keep their relative order. This deliberately follows the full
`buildThread` rule instead of reproducing native's arrival-order accident.
Final release suite/build results follow.

The final-form release workspace run (`thread-9-final-workspace.log`, exit 0)
waited 152 seconds, compiled in 1m 46s without reported warnings, and passed:
**1,678 core, 20 ignored; 398 headless window; zero failures; zero doc tests**.
The new ordering check passed by name; the prior whole-core run failed ONLY
that check before the fix. All carried case ports remained green. The extra
regression check is native integration coverage, not a 265th Electron case.
The projection fix and its check are committed as `135c9da6`.

### Source commits in this run

- `deccc368`: Carry the assistant text stream cases into native
- `d6bff282`: Show a cached mission start date in the native thread
- `34e41680`: Carry the changed file count cases into native
- `239e7d70`: Carry typed prompt cases across native route switches
- `8357e475`: Carry OpenCode plan payload cases into native
- `17462569`: Carry standalone plan cases into native
- `d14aa3cc`: Carry peer-only reply cases into native
- `70dfd755`: Carry completion-time tool target cases into native
- `135c9da6`: Put late-arriving plans before the native turns work

### Final release build and scope

The separate production `build --release --offline` (`thread-9-final-build.log`,
exit 0) waited 102 seconds and compiled in 1m 11s under the original 590-second
outer bound. **No warnings.** It ran after the final-form workspace suite, not
as a substitute for it. The final driver (`thread-9-final.log`) reports both
workspace and build exit 0.

`python3 scripts/recount.py` and `--check` passed; generated tables are unchanged
because missionView remains partial. `git diff --check` passed. Both fixture
scripts parse; all 83 core and both window control originals are present. After
the intentional ordering fix was committed, `git diff --exit-code -- core/src src`
confirmed no uncommitted runtime change or mutation remained.

Final scope: `exec/native-thread`, based on `e8a66f2a`; 24 additional Electron
case ports (173/264 total), plus original-turn cache, two headless start-label
checks and the late-plan ordering check. All source sections committed separately.
No new does-not-apply exemption. Native choices: use the existing ledger folds
instead of duplicating renderer plumbing, cache local clock words outside drawing,
and project the plan ahead of the work without rewriting evidence.

Still incomplete: the rejected append control and twenty unrun controls listed
above (including the inherited 12 exchange/rail controls); 91 Electron cases
listed in file order above. No real agent or paid turn, visible window, packaged
UI, other-platform run, push, version/changelog edit, Scout source edit, handoff
file commit, or reference-folder change. The only TypeScript-reader exception
was the handoff's verbatim first command; both subsequent full release runs used
this workspace's existing ignored reader, and no external source was edited.

## Thread-10 continuation — 14 Nov 2026

Scope: `exec/native-thread`, starting at `51da3024`. The starting tracked diff
was empty; the owner's untracked handoff files are excluded from every commit.
Only `apps/native/` changes; no Scout, release, dependency or credential work.

All six prepared controls completed, one at a time against the whole core
suite, each requiring its named assertion failure, exit 101 and a final suite
summary: `plan-content` (1,674 passed / 4 failed), `plan-state` (1,673 / 5),
`plan-own` (1,675 / 3), `peer-only-reply` (1,677 / 1), `late-target` (1,671 / 7),
`late-keep` (1,676 / 2). Each had 20 ignored. Each restored its source and was
followed by `git diff --exit-code -- core/src src`; the final diff was empty.
No timeout, compiler failure or aborted suite was accepted as evidence.

The first release workspace run (`thread-10-helpers.log`, exit 0) passed
1,680 core (20 ignored), 398 window, zero doc tests, without warnings. Both
helper case ports passed by name. The second (`thread-10-next.log`, exit 101)
finished the core suite: 1,686 passed, one failed, 20 ignored. The new reopened
handoff count test alone failed (expected 1, got 0); no window suite ran.
The JSONL decoder had no checkpoint-record branch, so real reopened receipts
never reached the existing checkpoint fold. The test's checkpoint fixture also
needed the ledger's schemaVersion. Both boundaries are corrected for the next
run. No pass is claimed for that test on the second run.

Prepared in file order: a runtime's own helper (2), peer messages in the thread
(2), reopening a handed-off mission (4), the running step (1), runtime notices
in the thread (4). Final baseline, controls, commits and counts follow below;
these drafts are not counted until their final-form named results are observed.

Implementation choices: reuse the native ledger fold and existing peer-card
projection, not Electron's renderer containers. Helper assignment words come
from one core function used by the actual row builder. Reply display uses the
existing share parser so a share is not repeated as unlabelled prose. Checkpoint
counts are projected while polling and carried on metadata; neither the seam
builder nor drawing accesses the disk. The running-step register already carries
thought versus action, so no redundant step enum was added. A finished turn with
no work promotes its runtime notices to readable rows; a provider retry says
"Trying the model again" rather than pretending the run is still starting.

The corrected release workspace run (`thread-10-all.log`, exit 0) passed
**1,691 core, 20 ignored; 400 headless window; zero failures; zero doc tests**,
with no warnings. All 13 new case ports passed by name, as did the extra helper
row-label and uncertain-handoff headless checks. Coverage is now **186/264**,
78 remaining, still partial. No does-not-apply exemptions. The five sections
above are carried; the remaining-case table from thread-9 still applies after
removing its first five rows. Next in file order: **what a model alias resolved
to** (3).

Specific behavior checks: `a_helper_the_runtime_never_named_is_said_to_be_unnamed_not_drawn_as_a_path`
pins the helper fallback used by the row; `hides_a_share_block_from_the_agent_bubble_keeping_the_prose`
pins the window's reply display function; `rebuilds_the_divider_from_the_route_switch_checkpoint_it_resumed_from`
reads real JSONL through Session, pinning decoding, receipt projection and seam
lookup together; `a_reopened_handoff_names_the_recorded_uncertain_actions`
checks the actual headless divider and a second, unselected checkpoint;
`says_a_notice_raised_before_any_work_once_the_turn_ends_with_no_fold_to_put_it_under`
pins ended-versus-running notice visibility; `shows_a_rate_limited_opencode_run_retrying_before_anything_has_run`
pins the runtime's retry message and live-line label/register. The running-step
case pins the already-existing label/register mapping, not new plumbing.

### New controls completed

All ten new core controls completed against the WHOLE core suite, each with the
named assertion failed, exit 101 and a final summary: `helper-unnamed`,
`helper-kind`, `peer-hide`, `peer-sort`, `handoff-checkpoint`, `handoff-fold`,
`handoff-decode`, `step-kind`, `notice-ended`, `notice-retry`. Nine finished with
1,690 passed / one failed / 20 ignored; `step-kind` also broke an existing
register check (1,689 passed / two failed / 20 ignored). Both window controls,
`helper-row` and `handoff-row`, completed with their named headless assertion
failing (one failed, 399 filtered). No timeout or build failure was accepted.
After each control the fixture wrote `git diff`; the batch compared SHA-256
hashes of EVERY Rust source file against the positive baseline. All matched.
No mutation remains. There were no surviving breaks needing an extra test.

### Section commits

- `dbcd37db`: runtime helper rows (2 case ports, extra row check, core and window controls).
- `59288ebf`: peer messages (2 case ports and controls).
- `ccca3ffd`: reopened handoffs (4 case ports, extra headless divider check and controls).
- `03d8ef4b`: running step (1 case port and control).
- `76c4ab07`: runtime notices (4 case ports and controls).

The parity row remains partial at 186/264. `recount.py` and `--check` passed;
the generated tables do not change for this partial row. Final restored release
suite/build results follow. The inherited backlog is still 15 verifications:
the rejected `assistant-append`, 12 exchange-reading/rail controls, and the two
older start-label controls (`draw`, `refresh`). The six prepared controls have
been cleared, not silently dropped. No real agent, visible window, packaged UI,
other-platform run, root TypeScript gate, network, release or push was performed.

### Final restored release verification

`thread-10-final-workspace.log` completed with exit 0 after all controls were
restored and all five source sections were committed: **1,691 core passed,
20 ignored; 400 headless window passed; zero failures; zero doc tests**.
All 13 final-form case ports and the two additional row checks were found by
name with `... ok` in this final log. No warnings or compiler errors.

The separate production `build --release --offline`, under the 590-second
outer bound through gentle-cargo (`thread-10-final-build.log`), completed with
exit 0 in 1m 05s, without warnings. No source changed between these runs or
after them. Final `recount.py --check`, fixture syntax/original checks, and
`git diff --check` passed. The tracked tree is clean after this notes commit;
the existing untracked handoff files remain untouched and uncommitted.

Final scope: 13 more Electron cases, 186/264 total, no does-not-apply additions.
**78 cases remain**, starting with the three model-alias cases. The 15 inherited
control verifications listed above also remain. The root TypeScript gate,
real agents, a visible window, packaging and other platforms were not run.
This is verified incremental parity, not completion of all missionView cases.

## Thread-11 continuation — 14 Nov 2026

Scope: `exec/native-thread`, starting at `1d24fee4`. All changes stay under
`apps/native/`; the owner's untracked handoff files remain excluded.

All 15 inherited controls are now verified individually and restored. Whole-core
controls: `assistant-append`, `peer-empty`, `peer-flat`, `peer-snippet`, `rail-tool`,
`rail-words`, `rail-order`, `rail-live`, `rail-tone`, `rail-plan`, `rail-once`,
`rail-helper`, `rail-cache`. Each finished with exit 101 and its named assertion
failure. Most had 1,690 passes / one failure / 20 ignores; append had 1,689 / two,
order 1,687 / four, once 1,688 / three. Headless `draw` and `refresh` each failed
its named assertion (one failed, 399 filtered). No surviving break needed a new
test. Every control restored and was followed by `git diff`; the inherited
product-source diff was checked empty after every run.

Two attempts were rejected: peer-empty aborted in the relay-roundtrip fixture,
and rail-cache aborted in the relay-prose fixture (cleanup poisoned a mutex).
Their complete retries are the accepted evidence. The first append attempt
used gentle-cargo's external default TypeScript reader before this was noticed;
its repeat used the existing workspace-local ignored reader at
`dist/thread-7-ts-reader/index.js`, as did all accepted subsequent runs. Fixtures
now explicitly select that reader and bound Cargo with outer `timeout 590` and
inner 570 seconds instead of the inherited 5,400/5,380 bounds. No script outside
`scripts/fixtures/` was changed.

### What a model alias resolved to — three case ports

`core/src/model_alias.rs` carries all three cases with the original model ids,
dates and map values. A pure projection learns only a different, nonempty runtime
model name; the newest mission wins independently of history iteration order.
History reading projects the name while polling, and the window caches the map
when its history job lands. The picker details and selected model chip use it;
drawing does not read the disk or start a process.

The native decoder previously discarded raw evidence before history could learn
anything from a start record. It now retains only the scrubbed `raw.model` as a
local display projection, then discards evidence and raw reasoning as before.
The completion's resolvedModel still takes precedence. This avoids keeping raw
evidence to support one display field. The new ledger integration check exercises
both completed and start-only JSONL histories. Its initial failure exposed missing
schemaVersion fields in the fixture and the decoder boundary; both were corrected.
An earlier build caught a wrong scrubber symbol; no failed compile is counted.

The full release workspace run `thread-11-alias-window.log` passed **1,695 core,
20 ignored; 401 headless window; zero failures, zero doc tests**, without warnings.
All three case ports, the ledger integration check and the headless picker/chip
check passed by name. The earlier baseline without that window check passed
1,695 core / 400 window. Five whole-core controls (`alias-start`, `alias-newest`,
`alias-same`, `alias-ledger`, `alias-decode`) each failed its named assertion and
restored; SHA-256 comparisons of every baseline Rust file matched after each.
Window-control results and section commit follow below.

The three window controls (`alias-cache`, `alias-row`, `alias-chip`) each completed
with its named headless assertion failed, exit 101 and one failed test. The cache
control failed the test's bounded wait assertion; none hit the Cargo timeout.
All baseline Rust hashes matched after each restoration. Coverage is **189/264**,
75 remaining, still partial; no does-not-apply exemptions. Next: reply sessions.

### Reply sessions — three case ports

The three named cases in `reply_session_tests.rs` use the original `thread-7`
and failure sentence. Native's existing session reader returned an empty string
as a session; it now rejects it. Terminal resume remains guarded by its existing
argument validation; no process starts during drawing. The shared release run
`thread-11-session-words.log` passed 1,702 core (20 ignored), 401 window, zero
failures/doc tests and no warnings. All seven new session/prompt cases passed
by name. The whole-core `session-empty` control failed its named assertion and
restored exactly. Coverage after this section: **192/264**, 72 remain.

### Which words a turn shows — four case ports

`turn_words_tests.rs` pins the exact prompts for a reply, a handoff, a reply
after a handoff, and a missing parent. All passed by name in the same complete
release run above. No product change was needed: the existing typed_prompt walk
already implements the rule. Whole-core `turn-followup`, `turn-orphan`, and the
existing `typed-prior` control each failed its named assertion and restored; all
Rust hashes matched. These controls also exercise the new cases without making
a second prompt walker. Coverage: **196/264**, 68 remain; no exemptions.

### Recent routes and time markers — five more case ports

`recent_routes.rs` and `recent_routes_tests.rs` carry both route-history cases
with the exact `codex:gpt-5` / `cursor:composer-2.5` order. The cached history keys
feed the real picker: recent models sort first within their runtime, and a
mixed-runtime Recent shortcut group reuses the existing turn-pick path. A
shortcut never creates a missing catalog model. Following the existing picker
specification, it is withheld for fewer than two available recents, lists of
8 or fewer models, or when it would duplicate half the list; at most four are
shown. Existing permission-policy filtering and per-turn account selection hold.
A native-only boundary test pins those limits. The headless test follows a
history refresh into the picker, checks newest-first placement, and presses a
Claude shortcut from a Codex conversation, checking the chosen runtime/model.

`thread_marker_case_tests.rs` carries all three time-marker cases using the
original 2026-08-31 UTC instants and `waited 40 min`. No product change was
needed: native already measures from the mission's first event and suppresses
gaps shorter than two minutes. Its before-item index is the native counterpart
of Electron's before-turn index; the fixture has one item per turn.

The release baseline `thread-11-routes-markers.log` passed **1,707 core,
20 ignored; 402 headless window; zero failures/doc tests**, without warnings.
All five ports and the new headless picker check passed by name. The extra
shortcut-boundary test was written after compilation began, so it is not
claimed present in that baseline; it passed by name in the subsequent whole-core
control run. Final positive-run evidence follows below.


## Thread-12 — 14 Nov 2026

Inherited baseline: commit `2794a17e` carries **201/264** cases (the row's old
196 count omitted the recent-route and marker ports). The initial inherited
`recent-latest` development control aborted in an unrelated relay fixture's
cleanup, exit 250; it restored its source and is not counted as evidence. Core
controls now use release, matching the required baseline. A release attempt
spent 298 seconds waiting for the shared lock and reached outer timeout 590
while compiling; no pass is claimed. Its completed retry,
`thread-12-baseline-retry.log`, passed **1,721 core, 20 ignored; 404 headless
window; zero failures/doc tests and no warnings**.

### A run the host started for a teammate — six cases

`relay_title_tests.rs` carries all six exact message/title cases. History refresh
uses the first received workroom message, flattened onto one line, and updates
both recent rows and cached summaries. Missing, empty and posted-only messages
never invent a title. The sidebar keeps its existing explicit rename precedence.
The headless history-refresh test passed by name, as did every new core case in
the baseline above. Native uses cached titles rather than reconstructing React
thread objects; the shared guarded prompt walk also exposes the relay title.
Mutation verification is pending below. No drawing performs file access.


### What the decision card may say about the workspace — four cases

`decision_standing.rs` carries all four cases, plus a boundary test distinguishing
failed-tool patch evidence from a started or merely named edit. The window now
uses the finished turn's recorded sandbox and completed/failed patch evidence,
not a mutable composer mode or an edit-like tool name. Only read-only guarantees
`nothing was changed`; missing evidence never establishes absence of writes.
All five core tests and the new headless check passed by name in the release
baseline above; the headless check changes the composer in both directions and
holds the decision sentence to the recorded sandbox. Controls are pending below.


### The waiting line's clock — two cases

`waiting_clock_tests.rs` pins both original timestamps: the explicit turn start
wins over later diagnostics, and a restored turn without one uses the first
event. Both passed by name in the release baseline. No product change was
needed: the existing live projection already uses this clock throughout.
Controls are pending below. Coverage is **213/264**, 51 remain, no exemptions.
Next in file order: the host's disk observation of a path the runtime named.
