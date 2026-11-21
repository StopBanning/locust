//! Electron's `main/torn-ledger.test.ts`, the listing half (mission-history's counts): a folder's damage is counted
//! over every file in it, before the page limit, from the size/mtime cache. An unreadable file is one file however
//! many issues it raised; a recovered conversation with a bad record is listed and counted as incomplete; a file
//! that recovered is never called unreadable, however old. The words are `native::ListDamage`'s
//! (`native/torn_words_tests`).
use super::*;
use crate::ledger::torn_ledger_tests::{clean, prefix, shape, Folder, ID, NOW, RECOVERING, UNREADABLE};

fn listed(folder: &Folder, limit: usize) -> (Vec<String>, Damage) {
    let (summaries, damage) = HistoryCache::new().list_recent_with_damage(&folder.0, limit).unwrap();
    (summaries.into_iter().map(|s| s.mission_id).collect(), damage)
}

fn incomplete(damage: &Damage) -> Vec<String> {
    let mut ids: Vec<String> = damage.incomplete.iter().cloned().collect();
    ids.sort();
    ids
}

fn clean_as(id: &str) -> String {
    clean().replace(ID, id)
}

#[test]
fn a_clean_folder_counts_nothing() {
    let folder = Folder::new();
    folder.write(ID, &clean(), false);
    let (ids, damage) = listed(&folder, 20);
    assert_eq!(ids, [ID]);
    assert_eq!(damage, Damage::default());
}

#[test]
fn an_invalid_header_beside_a_clean_ledger_is_one_unreadable_file_and_the_clean_one_is_listed() {
    let folder = Folder::new();
    folder.write(ID, "{broken\n", false);
    folder.write("mission_clean", &clean_as("mission_clean"), false);
    let (ids, damage) = listed(&folder, 20);
    assert_eq!(ids, ["mission_clean"]);
    assert_eq!(damage.unreadable, 1);
    assert!(damage.incomplete.is_empty(), "the clean neighbour is not tainted");
}

#[test]
fn every_recovering_shape_is_listed_and_counted_incomplete() {
    for name in RECOVERING {
        let folder = Folder::new();
        folder.write(ID, &shape(name).text, false);
        let (ids, damage) = listed(&folder, 20);
        assert_eq!(ids, [ID], "{name}");
        assert_eq!(damage.unreadable, 0, "{name}");
        assert_eq!(incomplete(&damage), [ID], "{name}");
    }
}

#[test]
fn every_unreadable_shape_is_one_file_and_lists_nothing() {
    for name in UNREADABLE {
        let shape = shape(name);
        let folder = Folder::new();
        folder.write(ID, &shape.text, shape.oversized);
        let (ids, damage) = listed(&folder, 20);
        assert!(ids.is_empty(), "{name}");
        assert_eq!(damage.unreadable, 1, "{name}: one file, however many issues it raised");
        assert!(damage.incomplete.is_empty(), "{name}");
    }
}

#[test]
fn mixed_damage_keeps_two_incomplete_and_one_unreadable_apart() {
    let folder = Folder::new();
    folder.write(ID, &(prefix() + "{broken\n"), false);
    folder.write("mission_second", &(prefix() + "{broken\n").replace(ID, "mission_second"), false);
    folder.write("mission_header", r#"{"schemaVersion":"#, false);
    let (ids, damage) = listed(&folder, 20);
    assert_eq!(ids.len(), 2);
    assert_eq!(incomplete(&damage), ["mission_second", ID]);
    assert_eq!(damage.unreadable, 1);
}

/// Twenty newer clean conversations beside an older one with a bad record: the older one recovered, so it is never
/// "could not be read", and it is counted incomplete wherever it falls.
#[test]
fn an_older_recovered_file_past_the_limit_is_never_unreadable() {
    let folder = Folder::new();
    folder.write(ID, &(prefix() + "{broken\n"), false);
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3_600);
    fs::File::options().write(true).open(folder.0.join(format!("{ID}.jsonl"))).unwrap().set_modified(old).unwrap();
    for i in 0..20 {
        let id = format!("mission_new_{i}");
        folder.write(&id, &clean_as(&id).replace(NOW, "2026-09-09T12:00:00.000Z"), false);
    }
    let (ids, damage) = listed(&folder, crate::native::MAX_LISTED_MISSIONS);
    assert_eq!(ids.len(), 21);
    assert_eq!(ids.last().map(String::as_str), Some(ID));
    assert_eq!(damage.unreadable, 0);
    assert_eq!(incomplete(&damage), [ID]);
    // Off a page of twenty it is still not unreadable.
    let (ids, damage) = listed(&folder, 20);
    assert_eq!(ids.len(), 20);
    assert!(!ids.contains(&ID.to_owned()));
    assert_eq!(damage.unreadable, 0);
}

/// The other way round: a file that could not be read is counted however far past the limit it is.
#[test]
fn an_unreadable_file_past_the_limit_is_still_counted() {
    let folder = Folder::new();
    folder.write("mission_header", r#"{"schemaVersion":"#, false);
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3_600);
    fs::File::options().write(true).open(folder.0.join("mission_header.jsonl")).unwrap().set_modified(old).unwrap();
    for i in 0..3 {
        let id = format!("mission_new_{i}");
        folder.write(&id, &clean_as(&id), false);
    }
    let (ids, damage) = listed(&folder, 2);
    assert_eq!(ids.len(), 2);
    assert_eq!(damage.unreadable, 1);
}

/// A cut last line is counted as Electron counts it, and marked as the one fault a turn being written also has for
/// the moment of a write, so the listing can leave out the turn this window is writing (`native::list_recent`).
#[test]
fn a_cut_last_line_is_counted_and_marked_as_the_only_fault() {
    let folder = Folder::new();
    folder.write(ID, &shape("truncated final JSON").text, false);
    folder.write("mission_second", &(prefix() + "{broken\n").replace(ID, "mission_second"), false);
    let (_, damage) = listed(&folder, 20);
    assert_eq!(incomplete(&damage), ["mission_second", ID]);
    assert_eq!(damage.cut_tail_only, HashSet::from([ID.to_owned()]), "an invalid record is never only a cut tail");
}

/// Locust's CPU budget: the counts come from the cache, so an unchanged file is not read again to count it.
#[test]
fn the_counts_come_from_the_cache_without_reading_an_unchanged_file_again() {
    let folder = Folder::new();
    folder.write(ID, &(prefix() + "{broken\n"), false);
    folder.write("mission_header", r#"{"schemaVersion":"#, false);
    folder.write("mission_clean", &clean_as("mission_clean"), false);
    let cache = HistoryCache::new();
    let (_, first, damage) = cache.read_history_at(&folder.0, 1, std::time::SystemTime::now()).unwrap();
    assert_eq!(first.len(), 3, "each file is read once, past the limit too");
    assert_eq!((damage.unreadable, damage.incomplete.len()), (1, 1));
    let (_, again, damage) = cache.read_history_at(&folder.0, 1, std::time::SystemTime::now()).unwrap();
    assert!(again.is_empty(), "nothing changed on disk: {again:?}");
    assert_eq!((damage.unreadable, damage.incomplete.len()), (1, 1));
}

/// A records folder that is there and cannot be listed is an error, not an empty folder.
#[test]
fn a_file_where_the_folder_should_be_cannot_be_listed() {
    let folder = Folder::new();
    let blocking = folder.0.join("mission-ledger");
    fs::write(&blocking, "blocking file, not a directory").unwrap();
    let error = HistoryCache::new().list_recent_with_damage(&blocking, 20).unwrap_err();
    assert_ne!(error.kind(), io::ErrorKind::NotFound);
    assert_eq!(fs::read_to_string(&blocking).unwrap(), "blocking file, not a directory");
}

/// A file there that could not be opened is a file that could not be read; one gone before it was read is not there.
#[test]
fn a_file_that_cannot_be_opened_is_counted_unreadable() {
    use std::os::unix::fs::PermissionsExt;
    let folder = Folder::new();
    let path = folder.write(ID, &clean(), false);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let opened = fs::File::open(&path).is_ok();
    let (ids, damage) = listed(&folder, 20);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    if opened {
        return; // run as a user every file opens for: nothing to show
    }
    assert!(ids.is_empty());
    assert_eq!(damage.unreadable, 1);
}
