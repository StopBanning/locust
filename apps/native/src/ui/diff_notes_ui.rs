//! Notes on a diff, in the window (Electron `DiffNotes.tsx`, `SentNotes.tsx` and the Composer's notes tile; the words
//! are `locust_core::diff_notes`). A "+" on a line of the Changes panel pins a note there; the notes wait as one tile
//! above the message box; the next message carries them all; and the sent bubble shows each one where it was.

use super::*;
use locust_core::changes::Row;
use locust_core::diff_notes::{self, DiffNote, SentDiffNote};

#[derive(Default)]
pub(super) struct Notes {
    /// The notes waiting for the next message.
    pub waiting: Vec<DiffNote>,
    /// The line a note is being written on: its path and row.
    pub writing: Option<(String, Row)>,
    pub draft: TextEdit,
}

/// A note can be left only where it will be sent: a conversation is on screen to send it in.
pub(super) fn may_note(s: &State) -> bool {
    selected_conversation(s).is_some()
}

/// The "+" at the end of a diff line, and the line's note under it once there is one.
pub(super) fn row_extras(s: &State, path: &str, line: &Row, p: &Palette) -> (Option<El>, Option<El>) {
    if !may_note(s) {
        return (None, None);
    }
    let key = diff_notes::diff_note_key(path, line);
    let place = diff_notes::diff_note_place(&diff_notes::shown_path(path, s.changes.folder.to_str()), if line.kind == '-' { line.old } else { line.new.or(line.old) }, line.kind);
    let (owned_path, owned_row) = (path.to_owned(), line.clone());
    let add = text("+")
        .font(type_::LABEL)
        .color(p.muted)
        .px(6.0)
        .hover(p.raised)
        .radius(4.0)
        .key(format!("diff-note-add-{key}"))
        .role("button")
        .label(format!("Add a note on {place}"))
        .on_press(move |s: &mut State| {
            let existing = s.diff_notes.waiting.iter().find(|n| n.key == diff_notes::diff_note_key(&owned_path, &owned_row)).map(|n| n.text.clone());
            s.diff_notes.draft = TextEdit::new(existing.as_deref().unwrap_or(""));
            s.diff_notes.draft.focused = true;
            s.diff_notes.writing = Some((owned_path.clone(), owned_row.clone()));
        });
    let writing_here = s.diff_notes.writing.as_ref().is_some_and(|(wp, wr)| diff_notes::diff_note_key(wp, wr) == key);
    let under = if writing_here {
        Some(
            row()
                .w_full()
                .gap(8.0)
                .px(8.0)
                .py(4.0)
                .child(
                    text_input(&s.diff_notes.draft, type_::BODY)
                        .key("diff-note-input")
                        .label(format!("Note on {place}"))
                        .grow(1.0)
                        .padding(6.0)
                        .radius(6.0)
                        .fill(p.field)
                        .color(p.text)
                        .on_edit(|s: &mut State| &mut s.diff_notes.draft)
                        .on_submit(|s: &mut State, _tx: &mut Tx| save(s)),
                )
                .child(text("Add note").font(type_::LABEL).color(p.bg).fill(p.text).px(10.0).py(5.0).radius(6.0).key("diff-note-save").role("button").on_press(save))
                .child(text("Cancel").font(type_::LABEL).color(p.secondary).px(10.0).py(5.0).radius(6.0).border(1.0, p.hairline).key("diff-note-cancel").role("button").on_press(|s: &mut State| s.diff_notes.writing = None)),
        )
    } else {
        s.diff_notes.waiting.iter().find(|n| n.key == key).map(|n| {
            row()
                .w_full()
                .px(8.0)
                .py(3.0)
                .fill(p.raised)
                .key(format!("diff-note-{key}"))
                .child(text(n.text.clone()).font(type_::BODY).color(p.text).wrap())
        })
    };
    (Some(add), under)
}

fn save(s: &mut State) {
    let Some((path, row)) = s.diff_notes.writing.take() else { return };
    let words = s.diff_notes.draft.text.trim().to_owned();
    if words.is_empty() {
        // An emptied note takes the note back.
        let key = diff_notes::diff_note_key(&path, &row);
        s.diff_notes.waiting.retain(|n| n.key != key);
        return;
    }
    let note = diff_notes::diff_note_for(&path, &row, &words);
    s.diff_notes.waiting = diff_notes::with_note(&s.diff_notes.waiting, note);
}

/// The tile above the message box, as an attached file is: how many notes, and a way to take them all back.
pub(super) fn tile(s: &State, p: &Palette) -> Option<El> {
    let count = s.diff_notes.waiting.len();
    if count == 0 {
        return None;
    }
    Some(
        row()
            .gap(8.0)
            .center_y()
            .px(10.0)
            .py(5.0)
            .radius(999.0)
            .border(1.0, p.hairline)
            .key("diff-notes-tile")
            .child(text(diff_notes::diff_notes_tile(count)).font(type_::LABEL).color(p.text))
            .child(text("×").font(type_::LABEL).color(p.muted).key("diff-notes-clear").role("button").label("Take the notes back").on_press(|s: &mut State| s.diff_notes.waiting.clear())),
    )
}

/// The message with the waiting notes, and the notes let go: the next message carries them all.
pub(super) fn take_into(s: &mut State, message: String) -> String {
    let notes = std::mem::take(&mut s.diff_notes.waiting);
    s.diff_notes.writing = None;
    diff_notes::with_diff_notes(&message, &notes)
}

/// A sent message's own words, and its notes drawn under it: each with where it was (relative to the folder, the
/// whole path on hover) and what the person wrote.
pub(super) fn sent_notes(notes: &[SentDiffNote], folder: Option<&str>, p: &Palette) -> El {
    let mut list = col().gap(4.0).key("sent-notes");
    for note in notes {
        let place = match note.line {
            Some(n) => format!("{}:{n}", diff_notes::shown_path(&note.path, folder)),
            None => diff_notes::shown_path(&note.path, folder),
        };
        list = list.child(
            row()
                .gap(8.0)
                .center_y()
                .child(text(place).font(type_::LABEL.mono()).color(p.muted).label(note.path.clone()))
                .child(text(note.text.clone()).font(type_::BODY).color(p.text).wrap().shrink()),
        );
    }
    list
}
