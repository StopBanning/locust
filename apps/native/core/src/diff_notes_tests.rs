//! Electron's tests: notes-on-a-diff-go-as-one-message, review-notes-survive-a-revision and
//! a-diff-notes-place-is-relative-to-its-folder (their pure parts; the window draws are tested in the window).
use super::*;
use crate::changes::{self, Row};

fn added() -> Row {
    Row { kind: '+', old: None, new: Some(11), text: "const port = 3001".into() }
}
fn removed() -> Row {
    Row { kind: '-', old: Some(11), new: None, text: "const port = 3000".into() }
}

#[test]
fn knows_its_file_and_the_line_a_person_reads_the_new_one_or_the_old_one_for_a_removed_line() {
    let note = diff_note_for("src/app.ts", &added(), "  read it from PORT  ");
    assert_eq!((note.path.as_str(), note.line, note.kind, note.code.as_str(), note.text.as_str()), ("src/app.ts", Some(11), '+', "const port = 3001", "read it from PORT"));
    let gone = diff_note_for("src/app.ts", &removed(), "why remove this?");
    assert_eq!((gone.line, gone.kind), (Some(11), '-'));
    assert_eq!(diff_note_key("src/app.ts", &added()), "src/app.ts:add::11");
}

#[test]
fn replaces_the_note_already_on_its_line() {
    let first = diff_note_for("src/app.ts", &added(), "one");
    let second = diff_note_for("src/app.ts", &added(), "two");
    assert_eq!(with_note(&[first], second.clone()), vec![second]);
}

fn three() -> Vec<DiffNote> {
    vec![
        diff_note_for("src/app.ts", &added(), "Read it from the PORT variable instead."),
        diff_note_for("README.md", &Row { kind: ' ', old: Some(4), new: Some(4), text: "Run it with npm start.".into() }, "This should say pnpm."),
        diff_note_for("src/app.ts", &removed(), "Keep a comment saying why it moved."),
    ]
}

#[test]
fn is_one_block_in_file_then_line_order_each_note_with_the_line_it_is_about() {
    assert_eq!(
        diff_notes_block(&three()),
        [
            "Notes on your changes (3):",
            "- README.md, line 4 `Run it with npm start.`: This should say pnpm.",
            "- src/app.ts, line 11 `const port = 3001`: Read it from the PORT variable instead.",
            "- src/app.ts, removed line 11 `const port = 3000`: Keep a comment saying why it moved.",
        ]
        .join("\n")
    );
}

#[test]
fn follows_the_persons_words_and_is_the_whole_message_when_there_are_none() {
    let one = &three()[..1];
    assert_eq!(with_diff_notes("Nearly there.", one), format!("Nearly there.\n\n{}", diff_notes_block(one)));
    assert_eq!(with_diff_notes("", one), diff_notes_block(one));
    assert_eq!(with_diff_notes("Just this.", &[]), "Just this.");
    assert_eq!(diff_notes_tile(1), "1 note on the changes");
    assert_eq!(diff_notes_tile(2), "2 notes on the changes");
}

fn note(path: &str, line: usize, code: &str, text: &str, kind: char) -> DiffNote {
    DiffNote { key: format!("{path}:{line}"), path: path.into(), line: Some(line), kind, code: code.into(), text: text.into() }
}

fn sent() -> String {
    with_diff_notes(
        "Tidy these up, please.",
        &[
            note("src/app.ts", 12, "const port = 3000", "Read the port from the environment.", '+'),
            note("src/app.ts", 40, "log(everything)", "Too noisy.", '+'),
            note("README.md", 3, "Run it with npm", "Say pnpm.", '+'),
            note("src/old.ts", 7, "legacy()", "Is this still called?", '-'),
        ],
    )
}

#[test]
fn are_read_back_out_of_it_the_persons_own_words_apart() {
    let (text, notes) = split_diff_notes(&sent());
    assert_eq!(text, "Tidy these up, please.");
    let read: Vec<(String, Option<usize>, bool, Option<String>, String)> = notes.into_iter().map(|n| (n.path, n.line, n.removed, n.code, n.text)).collect();
    let want = |p: &str, l: usize, r: bool, c: &str, t: &str| (p.to_owned(), Some(l), r, Some(c.to_owned()), t.to_owned());
    assert_eq!(
        read,
        vec![
            want("README.md", 3, false, "Run it with npm", "Say pnpm."),
            want("src/app.ts", 12, false, "const port = 3000", "Read the port from the environment."),
            want("src/app.ts", 40, false, "log(everything)", "Too noisy."),
            want("src/old.ts", 7, true, "legacy()", "Is this still called?"),
        ]
    );
}

#[test]
fn leave_a_message_with_no_notes_exactly_as_it_was() {
    assert_eq!(split_diff_notes("Just words. Notes on your changes are welcome."), ("Just words. Notes on your changes are welcome.".to_owned(), vec![]));
}

fn revision() -> Vec<changes::File> {
    changes::parse(
        &[
            "diff --git a/src/app.ts b/src/app.ts",
            "--- a/src/app.ts",
            "+++ b/src/app.ts",
            "@@ -11,3 +11,3 @@",
            " import { env } from \"./env\"",
            "-const port = 3000",
            "+const port = Number(env.PORT ?? 3000)",
            " serve(port)",
            "diff --git a/src/old.ts b/src/old.ts",
            "deleted file mode 100644",
            "--- a/src/old.ts",
            "+++ /dev/null",
            "@@ -1,1 +0,0 @@",
            "-legacy()",
            "",
        ]
        .join("\n"),
    )
}

#[test]
fn says_the_line_changed_only_the_file_changed_nothing_changed_or_the_file_went() {
    let (_, notes) = split_diff_notes(&sent());
    let edited = revision();
    assert_eq!(notes.iter().map(|n| note_outcome(n, &edited)).collect::<Vec<_>>(), [NoteOutcome::Untouched, NoteOutcome::Line, NoteOutcome::File, NoteOutcome::Deleted]);
    assert_eq!(note_outcome_words(NoteOutcome::Line), "Line changed");
    assert_eq!(note_outcome_words(NoteOutcome::File), "File changed, not this line");
    assert_eq!(note_outcome_words(NoteOutcome::Untouched), "Not changed");
    assert_eq!(note_outcome_words(NoteOutcome::Deleted), "File deleted");
}

#[test]
fn finds_the_file_however_the_runtime_wrote_its_path() {
    let (_, notes) = split_diff_notes(&sent());
    let absolute: Vec<changes::File> = revision().into_iter().map(|mut f| { f.path = format!("C:\\work\\{}", f.path.replace('/', "\\")); f }).collect();
    assert_eq!(note_outcome(&notes[1], &absolute), NoteOutcome::Line);
}

#[test]
fn caps_note_text_at_max_diff_note_and_quoted_code_at_quoted_code() {
    let long_code = "x".repeat(QUOTED_CODE + 25);
    let long_text = "a".repeat(MAX_DIFF_NOTE + 50);
    let n = diff_note_for("src/app.ts", &Row { kind: '+', old: None, new: Some(1), text: long_code }, &long_text);
    assert_eq!(n.text.chars().count(), MAX_DIFF_NOTE);
    let block = diff_notes_block(&[n]);
    let expected_quote = format!("`{}…`", "x".repeat(QUOTED_CODE - 1));
    assert!(block.contains(&expected_quote));
}

#[test]
fn edited_files_prefers_observed_edit_over_step_patches_and_uses_checkpoint_fallback() {
    use serde_json::json;
    let events = vec![
        json!({
            "type": "tool.completed",
            "payload": {
                "itemId": "step1",
                "toolKind": "edit",
                "patch": { "text": "--- a/src/app.ts\n+++ b/src/app.ts\n@@ -1 +1 @@\n-old1\n+new1\n" }
            }
        }),
        json!({
            "type": "tool.completed",
            "payload": {
                "itemId": "obs1",
                "toolKind": "observed_edit",
                "patch": { "text": "--- a/src/app.ts\n+++ b/src/app.ts\n@@ -12 +12 @@\n-const port = 3000\n+const port = 3001\n" }
            }
        }),
    ];
    let refs: Vec<&serde_json::Value> = events.iter().collect();
    let files = edited_files(refs, Some("/tmp/proj"), &[]);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].hunks[0].rows[0].old, Some(12));

    let cp_files = changes::parse("--- a/src/other.ts\n+++ b/src/other.ts\n@@ -2 +2 @@\n-a\n+b\n");
    let combined = edited_files(events.iter().collect::<Vec<_>>(), Some("/tmp/proj"), &cp_files);
    assert_eq!(combined.len(), 2);
}

#[test]
fn checkpoints_turn_diffs_and_ledger_session_track_per_turn_edits() {
    use serde_json::json;
    let unique = format!(
        "diff-notes-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    let ws = root.join("ws");
    let ledger_dir = root.join("ledger");
    std::fs::create_dir_all(ws.join("src")).unwrap();
    std::fs::create_dir_all(&ledger_dir).unwrap();
    std::fs::write(ws.join("src/app.ts"), "line1\nconst port = 3000\nline3\n").unwrap();

    let store = crate::store::Store::new(root.join("store"), root.join("electron")).unwrap();
    let turn = crate::checkpoints::Turn::begin(&store, &ws);
    std::fs::write(ws.join("src/app.ts"), "line1\nconst port = 3001\nline3\n").unwrap();
    turn.finish(&store, "m_turn1").unwrap();

    let cp_diffs = crate::checkpoints::turn_diffs(&store, &ws, "m_turn1").unwrap();
    assert_eq!(cp_diffs.len(), 1);
    assert_eq!(cp_diffs[0].status, "MODIFIED");

    // While running: Snapshot.turn_edits has None.
    let ledger_path = ledger_dir.join("m_turn1.jsonl");
    let created_record = json!({
        "schemaVersion": 27,
        "recordType": "mission.created",
        "metadata": {
            "missionId": "m_turn1",
            "prompt": "Fix port",
            "runtime": "codex",
            "model": "gpt-5",
            "createdAt": "2026-09-01T00:00:00Z"
        }
    });
    let start_record = json!({
        "schemaVersion": 27,
        "recordType": "mission.event",
        "event": {
            "sequence": 1,
            "occurredAt": "2026-09-01T00:00:01Z",
            "type": "run.started",
            "payload": {}
        }
    });
    std::fs::write(&ledger_path, format!("{created_record}\n{start_record}\n")).unwrap();

    let mut session = crate::ledger::Session::new(ledger_dir, "m_turn1".into())
        .with_folder(ws.to_str())
        .with_checkpoints(store, ws.clone());
    let snap_running = session.poll().unwrap();
    assert!(snap_running.running);
    assert_eq!(snap_running.turn_edits.get("m_turn1"), Some(&None));

    // After completion: Snapshot.turn_edits has Some(files) and line 2 is marked Line changed.
    let end_record = json!({
        "schemaVersion": 27,
        "recordType": "mission.event",
        "event": {
            "sequence": 2,
            "occurredAt": "2026-09-01T00:00:05Z",
            "type": "run.completed",
            "payload": {}
        }
    });
    std::fs::write(&ledger_path, format!("{created_record}\n{start_record}\n{end_record}\n")).unwrap();
    let snap_done = session.poll().unwrap();
    assert!(!snap_done.running);
    let edited = snap_done
        .turn_edits
        .get("m_turn1")
        .and_then(|o| o.as_ref())
        .expect("finished turn should expose edits");
    let n = SentDiffNote {
        path: ws.join("src/app.ts").to_string_lossy().into_owned(),
        line: Some(2),
        removed: false,
        code: Some("const port = 3000".into()),
        text: "change port".into(),
    };
    assert_eq!(note_outcome(&n, edited), NoteOutcome::Line);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_notes_place_is_shown_relative_to_its_folder_with_the_full_path_kept_in_the_message() {
    let folder = "C:\\work\\project";
    let path = format!("{folder}\\src\\app.js");
    assert_eq!(shown_path(&path, Some(folder)), "src/app.js");
    let n = diff_note_for(&path, &Row { kind: '+', old: None, new: Some(1), text: "x".into() }, "Use PORT.");
    assert_eq!(n.path, path);
    assert!(with_diff_notes("Please revise.", &[n]).contains(&path));
}

#[test]
fn a_notes_place_keeps_a_relative_name_unchanged_and_does_not_treat_another_folder_as_this_one() {
    let folder = "C:\\work\\project";
    assert_eq!(shown_path("src/app.js", Some(folder)), "src/app.js");
    assert_eq!(shown_path("C:\\work\\other\\src\\app.js", Some(folder)), "…/src/app.js");
}
