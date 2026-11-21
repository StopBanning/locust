//! Electron's `main/torn-ledger.test.ts`, "reader → history → preload → rendered Missions screen": the records'
//! damage read from real files through `list_recent`, said in Screens.tsx's words (`ListDamage::words`), which the
//! sidebar draws under the list (`src/ui/torn_ledger_line_tests.rs`).
//!
//! Rust has no "N conversations, M in this folder" header (Screens.tsx's meta, `missions-lists-conversations.test.tsx`
//! in the ledger), so these cases check the part after it: the damage words, and that nothing is said when the
//! records are whole.
use super::*;
use std::fs;
use crate::ledger::torn_ledger_tests::{clean, prefix, shape, Folder, ID, NOW, RECOVERING, UNREADABLE};

/// A store whose Electron folder holds `files` (`id`, text, oversized); the native folder is not made.
fn records(files: &[(&str, String, bool)]) -> (Folder, Store) {
    let folder = Folder::new();
    let ledger = folder.0.join("electron/mission-ledger");
    fs::create_dir_all(&ledger).unwrap();
    for (id, text, oversized) in files {
        let path = ledger.join(format!("{id}.jsonl"));
        fs::write(&path, text).unwrap();
        if *oversized {
            fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(64 * 1024 * 1024 + 1).unwrap();
        }
    }
    let store = Store::new(folder.0.join("native"), folder.0.join("electron")).unwrap();
    (folder, store)
}

fn said(store: Store) -> (Listed, Option<String>) {
    let _g = hooks::install(store, None);
    let listed = list_recent(MAX_LISTED_MISSIONS, true).expect("the records are listed");
    let words = listed.damage.words();
    (listed, words)
}

fn ids(listed: &Listed) -> Vec<&str> {
    listed.summaries.iter().map(|s| s.mission_id.as_str()).collect()
}

// case: invalid header beside a clean ledger: clean receipt survives and unreadable file is visible
#[test]
fn an_invalid_header_beside_a_clean_ledger_lists_the_clean_one_and_says_one_file_could_not_be_read() {
    let (_f, store) = records(&[(ID, "{broken\n".into(), false), ("mission_clean", clean().replace(ID, "mission_clean"), false)]);
    let (listed, words) = said(store);
    assert_eq!(ids(&listed), ["mission_clean"]);
    assert_eq!(listed.damage, ListDamage { incomplete: 0, unreadable: 1, folder_unreadable: false });
    assert_eq!(words.as_deref(), Some("1 file could not be read"));
}

// case: clean control: zero issues across the boundary, and nothing said about the records on screen
#[test]
fn clean_records_say_nothing_about_the_records() {
    let (_f, store) = records(&[(ID, clean(), false)]);
    let (listed, words) = said(store);
    assert_eq!(ids(&listed), [ID]);
    assert_eq!(listed.damage, ListDamage::default());
    assert_eq!(words, None);
}

fn says_incomplete(name: &str) {
    let (_f, store) = records(&[(ID, shape(name).text, false)]);
    let (listed, words) = said(store);
    assert_eq!(ids(&listed), [ID], "{name}");
    assert_eq!(listed.damage.unreadable, 0, "{name}");
    assert_eq!(words.as_deref(), Some("1 with an incomplete receipt"), "{name}");
}

// case: $name: issue becomes incomplete receipt wording
#[test]
fn truncated_final_json_says_one_with_an_incomplete_receipt() {
    says_incomplete(RECOVERING[0]);
}

// case: $name: issue becomes incomplete receipt wording
#[test]
fn an_invalid_record_type_says_one_with_an_incomplete_receipt() {
    says_incomplete(RECOVERING[1]);
}

// case: $name: issue becomes incomplete receipt wording
#[test]
fn a_noncontiguous_ledger_sequence_says_one_with_an_incomplete_receipt() {
    says_incomplete(RECOVERING[2]);
}

// case: $name: issue becomes incomplete receipt wording
#[test]
fn a_malformed_body_says_one_with_an_incomplete_receipt() {
    says_incomplete(RECOVERING[3]);
}

fn says_unreadable(name: &str) {
    let shape = shape(name);
    let (_f, store) = records(&[(ID, shape.text, shape.oversized)]);
    let (listed, words) = said(store);
    assert!(listed.summaries.is_empty(), "{name}");
    assert_eq!(listed.damage, ListDamage { incomplete: 0, unreadable: 1, folder_unreadable: false }, "{name}");
    assert_eq!(words.as_deref(), Some("1 file could not be read"), "{name}");
}

// case: $name: unreadable file count reaches the visible warning
#[test]
fn a_malformed_header_says_one_file_could_not_be_read() {
    says_unreadable(UNREADABLE[0]);
}

// case: $name: unreadable file count reaches the visible warning
#[test]
fn a_ledger_over_max_ledger_bytes_says_one_file_could_not_be_read() {
    says_unreadable(UNREADABLE[1]);
}

// case: $name: unreadable file count reaches the visible warning
#[test]
fn a_well_formed_invalid_header_says_one_file_could_not_be_read() {
    says_unreadable(UNREADABLE[2]);
}

// case: $name: unreadable file count reaches the visible warning
#[test]
fn a_truncated_header_is_one_file_that_could_not_be_read_not_two() {
    says_unreadable(UNREADABLE[3]);
}

// case: directory failure: real file at mission-ledger path renders unavailable, not verified
#[test]
fn a_file_where_the_records_folder_should_be_says_the_records_could_not_be_read() {
    for native_engine in [true, false] {
        let folder = Folder::new();
        fs::create_dir_all(folder.0.join("electron")).unwrap();
        let blocking = folder.0.join("electron/mission-ledger");
        fs::write(&blocking, "blocking file, not a directory").unwrap();
        let store = Store::new(folder.0.join("native"), folder.0.join("electron")).unwrap();
        let _g = hooks::install(store, None);
        let listed = list_recent(MAX_LISTED_MISSIONS, native_engine).expect("a folder that cannot be listed is said");
        assert!(listed.summaries.is_empty());
        assert!(listed.damage.folder_unreadable, "native engine {native_engine}");
        assert_eq!(listed.damage.words().as_deref(), Some("the records could not be read"));
        assert!(blocking.is_file());
        assert_eq!(fs::read_to_string(&blocking).unwrap(), "blocking file, not a directory");
    }
}

// case: mixed damage: two recovered incomplete receipts and one unreadable file remain separate
#[test]
fn two_incomplete_receipts_and_one_unreadable_file_are_said_apart() {
    let (_f, store) = records(&[
        (ID, prefix() + "{broken\n", false),
        ("mission_second", (prefix() + "{broken\n").replace(ID, "mission_second"), false),
        ("mission_header", r#"{"schemaVersion":"#.into(), false),
    ]);
    let (listed, words) = said(store);
    assert_eq!(listed.summaries.len(), 2);
    assert_eq!(listed.damage, ListDamage { incomplete: 2, unreadable: 1, folder_unreadable: false });
    assert_eq!(words.as_deref(), Some("2 with an incomplete receipt · 1 file could not be read"));
}

// case: does not call an older recoverable receipt unreadable when it falls off the page
#[test]
fn an_older_recoverable_receipt_is_listed_and_never_called_unreadable() {
    let mut files = vec![(ID, prefix() + "{broken\n", false)];
    let newer: Vec<String> = (0..20).map(|i| format!("mission_new_{i}")).collect();
    for id in &newer {
        files.push((id.as_str(), clean().replace(ID, id).replace(NOW, "2026-09-09T12:00:00.000Z"), false));
    }
    let (_f, store) = records(&files);
    let (listed, words) = said(store);
    assert_eq!(listed.summaries.len(), 21);
    assert_eq!(listed.summaries.last().map(|s| s.mission_id.as_str()), Some(ID));
    assert_eq!(listed.damage.unreadable, 0);
    assert_eq!(listed.damage.incomplete, 1);
    let words = words.unwrap_or_default();
    assert!(!words.contains("could not be read"), "{words}");
    assert!(words.contains("with an incomplete receipt"), "{words}");
}

#[test]
fn the_words_count_files_and_keep_the_two_facts_apart() {
    let words = |incomplete, unreadable| ListDamage { incomplete, unreadable, folder_unreadable: false }.words();
    assert_eq!(words(0, 0), None);
    assert_eq!(words(0, 2).as_deref(), Some("2 files could not be read"));
    assert_eq!(words(3, 0).as_deref(), Some("3 with an incomplete receipt"));
    assert_eq!(words(1, 1).as_deref(), Some("1 with an incomplete receipt · 1 file could not be read"));
    let folder = ListDamage { incomplete: 1, unreadable: 1, folder_unreadable: true };
    assert_eq!(folder.words().as_deref(), Some("the records could not be read"));
}

/// The turn this window is writing has a last line with no end for the moment of a write: the record arriving, not
/// an incomplete receipt. The same cut in a turn nobody here is writing is one (above).
#[test]
fn the_turn_this_window_is_writing_is_not_called_incomplete_mid_write() {
    let (_f, store) = records(&[(ID, shape(RECOVERING[0]).text, false), ("mission_second", (prefix() + "{broken\n").replace(ID, "mission_second"), false)]);
    let _g = hooks::install(store, None);
    with_running(|m| m.insert(ID.to_owned(), Arc::new(AtomicBool::new(false))));
    let listed = list_recent(MAX_LISTED_MISSIONS, true).unwrap();
    with_running(|m| m.remove(ID));
    assert_eq!(listed.summaries.len(), 2);
    assert_eq!(listed.damage.incomplete, 1, "only the turn with a bad record");
    assert_eq!(list_recent(MAX_LISTED_MISSIONS, true).unwrap().damage.incomplete, 2, "once nobody here writes it");
}
