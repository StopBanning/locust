# Edit an earlier message, optionally putting files back (Electron 0.498, 0.502, 0.512)

Row: "Copy reply and edit earlier message, optionally restoring files" (area 1), now `done`, alone `yes`, live `no`.

## What was ported

Each message the person typed carries Edit (a pencil left of the bubble; not while a turn runs or a send is in flight,
and not on a turn an app or the host started, a route switch, or a bubble with notes on a diff). Its words go back in
the box under a line that says what sending will do, with Cancel (which empties the box). Sending starts the
conversation again from the turn BEFORE the edited one: a new branch, cold, told what was said up to there, recorded
with `continuesFrom.edited = true`. The turns after it are left exactly as they were and the earlier version is one
link away ("Show the version before" / "Show the edited version"). With the tick on, the files the set-aside turns
changed are put back first, all or none. Messages the set-aside turns posted and nobody read are never delivered.

| Piece | Where |
| --- | --- |
| Words the box and the thread use, version finder, set-aside messages | `core/src/edit_earlier.rs` |
| Put back all or none (plan, write, roll back, receipts) | `core/src/checkpoints.rs` `plan_put_back`, `put_back` |
| Start again from the turn before; cold brief; `edited` link; retire | `core/src/native.rs` `Rewind`, `send_from` |
| `edited` / `continues_reason` read from the ledger | `core/src/ledger.rs` (`Metadata`, `Summary`) |
| The wire form for the engine process | `core/src/engine_process.rs` (`rewind`) |
| Edit, line over the box, tick, Cancel, note, version links | `src/ui/edit_message.rs`, `messages.rs`, `thread_view.rs`, `main.rs` `send` |
| A fake agent scenario that fails the turn unless it is a cold start with the brief | `scripts/fixtures/m1-app-server.py` (`rewind`) |

Disk work never happens while drawing: the folder's records are read in `edit_offer_job` when Edit is pressed; the
put-back runs inside the send's own job, before the engine is asked to start; the other version of a turn is worked
out in `poll` from the recent list already in memory.

## Electron's special cases, where Rust holds them, and the test that fails when it is broken

| Special case | Rust line | Test that fails when broken |
| --- | --- | --- |
| An edit continues the turn before it and records itself as an edit | `native.rs` `rewind_link` (`"edited": true`) | `native::tests::an_edit_starts_again_from_the_turn_before_it_and_leaves_the_set_aside_turn_as_it_was` (broken: edited flag dropped, caught) |
| Cold start told what was said up to there, never a resume of the later session | `native.rs` `rewound = Some(turns_through(before))` | same test; the fake `rewind` scenario fails the turn without the brief (broken: brief dropped, caught) |
| Editing the FIRST message keeps the old conversation: nothing retired, no link | `native.rs` `.filter(\|r\| r.before.is_some())` | `an_edit_retires_what_set_aside_turns_sent_only_when_it_is_partway_through` (broken: filter removed, caught) |
| What set-aside turns sent unread is recorded as delivered to the turn that sent it | `edit_earlier.rs` `retire_set_aside_messages` | `a_message_the_set_aside_turns_sent_still_unread_is_never_handed_to_its_recipient`, `a_message_already_delivered_stays_as_it_was...`, `ignores_a_list_that_is_not_one` |
| All of them, or none | `checkpoints.rs` `put_back`: `if !plan.cannot.is_empty() { return held }` | `the_host_writes_nothing_when_any_file_cannot_go_back_and_says_which_could_have` (broken: guard disabled, caught) |
| A write that fails puts the written ones back | `checkpoints.rs` the `for undo in done.iter().rev()` loop | `a_write_that_fails_puts_the_ones_already_written_back_as_they_were` (broken: loop removed, caught) |
| Two turns' changes to one file go back to before the first | `checkpoints.rs` `plan_put_back` chains | `putting_back_returns_each_file_to_how_it_was_before_the_first_set_aside_turn` |
| Records that do not follow one another, shared folder, no record, changed or removed since | `plan_put_back` | `turns_whose_records_do_not_follow_one_another...`, `a_reply_that_shared_the_folder...`, `a_reply_with_no_record_of_its_changes...`, `a_file_removed_since_the_replies...` |
| No Edit while a turn runs, or on a turn the person did not type | `edit_message.rs` `editable_turn` | `a_sent_message_offers_edit_on_each_turn_the_person_typed_and_nothing_while_a_turn_runs_or_on_one_they_did_not_type` (broken: running check removed, caught) |
| The window names the turn before, the tip and the turns set aside | `edit_message.rs` `start` (`metas[at..]`) | `editing_puts_the_words_in_the_box_under_a_line...` (broken: `at + 1`, caught) |
| No tick when some file cannot go back, and the line says why | `edit_message.rs` `Offer::tickable`, `edit_earlier::editing_line` | `nothing_is_offered_to_tick_when_a_file_cannot_go_back...`, `the_window_offers_nothing_to_tick_when_some_cannot_go_back_and_says_why` |
| The host checks again at send: a file changed after the offer holds everything back, the edit still starts | `put_back` inside the send job | `a_file_changed_after_the_box_offered_it_holds_everything_back_and_the_edit_still_starts` |
| The edit refuses while the turn it replaces runs, and makes nothing | `native.rs` `owns(&rewind.tip)` | `an_edit_is_refused_while_the_turn_it_replaces_is_running` |
| Started-again note (and what went back, what was left, what was held) | `edit_earlier::started_again_note` | `the_note_above_the_new_turn_says_what_went_back...`, `sending_an_edit_starts_again...`, `the_tick_puts_the_replies_files_back...` |
| The version before an edit, both ways; two follow-ups that are not an edit | `edit_earlier::turn_versions_of` | the four `finds_...` / `says_so_above_the_message...` tests, and the window test that follows the link both ways |

Break checks done this round (file copied to `~/work/scratch/brk/`, line broken, named test confirmed failing, file
restored and `git diff` empty): first-message filter, all-or-none guard, rollback loop, `edited` flag, cold brief,
running check, set-aside slice.

## What differs, and what remains

- Put-back reads Locust's own per-turn checkpoint (before/after hashes and kept bytes), not the diffs Electron reverses
  line by line. The all-or-none rule and every refusal are the same; the wording of a refusal differs ("changed since
  the replies"). A turn with no native record (one run in Electron, or whose checkpoint was not kept) holds the whole
  set back with "no record of what it changed was kept". A file too large to keep a copy of is ignored, as Undo
  ignores it; Electron would hold the set back for a change "cut short".
- The put-back marks each turn whose files went back as undone (a receipt, `putBackByEdit: true`), so Undo has nothing
  left to do on it and the Changes panel shows it undone.
- The "Started again" note with what was put back lives in the window's memory (as in Electron); reopening the thread
  later says "Edited." with the way to the other version, which comes from the ledger. Writing the put-back into the
  ledger would add an unknown metadata key Electron's strict reader may refuse.
- Edit sits left of the bubble, not inside it. Edit is not offered on a bubble that carries notes on a diff; Electron
  also withholds it for attached files, which this window attaches differently (not checked).
- Antigravity-only refusal ("cannot start again from an earlier message") does not apply: the Rust engine starts every
  runtime cold from the brief.
- Not ported: advancing a teammate's relay hub to the new branch (`advanceHub`): the native relay keeps no hub record.
  The Electron-run 0.517 file-version viewer test (`an-earlier-version-reads-as-it-was.test.ts`, `fileAsItWas`)
  belongs to the Changes panel's per-turn file versions, not to edit; it stays `waiting`.
- Not seen in the real window (live `no`): all of this ran headless, on the fake agent.
