# The engine packages' tests, carried into Rust (16-17 Nov 2026)

Electron's tests under `packages/runtime-adapters`, `packages/mission-store` and `apps/desktop/src/shared`, carried
case by case into Rust tests with Electron's own values and words. One Electron case is one Rust test.

## The answer key
`~/work/locust` at 4ce2f37a, not this worktree's own `packages/` copy, which is older: two of the files waiting here
(`a-large-cursor-result-keeps-its-terminal-status`, `claude-thinking-is-kept-for-its-thinking-row`) exist only in
`~/work/locust`. Of the 74 files that exist in both, only `schema-compat.test.ts` differs between the two; the sources
that differ are `claude-events.ts`, `commands.ts`, `opencode-serve-run.ts`, `process-runner.ts` (plus the new
`cursor-result-stream.ts`), `mission-store/src/index.ts` and five `shared/` files.

## How the breaks were run
`~/work/scratch/maxlane/mutfilter.py . <entries> locust-core 300 '<filter>'`, in the background, one break at a time,
with only the batch's tests run. A break counts only when one of the batch's named tests fails. With three lanes on the
one build lock a break took 2 to 6 minutes, mostly waiting for the lock.

## Batch 1: six runtime-adapters files (commit below)
Carried whole (21 tests):
- `copilots-search-index-record-is-not-news` (1) and `copilots-new-records-are-not-news` (2):
  `runtime/copilot_quiet_records_tests.rs`.
- `a-denied-read-and-a-file-row-are-said-as-they-are` (4): `runtime/denied_read_and_file_row_tests.rs`. The Codex file
  row is checked through the app-server reader (`events::Normalizer::accept`), where Rust builds it inline.
- `an-oversized-cursor-edit-keeps-its-counts` (3, plus 2 of Rust's own): `runtime/oversized_cursor_edit_tests.rs`.
  Rust has no runner cap to set to 1 KB as Electron's test does, so the stand-in is built directly
  (`cursor::oversized_stand_in`).
- `copilot-effort-levels` (2): `copilot_effort_levels_tests.rs`.
- `message-tail` (7): `runtime/message_tail_tests.rs`. `events::text(value, 16_384)` is `boundedMessageText` and
  `text(value, 131_072)` is `boundedAnswerText`.

Bugs fixed in `runtime/cursor.rs` (the stand-in for an oversized Cursor record):
- **The call id.** Rust's pattern refused any id with a backslash in it, so an id holding an escaped line break
  (`call-big-0\nfc_big_0`, as Cursor sends it) was never found: the edit stayed open and the files card lost its
  `+1 −1`. It is Electron's `callIdsIn` now: the same keys (`tool_use_id`, `callId` too), the same characters, the
  escapes read, white space as `|`, and each id once.
- **The edit's counts.** Rust took `added`/`removed` keys as well as `linesAdded`/`linesRemoved`, from any record, and
  kept one count without the other. It is Electron's `editReportIn` now: only an `editToolCall`, only with both
  counts, and the diff read as the JSON string it is (Rust undid three escapes by hand, in an order that turned
  `\\n` into a line break) and left out when empty or past 16,384 pieces. Rust's regex cannot hold Electron's
  `{0,16384}` bound (the pattern is too large to compile), so the pieces are counted by hand.
- **A head cut inside a character** panicked the reader: the first 4 KB and 64 KB were cut by bytes. They are cut
  back to a whole character now (Rust's own test).

Breaks: 22 run. 19 caught by name. One timed out waiting for the lock (an empty background notice said as news),
rerun with batch 2. Two survived:
- dropping the dedup of call ids: no Electron case checks it (the normalizer closes the call on the first id). Rust's
  own test `an_oversized_edits_stand_in_names_its_call_once` now does.
- making `choices:` required in the effort pattern: both Electron cases happen to write `choices:`. Left as it is.

## Batch 2: five runtime-adapters files and five shared files (67514082, cb6c8fcf, then the break checks' commit)
Written by the first chat on this brief, which stopped at 22:47 on 16 Nov; the Mac chat committed it. The break checks
were finished on 17 Nov by the next chat.

Carried whole:
- `a-backgrounded-call-says-so` (4) and `background-work-ends-with-the-run` (7): `runtime/claude_background_calls_tests.rs`.
- `a-config-warning-says-its-summary` (6): `runtime/codex_config_warning_tests.rs`.
- `a-helpers-calls-are-its-rows-children` (3): `runtime/claude_helper_children_tests.rs`.
- `a-large-cursor-result-keeps-its-terminal-status` (1): `runtime/large_cursor_result_tests.rs`.
- `a-block-in-code-is-an-example`: `block_in_code_tests.rs`. `a-point-can-count-twice` (4) and `points-are-drafted` (3):
  `compare_points_tests.rs`. `the-example-file-is-never-a-file` (1): `example_file_tests.rs`.
  `which-notes-bear-on-a-question` (4): `notes_for_a_question_tests.rs`.

Breaks: 18 on the runtime readers (16 Nov, `~/work/scratch/opus2/b2-breaks.log`), then 19 and one more on 17 Nov
(`b2b-breaks.log`). Five survived at first; each now has a Rust test of its own that catches it by name:
- the background flag on the restated `tool.started` (Electron checks it only on the finished event):
  `a_backgrounded_call_says_so_when_its_input_is_filled_in`;
- a helper's call that streams its own start (Electron: "measured not to happen"):
  `a_helpers_call_that_streams_its_start_is_still_the_helpers`;
- a `\u` escape across the 64 Ki cut in Cursor's result: `a_unicode_escape_at_the_cut_is_dropped_whole`;
- the driver's reader passing on a short last line with no line end:
  `a_short_last_cursor_line_without_a_line_end_still_reaches_the_reader`;
- the newer-first tie-break among notes a question lifts alike: `notes_the_question_lifts_alike_come_newest_first`.
One break was left as it is: the `parentItemId` on a restated `tool.started`. A helper's whole record opens its calls
and returns before the restating loop, so it is reached only by a helper's call that streamed its start and whose whole
record then came without the helper's id, which no recording shows (Electron is built the same way).

## Batch 3: six shared files (17 Nov)
The first chat's five drafts, finished. Carried whole (39 tests):
- `approval-data` (8, plus 2 of the card's rows): `approval_data_tests.rs`.
- `a-question-that-is-not-a-card-stays-in-the-reply` (4, the it.each's shapes a test each: 7): `question_not_a_card_tests.rs`.
- `claim-what-you-start` (4) and `the-named-teammate-claims` (8, plus 2 of Rust's own): `room_task_claims_tests.rs`.
- `step-budget` (5): `step_budget_tests.rs`. The budget's bound is a compile-time assertion, so its break cannot be
  run (a break that does not build proves nothing).
- `a-torn-tag-keeps-the-letters-before-it` (1): `torn_tag_tests.rs`.

Bug fixed in `approval.rs` (the approval card's Effects rows): Electron's `dataSentLine` and `commandReachesNetwork`
were inline in `effect_lines`, and two cases came out differently. A command that names no network tool, on a card
whose route gave its own undo words, said "This command can reach the network" (Electron: "Unknown. Locust cannot
tell what a command sends…"). A question card was given a connector's Data sent and Reversible rows (Electron: no Data
sent row, and "Nothing is changed by answering."). Both are `approval::data_sent_line` and
`approval::command_reaches_network` now, as Electron has them, and the card reads its rows through them.

Breaks: 35 (`~/work/scratch/opus2/b3-breaks.log`). 33 caught by name at first; the two that survived (a post naming
two rows; the whole post read when the row's sentence names nobody) are reached by no Electron case, and each has a
Rust test of its own now that catches it.

## Batch 4: six runtime-adapters files and one mission-store file (17 Nov)
Carried whole:
- `effort-choices` (5, plus 1 of Rust's own): `effort_choices_tests.rs`.
- `connector-allow-rules` (6, plus 1) and `the-browser-shares-one-flag-with-the-bridge` (4):
  `runtime/claude_command_connectors_tests.rs`. Rust runs Codex through its app server, so "ignored by every runtime
  that has no such flag" is checked on `codex::app_server_args`.
- `every-runtime-can-be-written-down` (1, plus 1): `every_runtime_ledger_tests.rs`, for the nine runtimes Rust offers.
  Electron's list also has Gemini; Rust has no Gemini runtime and its ledger refuses one.

Partial:
- `a-run-without-connectors-leaves-them-out`: its "where nothing was said" half has no Rust form
  (`PrintOptions.account_connectors` is a yes or no, and no is the default).
- `a-side-question-forks-the-session`: Claude Code's cases only. Rust has no `opencode run` command line; it forks an
  OpenCode session through the served session's `/session/{id}/fork`.
- `a-sealed-run-uses-no-tool`: the config and environment `configure_with_seal` writes are checked; the served run's
  whole command needs the `opencode` binary (its version is probed first), and Rust gives OpenCode no `locust_git`.

No Rust code changed; no bug found. Breaks: 22, then 3 reruns (`b4-breaks.log`). Three survived at first: the
dot-all flag in the effort pattern (caught by batch 1's `copilots_effort_levels_are_read_from_its_1_0_88_help` once
it was in the run), an empty level, and the run-collapsing of a `claude.ai` connector name; the last two have Rust
tests of their own now.

## Batch 5: an OpenCode 2 step, and notes on six files Rust has not built (17 Nov)
Carried whole: `an-opencode-2-step-says-what-it-did` (6, plus 1 of Rust's own): `runtime/opencode_2_step_tests.rs`.
Copilot's first `tool_call` from `fixtures/acp/copilot-1.0.88-bash-rejected.jsonl` is written into the test, as the
fixture is not in `parity/events.json`.

Breaks: 8, then 7 reruns (the first run's 60 s limit timed out waiting for the lock; a timeout is not a catch), then
1 (`b5-breaks.log`, `b5b-breaks.log`). Caught by name but one:
- dropping `title != command` (a title that only repeats the command is not kept) survived. No Electron case reaches it.
  Round 2's `a_title_the_later_command_repeats_is_left_out_of_the_finished_row` catches it now (the rule moved into
  `OpenTool::row`).

Found here, fixed in round 2 (below): Rust's ACP reader renames an open call more widely than
Electron's (`acp-events.ts` ~line 403). Electron renames a call still named `other`/`tool` only when the update's own
kind is absent or `other`, reads the earlier title when the update has none, and never renames to `other` or `tool`.
Rust (`acp_events.rs`, "if matches!(open["name"] ... Some("other" | "tool"))") renames on any update, to whatever
`naming` gives: an update with no title renames such a call to `tool`, and an update of kind `read` renames it even
though Electron would not. Electron's later-title rule differs too: it replaces a title only while the title is still a
bare tool word (`plainAcpToolWord`). Each wants a Rust test with Electron's values, then the fix.

Left waiting, with a note in each row of what is missing: `qa-0365-opencode-worktree` and `a-refusal-is-not-a-crash`
(both read an `opencode run` process's stderr; Rust serves OpenCode 1), `the-parent-git-is-reachable-on-every-platform`
(no OpenCode worktree config), `a-self-updated-cli-is-asked-again` (no CLI facts cache),
`a-result-too-large-to-keep-still-answered` (Rust's oversized stand-in is Cursor's only),
`claude-thinking-is-kept-for-its-thinking-row` (Rust's Claude reader drops thinking text).

# Round 2 (17 Nov, from HANDOFF-max-engine-packages-2)

## Batch 6: the ACP rename bug batch 5 found (commit below)
`runtime/acp_events.rs`'s `tool_call` / `tool_call_update` branch is Electron's now (`acp-events.ts` at 4ce2f37a,
~line 364-408), written as `shown_tool` (`acpShownTool`), `plain_tool_word` (`plainAcpToolWord`), `is_shell`
(`acpCallIsShell`), `file_target` (`fileTarget`) and an `OpenTool` that holds the raw command and title as Electron's
`openTools` does, bounded only when a row is written. What was different:
- an open call named `other`/`tool` was renamed on ANY update, to whatever its kind gave: an update of kind `read`
  renamed it, and an update with no kind or title renamed `other` to `tool` (and `tool` to `other`). Electron renames
  only when the update's kind is absent or `other`, reads the earlier title when the update has none, and never
  renames to `other` or `tool`;
- any later title replaced the earlier one, and a call opened with no title took the next one. Electron replaces a
  title only while it is still a bare tool word;
- a title was compared with the command only when it was stored, so a title the later command repeated stayed on the
  finished row (Electron leaves it out);
- a shell word in the title (`bash`, `execute`, ... with kind `other` or none, the update's or else the earlier one)
  did not make the call a shell, so its folder in `locations` was taken for its command.

Tests: `runtime/acp_rename_tests.rs`, 12, each with Electron's values; the first 8 were written first and all 8 failed
on the old code. No Electron test file covers these rules (none of `acp-events.test.ts`'s cases renames or retitles),
so no row moves; the values come from the source.

Breaks: 13, then 4 reruns (`~/work/scratch/opus2/r2-acp-breaks*.log`). One run was cut by the 10-minute command limit
and its restore failed (`killpg` refused): break 6 was left applied and put back by hand; the rest ran in the
background. Four survived at first (a title equal to the name, the earlier title's shell word, `execute` as a shell
word, the shell-name check; the last had timed out on the lock): each has a test now that catches it by name. One left:
`Some(t) != open.title` in the retitle rule, which only stops a title being replaced by itself (no row can differ).

## Batch 7: mission-store's record files and the checkpoint (commit below)
Rust's counterpart of Electron's `getMission` is `mission_export::recover_mission_from_path` (it says it matches
`readLedgerFile`'s shape); the writers are `ledger_write`'s; Electron's `reconcileMission` is `carry_on::reconcile`.

Partial (each row names what is not built):
- `a-check-result-is-kept-with-its-turn` (5 of 6), `a-card-answered-is-kept-with-its-turn` (4 of 9),
  `a-helpers-calls-are-kept-as-its-own` (2 of 3): `mission_store_records_tests.rs`, plus 3 of Rust's own.
- `checkpoint` (7 of 12, one in half): `checkpoint_reconcile_tests.rs`.
Does not apply: `the-home-list-bounds-its-reads` (Node promise pools; Rust lists one file at a time).
Left waiting, with a note: `a-long-field-is-kept-shortened` (no ledger-side fit), `a-busy-file-does-not-end-a-run`
(no retry of a busy open), `unreadable-count` (the list has no such count), `the-storage-report-reads-only-what-changed`
(no per-file cache).

Bugs fixed in `ledger_write.rs`: the writers wrote what the reader refuses. `append_edit_check` wrote an outcome
nobody defined and any number of lines (`ledger::decode` then dropped the record); `append_approval` wrote any answer
and any answerer. They now run mission-store's `validateEditCheck` and `validateApproval` first
(`ledger_write::validate_edit_check`, `validate_approval`, with `requireText` and `requireTimestamp`), and
`bounded_edit_check` is `boundedEditCheck` (60 lines of 500, the command 500: the record's bound, wider than the
checker's own 30 by 300). A timestamp is read as RFC 3339, narrower than JavaScript's `Date.parse`.

Found, not fixed (the readers are in `ledger.rs` and `mission_export.rs`, which the maxlane lane is changing; they
belong with `schema-compat`): `recover_mission_from_path` reads every record leniently, so a check result in a v16 file,
a card in a v19 file and an earlier Always in a v20 file come back as kept, where Electron reads each as damage
(`invalid-record`). `ledger::decode` checks a checkpoint's field types only: it keeps one for another mission or run,
one that says `safe` while listing an open action, a malformed digest, epoch 0, an unknown reason or safety, and an epoch
out of order, each of which Electron's `parsedCheckpoint` refuses.

Breaks: 19 on the writers, then 9 on `reconcile` and 1 rerun (`~/work/scratch/opus2/b7-breaks.log`, `b7b-breaks.log`).
All caught by name but the command's cut in `bounded_edit_check`, which no Electron case reaches; Rust's
`a_check_results_long_command_is_cut_to_what_the_reader_accepts` catches it now. The on-disk checkpoint tests read the
file the way `native::turn_records` does, copied into the test (it finds its file through the app's store), so a break in
`turn_records` itself would not be caught by them.
