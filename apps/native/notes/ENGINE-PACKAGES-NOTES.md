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
Left waiting: `the-home-list-bounds-its-reads` (its pool half is Node's; its order half applies and is not carried).
Left waiting, with a note: `a-long-field-is-kept-shortened` (no ledger-side fit), `a-busy-file-does-not-end-a-run`
(no retry of a busy open), `unreadable-count` (the list has no such count), `the-storage-report-reads-only-what-changed`
(no per-file cache).

Bugs fixed in `ledger_write.rs`: the writers wrote what the reader refuses. `append_edit_check` wrote an outcome
nobody defined and any number of lines (`ledger::decode` then dropped the record); `append_approval` wrote any answer
and any answerer. They now run mission-store's `validateEditCheck` and `validateApproval` first
(`ledger_write::validate_edit_check`, `validate_approval`, with `requireText` and `requireTimestamp`), and
`bounded_edit_check` is `boundedEditCheck` (60 lines of 500, the command 500: the record's bound, wider than the
checker's own 30 by 300). Not wired: Electron's host bounds before it appends (`codex-mission.ts:1454`), Rust's
(`native.rs:2711`) passes the record straight in, so a command over 500 is now refused there rather than cut. `native.rs`
is the maxlane lane's; it wants `bounded_edit_check(&rec)` at that call. A timestamp is read as RFC 3339, narrower than JavaScript's `Date.parse`.

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

## Batch 8: mission-store.test.ts (commit below)
Partial, 9 of 15 (four in half): `mission_store_ledger_tests.rs`, on `LedgerWriter`, `recover_mission_from_path`,
`try_list_recent` and `trash::delete_mission`. The row names the six not built.

Fixed in `ledger_write.rs`: the words of a refused append. Rust said "Ledger changed outside the active writer" and then
"Ledger write previously failed; refusing further appends"; Electron says "Mission ledger changed outside the active
writer" and then "Mission ledger is unavailable". Nothing in Rust matched on the old words.

Found, not fixed (`ledger.rs`, the maxlane lane's): `try_list_recent` orders by the file's modified time, Electron by
the last time the ledger records. They differ only when the times recorded are not the order of the writes (the
Electron case writes createdAt 10:00, 12:00, 11:00 in that order).

Breaks: 11 (`~/work/scratch/opus2/b8-breaks.log`), on the writer's refusals and words, `trash::delete_mission`, the
reader's version range, the list's order and `recover_mission_from_path`'s tail, phase and damage: all caught by name.

## Batch 9: schema-compat, and the shared files whose note was empty (commit below)
Partial: `schema-compat` (13 of 52, one in half): `schema_compat_tests.rs`. Rust's readers check types, not versions,
so the 30-odd cases that refuse a hand-edited older file, or a field `createMission` would not write, are not built
(the row lists them).

Bug fixed in `ledger_write.rs`: `append_peer_links` wrote a peer link into a file of any version, and any link. It is
mission-store's `appendPeerLinks` now: each link checked first (`validatePeerLink`: the direction, a safe message and
teammate id, a timestamp), then "Mission ledger version cannot hold peer links" before version 5.

Shared files:
- carried: `the-about-you-line-is-taught-once` (2, on `people::brief`; `shared_rules_tests.rs`) and
  `a-run-started-for-a-teammate-is-theirs` (5; `route_at_start_tests.rs`, written in batch 2, its row never set until
  now, break checked here);
- partial: `a-first-run-names-no-terminal` and `kilo-installs-in-one-click` (1 of 2 each; the other case reads
  Electron's renderer source);
- waiting, with a note: `messageFragments` (Rust does not repair a recorded restatement), `a-teammate-is-named...`
  (no chat-app bridge), `a-lesson-reads-as-one-sentence` (a Rust lesson is one line, no when/do pair),
  `a-voice-call-switches-every-add-on-off` (no voice call), the three `an-answer-reads-right-in-*` (no chat-app
  formatter), `commit-and-land-from-a-browser` and `settings-from-a-browser` (the phone desk, networked).

Round 1's deferred runtime rows, re-read: `a-busy-model-served-says-so` and `a-message-taken-and-never-begun-ends-the-run`
are built in Rust (`opencode_serve.rs` turns `session.status` retry into `locust.retry`, which `opencode_events.rs` says
once; `recover` asks the server about a silent session), but carrying them needs new scenarios in the fake server
`scripts/fixtures/m4-opencode-serve.py`, and that file, `opencode_serve.rs` and `opencode_tests.rs` are the maxlane
lane's (9 of its commits this week). Left for when that lane is done. `a-rate-limited-opencode-run-says-it-is-retrying`
reads `opencode run`'s stderr, which Rust does not run (it serves OpenCode). The other deferred rows (the binary consent cache, the
probe scheduler and its progress, a Windows cmd shim, context inspection, user-provider pricing, per-run skills, Cursor
connectors, OpenCode's question card, Node's process runner, reference folders on a turn) were not re-checked in this
round; their notes stand as round 1 wrote them.

Breaks: 18, then 2 reruns (`~/work/scratch/opus2/b9-breaks.log`, `b9b-`, `b9c-`): on `append_peer_links`, `append_note`'s
version, `ledger::metadata` and `decode`, `route_at_start::start_as`, `people`'s memory block and `agents`' install
facts. All caught by name but two at first, the teammate id and the timestamp of a peer link, which no Electron case
reaches; `a_peer_link_with_an_unsafe_teammate_id_or_a_time_that_is_not_one_is_refused` catches both now.

## After round 2: a change the batch 7 validators made, for Colin to decide
`approval.rs:989` records a command the hard command guard refused before any card as an approval with
`by: "command-guard"`. Electron's mission-store knows no such answerer (`card`, `card-saving-a-rule`, `saved-rule`,
`earlier-always`), and Electron records no approval for its guard (the guard is a hook, not a card). Since batch 7,
`validate_approval` refuses that record, and `native.rs:2983` drops the error (`let _ =`), so a guard refusal is no longer
written to the ledger (the refusal itself still happens; `approval.rs`'s own test reads the channel, not the ledger, and
passes). Before batch 7 Rust wrote it, and Electron's reader would have stopped reading that file at it. Either keep it
as Electron (no record of a guard refusal) or allow `command-guard` as Rust's own answerer (kept in Rust's saved record,
a file Electron's reader stops at). Not changed until decided.

Decided (Colin, 17 Nov): kept as Electron. A guard refusal leaves no line in the conversation's record, and every file
stays readable by Electron. `a_card_answer_the_reader_would_refuse_in_any_field_is_never_written` now holds that
`command-guard` is refused. Still to tidy, in `approval.rs` (the maxlane lane's): it need not make the record at all
(`record.by = "command-guard"` at :989, and its test at :1794 reads it from the channel), so `native.rs:2983` stops
trying a write that is always refused.

Checked at the end: the peer-link callers (`native.rs:1656`, `:2666`) pass workroom message ids and `posted_at` times
the checks accept; every approval's `askedAt` is `now_iso()` (RFC 3339) except `engine_process.rs:264`, which takes the
card's own `requestedAt` and defaults to empty, which `validate_approval` refuses. The old writer messages appear nowhere
else in `src`, `scripts` or `core`.

# Round 3: window tests (17-18 Nov, from HANDOFF-max-renderer-3)

The 273 waiting files under `apps/desktop/src/renderer/` outside Sol's (teammates and the conversation thread). The
answer key is `~/work/locust` (4ce2f37a). Many of these are about the window's drawing, and the drawing of the
composer, the picker and the thread lives in Sol's files (`src/ui/thread_view.rs`, `messages.rs`, `people_panel.rs`,
`core/src/teammate_*.rs`); where the rule is there, the row says so and is left for Sol.

A quick survey (each file's imports against Rust's function names, `~/work/scratch/opus2/r3-files.txt`): about 50 of
the 273 import something Rust has by name. Not built at all, so left waiting: the effort scale and its descriptions
(`effortScale`, `effortLevels`), `cost.ts` (`benchmark::cost_line` is Compare's record line, with other words: "$0",
"$0.0010" where `cost.ts` says "$0.00", "< $0.01"), and the catalog spellings (`rememberOwnModels`).

## Batch 1 (window-1): how a model is named
Rust's `teammate_tooltip::route_model_name` is Electron's `routeModelName`: a Claude alias or id read through
`claudeModelName`, every other runtime `modelDisplayName` with no catalog read. Rust has no separate
`modelDisplayName`, so the one Claude id the Electron case spells (`claude-haiku-4-5-20251001`, "Haiku 4.5 20251001")
is read on the hover (`teammate_tooltip`), which spells a Claude id without the alias table, as `modelDisplayName`
does. `route_model_name` says "Haiku 4.5" there, as Electron's `routeModelName` does: no bug.

Carried: `routeName` (10): `route_name_tests.rs`.
Partial: `a-model-has-one-name` (1 of 5), `a-kilo-model-has-one-name` (1 of 3), `your-own-model-is-named-as-yours`
(1 of 3). The catalog cases are not counted: they pass only because Rust has no catalog spelling to take.
Left waiting, for Sol: `a-picker-row-is-named-not-an-id` and `the-picker-reads-as-names`. Rust's picker
(`thread_view.rs` `model_picker`) draws the catalog's name raw (`m.name`). Electron's `RoutePicker` spells it with
`modelDisplayName` when the name is the id, when the id ends in `/<name>` (OpenCode's `ling-3.0-flash-fin-free`), or
when it is `account-default` ("Account Default", where `models.rs` writes "Account default"); a name the runtime gave
is kept. The rule belongs at the picker, not in `engine::parse_models` (Electron keeps the catalog's name and
decides at the picker; `engine.rs`'s own test holds `beam-free`).

No Rust code changed. Breaks: 16 on `teammate_tooltip.rs` and `own_models.rs` (`~/work/scratch/opus2/r3b1-breaks.log`), then
1 rerun (`r3b1r-`; the first timed out waiting for the lock): all caught by name.

## Batch 2 (window-2): the free model a new person starts on
Partial, 4 files (`free_model_order_tests.rs`, 10 tests): `a-new-person-starts-on-the-steadiest-free-model`,
`a-free-model-that-is-down-is-passed-over`, `a-free-model-that-answered-comes-first`, `a-retired-free-model-is-not-offered`.
Rust has `connectors_off::free_start_model` and `free_models_best_first`, which match Electron's `freeStartModel` and
`freeModelsBestFirst` on every value these files give. It has none of what feeds them in Electron: `downFreeModels`
(six hours after "Model is unavailable"), `freeModelsThatAnswered`, `retiredFreeModels` / `modelRetired`
(`KNOWN_RETIRED_MODELS` and "has been deprecated"), the catalogue-aware `nextFreeModel` (Scout's
`next_free_model` walks `FREE_MODELS_BEST_FIRST` only, with no catalogue, no down set and no wrap round), and
Compare's `defaultComparePicks`. Where a case passes one of those into `freeStartModel`, its result is written into
the Rust test, and the case counts as half.

One difference in form, not a bug: Electron marks a model of your own `own: true`; Rust has no such flag on
`engine::Model`, and knows one by its `own-<id>/` route, so "never a model of the person's own, even one named -free"
is written with `own-a1b2c3d0/mine-free`.

No Rust code changed. Breaks: 10 on `connectors_off.rs` (`r3b2-breaks.log`): the named order, the unnamed after it and
its tie-break, the own and -free filters, the down set, the answered list and its listed check, the fall-back when
all are down, the first of those up. All caught by name.

## Batch 3 (window-3): a restored queue waits for Send
Partial: `a-restored-queue-waits-for-send` (7 of 16, two in half): `src/composer/restored_queue_tests.rs`, on
`composer::queue` (Electron's `steering.ts`) over rows written and read back through `queue_store` (a real file under
a store of the test's own). A row read back from the file is `restored`, as Electron's is after a relaunch. Electron's
`createConversationQueue` (serialized writes, the first read awaited, Send that removes its row before it dispatches,
one Send at a time, a refused Send held) has no Rust counterpart outside the window: `src/ui/composer_wiring.rs`
(Sol's) reads and writes the file around its own `State`. Rust has no "not sent — press Send to try again" words.
The unreadable-file case is Rust's form of Electron's: Electron's queue refuses the update and never writes; Rust's
`write_queue` refuses to replace a file it cannot read.

No Rust code changed. Breaks: 11 on `queue.rs` and `queue_store.rs` (`r3b3-breaks.log`): the restored and held verdicts,
a row read back as restored, Discard's and Send's removal, the rekey, the restored guard on folding, `queued_in`'s key,
the refusal to replace an unreadable file, the person-only filter and the saved attachments. All caught by name.

## Batch 4 (window-4): "Wren finished"
Partial: `finishedToast` (5 of 6, one in half): `core/src/finished_toast_tests.rs`. Rust's `attention::finished_toast`
takes what Electron's `EndedRun` holds as plain values; Electron's `lastSaid` is `carry_on::latest_final` plus the
first-line pick in `finished_toast`. Found, left as they are (no Electron case reaches either; for Colin or the
lane that owns `attention.rs`): Rust's `clip` folds every run of white space into one space, where Electron's
`lastSaid` keeps the line as written; Electron passes over a final message with no words for an earlier one, where
Rust's `latest_final` keeps the last final message and its toast then says "Done.". Electron also stays quiet when
the run ends on an open question (`openQuestion`); neither Rust's toast nor its caller (`attention_watch.rs`) checks for one. No case in this file reaches it.

No Rust code changed. Breaks: 11 on `attention.rs` and `carry_on.rs` (`r3b4-breaks.log`): the minute, who started it,
the ended phases, the stop's title, its first line and "after", the first line said and its bound, "Done.", hours, and
the final-only read. All caught by name.

## Batch 5 (window-5): the rail's card, Claude's version names, the route after a handoff
Carried: `railFlyout` (7): `src/ui/rail_flyout_tests.rs`, on `rail.rs`'s own helpers (its own test used other values).
Partial: `a-claude-route-names-its-version` (2 of 9): `claude_route_name_tests.rs`, on `models::claude_model_name`;
`a-handoff-keeps-the-route-handed-to` (1 of 2): `handoff_route_tests.rs`, on `route_at_start::composer_route_for` (the
other case reads Electron's `App.tsx`).

**A difference, for Sol (the code is in `core/src/teammate_tooltip.rs`, Sol's):** Rust's `route_model_name` is meant
to be Electron's `routeModelName`, but for Claude it reads only a full id (`claude_model_name`). Electron reads an alias
through `claudeRouteModelName`: a finished run's report, else Claude Code's own name for it, else
`CLAUDE_ALIAS_DEFAULTS` (which `models.rs` has, privately, with the same four entries). So `route_model_name("claude",
"fable")` is "Fable" where Electron's case says "Fable 5.1" (seen in a test run, 18 Nov). Where it shows: the routine row
(`routines.rs` `model_said`, Electron's `routineChain` with models, which calls `routeModelName`): "Wren (Opus)" where
Electron says "Wren (Opus 5.5)". Not changed here: the fix belongs in `route_model_name`, which is Sol's.

No Rust code changed. Breaks: 5 on `models.rs` and `route_at_start.rs` (`r3b5c-breaks.log`), 8 on `rail.rs` and
`conversation_list.rs` (`r3b5n-breaks.log`): all caught by name.

## Batch 6 (window-6): a pull request's news, What's new's dates, what a comparison changed
Carried: `pull-request-news-is-said` (3): `src/ui/pull_request_news_tests.rs`, drawn headless. Electron's `is-plain`
class is Rust's plain fill (`bg-raised`; news to fix is `amber-tint`), and `aria-pressed` is "Watching" on a lime fill.
Partial: `whats-new-reads-like-claude-codes` (1 of 6): `src/whats_new_tests.rs`; `a-comparison-that-edits-says-what-each-changed`
(1 of 5, one in half): `compare_changes_line_tests.rs`.

**Bug fixed in `src/changelog.rs`:** `changelog_date` said nothing for a date not written as three numbers (`someday`),
so the heading fell back to "Version 0.281.0"; it also read `2026-9-3` and refused a day of `00` or `32`. It is
Electron's `changelogDate` now: exactly `YYYY-MM-DD` with a real month reads as words, anything else is said as written.
The Electron case was written first and failed on the old code (`left: None, right: Some("someday")`).

Not built in Compare, for Colin (no such words in Rust): Electron's bar adds "On Auto, a model can still change files
outside its copy if it is asked to." when the columns run on Auto (0.555, Sol's run that wrote into Colin's folder);
Keep says "Its changes come into your folder, not committed", and in a folder too big to copy "its changes are there
already". Rust's What's new heading is "0.281.0 · September 23, 2026" in one line where Electron draws the date with
the version as a badge (`home_panel.rs`, Sol's).

## Batch 7 (window-7): the judge's scorecard
Partial: `a-judge-scorecard-sits-under-the-answers` (3 of 4, one in half): `src/ui/judge_scorecard_tests.rs`, on
`render_scorecard` (called directly from a test module inside `compare_view.rs`). Where the card sits, before the
judge's own words, is drawn in `view` and not checked. A verdict and its why are one cell in Rust.

Breaks for batches 6 and 7: 26, after a first run where every break timed out waiting for the lock (stopped, rerun with
1500 s each; `r3b67c-breaks.log`, `r3b67n-breaks.log`): the old `changelog_date` put back, its words and day, the
banner's ask, words, fill and sentence, the toggle's words and fill, `pull_request_ask`, `changes_line`'s noun, zero
and sign, the scorecard's headers, verdict, why, best, time, spent (twice), the judge's note, column order and
`blind_name`. All caught by name.

## Batch 8 (window-8): Scout's fix line, a new person's starters and usage line
Partial: `a-scout-fix-says-where-it-stands` (2 of 3): `src/ui/scout_fix_tests.rs`, on `render_fix_line` drawn headless
and `scout::view`'s `fix_said`, `fix_files_line` and `card_shows_draft`. Not checked: the Settings switch (its words
are in `scout_setting.rs` and have Rust tests of their own, not one per Electron case).
Partial: `a-new-person-is-met-where-they-are` (4 of 7, one in half): `src/ui/new_person_tests.rs`. Two cases read
Electron's source (`App.tsx`, `FirstLaunch.tsx`); the starter row's wiring in the third is the same.
No Rust code changed. Breaks: 15 on `scout/view.rs`, `scout_cards.rs`, `starters.rs`, `idle_teammate.rs` and
`usage.rs` (`r3b8-breaks.log` and `r3b8-fix.txt`): all caught by name.

## Batch 9 (window-9): command cards, Memory screen's forgotten/before rows, picker scroll
Carried: `a-commands-card-says-unknown-once` (2): `src/ui/commands_card_tests.rs`, drawn headless through
`approval_card`.
Partial: `a-commands-card-says-what-it-is-for` (3 of 5): `src/ui/commands_card_tests.rs`, plus
`command_plain::plain_command`'s `careful` flag. `approval_card.rs` draws `In short:` in `p.text` rather than amber
when `careful` is true (`plain_action` returns `Option<String>`), and `phone-page.ts` is not in native.
Partial: `a-forgotten-memory-shows-with-restore` (2 of 2, first in half) and `a-memory-shows-what-it-said-before` (2 of
2, first in half): `src/ui/memory_screen_tests.rs`, drawn headless through `people_panel::memory_screen`. Rust draws
`Recently forgotten` / `Restore` and `Before: …` without Electron's extra subtitle, `forgotten by`, date, or `Put it back`.
Does-not-apply: `the-picker-opens-on-what-is-chosen` (reads `RoutePicker.tsx?raw` for React DOM `listRef` / `scrollTop`).

**Bug fixed in `core/src/command_plain.rs`:** `redirect_targets` used `invariants::regex(r#"^(["'])(.*)\1$"#)` to
unquote redirect targets. Rust's `regex` crate does not support backreferences (`\1`), so any command with a redirect
(like `echo LOCUST-ACP-OK > approved.txt` from Electron's test) panicked at runtime. Replaced with `strip_prefix`/`strip_suffix`
for matching `"` or `'` quotes. The Electron case was written first and panicked on the old code.
Breaks: 8 on `approval_card.rs` and `people_panel.rs` (`r3b9-breaks.log`): all caught by name.

## Batch 10 (window-10): other-app access in Settings, stay-awake switch, picker empty states
Partial: `other-app-access-is-an-explicit-setting` (5 of 8): `src/ui/settings_door_tests.rs`, drawn headless through
`settings_panel::door_section`. Not checked: React mock IPC toggle-off, OpenClaw line, `editorSeen` / `editorAnswers`,
and `turnPromptLine`.
Partial: `a-plugged-in-computer-stays-awake-in-settings` (1 of 1 in half): `src/ui/stay_awake_tests.rs`, drawn headless
through `quit_guard_dialog::startup_section`.
Partial: `picker-empty-states` (2 of 3 in half): `src/ui/picker_empty_tests.rs`, drawn headless through
`thread_view::model_picker` ("Reading the models…" vs Electron's "Still reading the model list").
No Rust code changed. Breaks: 9 on `settings_panel.rs`, `quit_guard_dialog.rs` and `thread_view.rs` (`r3b10-breaks.log`
and `r3b10-fix.txt`): all caught by name.
