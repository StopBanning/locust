//! Electron's `an-attention-mark-says-when-you-were-last-here.test.ts` (2 cases): the moment the window last had the
//! person is written whole and read back; missing or unreadable means nobody was here before, with no guess.
use super::*;

struct Temp(std::path::PathBuf, Store);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store() -> Temp {
    let root = std::env::temp_dir().join(format!("locust-mark-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    Temp(root, store)
}

fn leftovers(store: &Store) -> Vec<String> {
    std::fs::read_dir(store.root())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect()
}

// case: reads back what was written, making the folder on the way, and leaves no half-written file
#[test]
fn reads_back_what_was_written_and_leaves_no_half_written_file() {
    let t = store();
    assert_eq!(read_attention_mark(&t.1), None);
    write_attention_mark(&t.1, "2026-10-04T01:00:00.000Z").unwrap();
    assert_eq!(read_attention_mark(&t.1).as_deref(), Some("2026-10-04T01:00:00.000Z"));
    let written: Value = serde_json::from_str(&std::fs::read_to_string(t.1.root().join(AWAY_FILE)).unwrap()).unwrap();
    assert_eq!(written, json!({"at": "2026-10-04T01:00:00.000Z"}));
    assert!(leftovers(&t.1).is_empty(), "{:?}", leftovers(&t.1));
    write_attention_mark(&t.1, "2026-10-04T02:00:00.000Z").unwrap();
    assert_eq!(read_attention_mark(&t.1).as_deref(), Some("2026-10-04T02:00:00.000Z"));
    assert!(leftovers(&t.1).is_empty());
}

// case: treats a file it cannot read as no mark (control)
#[test]
fn treats_a_file_it_cannot_read_as_no_mark() {
    let t = store();
    write_attention_mark(&t.1, "2026-10-04T01:00:00.000Z").unwrap();
    std::fs::write(t.1.root().join(AWAY_FILE), "{ not json").unwrap();
    assert_eq!(read_attention_mark(&t.1), None);
    assert_eq!(attention_mark_from(&json!({"at": "yesterday"})), None);
    assert_eq!(attention_mark_from(&json!({"at": 12})), None);
    assert_eq!(attention_mark_from(&Value::Null), None);
    assert_eq!(attention_mark_from(&json!({"at": "2026-10-04T01:00:00.000Z"})).as_deref(), Some("2026-10-04T01:00:00.000Z"));
}
