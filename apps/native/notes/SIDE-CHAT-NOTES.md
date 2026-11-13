# Ask about a selection and the read-only side chat in Rust

Notes on porting Electron's selection-ask and side-chat features into `apps/native/` to match:

- `apps/desktop/src/renderer/src/components/SelectionAsk.tsx`
- `apps/desktop/src/renderer/src/components/SideChat.tsx`
- `apps/desktop/src/main/codex-mission.ts` (`side`, `SIDE_QUESTION_PREFACE`, `SIDE_RUNNING_ASK_CHARS`)
- `apps/desktop/src/main/a-side-question-forks-its-conversation.test.ts`
- `apps/desktop/src/renderer/src/a-side-question-can-be-approved-and-stopped.test.tsx`
- `apps/desktop/src/renderer/src/a-part-of-a-reply-can-be-asked-about.test.tsx` (selection-quote part)

## What was built

- **Engine side-question orchestration (`apps/native/core/src/side_question.rs`, `apps/native/core/src/native.rs`)**:
  - `SIDE_QUESTION_PREFACE`, `SIDE_RUNNING_ASK_CHARS` (`2_000`), `parse_started_by`, `can_fork_runtime` (`claude`, `codex`, `opencode`), `build_side_prompt`, and `prepare`.
  - `prepare` enforces `can_fork_runtime` and `Mode::Ask` (read-only), resolves the target conversation's `runtimeThreadId` (or walks up to 8 `continuesFrom` hops when the latest turn is still running and has not emitted its own session yet, carrying that running turn's prompt in `build_side_prompt`), and refuses when no session exists to copy.
  - `native::send_from` handles `startedBy.kind == "side"` without setting `continuesFrom`, without touching the main conversation's `owns` slot (registering `side:<of>` + `<mission_id>` instead, and returning the new side `mission_id`), without recording memory (`record_reply`), and without committing to a teammate's worktree branch (`checkpoint_turn` is skipped).
  - Runtime fork support:
    - Claude Code: `claude_args_with_fork(..., fork: true)` passes `--resume <id> --fork-session`.
    - Codex: `Codex::with_fork(true)` sends `thread/fork` instead of `thread/resume`.
    - OpenCode (V1 local server): `OpenCode::with_fork(true)` / `Serve::with_fork(true)` calls `POST /session/<id>/fork`.
  - `ledger::try_list_recent` skips missions with `startedBy.kind == "side"`, so side questions never appear in the sidebar or recent conversation list.
- **Window UI (`apps/native/src/ui/side_chat.rs`, `apps/native/src/ui/messages.rs`, `apps/native/src/ui/thread_view.rs`, `apps/native/src/ui/app_state.rs`, `apps/native/src/main.rs`)**:
  - `quote_of` and `MAX_QUOTE` (`1_500`) quote selected text line-by-line and cap with an ellipsis (`…`).
  - `messages.rs` wraps each assistant reply body in `.selection_group(true).on_selection_change(...)` so selecting text in a reply (once pointer drag finishes) shows the `"Ask about this"` chip (`key("selection-ask")`) in `thread_view.rs`.
  - `side_chat::header_button` adds `"Ask on the side"` to the thread header for `claude`, `codex`, and `opencode` conversations.
  - `side_chat::render_panel` renders `"On the side"` (`"{model} · a copy of this conversation"`), the empty-state explanation, side turns polled from their own `ledger::Session`, waiting approval cards (`"Approve once"` / `"Deny"` via `engine_api::api().decide`), `"Stop this answer"` while running (`engine_api::api().stop`), and `"Ask on the side"` when idle (`engine_api::api().send`).
- **Headless window test & fixture (`apps/native/src/ui/window_drive_tests.rs`, `apps/native/scripts/fixtures/m1-app-server.py`)**:
  - `m1-app-server.py` handles `thread/fork` and replies `"Side answer from copy."`.
  - `selecting_text_in_a_reply_asks_on_the_side_and_keeps_main_ledger_unchanged` sends a main turn, drags to select `"Hello world"` in the reply, clicks `"Ask about this"`, sends `"Why?"` in the side chat, observes `"Side answer from copy."`, and verifies the main conversation's ledger is byte-for-byte unchanged and the side turn is excluded from `list_recent`.

## Judgement calls

1. **`"Ask about this"` opens the side chat with the quote when the runtime supports it**:
   - In Electron, `SelectionAsk` quotes into the main composer, while `"Ask on the side"` in the `...` menu opens `SideChat`. In `apps/native`, clicking `"Ask about this"` opens `SideChat` with `quote_of(selected)` in `side_chat.draft` for `claude`, `codex`, and `opencode` conversations (and falls back to quoting into the main composer `s.draft` for runtimes that cannot fork), while `"Ask on the side"` in the header also opens `SideChat` directly.
2. **Builder `with_fork(bool)` on `Codex` and `OpenCode` rather than adding `fork` to `runtime::Turn`**:
   - Adding a `with_fork` builder method on `Codex`, `OpenCode`, and `Serve` (matching `with_cancel`) avoids modifying 25+ `Turn { ... }` struct literals across existing runtime tests.
3. **`Started.conversation_id` returns the side turn's `mission_id` while `Registered` holds `side:<of>`**:
   - Registering both `side:<of>` and `<mission_id>` in `native::RUNNING` enforces one concurrent side question per conversation without blocking the main conversation, while returning `<mission_id>` from `engine_api::api().send` lets the window tail the side ledger, filter `waiting_for(&mission_id)`, and call `stop(&mission_id)` through `EngineApi` without any direct `native::` calls.

## Which TypeScript tests came over

- `apps/desktop/src/main/a-side-question-forks-its-conversation.test.ts` (all 7 tests -> `locust_core::side_question::tests`):
  - `forks_the_conversations_session_read_only_and_records_as_side_not_next_turn`
  - `preface_tells_it_only_what_it_is`
  - `copies_the_session_a_turn_still_running_resumed_before_it_has_said_its_own`
  - `is_told_what_the_running_turn_was_asked_only_when_copy_ends_before_it`
  - `is_refused_when_there_is_no_session_to_copy`
  - `is_only_ever_read_only`
  - `is_refused_on_a_runtime_that_cannot_copy_a_session`
- `apps/desktop/src/renderer/src/a-side-question-can-be-approved-and-stopped.test.tsx` (both tests -> `ui::side_chat::tests`):
  - `the_side_panel_shows_approval_with_buttons`
  - `the_side_panel_offers_stop_while_answering_and_ask_when_done`
- `apps/desktop/src/renderer/src/a-part-of-a-reply-can-be-asked-about.test.tsx` (selection-quote tests -> `ui::side_chat::tests`):
  - `a_part_of_a_reply_is_quoted_line_by_line_and_capped`

## What was left out

- **OpenCode V2 (ACP) fresh-session fallback**: Electron falls back to briefing a fresh session when OpenCode runs over ACP (`opencodeAcp`), because ACP has no fork. In Rust, `OpenCode::with_fork` wires `/session/{id}/fork` for OpenCode V1 (`serve`) and does not implement a fresh-session fallback for V2 ACP.
- **Ask-question multi-option cards inside `SideChat`**: `SideChat` renders `"Approve once"` and `"Deny"` buttons on waiting `approval::Request` cards; structured multi-choice `questions` in `SideChat` use the same `Approve once`/`Deny` surface rather than a separate question-answer form.
- **Live `/tune` and `compare` integration**: Electron's `askOnTheSide` also serves `/tune` and kept comparison columns; those callers are not wired here.
