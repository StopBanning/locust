# Git, Changes Panel & Pull-Request Links (exec/native-git, round 1 — 18 Nov 2026)

## Part 1: Worktrees and the turn's own branch — DONE (5/5 carried)
- `main/worktrees.test.ts` (12/12 named in `core/src/worktrees_tests.rs`):
  - Added `two_teammates_get_two_trees_that_do_not_see_each_other_and_the_main_checkout_stays_untouched`.
  - Strengthened `two_teammates_whose_names_fold_to_one_branch_each_get_a_tree` with exact sorted branch names.
  - Strengthened `removes_a_clean_copy_at_once` with a `with_git_program` wrapper verifying `git worktree remove` is called without `--force`.
  - Verified `uncommitted()` names the exact uncommitted files on C1 refusal.
- `main/a-turn-is-committed-on-its-own-branch.test.ts` (7/7 named in `core/src/worktrees_tests.rs`):
  - All 7 cases already implemented and wired via `Manager::checkpoint`, `Manager::review`, `Manager::turn_diff`, `status_entries_of`, `branch_turns_of` (`native.rs::checkpoint_turn`, `src/ui/review_changes.rs`).
- `main/a-codex-turn-on-its-branch-is-completed.test.ts` (3/3 named in `core/src/worktrees_tests.rs`):
  - Added `worktrees::turn_outcome_of(event_types: &[&str]) -> TurnOutcome` (newest terminal event wins; none -> `Failed`) and wired `native.rs::checkpoint_turn` to read raw ledger event types and call `turn_outcome_of`.
- `main/a-run-in-its-own-copy-is-not-told-the-project-folder.test.ts` (5/5 named in `core/src/worktrees_tests.rs`):
  - Added `teammate_setup::WorkspaceBrief` + `read_workspace_brief_text` (returning bounded `LOCUST.md` text as well as line count and truncation flag).
  - Added `people::where_section_text`, `people::worktree_section`, `people::brief_section`, `people::folder_sentences`, and `people::turn_brief_with_group_room_and_copy`.
  - Wired `native.rs` to pass `own_folder` whenever `repository_root.is_some()` or `origin.slot.cwd.is_some()` so worktree and comparison-copy runs never hear the project folder name or path, while still receiving `LOCUST.md`.
- `main/a-worktree-survives-only-a-finished-checkout.test.ts` (9/9 named in `core/src/worktree_checkout_tests.rs`):
  - Stored full untruncated `stderr` on `worktrees::GitError`.
  - Added `Manager::with_note` (called with full hook stderr when a finished checkout survives a nonzero hook exit, wrapped in `catch_unwind` so a failing callback never fails a usable worktree) and `Manager::with_checkout_timeout`.
  - Added `worktree_add_args_for_platform(path, branch, branch_exists, from, windows)` to test `win32`/`darwin`/`linux` longpaths flags on both branch forms.
  - Added all remaining cases: full stderr in `note`, `rev-parse --verify HEAD` required, timeout not treated as finished hook failure, failing `note` callback tolerated, `worktree_add_args` recorded on add without changing `core.longpaths`, and platform longpaths matrix.
- Mutation checks (`mutsafe.py`, 7/7 killed):
  - `worktrees.rs` clean remove `--force` -> `worktrees::tests::removes_a_clean_copy_at_once`
  - `worktrees.rs` checkpoint drop `--no-verify` -> `worktrees::tests::commits_every_change_new_files_too_as_the_teammate_past_a_hook_that_would_refuse_and_leaves_the_persons_branch_alone`
  - `worktrees.rs` `turn_outcome_of` forward iter -> `worktrees::tests::takes_the_newest_ending_and_says_failed_when_there_is_none`
  - `people.rs` `folder_sentences` leak folder path -> `worktrees::tests::and_a_run_in_its_own_copy_still_hears_neither`, `hands_the_runs_own_folder_to_the_brief_for_a_teammates_run_and_for_nobodys`, `tells_a_run_in_its_own_copy_that_it_has_one_and_never_the_folders_name`
  - `worktrees.rs` skip HEAD verification -> `worktree_checkout_tests::requires_head_verification_to_succeed_before_accepting_a_nonzero_add`
  - `worktrees.rs` treat timeout as finished hook failure -> `worktree_checkout_tests::does_not_treat_a_timeout_as_a_finished_hook_failure`
  - `worktrees.rs` disable Windows longpaths -> `worktree_checkout_tests::adds_the_longpaths_flag_only_for_windows_on_both_branch_forms`

## Part 2: Reading the folder's changes — DONE (5/5 carried)
- `main/the-folders-changes-are-read.test.ts` (4/4 named in `core/src/changes.rs`):
  - Fixed `changes::new_file` to emit `@@ -0,0 +1 @@` for single-line files and `\ No newline at end of file\n` when the file does not end with `\n`.
  - Added `showing` and `raw` fields on `changes::Folder`.
- `main/the-changes-panel-reads-every-change.test.ts` (4/4 named in `core/src/changes.rs`):
  - All 4 cases (CH1–CH4) named and tested, including amended-commit fallback with `showing == "all"`.
- `main/the-history-reads-only-what-changed.test.ts` (4/4 named in `core/src/history_cache.rs`):
  - Added `core/src/history_cache.rs` (`HistoryCache`) caching parsed ledger summaries by `(size, mtime)`, preserving `event_count` and `waiting_question` (via `protocol_tags::parse_decision`) on light history rows (older than newest 20).
  - Wired `history_cache::global().list_recent_summaries` into `core/src/native.rs::list_recent` without modifying `core/src/ledger.rs`.
- `main/a-big-folder-compares-in-place.test.ts` (3/3 named in `src/ui/compare_view.rs`):
  - Extracted `resolve_changes_in` and `keep_needs_bring_in` and verified in-place `ChangesIn::Folder` fallback, Auto staying enabled, and `keep_column` skipping bring-in.
- `renderer/src/the-changes-panel-lists-every-file.test.tsx` (2/2 named in `src/ui/changes_panel.rs`):
  - Added `changes::TreeFolder` + `changes::tree_of` and replaced the flat list in `src/ui/changes_panel.rs` with a collapsible folder-first file tree (`render_file_tree`), verified in headless `rune::Runtime`.
- Mutation checks (`mutsafe.py`, 8/8 killed):
  - `changes.rs` single-line range always `+1,N` -> `changes::tests::adds_every_line_says_when_the_last_has_no_newline_and_calls_a_binary_file_binary`
  - `changes.rs` drop no-newline marker -> same test
  - `changes.rs` `showing` default empty -> `on_a_branch_its_commits_and_what_is_not_committed_yet_against_main_new_files_whole`, `shows_all_the_changes_for_a_commit_no_longer_on_the_branch_and_says_it_shows_all`
  - `changes.rs` `tree_of` flat (no folders) -> `changes_panel::tests::groups_them_by_folder_each_folders_own_files_under_it` (locust-native; survives a core-only run)
  - `history_cache.rs` never hit the cache -> `reads_no_file_twice_when_nothing_changed`, `reads_the_one_file_that_changed_and_the_answer_carries_the_change`
  - `history_cache.rs` drop `waiting_question` -> `keeps_its_question_on_light_history_rows_on_the_first_and_cached_reads`
  - `compare_view.rs` big plain folder goes to a copy -> `is_never_refused_for_its_size_it_works_in_the_folder_itself`, `the_menu_never_greys_auto_out_for_it`
  - `compare_view.rs` keep in place brings in -> `keeping_one_brings_nothing_in_its_changes_are_already_there`
- Surprise: case-map reads the comment `("b/caf\303\251.txt")` in the CH test's header as a case; it is listed as unnamed noise in `parity/cases.tsv`.

### Part 2 fix (after the first workspace run)
- The first `nextest run --workspace` after `e580f32c` failed two relay window tests (`relay_share_starts_recipient_and_returns_reply_to_origin_thread`, `relay_stops_at_hop_cap_and_exchange_strip_stop_ends_live_run`). `history_cache.rs` had its own copy of the ledger summary that had drifted from `ledger::try_list_recent`:
  - it read peer links from `peer.link`/`peerLink`, but the ledger writes `mission.peer`/`link`, so no ExchangeStrip was shown;
  - it cached `running` with the freshness of the moment it was read;
  - it chose the `limit` rows by `updated` instead of by file time.
- Fixed all three in `history_cache.rs` (`ledger.rs` is another lane's). `answers_exactly_what_the_whole_listing_answered` now compares whole `Summary` values (peer links included) on the first read and the cached one, and the picks under a small limit. Added `a_cached_run_stops_counting_as_running_once_its_file_has_been_quiet_for_the_window`.
- Breaks: peer-link record shape -> `answers_exactly…`; cached running -> `a_cached_run_stops…`; no limit -> `answers_exactly…`.

## Remaining
- Part 3: Committing (4 files)
- Part 4: Pull-request and issue links (7 files)
