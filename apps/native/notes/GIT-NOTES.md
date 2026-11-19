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

## Remaining
- Part 2: Reading the folder's changes (5 files)
- Part 3: Committing (4 files)
- Part 4: Pull-request and issue links (7 files)
