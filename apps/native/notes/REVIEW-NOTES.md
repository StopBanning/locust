# Review notes on a diff (Hydrogen, reviewed and finished 14 Nov 2026)

A note left on a line of a teammate's diff in the Changes panel (or Review changes, for the teammate whose conversation
is on screen) waits on one tile above the message box, goes with the next message as one block ("Notes on your
changes (N):"), and is drawn under the sent bubble with its place, relative to the conversation's folder, and what the
turn that read it did there: Line changed, File changed, not this line, File deleted, or Not changed.

## Built

- `core/src/diff_notes.rs`: `edited_files` (missionView `editedFiles`: event patches, an observed edit outranking a
  step diff, the turn's checkpoint outranking a tool's patch, one file however its case or leading `./` was written),
  `split_diff_notes`, `note_outcome`, `note_outcome_words`.
- `core/src/checkpoints.rs`: `turn_diffs` (a turn's own before/after as hunks) and `turn_recorded`.
- `core/src/ledger.rs`: `Snapshot.turn_edits`, each finished turn's files.
- `src/ui/diff_notes_ui.rs`: the note box, the tile, the sent notes with their outcome chips.
- `core/src/room_task.rs`: `board-is-not-behind.test.ts` (`rowNamedByPost`).

## Found in review

- **A turn's outcome was "Not changed" when it had changed the line.** The engine finishes a turn's checkpoint just
  after the turn's last ledger record; a poll in between read the turn as ended, found no diff and cached "nothing
  changed" for good. The snapshot now keeps a turn's files only once its checkpoint is on disk
  (`checkpoints::turn_recorded`); until then a note wears no chip, never a wrong one.
  `a_turn_read_as_ended_before_its_checkpoint_is_written_wears_no_outcome_until_it_is` fails without it. The window
  test now waits for the turn's diff too.
- Nine breaks no test caught, each now with its test (diff_notes_tests.rs, "Review (14 Nov)"): path case and `./`, the
  checkpoint outranking a tool's patch, an observed edit known from any of its call's events, a removed-line note said
  against the file, only removed rows (not context) changing a note's line, the 1,000-character cap, a notes block
  only after a blank line, a block with a stray row left as the person's words, and the checkpoint race above.
- The parity rows were not marked; now carried: notes-on-a-diff (7), review-notes-survive (5), place (4),
  note-in-the-changes-panel (2), board-is-not-behind (7).

## Left

- Notes on a browser page point (`PagePointChips`) wait for the browser's point-at-page feature.
