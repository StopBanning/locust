# Decision cards, suggested-task cards and handed-over files (12 Nov 2026)

A teammate's reply can carry `<locust-ask>`, `<locust-next>`, and `<locust-file>` tags. The Rust thread now strips and unwraps those tags from reply prose and renders all three as interactive cards matching Electron's behaviour and words.

## What was built

- **One shared parser** (`core/src/protocol_tags.rs`, from `shared/protocolTags.ts`, `shared/decision.ts`, `shared/next-steps.ts`, and `shared/handover.ts`):
  - `unwrap_protocol_tags`, `defang_protocol_blocks`, `blocks_outside_code`, `strip_blocks_outside_code`, and `shown_text` are shared between the door (`core/src/door_tools.rs` now re-exports `unwrap_protocol_tags` instead of keeping a second copy) and the native thread UI.
  - `parse_decision`, `strip_decision_blocks`, `decision_reply`, `without_template_marks`, and `is_placeholder_label` implement `<locust-ask>` parsing, bounds (`MAX_QUESTION_LENGTH = 600`, `MAX_OPTION_LABEL_LENGTH = 90`, `MAX_OPTION_NOTE_LENGTH = 160`, `MIN_OPTIONS = 2`, `MAX_OPTIONS = 4`), bare-assent rejection, duplicate-label rejection, and plain-text fallback for refused or subsequent ask blocks.
  - `parse_next_steps`, `strip_next_blocks`, and `next_step_prompt` implement `<locust-next>` parsing (`MAX_NEXT_STEPS = 3`, `MAX_NEXT_TITLE = 80`, `MAX_NEXT_BRIEF = 600`, placeholder filtering, case-insensitive deduplication).
  - `parse_file_blocks`, `refused_file_lines`, `strip_file_blocks`, `is_whole_path`, and `handed_file_on_disk` implement `<locust-file>` parsing (`MAX_FILES_PER_REPLY = 4`, `MAX_FILE_NOTE_LENGTH = 120`, `FILE_BLOCK_EXAMPLE_PATH` filtering, UNC/drive-relative refusal).
- **Decision card** (`src/ui/decision_card.rs`, from `DecisionCard.tsx` and `missionView.ts` `decisionStanding`):
  - Shown at the bottom of the thread only when the latest turn has ended, no user message followed it, and no approval card is waiting.
  - Neutral styling (never amber), showing `"{teammate} needs a decision to continue"` or `"A decision is needed to continue"` when there is no teammate (e.g. `@ledger`), the standing line (`"stopped here · nothing was changed"`, `"stopped here · work already done is kept"`, or `"stopped here · this run could edit files"`), numbered options (`1`..`4`), and `"Or reply below — you are not limited to these"`.
  - Clicking an option or pressing `1`..`9` in `on_key` (only when no text field or modal holds focus) sends `decision_reply(option)` as the next turn.
- **Suggested-task cards** (`src/ui/next_steps.rs`, from `NextSteps.tsx`):
  - Drawn under finished assistant replies with `"1 suggested task"` / `"N suggested tasks"`, `"Start all"` (when more than one), `"×"` (`Put away: <title>`), `"Do it here"` (when the turn is idle), and `"Start"`.
- **Handed-over files** (`src/ui/handed_files.rs`, from `Thread.tsx` `HandedFiles` and `missionView.ts`):
  - Drawn under finished assistant replies as file pills (showing filename for whole paths, relative path otherwise, plus optional note) that reveal the file in the file manager via `file_menu::decide_reveal` and `file_menu::reveal_path` (never opening or running the file).
  - Refused paths (such as UNC shares) render `"Not shown as a file: … -- a file is handed over by its path on this computer, and this is not one."`.
- **Fixture extension** (`scripts/fixtures/m1-app-server.py`):
  - Added `cards-decision`, `cards-next`, `cards-file`, and `cards-streaming` scenarios for headless window tests.

## Judgement calls

- **Decision focus handoff**: When a turn finishes with an active decision card and the composer draft is empty, `ledger_tick` clears `s.draft.focused` so pressing `1`..`4` immediately answers the decision (matching Electron's `DecisionCard.tsx` focus behaviour). Clicking the composer re-focuses `s.draft`, so digits typed into the composer go to the text box.
- **`Start all` on suggested tasks (changed in review)**: the native window runs one turn of its own at a time. The first version put every task away and started only the first, which would lose the rest unstarted. Start all is now left out; each task keeps its own Start until the window can start several turns at once.
- **Handed-file copy/save-as buttons**: Electron's `HandedFiles` pill also includes a clipboard copy button and a "Save a copy…" download button alongside reveal. Native renders the reveal pill (and refusal status) and omits the secondary copy/save-as buttons for now.
- **Standing check for "work already done is kept"**: `decision_standing` checks whether any `ledger::Item::Tool` in the latest turn has an edit/write/patch/fileChange tool name, rather than inspecting raw event payloads.

## Which TypeScript tests came over

- `shared/protocol-tags.test.ts` -> `core/src/protocol_tags.rs` (`defangs_every_tag_the_host_parses`, `defanged_blocks_leave_ordinary_prose_and_catch_odd_casing`)
- `shared/decision.test.ts` -> `core/src/protocol_tags.rs` (`reads_dashed_numbered_and_asterisk_options`, `option_named_after_template_uses_note_and_keeps_real_names`)
- `shared/a-tidy-block-is-read-exactly.test.ts` -> protocol/defang behaviour covered in `protocol_tags::tests` (tidy-specific memory parser is not part of native cards)
- `shared/a-tag-quoted-in-code-swallows-nothing.test.ts` -> `core/src/protocol_tags.rs` (`tag_quoted_in_code_swallows_nothing`)
- `shared/a-block-in-code-is-an-example.test.ts` -> `core/src/protocol_tags.rs` (`blocks_inside_code_are_not_parsed`)
- `shared/a-torn-tag-keeps-the-letters-before-it.test.ts` -> `core/src/protocol_tags.rs` (`torn_tag_at_end_is_removed_and_letters_before_it_kept`)
- `shared/a-question-that-is-not-a-card-stays-in-the-reply.test.ts` -> `core/src/protocol_tags.rs` (`ask_block_card_is_removed_and_refused_or_second_ask_stays_as_plain_words`)
- `shared/a-half-written-tag-is-never-shown-to-a-person.test.ts` -> `core/src/protocol_tags.rs` (`half_written_tag_is_never_shown`) + `src/ui/window_drive_tests.rs` (`a_reply_hands_over_files_and_half_written_tags_never_show_while_streaming`)
- `renderer/src/a-decision-is-answered-by-number.test.tsx` & `renderer/src/a-decision-without-a-teammate-names-nobody.test.tsx` -> `src/ui/window_drive_tests.rs` (`a_decision_is_answered_by_number_and_without_a_teammate_names_nobody`)
- `renderer/src/a-reply-suggests-tasks-as-cards.test.tsx` -> `core/src/protocol_tags.rs` (`next_steps_parses_caps_dedups_and_prompts`) + `src/ui/window_drive_tests.rs` (`a_reply_suggests_tasks_as_cards_and_acts_on_do_it_here`)

## What was left out

- Copy-text and Save-a-copy buttons on `HandedFiles` pills.
- Multi-conversation background fan-out for `Start all` beyond starting the first suggested task.
- `locust-tidy` memory-tidy suggestion parsing (not one of the three reply cards).
