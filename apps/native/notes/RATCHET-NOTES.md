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

## Ratchet 002 (20 Nov 2026)
- **Who did what.**
  - Row 1 (a message is delivered once) was done by opencode/deepseek-v4.1-flash-free through the Rust door (8df71146).
  - The model then stalled twice on row 2. The cause was OpenCode's free quota ("Free usage exceeded, subscribe to
    Go"), with OpenCode retrying on its own while the door kept saying "Still working".
  - mimo-v2.6-flash-free hit the same quota at once. So rows 2-4 were written by the Mac chat (Opus) from the same
    plan.
  - The door now says a held-up run ("Still working, but held up: …", door-held-1). The stumbles are in
    `~/work/scratch/ratchet2/STUMBLES.md`.
- **Row 1, checked against Electron's own code.** A reply read again posts no second workroom message, but it does
  relay again. Electron's `share` does the same (its dedup is within one reply), so the test holds what both apps do.
  Whether a re-read should relay at all is a question for both apps, not a port difference.
- **Row 2.**
  - A turn whose reply is a `<locust-file>` or `<locust-ask>` block, a plan, or a summarized conversation is not
    "ended without a reply".
  - A turn with nothing, or only spaces, still is.
  - The summarized conversation's own line is shown.
  - Rust's Transcript has no latest-turn flag, so Electron's `latestTurn: false` runs are each tested once.
- **Row 3.** OpenCode's mode refusals name the tool (bash, edit, write, patch) and say "Nothing was changed"; the
  folder refusal names the folder; only an empty stderr says the vague sentence.
- **Row 4.** Against real git: a repository whose `core.fsmonitor` names a program is looked at, and a teammate's
  worktree made and removed, without running it. The control proves plain `git status` does run it.
