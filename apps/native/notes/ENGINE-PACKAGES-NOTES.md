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
