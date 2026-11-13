# Conversation Groups (Projects), Standing Instructions, Settings Dialog & Thread Boundaries — Notes

## What was built

- **Core Groups Store, Chain Lookup, Briefing & Thread Boundaries (`apps/native/core/src/`)**:
  - `groups.rs`:
    - Electron-shaped `groups.json` (`schemaVersion: 1`, 4 MiB cap `MAX_FILE_BYTES`, `GROUPS_UNREADABLE` fail-closed guard) with read-only fallback to Electron's `groups.json` and atomic writes (`Store::create_new_file_in` + `Store::check`, serialized by `WRITES` mutex) only inside native's data directory. Preserves unknown JSON fields and partial legacy `route` objects.
    - Limits matching Electron's `group-store.ts`: `MAX_GROUPS = 64`, `MAX_GROUP_NAME_LENGTH = 60` (UTF-16, whitespace collapsed), `MAX_GROUP_MEMBERS = 5000`, `MAX_GROUP_INSTRUCTIONS_LENGTH = 4000` (UTF-16), `MAX_LEFT_PER_CONVERSATION = 8`, `MAX_CHAIN_HOPS = 64`.
    - CRUD & assignment: `create_group`, `rename_group`, `remove_group` (records `LeftMembership` via `with_leave` for each removed member), `assign_to_group`, `assign_conversation` (migrates prior turn keys to `root`, keeps `at` when already in the same group, records `with_leave` on unassign or move), `set_group_route` (validates `runtime`, `model`, `mode`, `account`, `effort`), and `set_group_instructions` (refuses null bytes and key-shaped secrets via `secrets::secret_in` / `secrets::instructions_secret_refusal`).
    - Turn briefing & chain walk: `keys_before` walks `continuesFrom.missionId` across both native and Electron ledgers up to `MAX_CHAIN_HOPS` with cycle protection; `group_brief_for` / `group_brief_for_tip` resolves active group instructions (unreadable `groups.json` never blocks a turn or `/setup`); `group_section` formats the standing-instructions block.
    - Thread boundary placement: `group_boundary`, `group_joins`, and `group_leavings` place `<Group>'s instructions brief every turn from here` and `<Group>'s instructions no longer apply from here` at `before_turn` indices (skipping turns without start times, blank instructions, or missing timestamps).
  - `people.rs` & `native.rs` & `teammate_setup.rs`:
    - `people::turn_brief_with_group_and_room` & `workspace_sections` insert `group_section` after the folder's `where_section` and before `teammate_section`.
    - `native::send_from` resolves `groups::group_brief_for_tip(&store, previous.as_deref())` for each turn.
    - `teammate_setup::read_teammate_setup_with_group` includes the conversation's active group standing instructions for `/setup`.

- **UI & Window Integration (`apps/native/src/`)**:
  - `ui/group_settings_dialog.rs`: `Project settings` modal (`Standing instructions` with claim, `LOCUST.md` provenance line, `> 3600` UTF-16 character counter, `Clear instructions` save label when clearing, `Default route` with composer-style `route_label`, `Use current`, disabled-when-empty `Clear`, inline secret refusal keeping dialog open, and `seed_route_from_group` on group join).
  - `ui/org.rs`: wraps `locust_core::groups` operations for window state and blocks group mutations when `groups_unreadable` is set.
  - `ui/sidebar.rs`: renders the `Groups could not be read — everything is listed ungrouped.` banner above conversations when unreadable, collapsible project groups (`folded_groups`), project `…` menu (`Project settings…`, `Rename`, two-click `Remove project` -> `Remove for good?`), conversation `Move to project` list (`Ungrouped`, projects, `New project…`), and seeds composer route on assignment.
  - `ui/messages.rs`: `thread_group_lines` & `emit_group_lines_at` draw join lines (`view`/`hide` expandable instructions + `edit in Project settings` when still a member) and leaving lines at their turn boundaries, preserving past memberships from `left`.
  - `ui/setup_card.rs`: renders active group instructions character count between `LOCUST.md` and `Its instructions`.
  - `ui/activity_drawers.rs`: keeps live/growing activity-step groups folded until pressed (`activity_group_stays_folded_until_opened_and_then_shows_every_row`).
  - `main.rs`: Escape closes `group_settings` and `group_menu_id`; `view` renders `group_settings_dialog::view`.
  - `ui/window_drive_tests.rs`: two headless window tests (`group_standing_instructions_brief_turns_after_folder_and_render_join_note_in_window`, `leaving_a_group_keeps_join_note_shows_leaving_note_and_stops_briefing_future_turns`) using `scripts/fixtures/m1-app-server.py`.

## Special cases & invariants

`python3 apps/native/scripts/special-cases.py apps/desktop/src/main/group-store.ts` reported `0 special cases`. All Electron limits and special cases below were checked by breaking the Rust implementation one at a time against the whole `locust-core` suite (batch 1 limits pinned as literal values in `18e2082c`; batch 2 and wiring checks run on `18e2082c`):

| Check | Rust location | Test that fails when broken |
|---|---|---|
| `MAX_GROUPS = 64` | `core/src/groups.rs:19, 560` | `groups::tests::refuses_creating_more_than_max_groups` |
| `MAX_GROUP_NAME_LENGTH = 60` (UTF-16) | `core/src/groups.rs:20, 127` | `groups::tests::refuses_empty_or_whitespace_name_and_caps_name_length` |
| `MAX_GROUP_MEMBERS = 5000` | `core/src/groups.rs:21, 686` | `groups::tests::enforces_max_group_members_except_moving_existing_member` |
| `MAX_GROUP_INSTRUCTIONS_LENGTH = 4000` (UTF-16) | `core/src/groups.rs:22, 754` | `groups::tests::sets_trims_caps_clears_and_refuses_secrets_in_instructions` |
| `MAX_LEFT_PER_CONVERSATION = 8` | `core/src/groups.rs:23, 547` | `groups::tests::caps_left_history_per_conversation_to_eight_newest` |
| `MAX_CHAIN_HOPS = 64` | `core/src/groups.rs:24, 819` | `groups::tests::keys_before_walks_chain_stops_at_max_hops_and_breaks_cycles` |
| Unreadable `groups.json` blocks all writes & never overwrites | `core/src/groups.rs:385-388` | `groups::tests::unreadability_blocks_all_mutations_and_never_overwrites_file` |
| Electron `groups.json` is read-only when native writes | `core/src/groups.rs:507` (`store.groups_json_path()`) | `groups::tests::electron_groups_json_is_read_only_when_native_writes` (+ 20 other group tests) |
| Key-shaped secret in group instructions is refused | `core/src/groups.rs:762-764` | `groups::tests::sets_trims_caps_clears_and_refuses_secrets_in_instructions` |
| `send_from` wires `group_brief_for_tip` into turn brief | `core/src/native.rs:2029` | `native::tests::follow_up_turn_in_grouped_conversation_receives_group_instructions` |
| `group_section` placed after folder and before teammate | `core/src/people.rs:976-980` | `people::tests::turn_brief_places_group_instructions_after_folder_and_before_teammate` |

## Which TypeScript tests came over

- **Carried (4 files — every `it(...)` has a Rust test):**
  - `apps/desktop/src/renderer/src/group-boundary.test.ts` (4/4) -> `groups::tests::{group_boundary_places_line_before_first_turn_at_or_after_joined_at, group_boundary_returns_none_for_missing_at_blank_instructions_or_invalid_time, group_boundary_skips_turns_without_start_time}`, `messages::tests::thread_renders_group_join_expandable_instructions_and_leaving_line`, `window_drive_tests::group_standing_instructions_brief_turns_after_folder_and_render_join_note_in_window`
  - `apps/desktop/src/renderer/src/group-join-line-survives-leaving.test.ts` (4/4) -> `groups::tests::{group_joins_keeps_join_boundary_after_leaving, group_joins_carries_the_instructions_that_actually_briefed, group_joins_skips_missing_at_or_blank_instructions, group_joins_keeps_rejoin_history_in_order}`, `messages::tests::thread_renders_group_join_expandable_instructions_and_leaving_line`, `window_drive_tests::leaving_a_group_keeps_join_note_shows_leaving_note_and_stops_briefing_future_turns`
  - `apps/desktop/src/renderer/src/group-leaving-line.test.ts` (4/4) -> `groups::tests::{group_leavings_places_line_before_first_turn_at_or_after_until, group_leavings_sits_at_end_when_conversation_has_not_continued_since, group_leavings_skips_blank_instructions_and_unparseable_until, group_leavings_skips_turns_without_start_time_and_handles_multiple}`
  - `apps/desktop/src/renderer/src/the-newest-group-stays-folded.test.tsx` (1/1; activity-step group) -> `activity_drawers::tests::activity_group_stays_folded_until_opened_and_then_shows_every_row`

- **Partial (6 files):**
  - `apps/desktop/src/main/groups-hold-conversations.test.ts` (28/35) -> `groups::tests::*`, `sidebar::review_tests::sidebar_folds_project_groups_and_seeds_composer_route_on_assign`. Left out: `remove_group` on an unknown id returns `Err("That group does not exist")` in Rust (vs no-op in TS); `assign` move test does not explicitly assert `at` timestamp change; the 6 `App.tsx` source-text wiring assertions (startup `.listGroups()`/`setGroups`/`setGroupMembers` and `Move to project` tick/submenu/newest-match-by-`createdAt` checks) have no Rust UI test counterpart.
  - `apps/desktop/src/main/groups-remember-leaving.test.ts` (5/6) -> `groups::tests::{records_leaving_when_unassigned_and_preserves_at_and_instructions, records_leaving_when_moving_to_another_group, assigning_to_same_group_twice_records_no_leaving_and_keeps_joined_at, removing_group_records_leaving_for_all_its_members, unassigning_bare_legacy_membership_records_leaving_without_at, caps_left_history_per_conversation_to_eight_newest}`. Left out: no test explicitly renames/edits a group *after* unassigning to assert the saved `left` snapshot keeps the old name and instructions (true by data model: `LeftMembership` owns `name` and `instructions`).
  - `apps/desktop/src/renderer/src/a-conversation-keeps-its-name-and-group.test.ts` (10/11) -> `conversation_list::tests::a_conversation_keeps_its_name_and_group`, `sidebar::review_tests::real_sidebar_rows_collapse_turns_keep_names_groups_and_search_every_prompt`, `org::identity_review_tests::clearing_names_groups_pins_and_settled_marks_clears_old_turn_keys`. Left out: raw `App.tsx` header title precedence assertion (native resolves `conversation_title_for` in `main.rs` `open_recent`).
  - `apps/desktop/src/renderer/src/a-growing-group-shows-its-rows.test.tsx` (1/2; activity-step group) -> `activity_drawers::tests::activity_group_stays_folded_until_opened_and_then_shows_every_row` carries the folded control case; the 0.584 auto-open-while-growing behavior was intentionally superseded in 0.610 by `the-newest-group-stays-folded`.
  - `apps/desktop/src/renderer/src/group-settings-dialog.test.tsx` (2/5 fully asserted; UI implements all 5) -> `group_settings_dialog::tests::{renders_project_settings_instructions_and_default_route_empty_state, shows_character_counter_above_3600_and_clear_instructions_label, use_current_and_clear_update_route_and_save_persists_both, save_does_nothing_when_unchanged_and_refuses_secrets_while_keeping_open, seed_route_from_group_populates_composer_pick_and_mode}`. Left out in assertions: provenance `FOOT` text (`Given after the folder's own LOCUST.md.`), absence of the counter at `<= 3600`, and disabled state of `Clear` when no route is set are implemented in `view` but not explicitly asserted in the unit tests.
  - `apps/desktop/src/renderer/src/groups-unreadable-is-a-line.test.tsx` (2/4) -> `sidebar::review_tests::groups_unreadable_shows_warning_hides_groups_and_recovers_on_reload`, `conversation_list::tests::narrowing_and_unreadable_words`. Left out: empty-folder control (`groups: []`, `unreadable: []`) is not tested in `sidebar.rs`; `sidebar.rs` collects `groups` and `teammates` unreadable flags (not `rooms`/`routines`) and does not test the multi-store `"Teammates and routines could not be read."` sentence.

## What was left out

- `remove_group` on a nonexistent `group_id` returns `Err("That group does not exist")` in Rust (`groups.rs:602`), whereas Electron's `remove` treated a missing group as a no-op.
- `sidebar.rs` `org_menu` lists `Move to project` (`Ungrouped`, projects, `New project…`) without a `✓` indicator on the group the conversation currently belongs to, and `sidebar.rs` `unreadable` warning banner only checks `groups` and `teammates` (not `rooms` or `routines`).
- No `App.tsx`-equivalent source-grep tests for startup loading or `Move to project` submenu ordering/ticks.
