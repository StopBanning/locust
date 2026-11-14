# Approvals & surrounding helpers parity notes

Carried nine Electron test files into `apps/native/` on branch `exec/native-approvals` with one Rust `#[test]` per applicable TypeScript case, exact Electron copy/limits, and native runtime/UI wiring:

1. `apps/desktop/src/shared/a-command-output-keeps-its-tail.test.ts` (3 cases) -> `apps/native/core/src/own_command.rs` (`d29222a6`).
2. `apps/desktop/src/shared/the-memories-that-bear-on-the-ask-come-first.test.ts` (7 cases) -> `apps/native/core/src/people.rs` (`525e9588`).
3. `apps/desktop/src/main/approval-patch.test.ts` (5 cases) -> `apps/native/core/src/approval_patch.rs`, `apps/native/core/src/lib.rs` (`a14de416`).
4. `apps/desktop/src/renderer/src/approval-patch.test.ts` (2 cases) -> `apps/native/core/src/approval_patch.rs`, `apps/native/core/src/changes.rs` (`4adaa156`).
5. `apps/desktop/src/main/an-approval-says-what-it-allows.test.ts` (5 cases + wiring tests) -> `apps/native/core/src/approval.rs`, `core/src/engine_process.rs`, `core/src/engine_process_tests.rs`, `core/src/runtime/codex.rs`, `core/src/runtime/opencode_serve.rs`, `core/src/runtime/opencode_tests.rs`, `src/ui/{home_panel,scenes,side_chat}.rs` (`b88fb070`, `59636036`).
6. `apps/desktop/src/renderer/src/an-approval-names-whose-change-it-is.test.tsx` (2 cases) -> `apps/native/src/ui/approval_card.rs` (`4a15017a`).
7. `apps/desktop/src/renderer/src/an-approval-card-goes-with-its-run.test.ts` (4 cases + UI state test) -> `apps/native/src/ui/side_chat.rs` (`c05f5d55`).
8. `apps/desktop/src/renderer/src/every-saved-rule-can-go-at-once.test.tsx` (2 cases) -> `apps/native/src/ui/people_panel.rs` (`a770a13a`).
9. `apps/desktop/src/shared/the-browser-asks-only-what-it-may.test.ts` (5 of 7 cases; 2 `does-not-apply`) -> `apps/native/core/src/desk.rs`, `apps/native/core/src/lib.rs` (`07e2d1f6`).

## What changed in native

- **Command output tailing (`core/src/own_command.rs`)**: Exposed `COMMAND_OUTPUT_MAX_LINES = 200` and `COMMAND_OUTPUT_MAX_BYTES = 32 * 1024`, keeping trailing 200 lines / 32 KiB and carrying the 3 Electron test cases (`keeps_the_last_200_lines_and_says_how_many_were_left_out`, `keeps_the_last_32_kb_when_one_line_is_longer_than_that`, `leaves_a_short_command_untouched`).
- **Prompt-ranked memory recall (`core/src/people.rs`)**: Added `MEMORY_BRIEF_LINES = 24`, `MEMORY_BRIEF_BUDGET = 4000`, `MEMORY_BRIEF_LINES_WITH_FILE = 8`, `MEMORY_BRIEF_BUDGET_WITH_FILE = 1500`, `TOPICLESS` stop-words, `topic_words`, `memories_for_brief`, and `briefed_memories`, and wired `brief_with_instructions` -> `workspace_sections` (`turn_brief_with_group_and_room`) to pass the user's prompt so bearing memories outrank newer unrelated ones while still placing folder memories before global memories among ties.
- **Approval file-change patches (`core/src/approval_patch.rs`, `core/src/changes.rs`)**: Ported `MAX_CHANGES_PER_ITEM = 200`, `FileChangeKind`, `FileChangeRecord`, `relative_to_folder`, `content_hunk`, `unified_patch_text`, `file_changes_of`, `item_of`, `tool_patch_from`, and `approval_patch_from`; updated `changes::parse` to mark `RENAMED` when `--- a/...` differs from `+++ b/...`.
- **Approval wording, patches, and runtime wiring (`core/src/approval.rs`, `core/src/runtime/codex.rs`, `core/src/runtime/opencode_serve.rs`, `core/src/engine_process.rs`)**:
  - Added `always_covers` (`"every command it runs"` for `"Bash"`, otherwise the tool name), `describe_claude_tool` outside-folder detection (`OUTSIDE_NOT_UNDOABLE`), `diff_patch_from`, `ask_codex_with_changes` (up to 250ms wait via 10x 25ms polls for `item/started` file changes), `open_code_permission_request` (`always == ["*"]`, `bySubagent`, outside-folder note, metadata diff patch), `open_code_reply_for` (`"once"` / `"always"` / `"reject"` with optional denial reason), and `request_view` passing `patch_text` into `approval_rules::ruled_action_of`.
  - Wired `Codex` `changes_by_item` from `item/started` into `ask_codex_with_changes`, `OpenCodeServe` `permission.asked` / reply through `open_code_permission_request` / `open_code_reply_for`, and `engine_process::card_json` / `card_from` to round-trip `patch`.
- **Approval card runtime header (`src/ui/approval_card.rs`)**: Added `runtime_display_name` (falling back to `"Codex CLI"` when `runtime` is empty) so patch preview boxes render `"The change, as {who} would apply it"`.
- **Live-run approval filtering (`src/ui/side_chat.rs`)**: Added `approvals_of_live_runs` (`ENDED = ["completed", "failed", "cancelled"]`, matching `mission_id` by run key or `data_run_id` and returning `None` when nothing is dropped) and wired `apply_waiting_approvals` so waiting cards clear once their run ends.
- **Saved approval rules bulk remove confirmation (`src/ui/people_panel.rs`)**: Wired `confirming_remove_all` inline confirmation (`"Remove the 1 saved rule? Every card asks again afterwards."` / `"Remove all {count} saved rules? Every card asks again afterwards."`) with confirm/cancel controls before clearing rules.
- **Desk request/event guard & frame chunking (`core/src/desk.rs`)**: Ported Electron's desk channel access policy (`desk_access`, `WINDOW_ONLY_SAYS = "This works in Locust on your computer."`, read/act allowlists, `"mission-approval:decide"` blocking `"approve-always"`, `"agent-accounts:call"` restricted to `"list"`), event filter (`is_desk_event`, blocking pairing/sign-in code events), and frame chunker/reassembler (`DESK_PART_CHARS = 240_000`, `DESK_MOST_CHARS = 48 * 1024 * 1024`, `DESK_DEFAULT_WAIT_MS = 120_000`, `desk_parts`, `DeskJoiner`).

## Cases marked `does-not-apply`

- `apps/desktop/src/shared/the-browser-asks-only-what-it-may.test.ts` (2 of 7 cases):
  1. Static check reading `desktop-api.ts` and asserting every `ipcRenderer` channel appears in `READ_CHANNELS` / `ACT_CHANNELS` / `WINDOW_ONLY`: Electron preload/IPC inventory has no Rust counterpart.
  2. Structured-clone IPC serialization round-trip over `Uint8Array` and `undefined` across Electron `desk-host`: native does not use V8 structured clone or Electron IPC, and HTTP/WebSocket `desk-host` itself remains unbuilt on parity row 122.

## Special-cases audit (`scripts/special-cases.py`)

Audited the matching Electron source files before finishing:
- `apps/desktop/src/main/approval-channel.ts` (13), `main/approval-patch.ts` (2), `renderer/src/components/ApprovalCard.tsx` (7), `main/desk-host.ts` (3), `main/memory-store.ts` (22), `shared/desk.ts` (3), `shared/desk-calls.ts` (1), `shared/approval-patch.ts` (5); `permission-host.ts`, `approval-rule-store.ts`, `shared/memory.ts`, `shared/own-commands.ts`, `renderer/src/lib/approvalsOfLiveRuns.ts`, and `renderer/src/components/SavedApprovalRules.tsx` each reported 0.
- All special cases within the scope of these 9 test files are handled cleanly in the Rust implementations above; the only omitted Electron branches belong to uncarried modules (`desk-host.ts` HTTP/WS `hello`/`take`/`replay`/`call` transport in `shared/desk.ts`, deferred on row 122, and unrelated features in `memory-store.ts`).

## Judgement calls

- **Desk HTTP/WS host scope (Option 2)**: Carried `the-browser-asks-only-what-it-may.test.ts` policy/event-guard/chunker logic into standalone `core/src/desk.rs` and marked parity row 122 `partial` rather than inventing an unbuilt WebSocket desk server.
- **Additional wiring & branch coverage tests**: Added four targeted tests beyond the direct Electron `it(...)` ports (`approval::tests::diff_patch_from_and_opencode_edit_cover_outside_folder_and_replies`, `runtime::codex::tests::file_change_approval_attaches_patch_from_item_started`, `runtime::opencode::tests::opencode_serve_permission_asked_raises_card_with_always_key_and_reply`, and `patch` round-trip assertions in `engine_process::tests::the_wire_carries_a_send_an_origin_and_a_card_whole`) so runtime/process serialization paths are exercised end-to-end.
- **Standalone window test check**: Verified `window_drive_tests::a_note_on_a_changed_line_goes_with_the_next_message_and_shows_under_the_bubble` passes alone in release mode (`cargo test --release -p locust-native a_note_on_a_changed_line`: `1 passed; 0 failed; 416 filtered out; finished in 4.13s`) rather than modifying unrelated diff-notes code.

## Break checks and final run

Not finished in Hydrogen's run (it reached its time cap during break check 3); done in review on exec/native-engine.
