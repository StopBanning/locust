//! Electron's `stores-fail-closed.test.ts` (7 cases): a store that cannot reach the disk says so. The fault is real:
//! a plain file sits where the store's folder needs to be, so every write fails through the same calls a full or
//! read-only disk fails through.
use crate::store::Store;
use serde_json::json;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn base() -> (Temp, std::path::PathBuf) {
    let base = std::env::temp_dir().join(format!("locust-fault-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(base.join("electron")).unwrap();
    // Rust's routine save checks its teammate exists, which Electron's does not: Wren is on the roster in both roots,
    // with a folder to work in, so every call below gets as far as the disk.
    std::fs::create_dir_all(base.join("shop")).unwrap();
    std::fs::write(
        base.join("electron").join("teammates.json"),
        json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_wren", "name": "Wren", "role": "Code & Migrations", "hue": "lime", "createdAt": "2026-09-16T00:00:00.000Z", "folder": base.join("shop")}], "missionOwners": {}, "settings": {}}).to_string(),
    )
    .unwrap();
    (Temp(base.clone()), base)
}

/// A store whose root no write can get into: a file blocks its path.
fn unwritable() -> (Temp, Store) {
    let (t, base) = base();
    std::fs::write(base.join("blocked"), "this is a file where a directory is needed").unwrap();
    let store = Store::new(base.join("blocked"), base.join("electron")).unwrap();
    (t, store)
}

fn writable() -> (Temp, Store) {
    let (t, base) = base();
    std::fs::create_dir_all(base.join("native")).unwrap();
    let store = Store::new(base.join("native"), base.join("electron")).unwrap();
    (t, store)
}

fn teammate(store: &Store) -> Result<(), String> {
    crate::people::save_teammate(store, json!({"name": "Atlas", "hue": "lime", "role": "Code & Migrations"}))
}
fn settings(store: &Store) -> Result<(), String> {
    let mut s = crate::workspace_settings::WorkspaceSettings::default();
    s.relay = false;
    crate::workspace_settings::save(store, &s)
}
fn routine(store: &Store) -> Result<(), String> {
    crate::routines::save(
        store,
        json!({"name": "Nightly", "teammateId": "tm_wren", "route": {"runtime": "codex", "model": "gpt-6-astra", "mode": "accept-edits"}, "steps": ["Check the build."], "learnedFrom": []}),
    )
}
fn room(store: &Store) -> Result<(), String> {
    crate::room_store::create(store, "Standup", &["tm_wren".to_owned()]).map(|_| ())
}
fn memory(store: &Store) -> Result<(), String> {
    crate::people::add_memory(store, "The build is green on Windows only.", None, "global", "ws_1", json!({"name": "Colin"}), "kept")
}

// case: the fault is real: the root genuinely cannot be written into
#[test]
fn the_fault_is_real_the_root_genuinely_cannot_be_written_into() {
    let (_t, store) = unwritable();
    assert!(std::fs::write(store.root().join("probe.json"), "x").is_err());
}

// case: and the SAME calls all succeed on a root that works
#[test]
fn and_the_same_calls_all_succeed_on_a_root_that_works() {
    let calls: [(&str, fn(&Store) -> Result<(), String>); 5] =
        [("teammate", teammate), ("settings", settings), ("routine", routine), ("room", room), ("memory", memory)];
    for (what, call) in calls {
        let (_t, store) = writable();
        assert_eq!(call(&store), Ok(()), "{what}");
    }
}

// case: the teammate roster rejects rather than pretending to save
#[test]
fn the_teammate_roster_rejects_rather_than_pretending_to_save() {
    assert!(teammate(&unwritable().1).is_err());
}

// case: workspace settings reject rather than pretending to save
#[test]
fn workspace_settings_reject_rather_than_pretending_to_save() {
    assert!(settings(&unwritable().1).is_err());
}

// case: a routine rejects rather than pretending to save
#[test]
fn a_routine_rejects_rather_than_pretending_to_save() {
    assert!(routine(&unwritable().1).is_err());
}

// case: a room rejects rather than pretending to save
#[test]
fn a_room_rejects_rather_than_pretending_to_save() {
    assert!(room(&unwritable().1).is_err());
}

// case: a memory rejects rather than pretending to save
#[test]
fn a_memory_rejects_rather_than_pretending_to_save() {
    assert!(memory(&unwritable().1).is_err());
}
