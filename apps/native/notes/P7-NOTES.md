# P7 — Changes, diffs and Undo (partial parity)

This copy implements the native path, but does **not** close the three composite Area 6 rows. The recount is **25 done, 24 partial, 104 missing, 153 total**. M5 is not reached. No approval implementation or approval-card changes were intentionally made.

## Implemented

- `src/changes.rs`: read-only branch/base diffs, local main/master and origin default-branch lookup, merge-base, branch commit selection, Unicode paths, `.locust` exclusion, untracked additions, binary/empty/mode-only/rename headers. The panel uses Electron's `folder-commit.ts` diff path, not its commit/status mutation path. No fetch, authentication, repository config write, index write or credential access is used by the panel.
- Git subprocesses prepend `-c core.fsmonitor=false`; diff/show prepend `-c core.quotePath=false`, and use `--no-color --no-ext-diff -M`, followed by `-- . :(exclude).locust`. Untracked files use `ls-files --others --exclude-standard -z` with those pathspecs. Bounds: 3 MiB diff, 200 untracked files, 512 KiB per untracked file, 100 commits; cut output is stated. Processes have a 30-second timeout. Repository-top probes and folder reads run through `Latest`, off the UI thread.
- `src/ui/changes_panel.rs`: folder-wide file list, extension badges, zero-free line counts, refresh, show/hide files, collapse/expand all, individual file folding, initially two hunks, explicit completeness/remainder text, three context rows on either side of edits, expandable context. Syntax highlighting reads the whole before/after sides independently, preserving multiline comments. Added/deleted rows have green/red backgrounds in both themes. Copy at the diff's foot copies new code from the hunks; Copy on the file reads the current file from the bound folder when pressed, with the existing 1 MiB UTF-8 reader limit. Deleted files offer neither copy. Cmd+Shift+Y and Escape are wired.
- `src/checkpoints.rs`: before/after byte snapshots, SHA-256 blobs under the checked native profile, per-turn records, 20,000-file/300-MiB total/10-MiB per-file bounds. Oversized files are named and excluded from restoration. Git-backed enumeration uses a separate bare listing store and the project's ignore rules; without git the walk skips dot-folders, dependencies and the same desktop temporary files as Electron. It refuses to snapshot a folder containing its own checkpoint store.
- Native window turns and CLI `drive` take the checkpoint before `runtime.start`, then save the after state after completion/failure/stop. Native conversation ownership stays held while checkpoint/memory writes finish. Ledger polling keeps the conversation busy until ownership is released, preventing a continuation from racing the after snapshot.
- Undo asks Electron's question and offers `Undo` / `Keep them`. It restores changed files only, leaves later edits/deletions alone with reasons, restores deleted binary/CRLF files byte for byte, removes turn-created files, and refuses overlapping native turns. Native profile writes use the existing checked, exclusive no-follow store API. Workspace restores anchor parent descriptors with `openat(O_DIRECTORY | O_NOFOLLOW)` on Unix and replace directory entries instead of truncating hard-linked inodes. A durable claim precedes mutation; an interrupted action is not retried. Completed receipts make repeat calls read-only. Non-Unix restore explicitly refuses rather than using an unsafe fallback.

## Remaining parity work — not represented as done

1. The file list is flat, not Electron's folder tree; badges are textual rather than language-coloured icons. There is no aggregate count in the header, commit author/age metadata, file jump scrolling, or full More-menu/context-menu parity. Probe failures currently hide Changes rather than preserving the button on an indeterminate failure. The full show-files chord matrix and tooltip wording are not tested.
2. Word-level paired marks, `paintRow`'s mismatch fallback, `documentChange.ts` passage rendering and large/truncated runtime-reported count handling are not ported. The panel currently colours code using Rune's grammars, not Electron's grammar package. Copying the OS clipboard and its feedback was not exercised live. Copy whole file has the existing native text-reader limit, not Electron's full file-copy behavior.
3. Checkpoint persistence is native SHA-256 blob/maps even when Git is installed; it does not reproduce Electron's git object/commit/ref format. `fileAt`, held-state identifiers, the 60-turn retention/pruning lifecycle, stat-cache optimization and ten-minute too-big cache are not ported. Checkpoint failure is reported to stderr, not a turn notice. Undo is accessible through the folder's panel and keyed to turn IDs; it is not placed under each thread turn, does not mark its activity-card heading undone, and does not notify the next runtime prompt that the preceding turn was undone. `Undoing…` asynchronous UI state and showing each left-alone reason after reopening the panel remain. Undo filesystem work still runs on the UI thread. The exclusive claim intentionally makes crash recovery fail closed rather than retrying.
4. Snapshot reads and git private-store initialization have not been subjected to concurrent directory-replacement fault injection; Unix restoration has descriptor-relative parent handling, but this is not a certification against every filesystem race. Cross-process writers are detected by before/after byte comparison, not by a cross-process turn lock. Native concurrent turns are marked shared.
5. `main/turn-checkpoint.ts` is Electron's **own-branch commit** lifecycle/notice formatter, distinct from byte checkpoints. It was read; it is not implemented here because own-branch commits/worktree lifecycle is a separate still-missing Area 6 row. No turn commits, git reset/checkout/clean or automatic repository mutation are introduced.

## TypeScript test accounting

Sources resolve under `/Users/locust/work/locust/apps/desktop/src` and were read as reference only. Existing files are not instructions. This is a behavioral subset port, with the unported assertions explicitly retained below; none are silently treated as passing.

| TS suite | Native evidence / remaining assertions |
| --- | --- |
| `renderer/src/diff.test.ts` | `changes::tests::{added_cursor_file_counts_numbers_ranges_and_copy, header_like_rows_and_next_file, ranges_derive_from_rows_and_completeness_never_silent, context_folds_exactly_and_never_hides_changes}` cover added/modified input, numbering, row-derived totals/ranges, dishonest counts, folding, completeness and non-silence. WordDiff, pairedSpans and fileRowCounts' reported/truncated combinations remain unported. |
| `renderer/src/a-change-can-be-copied.test.tsx` | `copies_after_code_with_parts_apart_and_no_deleted_code` covers separated hunks, new/deleted code behavior. Scene inspection covers visible copy controls. React static-markup assertions are inapplicable to Rune; equivalent action-level UI tests for file vs change clipboard content remain unported. |
| `renderer/src/a-change-is-coloured.test.tsx` | `a_change_is_coloured_as_two_whole_sides` covers before/after construction and valid syntax ranges; dark/light scenes show colours. Pair overlays, character-conservation/mismatch fallback and async React grammar rendering assertions remain unported (React scheduling itself does not apply). |
| `renderer/src/a-diff-is-not-read-out-of-a-files-content.test.ts` | `a_diff_is_not_read_out_of_a_files_content` and `header_like_rows_and_next_file` cover new-file diff-looking content, removed SQL comments, added ++ rows and two files. Approval-patch/toolPatchFrom wrapper integration belongs to approvals and was not changed. |
| `renderer/src/a-file-git-shows-without-lines-is-listed.test.ts` | `binary_empty_rename_mode_and_unicode_paths_stay_listed` covers binary/empty/rename/mode-only header records; mixed text files exercised by folder tests. Exact original fixture matrix is not fully replicated. |
| `renderer/src/a-count-says-only-what-changed.test.tsx` | `counts_only_say_what_changed` ports all four outputs: addition, deletion, mixed edit, neither; CSS class assertions do not apply. |
| `renderer/src/the-changes-panel-lists-every-file.test.tsx` | Row-derived counts covered by parser tests. `treeOf` grouping assertion remains unported because the UI list is flat. |
| `renderer/src/the-changes-panel-shows-its-files-with-cmd-on-a-mac.test.ts` | Native shortcut wired; source-string/React tooltip assertions do not apply. Behavioral modifier/platform matrix remains unported. |
| `renderer/src/a-document-change-reads-as-words.test.ts` | All three tests remain unported: passages, whole-word cuts, Markdown-as-read handling. Document passage mode is not claimed. |
| `renderer/src/a-new-document-reads-as-one.test.tsx` | Document prose presentation is outside this unified-diff implementation and remains missing. |
| `main/a-folder-without-git-offers-no-changes-panel.test.ts` | `a_folder_without_git_offers_no_changes` covers plain folder/repository/non-top subfolder. Explicit absent-git and indeterminate-error test seams remain unported. |
| `main/the-changes-panel-reads-every-change.test.ts` | `reads_master_merge_base_commits_untracked_unicode_and_excludes_locust` and `large_new_files_listed_and_new_file_limit_disclosed` cover accented paths, local master base, committed/untracked changes, invalid selected commit fallback, oversized/untracked-count cut. Amend-specific fixture remains unported. |
| `main/a-turn-can-be-undone.test.ts` | `checkpoints::tests::{restores_bytes_edits_additions_and_deletions_once, later_edits_and_removals_are_left_alone, overlapping_turns_are_never_offered_or_undone, git_ignore_and_raw_bytes_never_run_filters_or_fsmonitor, too_large_files_named_never_deleted_by_undo}` cover all seven behavioral cases, with native blob persistence instead of assertions on Electron git commit IDs. |
| `main/a-turn-can-be-undone-without-git.test.ts` | `without_git_walk_skips_dot_folders_and_dependencies` forces the no-git path; other byte/later-edit/oversize/record tests share its restore path. `fileAt`, stable plain state IDs, hold and git-vs-plain dispatch assertions remain unported. |
| `renderer/src/an-undone-turn-says-so-on-its-card.test.tsx` | Both assertions remain unported: activity-card heading and callback integration. Panel receipts alone do not satisfy them. |
| `main/a-teammate-hears-its-turn-was-undone.test.ts` | Runtime notice/prompt reconciliation remains unported. No claim of coverage. |
| `main/your-line-breaks-are-kept.test.ts` | This is conversation/room bubble CSS, not checkpoint byte preservation. Its CSS assertions do not apply to this change. CRLF and binary restoration are instead tested directly. |

Additional native safety coverage: `refuses_traversal_and_symlink_restore`. No new ignored tests or production `.unwrap()` calls were added.

## Verification, 10 November 2026

Run from this copy, with `PATH=$HOME/.cargo/bin:$PATH`, `LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js` and `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p7`:

- `cargo test --release` **three final runs: 360 passed, 0 failed, 13 ignored** each. The existing ignored tests require real runtimes/user data and were not requested or executed. Logs: `work/p7-checks/p7-final-test-{1,2,3}.log`. No test was hidden by a filter.
- `cargo build --release`: exit 0, **zero warnings** (`work/p7-checks/p7-final-build.log`).
- `cargo clippy --release`: exit 0, no findings in the new modules. Four existing `too_many_arguments` findings remain in `approval.rs::ask_claude`, `ledger_write.rs::create_runtime`, `native.rs::command_for_turn`, `native.rs::run_turn`. No suppression was added. Log: `work/p7-checks/p7-final-clippy.log`.
- `scripts/test-m1-cli.py` and `scripts/test-m3a-cli.py` against the release binary: exit 0, synthetic subprocess fixtures only. Logs: `work/p7-checks/p7-m1.log`, `work/p7-checks/p7-m3a.log`.
- Rendered and **looked at** Changes and Undo confirmation in dark/light, each `--size 900x900 --scale 1`: `/Users/locust/work/scratch/p7-snaps/{changes-dark,changes-light,undo-dark,undo-light}.png`. The question, oversize caveat, Undo/Keep them, syntax colours and copy controls are readable without clipping. These are deterministic scenes, not a live filesystem Undo.

The first development run failed on macOS `/var` versus `/private/var` normalization and a continuation/after-snapshot race. Both were fixed; the final three runs above include the fixes. All git mutation tests create their own temporary repositories. No live window or real turn was launched. Clipboard integration, real concurrent editors, crash/fault-injection, and Windows/Linux Undo were not verified. The three Area 6 rows remain partial for the gaps above.

## Review and fixes

Parity pass against Electron (`ChangesPanel.tsx`, `FileTypeIcon.tsx`, `LineCounts.tsx`, `changesKeys.ts`, `DiffView.tsx`, `diff.ts`, `diffColours.ts`, `documentChange.ts`, `TurnUndo.tsx`, `main/checkpoints.ts`, `main/checkpoints-plain.ts`, `main/turn-checkpoint.ts`, and the read-only git path in `main/folder-commit.ts`). Fixes landed in this copy only. No PARITY row was promoted to done. `turn-checkpoint.ts` remains the separate own-branch commit lifecycle and was not implemented.

### Undo safety

- Checkpoint `save` now writes beside then renames into place (Electron `writeWhole`). A corrupt leftover final name is replaced; a complete byte match is left alone. Covered by `save_replaces_corrupt_blob_atomically`.
- Restore-by-rename now `fchmod`s the replacement to the existing file’s mode so `+x` and similar bits survive Undo. Covered by `restore_keeps_existing_file_mode`.
- One unreadable path no longer aborts the whole before/after snapshot; it is skipped like Electron’s lstat/read catch. Covered by `unreadable_paths_are_skipped_not_fatal`.
- Ask words, later-edit leave-alone, claim/receipt, symlink refusal, and no git config/credential use were already matched; re-checked, left as-is.

### Changes panel words, git reads, no-git gating

- Empty diff copy splits clean vs unshowable: *Nothing has changed here since the branch left its base.* when there are no commits; *This commit changes no files Locust can show.* when commits exist but Locust has no files. Covered by `empty_diff_words_split_clean_and_unshowable_commit`.
- Panel closes when the bound folder is not a repository or the conversation folder changes; the thread view also requires repo probe true or still indeterminate (`main.rs` + `changes_panel::poll`).
- `git` keeps non-empty stdout on non-zero exit (Electron `printed` for diff/show). Covered by `git_keeps_stdout_when_diff_exits_nonzero`.
- Untracked symlinks to files are listed (follow like Electron `stat`/`readFile`). Covered by `untracked_symlink_to_file_is_listed`.

### Diff counts, colours, completeness, folds

- Completeness strings use Electron’s `hunk`/`hunks` and `line`/`lines` (`completeness` / `ranges_derive_from_rows_and_completeness_never_silent`).
- Line counts render as separate green `+N` / red `−M` marks (`LineCounts` parity) while keeping zero-free wording via `line_counts`.
- Diff gutters mute line numbers; only the sign is coloured; deletes use U+2212 `−`.
- Context unfolds are keyed per fold run `(file, hunk, start)` with Electron’s *N unchanged line(s)* labels (no “Show” prefix). Truncation completeness is only stated once every recorded hunk is shown.

### Left unchanged (still partial / deferred)

- Flat file list, badge colours, commit author/age, More menus, word-level paired marks, `documentChange` passages, Electron grammar package, turn-card “undone” heading, runtime undo notice, git object/ref checkpoint format, retention/`fileAt`/hold, live clipboard and live window/exit.

### Verification this review

```sh
PATH=$HOME/.cargo/bin:$PATH \
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p7 cargo test --release
# ×3 after last source edit → each: 366 passed; 0 failed; 13 ignored

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p7 cargo build --release
# zero compiler warnings

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p7 cargo clippy --release
# four existing too_many_arguments only (approval/ledger_write/native); none in P7 review modules

scripts/test-m1-cli.py /Users/locust/work/scratch/native-target-p7/release/locust-native
scripts/test-m3a-cli.py /Users/locust/work/scratch/native-target-p7/release/locust-native
# both exit 0

# Snaps → /Users/locust/work/scratch/p7-snaps/
# changes-dark, changes-light, undo-dark, undo-light at 900×900 scale 1
# Inspected: green/red +1 −1 marks, muted gutter + coloured −/+, Undo question with
# oversize caveat and Undo/Keep them, “The whole change is shown”, copy controls.
```

Logs: `work/p7-review/` (including `test-final-*.log`, `build-final.log`, `clippy-final.log`, `m1-cli.log`, `m3a-cli.log`).

Not verified in this review: live window with Electron quit, live OS Undo against a real turn, live clipboard, crash/fault injection on checkpoint writes, Windows/Linux restore, word-level marks, and the remaining TypeScript suites listed under Remaining parity work.

## 13 Nov: Undo off the window's thread, and the next turn told

- Undo's folder work runs in a job (`undo_job`); the button reads "Undoing…" while it runs, and the panel reads the
  records again afterwards, so the turn shows its receipt and no second Undo. Window test
  `undo_puts_a_turns_files_back_off_the_windows_thread_and_says_so` presses the real buttons.
- `core/src/undone_note.rs` ports `undone-note.ts`; `native::send_from` puts it before the runtime prompt of a follow-up
  (same runtime, not a side question, not sealed) when the turn it continues has an Undo receipt. The ledger keeps the
  person's words only.
- Still not ported: "· undone" on the turn's heading (the native thread has no files heading yet), Undo under each
  turn in the thread.
