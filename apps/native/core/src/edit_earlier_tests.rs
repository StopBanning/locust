//! Electron's an-earlier-message-can-be-edited.test.tsx (the words and the versions), a-set-aside-turn-sends-nothing.test.ts
//! and a-project-is-never-half-put-back.test.ts, for the parts that are core logic. The window's half (Edit on a bubble,
//! the line over the box, Cancel, the note above the new turn) is in `src/ui/window_drive_tests.rs`.
use super::*;
use crate::checkpoints::{Turn, plan_put_back, put_back, undo};
use crate::store::Store;
use crate::workroom::Party;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        // A folder left read-only by a test is made writable again so it can go.
        #[cfg(unix)]
        if let Ok(entries) = fs::read_dir(self.0.join("workspace")) {
            use std::os::unix::fs::PermissionsExt;
            for e in entries.flatten() {
                let _ = fs::set_permissions(e.path(), fs::Permissions::from_mode(0o755));
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn scratch() -> Scratch {
    let root = std::env::temp_dir().join(format!("locust-edit-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    fs::create_dir_all(root.join("workspace")).unwrap();
    Scratch(root)
}

// ---- the thread's own words ---------------------------------------------------------------------------------------

#[test]
fn says_above_the_new_turn_that_it_started_again_and_not_that_a_fresh_session_began() {
    let said = started_again_note(None);
    assert!(said.contains("Started again from an edited message."), "{said}");
    assert!(!said.contains("A fresh session"), "{said}");
    assert!(said.contains("files they changed are as they left them"), "{said}");
}

#[test]
fn says_what_sending_will_do_and_nothing_of_the_kind_when_no_edit_is_open() {
    let line = editing_line(0, 0);
    assert!(line.contains("Editing an earlier message"), "{line}");
    assert!(line.contains("Files stay as they are."), "{line}");
    // The tick appears only when files can go back, and counts them.
    assert!(!editing_line(3, 0).contains("Files stay as they are"));
    assert_eq!(put_back_label(1), "Also put back the 1 file the replies after it changed");
    assert_eq!(put_back_label(3), "Also put back the 3 files the replies after it changed");
}

#[test]
fn the_window_offers_nothing_to_tick_when_some_cannot_go_back_and_says_why() {
    let line = editing_line(8, 2);
    assert!(line.contains("Files stay as they are: 2 of the 8 files the replies changed cannot be put back exactly"), "{line}");
    assert!(line.contains("putting back only the others could leave the project half changed."), "{line}");
    assert!(editing_line(8, 1).contains("Files stay as they are: 1 of the 8 files"));
}

#[test]
fn the_note_above_the_new_turn_says_what_went_back_what_was_left_and_what_was_held() {
    let two = PutBackNote { put_back: vec!["a.md".into(), "b.md".into()], ..Default::default() };
    assert_eq!(started_again_note(Some(&two)), "Started again from an edited message. The replies after it were set aside, and 2 files they changed were put back. ");
    let one = PutBackNote { put_back: vec!["a.md".into()], ..Default::default() };
    assert!(started_again_note(Some(&one)).contains(", and 1 file they changed was put back. "));
    let left = PutBackNote { put_back: vec!["a.md".into()], left_alone: vec![("c.md".into(), "changed since the replies".into())], held_back: 0 };
    assert!(started_again_note(Some(&left)).ends_with("Left as it is: c.md (changed since the replies). "));
    // Held back: all of them or none, said as that.
    let held = PutBackNote { put_back: vec![], left_alone: vec![("c.md".into(), "removed since the replies".into())], held_back: 5 };
    let said = started_again_note(Some(&held));
    assert!(said.contains("Nothing was put back: c.md (removed since the replies) could not be, and putting back only the other 5 could leave the project half changed. "), "{said}");
    assert!(!said.contains("Left as"), "{said}");
}

// ---- the version before an edit (0.498) -----------------------------------------------------------------------------

fn at(minute: u32) -> String {
    format!("2026-09-30T10:{minute:02}:00.000Z")
}
fn turn(id: &str, minute: u32, from: Option<&str>, edited: bool) -> TurnLink {
    TurnLink { mission_id: id.into(), created_at: at(minute), parent: from.map(str::to_owned), follow_up: from.is_some(), edited }
}
/// one -> two -> three, then two was edited: one -> two' -> three'.
fn edited_chain() -> Vec<TurnLink> {
    vec![
        turn("one", 1, None, false),
        turn("two", 2, Some("one"), false),
        turn("three", 3, Some("two"), false),
        turn("two_edited", 4, Some("one"), true),
        turn("three_edited", 5, Some("two_edited"), false),
    ]
}

#[test]
fn finds_for_the_edit_the_newest_turn_of_the_version_before_it() {
    assert_eq!(turn_versions("two_edited", &edited_chain()), Some(Versions { before: Some("three".into()), after: None }));
}

#[test]
fn finds_for_the_replaced_turn_the_newest_turn_of_the_edit() {
    assert_eq!(turn_versions("two", &edited_chain()), Some(Versions { before: None, after: Some("three_edited".into()) }));
}

#[test]
fn finds_nothing_for_a_turn_no_edit_touched_or_two_follow_ups_that_are_not_an_edit() {
    assert_eq!(turn_versions("one", &edited_chain()), None);
    assert_eq!(turn_versions("three_edited", &edited_chain()), None);
    let twice = vec![turn("a", 1, None, false), turn("b", 2, Some("a"), false), turn("c", 3, Some("a"), false)];
    assert_eq!(turn_versions("c", &twice), None);
    assert_eq!(turn_versions("b", &twice), None);
}

#[test]
fn says_so_above_the_message_with_the_way_to_the_other_version() {
    let chain = edited_chain();
    let edit = turn_versions("two_edited", &chain).unwrap();
    assert_eq!(edited_note(&edit), Some("Edited."));
    let replaced = turn_versions("two", &chain).unwrap();
    assert_eq!(edited_note(&replaced), Some("This message was edited later."));
    assert_eq!(edited_note(&Versions::default()), None);
    // A route switch between two turns is not a neighbour of an edit.
    let mut switched = chain.clone();
    switched.push(TurnLink { follow_up: false, ..turn("handed", 6, Some("one"), true) });
    assert_eq!(turn_versions("two", &switched), Some(Versions { before: None, after: Some("three_edited".into()) }));
}

// ---- what a set-aside turn sent, nobody gets (0.512) --------------------------------------------------------------------

fn workroom_at(root: &Path) -> Workroom {
    let next = AtomicU32::new(0);
    Workroom::with(root, Box::new(|| "2026-09-30T21:00:00.000Z".to_owned()), Box::new(move || format!("m{}", next.fetch_add(1, Ordering::SeqCst) + 1)))
}
fn builder() -> Party {
    Party { teammate_id: "tm_builder".into(), name: "Builder".into() }
}
fn reviewer() -> Party {
    Party { teammate_id: "tm_reviewer".into(), name: "Reviewer".into() }
}

#[test]
fn a_message_the_set_aside_turns_sent_still_unread_is_never_handed_to_its_recipient() {
    let s = scratch();
    let w = workroom_at(&s.0.join("room"));
    let stale = w.post(&builder(), "mission_json_5", &reviewer(), "Rewrite the README for JSON storage.", None).unwrap();
    let kept = w.post(&builder(), "mission_2", &reviewer(), "The tests live in test/.", None).unwrap();
    assert_eq!(retire_set_aside_messages(&w, &["mission_json_4".into(), "mission_json_5".into()]).unwrap(), vec![stale.message_id.clone()]);
    let waiting = w.unread("tm_reviewer", 10, &[]).unwrap();
    assert_eq!(waiting.messages.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(), ["The tests live in test/."]);
    // Recorded as back with the turn that sent it, and the file still reads whole.
    let snapshot = w.read();
    assert_eq!(snapshot.deliveries.len(), 1);
    assert_eq!((snapshot.deliveries[0].message_id.as_str(), snapshot.deliveries[0].mission_id.as_str()), (stale.message_id.as_str(), "mission_json_5"));
    assert!(snapshot.issues.is_empty());
    assert_ne!(kept.message_id, stale.message_id);
}

#[test]
fn a_message_already_delivered_stays_as_it_was_and_nothing_else_is_touched() {
    let s = scratch();
    let w = workroom_at(&s.0.join("room"));
    let read = w.post(&builder(), "mission_json_5", &reviewer(), "Already read.", None).unwrap();
    w.mark_delivered(std::slice::from_ref(&read.message_id), "mission_reviewer_1").unwrap();
    assert_eq!(retire_set_aside_messages(&w, &["mission_json_5".into()]).unwrap(), Vec::<String>::new());
    let deliveries = w.read().deliveries;
    assert_eq!(deliveries.len(), 1);
    assert_eq!((deliveries[0].message_id.as_str(), deliveries[0].mission_id.as_str()), (read.message_id.as_str(), "mission_reviewer_1"));
}

#[test]
fn ignores_a_list_that_is_not_one() {
    let s = scratch();
    let w = workroom_at(&s.0.join("room"));
    w.post(&builder(), "mission_json_5", &reviewer(), "Waiting.", None).unwrap();
    let too_many = vec!["mission_json_5".to_owned(); 501];
    for listed in [vec![], vec!["../etc".to_owned()], vec![String::new()], vec!["x".repeat(129)], too_many] {
        assert_eq!(retire_set_aside_messages(&w, &listed).unwrap(), Vec::<String>::new(), "{listed:?}");
    }
    assert_eq!(w.unread("tm_reviewer", 10, &[]).unwrap().messages.len(), 1);
}

// ---- a project is never half put back (0.502, 0.512) --------------------------------------------------------------------

fn store_in(s: &Scratch) -> Store {
    Store::with_codex(s.0.join("native"), s.0.join("electron"), vec![]).unwrap()
}
fn ws(s: &Scratch) -> PathBuf {
    s.0.join("workspace")
}

/// Two turns: t1 changes notes (v1 -> v2) and makes `made`; t2 changes notes (v2 -> v3) and deletes `gone`.
fn two_turns(s: &Scratch) -> Store {
    let store = store_in(s);
    let w = ws(s);
    fs::write(w.join("notes"), "v1\n").unwrap();
    fs::write(w.join("gone"), "keep me\n").unwrap();
    let t1 = Turn::begin(&store, &w);
    fs::write(w.join("notes"), "v2\n").unwrap();
    fs::write(w.join("made"), "new\n").unwrap();
    t1.finish(&store, "t1").unwrap();
    let t2 = Turn::begin(&store, &w);
    fs::write(w.join("notes"), "v3\n").unwrap();
    fs::remove_file(w.join("gone")).unwrap();
    t2.finish(&store, "t2").unwrap();
    store
}
fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}
fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if e.path().is_dir() {
            for (inner, bytes) in tree(&e.path()) {
                out.push((format!("{name}/{inner}"), bytes));
            }
        } else {
            out.push((name, fs::read(e.path()).unwrap()));
        }
    }
    out.sort();
    out
}

#[test]
fn putting_back_returns_each_file_to_how_it_was_before_the_first_set_aside_turn() {
    let s = scratch();
    let store = two_turns(&s);
    let w = ws(&s);
    let plan = plan_put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(plan.files, ["gone", "made", "notes"]);
    assert!(plan.cannot.is_empty(), "{:?}", plan.cannot);
    let done = put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(done["putBack"], json!(["gone", "made", "notes"]));
    assert_eq!(done["leftAlone"], json!([]));
    assert_eq!(fs::read_to_string(w.join("notes")).unwrap(), "v1\n", "two turns' changes to one file go back to before the first");
    assert_eq!(fs::read_to_string(w.join("gone")).unwrap(), "keep me\n");
    assert!(!w.join("made").exists(), "a file the turns made is gone");
    // Each turn then reads undone, so Undo has nothing left to do on it.
    let again = undo(&store, &w, "t1").unwrap();
    assert_eq!(again["restored"], json!(["made", "notes"]));
    assert_eq!(fs::read_to_string(w.join("notes")).unwrap(), "v1\n");
}

#[test]
fn the_host_writes_nothing_when_any_file_cannot_go_back_and_says_which_could_have() {
    let s = scratch();
    let store = two_turns(&s);
    let w = ws(&s);
    // The person changed one file by hand since the replies.
    fs::write(w.join("made"), "the person wrote this\n").unwrap();
    let before = tree(&w);
    let done = put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(done["putBack"], json!([]));
    assert_eq!(done["leftAlone"], json!([{"path": "made", "why": "changed since the replies"}]));
    assert_eq!(done["heldBack"], json!(["gone", "notes"]), "the files that could have gone back, held for the one that could not");
    assert_eq!(tree(&w), before, "nothing was written");
    // The window's own check says the same before anything is offered.
    let plan = plan_put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(plan.cannot, vec![("made".to_owned(), "changed since the replies".to_owned())]);
}

#[test]
fn a_file_removed_since_the_replies_cannot_go_back_either() {
    let s = scratch();
    let store = two_turns(&s);
    let w = ws(&s);
    fs::remove_file(w.join("made")).unwrap();
    let plan = plan_put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(plan.cannot, vec![("made".to_owned(), "removed since the replies".to_owned())]);
}

#[cfg(unix)]
#[test]
fn a_write_that_fails_puts_the_ones_already_written_back_as_they_were() {
    use std::os::unix::fs::PermissionsExt;
    let s = scratch();
    let store = store_in(&s);
    let w = ws(&s);
    fs::create_dir_all(w.join("sub")).unwrap();
    fs::write(w.join("a.txt"), "a1\n").unwrap();
    fs::write(w.join("sub/b.txt"), "b1\n").unwrap();
    let turn = Turn::begin(&store, &w);
    fs::write(w.join("a.txt"), "a2\n").unwrap();
    fs::write(w.join("sub/b.txt"), "b2\n").unwrap();
    turn.finish(&store, "t1").unwrap();
    // Every file can go back as far as the check can tell; then a folder stops taking writes.
    assert!(plan_put_back(&store, &w, &ids(&["t1"])).unwrap().cannot.is_empty());
    fs::set_permissions(w.join("sub"), fs::Permissions::from_mode(0o555)).unwrap();
    let done = put_back(&store, &w, &ids(&["t1"])).unwrap();
    assert_eq!(done["putBack"], json!([]));
    assert_eq!(done["leftAlone"][0]["path"], "sub/b.txt");
    assert!(done["leftAlone"][0]["why"].as_str().unwrap().starts_with("it could not be written"), "{done}");
    assert_eq!(done["heldBack"], json!(["a.txt"]));
    assert_eq!(fs::read_to_string(w.join("a.txt")).unwrap(), "a2\n", "the one already written was put back as it was");
    assert_eq!(fs::read_to_string(w.join("sub/b.txt")).unwrap(), "b2\n");
    fs::set_permissions(w.join("sub"), fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_reply_with_no_record_of_its_changes_holds_the_whole_set_back() {
    let s = scratch();
    let store = two_turns(&s);
    let w = ws(&s);
    let plan = plan_put_back(&store, &w, &ids(&["t1", "run_in_another_app", "t2"])).unwrap();
    assert_eq!(plan.cannot.len(), 1, "{:?}", plan.cannot);
    assert!(plan.cannot[0].1.contains("no record of what it changed"));
    let before = tree(&w);
    let done = put_back(&store, &w, &ids(&["t1", "run_in_another_app", "t2"])).unwrap();
    assert_eq!(done["putBack"], json!([]));
    assert_eq!(tree(&w), before);
}

#[test]
fn a_reply_that_shared_the_folder_with_another_run_holds_the_set_back() {
    let s = scratch();
    let store = store_in(&s);
    let w = ws(&s);
    fs::write(w.join("notes"), "v1\n").unwrap();
    let a = Turn::begin(&store, &w);
    let b = Turn::begin(&store, &w);
    fs::write(w.join("notes"), "v2\n").unwrap();
    a.finish(&store, "run_a").unwrap();
    b.finish(&store, "run_b").unwrap();
    let plan = plan_put_back(&store, &w, &ids(&["run_a"])).unwrap();
    assert_eq!(plan.cannot, vec![("notes".to_owned(), "the reply shared this folder with another run".to_owned())]);
}

#[test]
fn turns_whose_records_do_not_follow_one_another_are_not_put_back() {
    let s = scratch();
    let store = store_in(&s);
    let w = ws(&s);
    fs::write(w.join("notes"), "v1\n").unwrap();
    let t1 = Turn::begin(&store, &w);
    fs::write(w.join("notes"), "v2\n").unwrap();
    t1.finish(&store, "t1").unwrap();
    // Changed by hand between the turns, so t2 began from something t1 did not leave.
    fs::write(w.join("notes"), "by hand\n").unwrap();
    let t2 = Turn::begin(&store, &w);
    fs::write(w.join("notes"), "v3\n").unwrap();
    t2.finish(&store, "t2").unwrap();
    let plan = plan_put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(plan.cannot, vec![("notes".to_owned(), "the replies' recorded changes do not follow one another".to_owned())]);
}

#[test]
fn a_turn_already_undone_is_skipped_and_the_others_still_go_back() {
    let s = scratch();
    let store = two_turns(&s);
    let w = ws(&s);
    // t2 undone first (newest first, as Undo requires), then both set aside.
    undo(&store, &w, "t2").unwrap();
    let plan = plan_put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert!(plan.cannot.is_empty(), "{:?}", plan.cannot);
    assert_eq!(plan.files, ["made", "notes"]);
    put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(fs::read_to_string(w.join("notes")).unwrap(), "v1\n");
    assert!(!w.join("made").exists());
}
