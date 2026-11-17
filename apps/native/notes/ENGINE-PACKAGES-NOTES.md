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
