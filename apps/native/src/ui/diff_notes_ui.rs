//! Notes on a diff, in the window (Electron `DiffNotes.tsx`, `SentNotes.tsx` and the Composer's notes tile; the words
//! are `locust_core::diff_notes`). A "+" on a line of the Changes panel pins a note there; the notes wait as one tile
//! above the message box; the next message carries them all; and the sent bubble shows each one where it was.

use super::*;
use locust_core::changes::{File, Row};
use locust_core::diff_notes::{self, DiffNote, NoteOutcome, SentDiffNote};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Notes {
    /// The notes waiting for the next message in the active conversation.
    pub waiting: Vec<DiffNote>,
    /// The line a note is being written on: its path and row.
    pub writing: Option<(String, Row)>,
    pub draft: TextEdit,
    /// Notes parked by conversation key when switching teammates (Electron `App.tsx` `diffNotes`).
    pub by_conversation: HashMap<String, Vec<DiffNote>>,
}

impl Notes {
    pub(super) fn put(&mut self, key: &str) {
        let notes = std::mem::take(&mut self.waiting);
        self.writing = None;
        if notes.is_empty() {
            self.by_conversation.remove(key);
        } else {
            self.by_conversation.insert(key.to_owned(), notes);
        }
    }

    pub(super) fn switch(&mut self, next: Option<String>) {
        self.writing = None;
        self.waiting = next
            .and_then(|k| self.by_conversation.remove(&k))
            .unwrap_or_default();
    }
}

/// A note can be left only where it will be sent: a conversation is on screen to send it in.
pub(super) fn may_note(s: &State) -> bool {
    selected_conversation(s).is_some()
}

fn where_on_line(line: &Row) -> String {
    let n = if line.kind == '-' { line.old } else { line.new.or(line.old) };
    match (line.kind, n) {
        ('-', Some(n)) => format!("removed line {n}"),
        ('-', None) => "removed line ".to_owned(),
        (_, Some(n)) => format!("line {n}"),
        (_, None) => "line ".to_owned(),
    }
}

/// The "+" at the end of a diff line, the line's note (or editor) under it, and whether the line already holds a note.
pub(super) fn row_extras(
    s: &State,
    folder: Option<&str>,
    allowed: bool,
    path: &str,
    line: &Row,
    p: &Palette,
) -> (Option<El>, Option<El>, bool) {
    if !allowed {
        return (None, None, false);
    }
    let key = diff_notes::diff_note_key(path, line);
    let line_no = if line.kind == '-' { line.old } else { line.new.or(line.old) };
    let shown = diff_notes::shown_path(path, folder);
    let place = diff_notes::diff_note_place(&shown, line_no, line.kind);
    let where_part = where_on_line(line);
    let existing = s.diff_notes.waiting.iter().find(|n| n.key == key).cloned();
    let has_note = existing.is_some();
    let writing_here = s
        .diff_notes
        .writing
        .as_ref()
        .is_some_and(|(wp, wr)| diff_notes::diff_note_key(wp, wr) == key);

    let add = (!writing_here).then(|| {
        let (owned_path, owned_row) = (path.to_owned(), line.clone());
        text("+")
            .font(type_::LABEL)
            .color(p.muted)
            .px(6.0)
            .hover(p.raised)
            .radius(4.0)
            .key(format!("diff-note-add-{key}"))
            .role("button")
            .label(format!("Add a note on {place}"))
            .on_press(move |s: &mut State| {
                let text_now = s
                    .diff_notes
                    .waiting
                    .iter()
                    .find(|n| n.key == diff_notes::diff_note_key(&owned_path, &owned_row))
                    .map(|n| n.text.clone());
                s.diff_notes.draft = TextEdit::new(text_now.as_deref().unwrap_or(""));
                s.diff_notes.draft.focused = true;
                s.diff_notes.writing = Some((owned_path.clone(), owned_row.clone()));
            })
    });

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
                        .min_w(0.0)
                        .shrink()
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
        existing.map(|n| {
            let (edit_path, edit_row) = (path.to_owned(), line.clone());
            let edit_text = n.text.clone();
            let remove_key = key.clone();
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .px(8.0)
                .py(3.0)
                .fill(p.raised)
                .key(format!("diff-note-{key}"))
                .child(text(n.text).font(type_::BODY).color(p.text).wrap().shrink().grow(1.0))
                .child(
                    text("Edit")
                        .font(type_::LABEL)
                        .color(p.muted)
                        .key(format!("diff-note-edit-{key}"))
                        .role("button")
                        .on_press(move |s: &mut State| {
                            s.diff_notes.draft = TextEdit::new(&edit_text);
                            s.diff_notes.draft.focused = true;
                            s.diff_notes.writing = Some((edit_path.clone(), edit_row.clone()));
                        }),
                )
                .child(
                    text("Remove")
                        .font(type_::LABEL)
                        .color(p.muted)
                        .key(format!("diff-note-remove-{key}"))
                        .role("button")
                        .label(format!("Remove the note on {where_part}"))
                        .on_press(move |s: &mut State| {
                            s.diff_notes.waiting.retain(|entry| entry.key != remove_key);
                        }),
                )
        })
    };
    (add, under, has_note)
}

/// Append one diff line to `list`, adding its "+" button, "has-note" marker, and inline note/editor when allowed.
pub(super) fn append_noted_row(
    mut list: El,
    s: &State,
    folder: Option<&str>,
    allowed: bool,
    path: &str,
    r: &Row,
    line: El,
    p: &Palette,
) -> El {
    let (add, under, has_note) = row_extras(s, folder, allowed, path, r, p);
    let row_el = match add {
        Some(add) => row().w_full().center_y().child(line.grow(1.0).shrink()).child(add),
        None => line,
    };
    let row_el = if has_note {
        let key = diff_notes::diff_note_key(path, r);
        row_el.border(1.0, p.amber).key(format!("has-note-{key}"))
    } else {
        row_el
    };
    list = list.child(row_el);
    if let Some(under) = under {
        list = list.child(under);
    }
    list
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
            .child(
                text("×")
                    .font(type_::LABEL)
                    .color(p.muted)
                    .key("diff-notes-clear")
                    .role("button")
                    .label("Take the notes back")
                    .on_press(|s: &mut State| s.diff_notes.waiting.clear()),
            ),
    )
}

/// The message with the waiting notes, and the notes let go: the next message carries them all.
pub(super) fn take_into(s: &mut State, message: String) -> String {
    let notes = std::mem::take(&mut s.diff_notes.waiting);
    s.diff_notes.writing = None;
    diff_notes::with_diff_notes(&message, &notes)
}

fn outcome_color(outcome: NoteOutcome, p: &Palette) -> Color {
    match outcome {
        NoteOutcome::Line => p.green,
        NoteOutcome::File => p.amber,
        NoteOutcome::Deleted => p.red,
        NoteOutcome::Untouched => p.muted,
    }
}

/// A sent message's notes drawn under it: each with where it was (relative to the folder, the whole path on hover),
/// what became of it once the turn finished (`edited.is_some()`), optional quoted code, and what the person wrote.
pub(super) fn sent_notes(
    notes: &[SentDiffNote],
    edited: Option<&[File]>,
    folder: Option<&str>,
    p: &Palette,
) -> El {
    let mut list = col()
        .gap(6.0)
        .key("sent-notes")
        .label(diff_notes::diff_notes_tile(notes.len()));
    for (i, note) in notes.iter().enumerate() {
        let shown = diff_notes::shown_path(&note.path, folder);
        let place = match note.line {
            Some(n) => format!("{shown}:{n}"),
            None => shown,
        };
        let mut head = row()
            .gap(8.0)
            .center_y()
            .child(
                text(place)
                    .font(type_::LABEL.mono())
                    .color(p.muted)
                    .key(format!("sent-note-place-{i}"))
                    .label(note.path.clone()),
            );
        if let Some(files) = edited {
            let outcome = diff_notes::note_outcome(note, files);
            let words = diff_notes::note_outcome_words(outcome);
            head = head.child(
                el().px(6.0)
                    .py(1.5)
                    .radius(4.0)
                    .fill(p.raised)
                    .border(1.0, p.hairline)
                    .key(format!("sent-note-outcome-{i}"))
                    .child(text(words).font(type_::LABEL).color(outcome_color(outcome, p))),
            );
        }
        let mut card = col().gap(2.0).child(head);
        if let Some(code) = note.code.as_deref().filter(|c| !c.trim().is_empty()) {
            card = card.child(
                text(code.to_owned())
                    .font(type_::LABEL.mono())
                    .color(p.secondary)
                    .truncate(),
            );
        }
        card = card.child(text(note.text.clone()).font(type_::BODY).color(p.text).wrap());
        list = list.child(card);
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_core::changes;
    use locust_core::worktrees::BranchReview;
    use rune::{Env, Runtime};
    use std::path::PathBuf;

    const SAMPLE_DIFF: &str = "\
diff --git a/src/app.ts b/src/app.ts
--- a/src/app.ts
+++ b/src/app.ts
@@ -10,3 +10,3 @@
 import { env } from \"./env\"
-const port = 3000
+const port = 3001
 serve(port)
";

    fn render_view(state: State, build: fn(&State, &mut Cx<State>) -> El) -> Runtime<State> {
        let mut rt = Runtime::new(state, build, (1100.0, 900.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt
    }

    fn click_key(rt: &mut Runtime<State>, key: &str) {
        let (x, y) = rt
            .find(key)
            .unwrap_or_else(|| panic!("no element `{key}` on screen"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.016);
    }

    fn state_with_teammate(id: &str, name: &str, folder: Option<&str>, active: bool) -> State {
        let mut s = State::default();
        s.teammates.push(Teammate {
            id: id.into(),
            name: name.into(),
            role: "Builder".into(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "folder".into(),
            folder: folder.map(str::to_owned),
        });
        if active {
            s.conversations.insert(
                id.into(),
                Conversation {
                    conversation_id: Some(format!("conv-{id}")),
                    turns: vec![("You".into(), "hi".into())],
                },
            );
        }
        s
    }

    fn state_with_changes(folder: &str, file: changes::File, active: bool) -> State {
        let mut s = state_with_teammate("tm1", "Wren", Some(folder), active);
        s.changes.open = true;
        s.changes.folder = PathBuf::from(folder);
        s.changes.diff = Some(Ok(changes::Folder {
            base: "main".into(),
            branch: "main".into(),
            files: vec![file],
            truncated: false,
            commits: Vec::new(),
        }));
        s
    }

    fn render_changes_panel(s: &State, _cx: &mut Cx<State>) -> El {
        let p = Palette::new(true);
        col().w_full().h_full().child(crate::changes_panel::view(s, &p))
    }

    fn render_review_panel(s: &State, _cx: &mut Cx<State>) -> El {
        let p = Palette::new(true);
        col().w_full().h_full().children(crate::review_changes::panel(s, &p))
    }

    fn render_tile(s: &State, _cx: &mut Cx<State>) -> El {
        let p = Palette::new(true);
        col().w_full().h_full().children(tile(s, &p))
    }

    #[test]
    fn notes_on_a_diff_offers_a_note_on_every_line_where_one_can_be_sent_and_nowhere_else() {
        let file = changes::parse(SAMPLE_DIFF).remove(0);
        let row_count = file.hunks[0].rows.len();

        let with_conv = render_view(state_with_changes("/work/shop", file.clone(), true), render_changes_panel);
        let els = with_conv.elements();
        let adds: Vec<_> = els
            .iter()
            .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("diff-note-add-")))
            .collect();
        assert_eq!(adds.len(), row_count);
        assert!(
            els.iter()
                .any(|e| e.label.as_deref() == Some("Add a note on src/app.ts, line 11"))
        );

        let without_conv = render_view(state_with_changes("/work/shop", file, false), render_changes_panel);
        assert!(
            !without_conv
                .elements()
                .iter()
                .any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("diff-note-add-")))
        );
    }

    #[test]
    fn notes_on_a_diff_shows_a_note_under_its_line_and_marks_the_line() {
        let file = changes::parse(SAMPLE_DIFF).remove(0);
        let added = file.hunks[0]
            .rows
            .iter()
            .find(|r| r.kind == '+')
            .unwrap()
            .clone();
        let key = diff_notes::diff_note_key("src/app.ts", &added);
        assert_eq!(key, "src/app.ts:add::11");

        let mut s = state_with_changes("/work/shop", file, true);
        s.diff_notes.waiting = vec![diff_notes::diff_note_for("src/app.ts", &added, "Read it from PORT.")];
        let mut rt = render_view(s, render_changes_panel);

        assert!(rt.find(&format!("has-note-{key}")).is_some());
        assert!(rt.find(&format!("diff-note-{key}")).is_some());
        assert!(
            rt.elements()
                .iter()
                .any(|e| e.text.as_deref() == Some("Read it from PORT."))
        );
        assert!(
            rt.elements()
                .iter()
                .any(|e| e.label.as_deref() == Some("Remove the note on line 11"))
        );

        click_key(&mut rt, &format!("diff-note-edit-{key}"));
        assert!(rt.find("diff-note-input").is_some());
        click_key(&mut rt, "diff-note-cancel");
        assert!(rt.find("diff-note-input").is_none());

        click_key(&mut rt, &format!("diff-note-remove-{key}"));
        assert!(rt.state.diff_notes.waiting.is_empty());
        assert!(rt.find(&format!("has-note-{key}")).is_none());
    }

    #[test]
    fn notes_on_a_diff_holds_the_notes_as_one_tile_that_says_how_many_and_none_when_there_are_none() {
        let file = changes::parse(SAMPLE_DIFF).remove(0);
        let added = file.hunks[0].rows.iter().find(|r| r.kind == '+').unwrap().clone();
        let removed = file.hunks[0].rows.iter().find(|r| r.kind == '-').unwrap().clone();

        let empty_rt = render_view(State::default(), render_tile);
        assert!(empty_rt.find("diff-notes-tile").is_none());

        let mut one = State::default();
        one.diff_notes.waiting = vec![diff_notes::diff_note_for("src/app.ts", &added, "a")];
        let one_rt = render_view(one, render_tile);
        assert!(one_rt.elements().iter().any(|e| e.text.as_deref() == Some("1 note on the changes")));

        let mut two = State::default();
        two.diff_notes.waiting = vec![
            diff_notes::diff_note_for("src/app.ts", &added, "a"),
            diff_notes::diff_note_for("src/app.ts", &removed, "b"),
        ];
        let mut two_rt = render_view(two, render_tile);
        assert!(two_rt.elements().iter().any(|e| e.text.as_deref() == Some("2 notes on the changes")));

        click_key(&mut two_rt, "diff-notes-clear");
        assert!(two_rt.state.diff_notes.waiting.is_empty());
        assert!(two_rt.find("diff-notes-tile").is_none());
    }

    #[test]
    fn review_notes_survive_a_revision_are_drawn_under_the_bubble_each_with_where_it_was_and_what_became_of_it_once_the_turn_is_over() {
        let revision = changes::parse(
            "\
diff --git a/src/app.ts b/src/app.ts
--- a/src/app.ts
+++ b/src/app.ts
@@ -11,3 +11,3 @@
 import { env } from \"./env\"
-const port = 3000
+const port = Number(env.PORT ?? 3000)
 serve(port)
diff --git a/src/old.ts b/src/old.ts
deleted file mode 100644
--- a/src/old.ts
+++ /dev/null
@@ -1,1 +0,0 @@
-legacy()
",
        );
        let mk = |path: &str, line: usize, code: &str, text: &str, kind: char| DiffNote {
            key: format!("{path}:{line}"),
            path: path.into(),
            line: Some(line),
            kind,
            code: code.into(),
            text: text.into(),
        };
        let message = diff_notes::with_diff_notes(
            "Tidy these up, please.",
            &[
                mk("src/app.ts", 12, "const port = 3000", "Read the port from the environment.", '+'),
                mk("src/app.ts", 40, "log(everything)", "Too noisy.", '+'),
                mk("README.md", 3, "Run it with npm", "Say pnpm.", '+'),
                mk("src/old.ts", 7, "legacy()", "Is this still called?", '-'),
            ],
        );

        fn make_ledger_state(message: String, turn_edit: Option<Vec<changes::File>>) -> State {
            let mut s = state_with_teammate("tm1", "Wren", Some("/work/shop"), false);
            let mut snap = ledger::Snapshot::default();
            snap.items.push(ledger::Item::User(message));
            snap.turn_metas.push(ledger::Metadata {
                mission_id: "m1".into(),
                prompt: String::new(),
                ..Default::default()
            });
            snap.turn_edits.insert("m1".into(), turn_edit);
            s.ledger = Some(LedgerPane {
                header: s.teammates[0].clone(),
                conversation: Conversation {
                    conversation_id: Some("m1".into()),
                    turns: Vec::new(),
                },
                items: snap.items.clone(),
                worker: None,
                dir: PathBuf::from("/tmp"),
                snapshot: snap,
                due: 0.0,
                loading: false,
                after_tip: None,
                after_tip_deadline: None,
                backoff: 0.0,
            });
            s
        }

        fn render_thread(s: &State, _cx: &mut Cx<State>) -> El {
            let p = Palette::new(true);
            let items = s.ledger.as_ref().map(|l| l.items.as_slice()).unwrap_or(&[]);
            crate::messages::message_column(items, "Wren", &p, s)
        }

        let done_rt = render_view(make_ledger_state(message.clone(), Some(revision)), render_thread);
        let done_texts: Vec<String> = done_rt.elements().into_iter().filter_map(|e| e.text).collect();
        for expected in [
            "src/app.ts:12",
            "Line changed",
            "File changed, not this line",
            "Not changed",
            "File deleted",
            "Read the port from the environment.",
        ] {
            assert!(done_texts.iter().any(|t| t == expected), "missing `{expected}` in {done_texts:?}");
        }

        let going_rt = render_view(make_ledger_state(message, None), render_thread);
        assert!(going_rt.find("sent-note-outcome-0").is_none());
        assert!(!going_rt.elements().iter().any(|e| e.text.as_deref() == Some("Line changed")));
    }

    #[test]
    fn a_diff_notes_place_labels_the_note_button_by_the_relative_file_while_retaining_the_original_path_for_its_note() {
        let folder = "C:\\work\\project";
        let path = format!("{folder}\\src\\app.js");
        let mut file = changes::parse("--- a/app.js\n+++ b/app.js\n@@ -1 +1 @@\n-const port = 3000\n+const port = 3001\n").remove(0);
        file.path = path.clone();
        let added = file.hunks[0].rows.iter().find(|r| r.kind == '+').unwrap().clone();
        let note = diff_notes::diff_note_for(&path, &added, "Use PORT.");

        let mut s = state_with_changes(folder, file, true);
        s.diff_notes.waiting = vec![note.clone()];
        let rt = render_view(s, render_changes_panel);

        assert!(
            rt.elements()
                .iter()
                .any(|e| e.label.as_deref() == Some("Add a note on src/app.js, line 1"))
        );
        assert!(rt.find(&format!("has-note-{}", diff_notes::diff_note_key(&path, &added))).is_some());
        assert_eq!(note.path, path);
    }

    #[test]
    fn a_diff_notes_place_shows_the_sent_note_relative_to_its_folder_with_the_full_path_on_hover_and_in_the_message() {
        let folder = "C:\\work\\project";
        let path = format!("{folder}\\src\\app.js");
        let file = changes::parse("--- a/app.js\n+++ b/app.js\n@@ -1 +1 @@\n-const port = 3000\n+const port = 3001\n").remove(0);
        let added = file.hunks[0].rows.iter().find(|r| r.kind == '+').unwrap();
        let note = diff_notes::diff_note_for(&path, added, "Use PORT.");
        let message = diff_notes::with_diff_notes("Please revise.", &[note]);
        let (_, sent) = diff_notes::split_diff_notes(&message);
        assert!(message.contains(&path));

        fn render_sent(s: &State, _cx: &mut Cx<State>) -> El {
            let p = Palette::new(true);
            let message = &s.draft.text;
            let (_, notes) = diff_notes::split_diff_notes(message);
            sent_notes(&notes, None, Some("C:\\work\\project"), &p)
        }

        let mut s = State::default();
        s.draft = TextEdit::new(&message);
        let rt = render_view(s, render_sent);
        let els = rt.elements();
        let place = els.iter().find(|e| e.key.as_deref() == Some("sent-note-place-0")).unwrap();
        assert_eq!(place.text.as_deref(), Some("src/app.js:1"));
        assert_eq!(place.label.as_deref(), Some(path.as_str()));
        assert_eq!(sent.len(), 1);
    }

    #[test]
    fn a_diff_notes_place_keeps_a_relative_name_unchanged_and_does_not_treat_another_folder_as_this_one() {
        for (requested, shown) in [("src/app.js", "src/app.js"), ("C:\\work\\other\\src\\app.js", "…/src/app.js")] {
            let mut s = State::default();
            s.draft = TextEdit::new(requested);
            fn render_one(s: &State, _cx: &mut Cx<State>) -> El {
                let note = SentDiffNote {
                    path: s.draft.text.clone(),
                    line: Some(1),
                    removed: false,
                    code: Some("const port = 3001".into()),
                    text: "Use PORT.".into(),
                };
                sent_notes(&[note], None, Some("C:\\work\\project"), &Palette::new(true))
            }
            let rt = render_view(s, render_one);
            let els = rt.elements();
            let place = els.iter().find(|e| e.key.as_deref() == Some("sent-note-place-0")).unwrap();
            assert_eq!(place.text.as_deref(), Some(format!("{shown}:1").as_str()));
            assert_eq!(place.label.as_deref(), Some(requested));
        }
    }

    #[test]
    fn a_diff_notes_place_still_evaluates_a_revision_against_the_stored_absolute_note_path() {
        let folder = "C:\\work\\project";
        let path = format!("{folder}\\src\\app.js");
        let mut file = changes::parse("--- a/app.js\n+++ b/app.js\n@@ -1 +1 @@\n-const port = 3000\n+const port = 3001\n").remove(0);
        file.path = path.clone();
        let added = file.hunks[0].rows.iter().find(|r| r.kind == '+').unwrap();
        let note = diff_notes::diff_note_for(&path, added, "Use PORT.");
        let (_, sent) = diff_notes::split_diff_notes(&diff_notes::with_diff_notes("", &[note]));
        assert_eq!(sent[0].path, path);

        let mut s = State::default();
        s.draft = TextEdit::new(&diff_notes::with_diff_notes("", &[diff_notes::diff_note_for(&path, added, "Use PORT.")]));
        s.changes.diff = Some(Ok(changes::Folder {
            base: "main".into(),
            branch: "main".into(),
            files: vec![file],
            truncated: false,
            commits: Vec::new(),
        }));
        fn render_eval(s: &State, _cx: &mut Cx<State>) -> El {
            let (_, notes) = diff_notes::split_diff_notes(&s.draft.text);
            let edited = match &s.changes.diff {
                Some(Ok(folder)) => folder.files.as_slice(),
                _ => &[],
            };
            sent_notes(&notes, Some(edited), Some("C:\\work\\project"), &Palette::new(true))
        }
        let rt = render_view(s, render_eval);
        let els = rt.elements();
        assert_eq!(
            els.iter().find(|e| e.key.as_deref() == Some("sent-note-place-0")).and_then(|e| e.text.as_deref()),
            Some("src/app.js:1")
        );
        assert!(els.iter().any(|e| e.text.as_deref() == Some("Line changed")));
    }

    #[test]
    fn a_note_can_be_left_in_the_changes_panel_it_sits_inside_the_notes_of_the_conversation_on_screen() {
        let file = changes::parse(SAMPLE_DIFF).remove(0);
        let mut s = state_with_changes("/work/shop", file, true);
        s.teammates.push(Teammate {
            id: "tm2".into(),
            name: "Booty".into(),
            role: "Reviewer".into(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "folder".into(),
            folder: Some("/work/shop".into()),
        });
        let mut rt = render_view(s, render_changes_panel);
        click_key(&mut rt, "diff-note-add-src/app.ts:add::11");
        rt.paste("Check this");
        rt.frame(0.016);
        click_key(&mut rt, "diff-note-save");
        assert_eq!(rt.state.diff_notes.waiting.len(), 1);
        assert_eq!(rt.state.diff_notes.waiting[0].text, "Check this");

        // Notes follow the conversation on screen: switching conversations parks them under the first one.
        rt.update(|s, _| {
            s.diff_notes.put("conv:conv-tm1");
            s.selected = 1;
            s.diff_notes.switch(Some("teammate:tm2".into()));
        });
        assert!(rt.state.diff_notes.waiting.is_empty());
        rt.update(|s, _| {
            s.diff_notes.put("teammate:tm2");
            s.selected = 0;
            s.diff_notes.switch(Some("conv:conv-tm1".into()));
        });
        assert_eq!(rt.state.diff_notes.waiting.len(), 1);
    }

    #[test]
    fn a_note_can_be_left_in_review_changes_only_for_the_teammate_whose_conversation_is_on_screen() {
        let mut s_match = state_with_teammate("tm1", "Wren", Some("/work/shop"), true);
        s_match.review = Some(crate::review_changes::Review {
            teammate_id: "tm1".into(),
            name: "Wren".into(),
            folder: PathBuf::from("/work/shop"),
            review: Some(BranchReview {
                branch: "locust/wren".into(),
                against: Some("main".into()),
                base: "abc1234".into(),
                turns: Vec::new(),
                diff: Some(SAMPLE_DIFF.into()),
                uncommitted: Vec::new(),
            }),
            preview: None,
            problem: None,
            showing: None,
            turn_text: None,
            note: None,
            editing: false,
            message: TextEdit::default(),
            working: false,
            was_running: false,
        });

        let mut rt_match = render_view(s_match, render_review_panel);
        assert!(rt_match.find("diff-note-add-src/app.ts:add::11").is_some());
        click_key(&mut rt_match, "diff-note-add-src/app.ts:add::11");
        rt_match.paste("Review note");
        rt_match.frame(0.016);
        click_key(&mut rt_match, "diff-note-save");
        assert_eq!(rt_match.state.diff_notes.waiting.len(), 1);

        let mut s_other = state_with_teammate("tm1", "Wren", Some("/work/shop"), true);
        s_other.review = Some(crate::review_changes::Review {
            teammate_id: "tm2".into(),
            name: "Booty".into(),
            folder: PathBuf::from("/work/shop"),
            review: Some(BranchReview {
                branch: "locust/booty".into(),
                against: Some("main".into()),
                base: "abc1234".into(),
                turns: Vec::new(),
                diff: Some(SAMPLE_DIFF.into()),
                uncommitted: Vec::new(),
            }),
            preview: None,
            problem: None,
            showing: None,
            turn_text: None,
            note: None,
            editing: false,
            message: TextEdit::default(),
            working: false,
            was_running: false,
        });
        let rt_other = render_view(s_other, render_review_panel);
        assert!(rt_other.find("diff-note-add-src/app.ts:add::11").is_none());
    }
}
