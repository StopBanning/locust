// Carries apps/desktop/src/main/a-changed-memory-list-replaces-the-old-one.test.ts (0.418): a memory list sent again to
// a session that holds an earlier one says it replaces it, so an edited or forgotten memory is not read as still kept.
// Each case runs the real brief (`people::turn_brief_held`) twice, the second time as a resumed turn holding the first.
use crate::brief_sessions::MEMORY_LIST_REPLACES;
use crate::people::{add_memory, change_memory, memories, turn_brief_held, workspace_id};
use crate::store::Store;
use serde_json::json;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const PELICAN: &str = "The secret word for this project is PELICAN.";
const DIFFS: &str = "Colin wants diffs, not prose.";

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A temp data folder with Wren alone on the team, and a workspace folder named `shop`.
fn shop() -> (Temp, Store, PathBuf) {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("locust-memory-replaces-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let wren = json!({"teammateId": "tm_wren", "name": "Wren", "role": "Code & Migrations"});
    store.save_json("teammates.json", &json!({"schemaVersion":1,"teammates":[wren],"missionOwners":{},"settings":{}})).unwrap();
    let folder = root.join("shop");
    (Temp(root), store, folder)
}
fn remember(store: &Store, folder: &Path, text: &str) {
    add_memory(store, text, None, "workspace", &workspace_id(folder), json!({"name":"you"}), "kept").unwrap();
}
fn forget_all(store: &Store) {
    let file = memories(store).unwrap();
    for id in file["memories"].as_array().unwrap().iter().filter_map(|m| m["memoryId"].as_str()) {
        change_memory(store, id, "remove", None).unwrap();
    }
}
/// One turn's brief, and the paragraph keys it stands on; `held` is what the session was given before.
fn turn(store: &Store, folder: &Path, held: Option<&HashSet<String>>) -> (String, HashSet<String>) {
    let (prompt, _, given) =
        turn_brief_held(store, "tm_wren", folder, "Quote every remembered line.", None, None, Some("codex"), &[], 0, None, None, held).unwrap();
    (prompt, given.into_iter().collect())
}

#[test]
fn says_it_replaces_the_earlier_list_and_the_old_line_is_not_in_it() {
    let (_t, store, folder) = shop();
    remember(&store, &folder, PELICAN);
    let (_, given) = turn(&store, &folder, None);
    forget_all(&store);
    remember(&store, &folder, DIFFS);
    let (second, _) = turn(&store, &folder, Some(&given));
    assert!(second.contains(MEMORY_LIST_REPLACES), "{second}");
    assert!(second.contains(DIFFS), "{second}");
    assert!(!second.contains("PELICAN"), "{second}");
    // Said right above the list it is about.
    assert!(second.find(MEMORY_LIST_REPLACES).unwrap() < second.find("Colin wants diffs").unwrap());
}

#[test]
fn says_so_when_everything_was_forgotten_too() {
    let (_t, store, folder) = shop();
    remember(&store, &folder, PELICAN);
    let (_, given) = turn(&store, &folder, None);
    forget_all(&store);
    let (emptied, _) = turn(&store, &folder, Some(&given));
    assert!(emptied.contains(MEMORY_LIST_REPLACES), "{emptied}");
    assert!(emptied.contains("Nothing is remembered yet."), "{emptied}");
    assert!(!emptied.contains("PELICAN"), "{emptied}");
}

#[test]
fn is_not_said_on_a_first_brief_or_when_the_list_did_not_change() {
    let (_t, store, folder) = shop();
    remember(&store, &folder, PELICAN);
    let (first, given) = turn(&store, &folder, None);
    assert!(first.contains("PELICAN"), "the first brief carries the list: {first}");
    assert!(!first.contains(MEMORY_LIST_REPLACES));
    let (same, _) = turn(&store, &folder, Some(&given));
    assert!(!same.contains(MEMORY_LIST_REPLACES), "{same}");
    // Not sent again either: the session holds it.
    assert!(!same.contains("PELICAN"), "{same}");
}
