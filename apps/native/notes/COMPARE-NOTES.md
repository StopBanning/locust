# Compare, Blind, and Judges (`exec/native-compare-blind`)

Port of Electron's Compare/Blind side-by-side answers, isolated changes, weighted/drafted rubric points, and 1/3 blind judges to `apps/native`.

## What was built

- **`locust-core` (`apps/native/core/src/`)**
  - `compare.rs`: `Compare`, `CompareSlot`, `CompareSlotId`, `CompareRoute`, `CompareJudge`, `JudgePanelMember`, `ChangesIn`, `CompareStore` (`compares.json`), `compare_slot_key`, `compare_tree_id`, `changes_line`, `compares_gone_with`, `compare_membership`, `newest_judge_mission_ids`, `judge_mission_ids`, `rerun_mission_ids`, `compare_refusal_of`, `blind_name`, `versus_label`, `compare_record`, `judge_pick_of`, `with_column_folders`, `shuffled_columns`.
  - `compare_score.rs`: `ScoreVerdict`, `Scorecard`, `criteria_lines`, `criteria_weights`, `weighted`, `scorecard_request`, `parse_scorecard`, `without_scorecard`, `panel_vote`, `panel_pick`, `points_draft_request`, `drafted_points`, `score_of`.
  - `compare_copies.rs`: `compare_root`, `compare_root_from`, `copy_refusal`, `make_compare_copy`, `copy_changes`, `bring_in_copy`, `remove_compare_copies`, `copy_line_changes`, `bounded_copy_diff`, `copy_diff`, git worktree helpers (`is_git_repo`, `git_head`, `ensure_compare_worktree`, `discard_compare_worktrees`, `bring_in_compare_worktree`).
  - `compare_judge.rs`: `JudgedCheckOutcome`, `JudgedCheck`, `JudgedAnswer`, `JudgeChangesKind`, `judge_answer_text`, `same_failure_everywhere`, `judge_prompt`, `build_judge_brief`.
  - `native.rs`: `SlotOrigin { key, cwd, instructions, sealed }` on `Origin` for per-slot concurrency, isolated working folder override bypassing `own_branch_folder`, and sealed runs that omit teammate briefs/skills/browser tools.
  - `trash.rs`: `empty_trash` removes comparisons whose every answer mission is gone (`compares_gone_with`) along with their copies (`remove_compare_copies`).
- **UI (`apps/native/src/`)**
  - `ui/compare_view.rs`: `CompareComposerMode` (`Direct` / `Compare` / `Blind`), `composer_mode_chip`, `composer_mode_menu`, `composer_picks_row`, `start_comparison`, `keep_column`, `ask_judge`, `draft_points`, `poll`, side-by-side column cards, blind veiling until Keep, 1/3 blind judge view and weighted scorecard table.
  - Wired into `ui/app_state.rs`, `ui/thread_view.rs`, `ui/sidebar.rs` (filtering judge/rerun missions out of Recent and listing saved Comparisons), and `main.rs` (`can_send`, `send`, `poll`, `view`).
  - `ui/compare_window_tests.rs`: headless Rune window tests for Compare, Blind reveal on Keep, and 3-judge voting/pick.

## Special cases and verifying tests

1. **Column's slot (`SlotOrigin.key` / `compare_slot_key`)**: Each column and judge runs under its own slot key (`slot:<compareId>:<slot>`) so `owns(&slot.key)` gates concurrency per slot rather than per conversation. Tested by `native::tests::slot_busy_check_refuses_second_turn_in_same_slot` (fails if `owns(&slot.key)` check is removed in `native.rs`).
2. **Sealed slot (`SlotOrigin.sealed`)**: Blind columns and judges skip `people::brief`, teammate `skills`, and `browser_tools::Registration`. Tested by `native::tests::sealed_slot_omits_teammate_brief_and_skills` (fails if `sealed` is ignored in `native.rs`).
3. **Slot's own folder (`SlotOrigin.cwd`)**: When `cwd` is set on `SlotOrigin`, `native::send_from` runs in that copy/worktree folder and bypasses `own_branch_folder`. Tested by `native::tests::slot_cwd_bypasses_own_branch_folder` (fails if `slot.cwd` override is removed in `native.rs`).
4. **Auto carries over only when `changes == true`**: In `CompareStore::create` and `Compare::from_json`, `Mode::Auto` is downgraded to `Mode::Ask` unless `changes == true`. Tested by `compare::tests::auto_mode_carries_over_only_when_changes_is_true`.
5. **Empty file for git diff (`/dev/null` vs Windows `NUL`)**: Untracked/deleted files in `copy_line_changes` diff against `/dev/null` on Unix/macOS (matching Electron's `process.platform === 'win32' ? 'NUL' : '/dev/null'`; Windows would use `NUL` under `#[cfg(windows)]`). Tested by `compare_copies::tests::copy_diff_reads_git_and_plain_copies`.
6. **Trash cleanup (`compares_gone_with`)**: `trash::empty_trash` removes comparisons whose every answer mission was trashed and deletes their copies. Tested by `trash::tests::empty_trash_removes_compares_whose_answers_are_gone`.

## Electron TS tests ported

- `shared/the-judges-pick-is-marked.test.ts` -> `compare::tests::the_judges_pick_is_marked`
- `shared/a-judge-scores-each-answer.test.ts` -> `compare_score::tests::a_judge_scores_each_answer`
- `main/three-judges-are-kept.test.ts` -> `compare::tests::three_judges_are_kept`
- `renderer/three-judges-vote.test.tsx` -> `compare_score::tests::weighted_points_and_three_judges_vote` + `compare_window_tests::three_judges_score_the_answers_and_their_pick_is_marked`
- `main/a-compare-root-follows-the-environment.test.ts` -> `compare_copies::tests::a_compare_root_follows_the_environment`
- `main/a-compare-column-works-outside-the-folder.test.ts` -> `compare_copies::tests::a_compare_column_works_outside_the_folder`
- `main/a-judge-reads-the-answers-blind.test.ts` -> `compare_judge::tests::a_judge_reads_the_answers_blind`
- `main/a-judge-reads-each-columns-work.test.ts` -> `compare_judge::tests::a_judge_reads_each_columns_work`
- `main/a-long-judge-reads-files.test.ts` -> `compare_judge::tests::a_long_judge_reads_files`
- `main/judges-see-the-check.test.ts` -> `compare_judge::tests::judges_see_the_check`
- `renderer/a-blind-comparison-hides-the-names.test.tsx` -> `compare_window_tests::blind_comparison_hides_names_until_a_pick_then_shows_them`
- `renderer/a-compare-record-is-a-table.test.ts` -> `compare::tests::compare_record_counts_decided_comparisons`

## What was left out (separate PARITY rows)

- Per-column rerun (`main/compare-reruns.ts`)
- Build/merge answers and keep copies
- Comparison page previews
- Reusable benchmarks (`CompareBenchmark.tsx`, `BenchmarksScreen.tsx`)

## Review (13 Nov)

- `scripts/review-branch.py` passed: 1,118 tests and a clean release build. The three Compare window tests (in
  `compare_window_tests.rs`, which the script does not pick up) pass alone and together three times.
- **No compare run is sealed.** The brief said a blind column is sealed; that was the reviewer's mistake, not
  Hydrogen's. Electron gives a column a slot with its folder, instructions and skill, and runs a judge and the rubric
  draft in Ask with a slot of their own, none of them sealed (`index.ts` 7974, 8274, 8420). Only Scout's runs are
  sealed. The engine keeps `SlotOrigin.sealed` for Scout, with what it leaves out so far written beside it.
- The break check of "Auto carries over only when the column may change files" fails as the notes say.
