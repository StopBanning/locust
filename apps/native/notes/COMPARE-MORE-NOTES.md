# Compare More Notes (Area 9 Parity)

Implemented the two remaining Area 9 Compare/Benchmarks PARITY rows in `apps/native`:

1. **Per-column rerun, build/merge answers, keep copies and preview pages**:
   - `core/src/compare.rs`: Added `RERUN_TIMES`, `RerunStep`, `RerunProgress`, `rerun_refusal_of`, `CompareStore::add_rerun`, `CompareStore::settle`, and fixed `CompareRoute::from_json` / `CompareStore::create` to preserve empty-string `instructions: Some("")` while rejecting `\0` or >4000 chars.
   - `core/src/compare_copies.rs`: Updated `discard_compare_worktrees` to read `git worktree list --porcelain` before pruning/removing so `locust/` branches are deleted even when the worktree folder was already removed; added `compare_page_path` and `resolve_compare_page_path` (checks copy first, refuses non-kept copy after Keep, falls back to workspace folder for the kept column).
   - `core/src/compare_reruns.rs`: Ported `main/compare-reruns.ts` (`CompareReruns`, `CompareRerunsHost`) for sequential 1× / 3× reruns, stop/settle handling, one-time retry of failed columns before judging, and single/panel judging.
   - `src/ui/compare_view.rs`: Added per-column `Try again` (`retry_column`, disabling `Keep this one` on failed columns), follow-up asking across columns (`ask_compare`), adding another model column (`add_compare_column`), HTML page preview (`column_html_pages`, `preview_column_page`), and `NativeRerunsHost` wiring (`start_benchmark_rerun`, `stop_benchmark_rerun`, `use_model_for_teammate`).

2. **Reusable benchmarks, record tables and across-benchmark analysis**:
   - `core/src/benchmark.rs`: Ported `renderer/src/benchmark.ts` and `renderer/src/compareRecord.ts` (`median`, `average_cost`, `benchmark_rows`, `benchmark_leader`, `across_benchmarks`, `benchmark_chart`, `compare_record_rows`, `ordered_benchmarks`, `step_words`, `cost_line`, `cost_cell`).
   - `src/ui/compare_view.rs`: Added `render_benchmark_section` (`YOUR BENCHMARK · N RUNS`, Run 1..N selector chips, `Run 3 more times` / `Run once more` / `Stop`, benchmark & record tables, cost/time summary).
   - `src/ui/benchmarks_panel.rs`, `src/ui/sidebar.rs`, `src/ui/app_state.rs`, `src/main.rs`: Ported `BenchmarksScreen.tsx` (`AcrossBenchmarks` table, ordered benchmark rows, sidebar `"Benchmarks"` nav button).

## Special Cases Verified (Break-Check)

Each special case was temporarily removed, verified to fail its test, and restored:
- **`compare_copies::resolve_compare_page_path` post-Keep non-kept copy refusal**: Removing the non-kept check caused `compare_copies::tests::a_kept_page_forgets_its_removed_copy` to fail (`Ok(...)` instead of `Err(...)`).
- **`compare_copies::discard_compare_worktrees` reading `worktree list --porcelain` before `prune`**: Moving `prune` before `worktree list --porcelain` and dropping the fallback branch name caused `compare_copies::tests::kept_comparison_leaves_no_branches_even_if_folder_removed_first` to fail.
- **`compare::CompareRoute::from_json` empty-string `instructions` preservation**: Dropping `""` to `None` caused `compare::tests::keeps_column_instructions_through_restart_followup_and_retry` and `compare_reruns::tests::keeps_each_columns_instructions_when_whole_comparison_runs_again` to fail (`None` instead of `Some("")`).
- **`compare_copies::bring_in_copy` refusal over user edits**: `BringIn::YourChanges` guard prevents overwriting modified workspace files on Keep.

## TypeScript Tests & `grep -l` Findings

- `apps/desktop/src/main/what-running-it-again-does.test.ts` (located via `grep -l`) is for `routine-runner.ts` and `Sidebar.tsx` search, not Compare; left untouched.
- Carried tests (11):
  - `a-comparison-runs-again.test.ts` -> `compare_reruns::tests::*` (11 unit tests) + `compare::tests::rerun_refusal_messages_match_electron`
  - `a-comparison-keeps-each-columns-instructions.test.ts` -> `compare_reruns::tests::keeps_each_columns_instructions_when_whole_comparison_runs_again` + `compare::tests::keeps_column_instructions_through_restart_followup_and_retry`
  - `a-column-can-be-tried-again.test.ts` -> `compare_window_tests::a_column_run_again_answers_again`
  - `a-failed-column-cannot-be-kept.test.tsx` -> `compare_window_tests::a_column_run_again_answers_again`
  - `a-copy-column-brings-its-changes-in.test.ts` -> `compare_copies::tests::a_compare_column_works_outside_the_folder` + `compare_window_tests::keep_this_one_brings_column_change_into_folder_and_other_copies_go`
  - `a-kept-column-brings-its-changes-in.test.ts` -> `compare_window_tests::keep_this_one_brings_column_change_into_folder_and_other_copies_go`
  - `a-kept-comparison-leaves-no-branches.test.ts` -> `compare_copies::tests::kept_comparison_leaves_no_branches_even_if_folder_removed_first`
  - `a-kept-page-forgets-its-removed-copy.test.ts` -> `compare_copies::tests::a_kept_page_forgets_its_removed_copy`
  - `a-benchmark-reads-every-run.test.tsx` -> `benchmark::tests::{benchmark_reads_every_run_and_averages_points,charts_cost_or_time_and_handles_two_free_models}` + `compare_window_tests::a_benchmark_of_two_runs_reads_as_one_table`
  - `across-your-benchmarks.test.tsx` -> `benchmark::tests::{across_your_benchmarks_averages_and_excludes_setups,keeps_each_runtime_in_its_own_unit}`
  - `every-benchmark-is-listed-in-one-place.test.tsx` -> `benchmark::tests::every_benchmark_is_listed_in_one_place` + `compare_window_tests::a_benchmark_of_two_runs_reads_as_one_table`

## Standalone Window Tests

All three new headless window tests were run individually via `python3 scripts/gentle-cargo.py test --release -p locust-native -- compare_window_tests::<name> --exact` and passed:
- `keep_this_one_brings_column_change_into_folder_and_other_copies_go`
- `a_column_run_again_answers_again`
- `a_benchmark_of_two_runs_reads_as_one_table`
