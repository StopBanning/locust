# Ratchet notes

## ratchet-1: five Electron test files carried (19 Nov 2026)

Five rows in `parity/tests.tsv` that read `waiting` now read `ratchet-1 / carried`. The Rust behaviour already
existed; these are the Rust tests that pin it, one per Electron case, named after the case's own words.

| Electron file | Rust tests |
| --- | --- |
| `thinking-is-not-using-a-tool.test.ts` | `core/src/thinking_is_not_a_tool_tests.rs` |
| `thought-folds-to-one-line.test.ts` | `core/src/thought_folds_to_one_line_tests.rs` (the ledger), `src/ui/thought_folds_to_one_line_tests.rs` (drawn headless) |
| `a-read-row-names-the-file.test.ts` | `core/src/a_read_row_names_the_file_tests.rs` |
| `a-refused-command-never-ran.test.ts` | `core/src/a_refused_command_never_ran_tests.rs` |
| `two-files-with-one-name-stay-two.test.ts` | `core/src/two_files_with_one_name_stay_two_tests.rs` |

The judge (`~/work/scratch/ratchet1/breaks.txt`, 11 breaks) reports every line KILLED in one of the two `mut6.py`
runs. The five lines that survived before this batch are each killed by a new test:

- `thread_view.rs` reasoning register: `thinking_is_not_a_tool_tests::reads_thinking_for_a_reasoning_item`
- `activity_drawers.rs` `.filter(|ms| *ms >= 500)`: `thought_folds_to_one_line_tests::reads_as_one_line_and_says_nothing_about_time_it_cannot_vouch_for`
- `activity_drawers.rs` `Thought for {}`: the same test
- `activity_trace.rs` `{count} {state}`: `a_refused_command_never_ran_tests::is_counted_as_refused_not_as_run_or_as_exiting_non_zero`
- `activity_entries.rs` restatement signature: `two_files_with_one_name_stay_two_tests::are_two_rows_each_with_its_own_change`

No source behaviour changed: only new test files, their one-line `mod` includes, the five parity rows and this note.
