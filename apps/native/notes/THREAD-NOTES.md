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

`scripts/fixtures/thread-mutation.py NAME` changes one expression or constant, runs the entire core suite through gentle-cargo under `timeout 590`, requires the named test to fail by assertion, restores the source in `finally`, captures the restored `git diff` in that control's log, and runs `git diff --check`. Controls use `--target-dir target/thread-controls`, keeping the wrapper's shared lock but isolating Cargo artifacts from neighboring worktrees. The first attempt was rejected: a cached neighboring core binary ran 1,536 passing tests without the new tests, so it could not qualify as mutation evidence. Logs are local ignored files `notes/thread-mutation-NAME.log`.

Verification results are recorded below after the runs finish. Five initial isolated-target controls completed in the ordinary debug profile. The host-prompt attempt was rejected because the existing `relay_prose_return_posts_unanswered_prose_back_to_asker` fixture hit its deadline, poisoned a mutex during cleanup, and aborted the harness before the intended assertion ran. Remaining controls use dev opt-level 1 with debug symbols off and four harness threads to bound fixture contention, after a clean core baseline at that profile; the required final workspace run still uses the ordinary release profile.

The initial full core run caught the silent-turn message being appended after the edit-check receipt. It now precedes that receipt; `edit_check::tests::a_reopened_conversation_keeps_its_check` proves the ordering. A later core run passed 1,567 tests (20 ignored). The first complete headless UI run exposed an old group-summary expectation and two presses outside the viewport after the thread grew. The summary expectation now uses Electron's words; the project-join and earlier-version checks explicitly scroll their existing controls into view before pressing, retaining their assertions. These test adjustments change no product navigation behavior.

## Left to carry

196 missionView cases remain, including the full activity change/diff card, live-line registers/connectors/plan aside, finished trace totals, conversation stitching, cancellation-summary case ports (native already has its own implementation), path identity/no-change claims, ended questions, peer exchanges, signal rail, and all other sections not named in the parity row. Existing implementations of some of these do not constitute carried Electron tests.

This batch does not claim full buildThread parity: native still uses its ledger items and adjacent activity groups. It does not add Electron's full step-group lifetime rules, turn footer/diffs, notice folding, or the remaining live-register details. The new file-name rows apply to native's existing no-diff representation; diff parsing remains in the later change-card work.

No real-agent run, visible app launch, screenshot, live approval interaction, or packaged app check was performed. Native's release workspace suite is the scoped gate requested by the handoff; the repository-wide Electron gate is outside this native-only task.
