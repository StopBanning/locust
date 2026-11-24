// Carries apps/desktop/src/shared/a-rewritten-memory-is-new.test.ts (Electron 4ce2f37a): a memory rewritten in place
// is as new as its last write. Through what a turn's brief is built from: the store's kept memories for the folder
// (`people::briefed_rows`, which now puts them in order of their last write, as Electron's `byLastWritten`) and the
// brief itself (`people::brief`).
use crate::people::{add_memory, brief, briefed_rows, workspace_id};
use crate::store::Store;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> (Temp, Store, PathBuf) {
    let root = std::env::temp_dir().join(format!("locust-rewritten-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let folder = root.join("shop");
    (Temp(root), store, folder)
}

/// Keeps each memory in the store's order, then dates it as written: (text, by, createdAt, updatedAt).
fn keep(store: &Store, folder: &Path, rows: &[(&str, &str, &str, Option<&str>)]) {
    let ws = workspace_id(folder);
    for (text, by, _, _) in rows {
        add_memory(store, text, None, "workspace", &ws, json!({ "name": by }), "kept").unwrap();
    }
    let path = store.root().join("memories.json");
    let mut file: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (memory, (text, _, created, updated)) in file["memories"].as_array_mut().unwrap().iter_mut().zip(rows) {
        assert_eq!(memory["text"], *text);
        memory["createdAt"] = json!(created);
        match updated {
            Some(at) => memory["updatedAt"] = json!(at),
            None => {
                memory.as_object_mut().unwrap().remove("updatedAt");
            }
        }
    }
    std::fs::write(&path, file.to_string()).unwrap();
}
fn ago(days: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339()
}
fn line_with<'a>(text: &'a str, words: &str) -> &'a str {
    text.lines().find(|l| l.contains(words)).unwrap_or_else(|| panic!("no line with {words}: {text}"))
}

const STATUS: &str = "Orb suite: 31/31 passing.";
const OLDER: &str = "Orb suite: 29/31, two flaky.";

// case: is dated by its last write
#[test]
fn a_rewritten_memory_is_dated_by_when_it_was_last_written() {
    let (_t, store, folder) = fixture();
    let (made, rewritten, older) = (ago(23), ago(0), ago(10));
    keep(&store, &folder, &[(STATUS, "Yurt", &made, Some(&rewritten)), (OLDER, "Juno", &older, None)]);
    let rows = briefed_rows(&store, &folder, None).unwrap();
    let dated = |text: &str| rows.iter().find(|m| m["text"] == text).map(|m| crate::memory_file::last_written(m).unwrap().to_owned());
    assert_eq!(dated(STATUS), Some(rewritten));
    assert_eq!(dated(OLDER), Some(older));
    let said = brief(&store, "unknown", &folder, "hello").unwrap();
    assert!(line_with(&said, "31/31").contains("today"), "{said}");
    assert!(line_with(&said, "29/31").contains("10 days ago"), "{said}");
}

// case: sorts as the newest, whatever its first date
#[test]
fn a_rewritten_memory_sorts_as_the_newest_whatever_it_was_first_kept() {
    let (_t, store, folder) = fixture();
    // Stored order: the status first (kept on the 1st), the older note after.
    keep(&store, &folder, &[
        (STATUS, "Yurt", "2026-09-01T10:00:00.000Z", Some("2026-09-24T05:00:00.000Z")),
        (OLDER, "Juno", "2026-09-10T10:00:00.000Z", None),
    ]);
    let by: Vec<Value> = briefed_rows(&store, &folder, None).unwrap().iter().map(|m| m["by"]["name"].clone()).collect();
    assert_eq!(by, [json!("Juno"), json!("Yurt")]);
}

// case: leads the brief as the newest, and says it is from today
#[test]
fn a_memory_rewritten_today_leads_the_brief_and_says_today() {
    let (_t, store, folder) = fixture();
    let (made, rewritten, older) = (ago(23), ago(0), ago(10));
    keep(&store, &folder, &[(STATUS, "Yurt", &made, Some(&rewritten)), (OLDER, "Juno", &older, None)]);
    let said = brief(&store, "unknown", &folder, "hello").unwrap();
    let at = |words: &str| said.find(words).unwrap_or_else(|| panic!("{words}: {said}"));
    // With nothing asked that either shares, the brief opens with the newest: the rewritten status.
    assert!(at("31/31") < at("29/31"), "{said}");
    let status = line_with(&said, "31/31");
    assert!(!regex::Regex::new(r"weeks? ago|days? ago").unwrap().is_match(status), "{status}");
}

// case: keeps the store’s order for memories written at the same moment
#[test]
fn memories_written_at_one_moment_keep_the_stores_order() {
    let (_t, store, folder) = fixture();
    keep(&store, &folder, &[("a", "Juno", "2026-09-10T10:00:00.000Z", None), ("b", "Juno", "2026-09-10T10:00:00.000Z", None)]);
    let texts: Vec<Value> = briefed_rows(&store, &folder, None).unwrap().iter().map(|m| m["text"].clone()).collect();
    assert_eq!(texts, [json!("a"), json!("b")]);
}
