//! putting-a-file-back.test.ts (9 cases). Electron reverses the recorded diffs; Rust puts files back from the turns'
//! checkpoints (`checkpoints::put_back`), whole files by hash, so each case is carried on the outcome the person sees:
//! two later turns undone to before the first, line endings kept, a file changed since left alone, a broken record
//! left alone, a made file removed, a deleted file brought back.
use super::{put_back, Turn};
use crate::store::Store;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch() -> (Scratch, Store, PathBuf) {
    let root = std::env::temp_dir().join(format!("locust-putback-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    fs::create_dir_all(root.join("workspace")).unwrap();
    let store = Store::with_codex(root.join("native"), root.join("electron"), vec![]).unwrap();
    let w = root.join("workspace").canonicalize().unwrap();
    (Scratch(root), store, w)
}

fn turn(store: &Store, w: &std::path::Path, id: &str, change: impl FnOnce()) {
    let t = Turn::begin(store, w);
    change();
    t.finish(store, id).unwrap();
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// notes.md as Electron's two turns leave it: the first turns "two" into "TWO", the second adds "four".
fn two_turns(eol: &str) -> (Scratch, Store, PathBuf) {
    let (s, store, w) = scratch();
    let f = w.join("notes.md");
    fs::write(&f, ["one", "two", "three", ""].join(eol)).unwrap();
    turn(&store, &w, "t1", || fs::write(&f, ["one", "TWO", "three", ""].join(eol)).unwrap());
    turn(&store, &w, "t2", || fs::write(&f, ["one", "TWO", "three", "four", ""].join(eol)).unwrap());
    (s, store, w)
}

// case: is put back as it was before the first of them, newest change undone first
#[test]
fn two_later_turns_are_undone_to_before_the_first() {
    let (_s, store, w) = two_turns("\n");
    let done = put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(done["putBack"], json!(["notes.md"]), "{done}");
    assert_eq!(fs::read_to_string(w.join("notes.md")).unwrap(), "one\ntwo\nthree\n");
}

// case: keeps Windows line endings
#[test]
fn keeps_windows_line_endings() {
    let (_s, store, w) = two_turns("\r\n");
    put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(fs::read(w.join("notes.md")).unwrap(), b"one\r\ntwo\r\nthree\r\n");
}

// case: is left alone when it was changed after those turns
#[test]
fn a_file_changed_after_those_turns_is_left_alone() {
    let (_s, store, w) = two_turns("\n");
    let by_hand = "one\nTWO, edited by hand\nthree\nfour\n";
    fs::write(w.join("notes.md"), by_hand).unwrap();
    let done = put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(done["putBack"], json!([]));
    assert_eq!(done["leftAlone"], json!([{"path": "notes.md", "why": "changed since the replies"}]));
    assert_eq!(fs::read_to_string(w.join("notes.md")).unwrap(), by_hand);
}

// case: is left alone when a recorded change is incomplete
#[test]
fn a_record_that_does_not_follow_on_is_left_alone() {
    // Rust's record of a file is whole: the incomplete case is a chain with a gap, the file changed between the turns.
    let (_s, store, w) = scratch();
    let f = w.join("notes.md");
    fs::write(&f, "one\ntwo\nthree\n").unwrap();
    turn(&store, &w, "t1", || fs::write(&f, "one\nTWO\nthree\n").unwrap());
    fs::write(&f, "one\nTWO\nthree\nmine\n").unwrap();
    turn(&store, &w, "t2", || fs::write(&f, "one\nTWO\nthree\nmine\nfour\n").unwrap());
    let done = put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(done["putBack"], json!([]));
    assert_eq!(done["leftAlone"][0]["why"], "the replies' recorded changes do not follow one another", "{done}");
    assert_eq!(fs::read_to_string(&f).unwrap(), "one\nTWO\nthree\nmine\nfour\n");
}

// case: is removed, when it is exactly what was created
#[test]
fn a_made_file_is_removed_when_it_is_exactly_what_was_made() {
    let (_s, store, w) = scratch();
    turn(&store, &w, "t1", || fs::write(w.join("fresh.txt"), "hello\nworld\n").unwrap());
    let done = put_back(&store, &w, &ids(&["t1"])).unwrap();
    assert_eq!(done["putBack"], json!(["fresh.txt"]));
    assert!(!w.join("fresh.txt").exists());
}

// case: is left alone when something was added to it since
#[test]
fn a_made_file_added_to_since_is_left_alone() {
    let (_s, store, w) = scratch();
    turn(&store, &w, "t1", || fs::write(w.join("fresh.txt"), "hello\nworld\n").unwrap());
    fs::write(w.join("fresh.txt"), "hello\nworld\nmine\n").unwrap();
    let done = put_back(&store, &w, &ids(&["t1"])).unwrap();
    assert_eq!(done["putBack"], json!([]));
    assert_eq!(done["leftAlone"][0]["path"], "fresh.txt");
    assert_eq!(fs::read_to_string(w.join("fresh.txt")).unwrap(), "hello\nworld\nmine\n");
}

// case: created and then changed by the next turn, is removed
#[test]
fn made_then_changed_by_the_next_turn_is_removed() {
    let (_s, store, w) = scratch();
    turn(&store, &w, "t1", || fs::write(w.join("fresh.txt"), "hello\nworld\n").unwrap());
    turn(&store, &w, "t2", || fs::write(w.join("fresh.txt"), "hello\nworld\nagain\n").unwrap());
    let done = put_back(&store, &w, &ids(&["t1", "t2"])).unwrap();
    assert_eq!(done["putBack"], json!(["fresh.txt"]));
    assert!(!w.join("fresh.txt").exists());
}

// case: is brought back from a record of the whole file
#[test]
fn a_deleted_file_is_brought_back_whole() {
    let (_s, store, w) = scratch();
    fs::write(w.join("old.txt"), "kept\nlines\n").unwrap();
    turn(&store, &w, "t1", || fs::remove_file(w.join("old.txt")).unwrap());
    let done = put_back(&store, &w, &ids(&["t1"])).unwrap();
    assert_eq!(done["putBack"], json!(["old.txt"]));
    assert_eq!(fs::read_to_string(w.join("old.txt")).unwrap(), "kept\nlines\n");
}

// case: is left alone when something is there again
#[test]
fn a_deleted_file_with_something_there_again_is_left_alone() {
    let (_s, store, w) = scratch();
    fs::write(w.join("old.txt"), "kept\nlines\n").unwrap();
    turn(&store, &w, "t1", || fs::remove_file(w.join("old.txt")).unwrap());
    fs::write(w.join("old.txt"), "new\n").unwrap();
    let done = put_back(&store, &w, &ids(&["t1"])).unwrap();
    assert_eq!(done["putBack"], json!([]));
    assert_eq!(done["leftAlone"], json!([{"path": "old.txt", "why": "changed since the replies"}]));
    assert_eq!(fs::read_to_string(w.join("old.txt")).unwrap(), "new\n");
}
