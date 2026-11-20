//! Electron's `unreadable-is-not-empty.test.ts` (4 cases): a rooms file that cannot be read never says a room does
//! not exist, and changes nothing; a file simply not there is still an empty list.
use super::*;

struct Temp(std::path::PathBuf, Store);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp() -> Temp {
    let dir = std::env::temp_dir().join(format!("locust-rooms-unreadable-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(dir.join("root")).unwrap();
    std::fs::create_dir_all(dir.join("electron")).unwrap();
    let store = Store::new(dir.join("root"), dir.join("electron")).unwrap();
    Temp(dir, store)
}

/// The shape Electron's case used: the rooms file replaced by a directory.
fn unreadable() -> Temp {
    let t = temp();
    std::fs::create_dir_all(t.1.rooms_json_path().unwrap()).unwrap();
    t
}

#[test]
fn never_reports_that_a_room_does_not_exist() {
    let t = unreadable();
    let why = rename(&t.1, "room_abc", "Release-renamed").unwrap_err();
    assert!(why.to_lowercase().contains("could not be read"), "{why}");
    assert!(!why.contains("does not exist"), "{why}");
}

#[test]
fn says_every_room_is_as_it_was_because_nothing_was_established() {
    let t = unreadable();
    let why = rename(&t.1, "room_abc", "x").unwrap_err();
    assert!(why.contains("nothing was changed"), "{why}");
    assert!(why.contains("as it was"), "{why}");
}

#[test]
fn still_treats_a_file_that_is_simply_not_there_as_empty() {
    assert!(list(&temp().1).unwrap().is_empty());
}

#[test]
fn still_reports_a_genuinely_missing_room_as_missing() {
    let t = temp();
    std::fs::write(t.1.rooms_json_path().unwrap(), serde_json::json!({"schemaVersion": 1, "rooms": []}).to_string()).unwrap();
    assert!(rename(&t.1, "room_gone", "x").unwrap_err().contains("does not exist"));
}
