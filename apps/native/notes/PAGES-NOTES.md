# Pages 1: Rooms, Routines and Board look like Electron's (27 Nov 2026)

Lane `cloud/pages-1`, from `exec/native-engine` 95dbed55. The owner's live hour (26 Nov, `notes/LIVE-HOUR-1.md`, rows 37,
38, 39 and 41) drove the Rust window beside Electron 0.882 on the same data. Each fix below is held to Electron's own
words and rules, quoted by file and line in the tests. Run in a cloud container: Electron could not be seen running, so
its source and CSS were the answer key.

## Row 37: Rooms

Electron (`RoomScreen.tsx:684-851`) has one Rooms screen with no room open:

- the header `Rooms` with its meta beside it: `none yet · one post, every teammate answers` (`N room(s) · …`);
- `Your rooms`, a card per room, only when there are rooms;
- `New room`, the form as one card in three bands (what a room is; `Room name` and `Who is in it`; the summary
  `N of M teammates · each answers on its own model` beside `Create room`), teammates as chips (face, name, a tick when
  picked) that wrap, never rows with roles;
- `No rooms yet. The first one you make appears in the sidebar under its own name.` under the card, only with no rooms;
- `Make a teammate first; a room is a set of them.` in place of the card with no teammates;
- the reason Create room is not offered (`roomBlockedReason`), in the card.

The `+` menu's New room (`App.tsx:1014 openRoomsScreen`) opens that same screen. Rust had a separate New room screen
with a Cancel button and a list of teammate rows with roles in mono; `RoomView::New` now draws the same screen as
`RoomView::List` (the variant stays, as the `r` shortcut and the `+` menu's tests name it).

Tests: `src/ui/rooms_index_tests.rs` (9 for this row). Breaks: `notes/pages-breaks/37.entries`.

## Row 38: Routines

Electron (`AutomationsScreen.tsx`, `RoutineTemplates.tsx`, `Screens.tsx ScreenHeader`):

- the header is `Routines`, the count beside it, and the screen's actions, `Import routine` and `New routine`, as ghost
  buttons at the far end (`.lc-screen__actions { margin-left: auto }`). No Close: Rust had one, and a second on its
  `Starter templates` box.
- An empty shelf lists the templates right under `Start from a template, or press New routine to write your own.`, each
  row the whole button: its name, `templateMeta` at the far end, and its summary under both. No `Use template`.
- With routines, an add row `Start from a template` names the first few (`templatesLine`) and opens the picker:
  `Start from a template`, one ×, `Each opens as a preview. Nothing is added until you give it to a teammate.`, the list.
- A routine's actions are boxed ghost buttons at the row's far end, not bare words at the left.

Not done here: Electron's picker is a dialog over a scrim, drawn by the window's top level; this lane may not touch
`main.rs`, so the picker is drawn at the top of the Routines screen (it holds the keyboard as `useModal` does, Escape
closes it). The routine row's Pause/Edit/Remove are words in boxes, where Electron draws icons; the words are the
labels other tests and screen readers find. Its `Save as file` keeps that wording (Electron: `Export`).

`routines_shelf_tests.rs`'s `offers_new_routine_written_from_nothing_where_the_screen_is_given_one` read the New routine
button's own text; a boxed button holds its words in its label (as `look::controls::ghost_button` names it), so the
test reads the label now.

Tests: `src/ui/routines_page_tests.rs` (6 for this row). Breaks: `notes/pages-breaks/38.entries`.

## Row 41: Board

Electron (`BoardScreen.tsx`, shell.css `.lc-teamboard`, `.lc-boardsection`, `.lc-convgrid`, `.lc-convcard`):

- the header is `Board` with its meta right beside it (`all quiet`, `N waiting on you · M working`, `nothing running`).
  No Close.
- the columns stand side by side, 256 wide (232 at least), tops aligned, wrapping only when the window is too narrow;
  Done takes the rest of the line and lays its cards in a grid of 240-wide cards.
- a card is two lines beside its face: the title, then one line `Who · under … · state`.
- the runs Electron never lists are not on it (`App.tsx:2016 isHiddenRun`): Scout's own, a judge's, a question on the
  side, a panel's run (a commit message, a walk through).

**Blocked by ownership.** The Board is drawn by `home_panel.rs board_view` (the Home lane's file), called from
`main.rs:2929`, and its rows come from `sidebar.rs recent_as_missions` (the sidebar lane's). This lane may edit neither.
The approval answers on a card (`answer_board`) are private to `home_panel.rs`, so a second Board in this lane's files
would have to copy the approval path, which it must not. So `src/conversation_board.rs` now holds the pieces Electron's
Board is made of, each tested, and `board_view` is to be rebuilt from them by the Home lane:

| In `board_view` today | Use |
| --- | --- |
| `let mut missions = recent_as_missions(s);` | `let mut missions = conversation_board::without_hidden_runs(&recent_as_missions(s), \|id\| s.ledger_summaries.get(id).and_then(\|m\| m.started_by_kind.clone()));` |
| the title row (`BOARD_TITLE` grow, meta grow, Close) | `conversation_board::board_header(&l, &meta)` above the scroll; no Close |
| `sec_header` and the cards pushed straight into `body` | `conversation_board::board_column(&l, section.key, count, cards)` per section, all in `conversation_board::board_columns(columns)` |
| the card's `col` of title, who, under and state | `conversation_board::conv_card(&l, section.key, &mission_id, face, &title, &conversation_board::card_meta_line(who, under, &state_line))`, then `.on_press(…)` as now |

A card with an ask (Allow once / Deny, Answer, Review) keeps its actions under the card, as `lc-boardwait` draws them;
the Scout and routine wait cards go first in Needs you's column, as now. The face is the teammate's at 20
(`BOT_SIZE.conversationOwner`).

Who counted Scout's runs: `ledger::list` already leaves out a turn whose `startedBy.kind` is `scout`, so the 74 may
come by another path (the engine's list through the door, or Electron's own Scout ledgers read by Rust). Not seen from
here; `without_hidden_runs` covers it when the summaries carry the kind. Worth checking on the Mac with the same data.

Tests: `src/board_shape_tests.rs` (7). Breaks: `notes/pages-breaks/41.entries`.

## Row 39: a reading column

Electron's measures (shell.css 26105-26133):

- Rooms: `.lc-screen__scroll:has(.lc-roomform) > .lc-settings__section { max-width: 620px; margin: 0 auto }`, and 40
  above the first section.
- Routines: `.lc-empty:has(> .lc-savable) { max-width: 680px; margin: 0 auto }`: the empty shelf (its sentence, the
  conversations to save, the templates) in one 680 column, read from its left edge. The list of routines, once there
  are some, is not in a column: `.lc-automations > .lc-routinelist` spans the page with 24 either side, and so it does
  here.
- Board: no measure. Its columns span the page (`.lc-teamboard`), so there is nothing to centre; and its view is the
  Home lane's file (above).

Tests: `the_rooms_page_is_one_620_column_in_the_middle`, `a_narrow_window_gives_the_column_its_width` (rooms_index_tests.rs), `an_empty_shelf_is_one_680_column_in_the_middle`, `the_list_of_routines_spans_the_page` (routines_page_tests.rs).
Breaks: `notes/pages-breaks/39.entries`.

## Type

`look/type_tests.rs` lets only listed files in `src/ui` write a size of their own. The three headers take the screen
title (20, 650) and its meta (13) from `conversation_board::SCREEN_TITLE` / `SCREEN_META`, outside `src/ui`, one style
for the three pages. A room card's name is `l.ty.control` at 500, where Electron's is 14.5 at 500
(`--lc-text-teammate-name`); the templates picker's title is `l.ty.control` at 600. `room_screen.rs` writes one size of
its own now (3 allowed); the list is in `src/look/`, not this lane's, so it is left at 3.

## Tests on this container

`cargo test -p locust-native --no-fail-fast` (no cargo-nextest here): see the report for the count. The failures seen
are the same on the base commit 95dbed55 in this Linux container: six `window_drive_tests` that drive a fake agent
(time out), `phone_section::...the_routines_links_reach_the_relay...` ("Teammate id is invalid"), and
`claude_cloud_panel_rows_tests::a_follow_up_on_this_computer_runs_claude_code_for_its_receipt` (Linux says Claude's
cloud opens on Windows and macOS only).

## Renders

Headless renders work in the cloud container with Mesa's software Vulkan (`apt-get install mesa-vulkan-drivers
libvulkan1`, plus `libasound2-dev libxkbcommon-dev` to build). There is a `routines` and a `board` scene and no `rooms`
scene (`scenes.rs` is not this lane's).

## Breaks

Each break was applied by a small script, the affected tests run, the failing tests noted, the file restored.
Every break was caught by a named test (none only stopped the build).

### Row 37

| # | Break | Caught by |
| --- | --- | --- |
| 1 | `let how_many = if count == 0 { "no rooms".to_owned() } else` | `the_meta_counts_rooms_in_electrons_words`, `the_header_says_rooms_and_none_yet_beside_it` |
| 2 | `root = root.child(screen_header(l, "Rooms", String::new()));` | `the_header_says_rooms_and_none_yet_beside_it`, `the_meta_counts_rooms_in_electrons_words` |
| 3 | `let mut chips = col().key("room-create-members").w_full().gap(4.0)` | `teammates_are_chips_with_a_face_and_a_name_side_by_side_never_rows_with_roles` |
| 4 | `.key(format!("room-create-tick-off-{}", tm.id))` | `a_picked_chip_wears_a_tick_and_the_summary_counts_it` |
| 5 | `if !s.teammates.is_empty() {` | `rooms_there_are_are_cards_under_your_rooms_above_the_new_room_card` |
| 6 | `if let Some(reason) = &room_full_note(picked) {` | `the_card_says_why_create_room_is_not_offered` |
| 7 | `.child(col().child(text(tm.name.clone()).font(l.ty.body).color(l.c(ink))).child(text(tm.role.clone()).font(…` | `teammates_are_chips_with_a_face_and_a_name_side_by_side_never_rows_with_roles` |

### Row 38

| # | Break | Caught by |
| --- | --- | --- |
| 1 | `.child(routines_header(s, p).child(button("Close", p)))` | `an_empty_shelf_lists_the_templates_under_its_sentence_each_with_what_it_does`, `the_header_has_its_actions_at_the_far_end_and_no_close`, `with_routines_the_add_row_names_the_templates_and_opens_the_picker` |
| 2 | the header's spacer `.child(el().grow(1.0))` removed | `the_header_has_its_actions_at_the_far_end_and_no_close` |
| 3 | `shelf = shelf.child(text(head).key("routines-templates-head").font(type_::LABEL).color(p.secondary));` | `an_empty_shelf_lists_the_templates_under_its_sentence_each_with_what_it_does`, `pressing_a_template_row_opens_its_preview`, `a_template_row_says_its_facts_at_the_far_end_of_its_name_and_its_summary_under_both` |
| 4 | `.child(text(String::new()).key(format!("routine-template-what-{}", info.id))` | `an_empty_shelf_lists_the_templates_under_its_sentence_each_with_what_it_does` |
| 5 | `.child(text(info.name.clone()).font(l.ty.control).color(l.c("text-primary")).truncate().shrink())` | `a_template_row_says_its_facts_at_the_far_end_of_its_name_and_its_summary_under_both` |
| 6 | `list = list.child(entry.child(button("Use template", p).on_press(move \|s: &mut State\| use_template(s, &ti…` | `use_template_opens_a_chain_in_the_editor_and_a_plain_one_in_the_preview`, `an_empty_shelf_lists_the_templates_under_its_sentence_each_with_what_it_does`, `pressing_a_template_row_opens_its_preview` |
| 7 | the action row's `.justify_end()` removed | `a_routines_actions_are_boxed_at_the_rows_far_end` |
| 8 | `.child(text("Pick a template.")` | `with_routines_the_add_row_names_the_templates_and_opens_the_picker` |

### Row 41

| # | Break | Caught by |
| --- | --- | --- |
| 1 | `matches!(started_by_kind, Some("judge" \| "side" \| "commit-draft" \| "walkthrough"))` | `scouts_own_runs_judges_and_side_questions_are_not_on_the_board` |
| 2 | `.filter(\|row\| !conversation_keys(row).iter().any(\|id\| is_hidden_run(kind_of(id).as_deref())))` | `a_conversation_with_one_of_its_turns_a_persons_stays` |
| 3 | `.child(text(BOARD_TITLE).key("board-title").font(SCREEN_TITLE).color(l.c("text-primary")).grow(1.0))` (replaced a first try, the meta growing, which draws the same and was missed) | `the_header_says_board_and_its_meta_right_beside_it` |
| 4 | `columns.into_iter().fold(col().key("board-columns").w_full().gap(16.0).items_start(),` | `the_columns_stand_side_by_side_tops_aligned` |
| 5 | `if key == ConversationBoardKey::Done { section.w(256.0) } else { section.w(256.0).shrink() }` | `the_columns_stand_side_by_side_tops_aligned` |
| 6 | `.px(16.0)\n        .py(12.0)\n        .gap(24.0)\n        .center_y()\n        .h(96.0)` | `a_card_is_two_lines_its_title_then_who_and_state_on_one_line` |
| 7 | `who.into_iter().chain(std::iter::once(state)).chain(under)` | `the_meta_line_is_who_under_what_and_the_state` |
| 8 | `.child(text((count + 1).to_string()).key(format!("board-{name}-count"))` | `a_column_is_titled_with_its_name_and_count` |

### Row 39

| # | Break | Caught by |
| --- | --- | --- |
| 1 | `let mut body = col().key("rooms-column").w_full().gap(24.0).mt(40.0);` | `a_narrow_window_gives_the_column_its_width`, `the_rooms_page_is_one_620_column_in_the_middle` |
| 2 | `pub const ROOMS_COLUMN: f32 = 760.0;` | `the_rooms_page_is_one_620_column_in_the_middle` |
| 3 | `.max_w(ROOMS_COLUMN).self_start().gap(24.0).mt(40.0);` | `the_rooms_page_is_one_620_column_in_the_middle` |
| 4 | `.self_center().gap(24.0).mt(8.0);` | `the_rooms_page_is_one_620_column_in_the_middle` |
| 5 | `let mut shelf = col().gap(8.0).w_full().key("routines-empty").child(` | `an_empty_shelf_is_one_680_column_in_the_middle` |
| 6 | `.max_w(EMPTY_SHELF_COLUMN).key("routines-empty")` | `an_empty_shelf_is_one_680_column_in_the_middle` |
| 7 | `pub const EMPTY_SHELF_COLUMN: f32 = 2000.0;` | `an_empty_shelf_is_one_680_column_in_the_middle` |

