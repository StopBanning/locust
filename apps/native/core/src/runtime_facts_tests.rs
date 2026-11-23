//! `what-the-clis-said-survives-a-relaunch.test.ts`: the file comes back on the next launch, and nothing here throws.
use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

fn root() -> PathBuf {
    let made = std::env::temp_dir().join(format!("locust-facts-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&made).unwrap();
    made
}

fn facts(version: &str, capability: &str) -> BinaryFacts {
    BinaryFacts { version_text: version.into(), capability_text: capability.into() }
}

fn store(at: &Path) -> RuntimeFactsStore {
    RuntimeFactsStore::new(at, Box::new(now_ms))
}

// case: reads back what it wrote
#[test]
fn what_one_launch_wrote_the_next_reads_back() {
    let directory = root();
    let first = store(&directory);
    first.load();
    first.set("claude:1000:10", facts("2.1.0", "--resume"));
    let second = store(&directory);
    second.load();
    assert_eq!(second.get("claude:1000:10"), Some(facts("2.1.0", "--resume")));
    assert_eq!(second.get("claude:1000:99"), None);
    let _ = std::fs::remove_dir_all(&directory);
}

// case: starts empty when there is no file, and when the file is rubbish
#[test]
fn no_file_and_a_rubbish_file_both_start_empty() {
    let directory = root();
    let fresh = store(&directory);
    fresh.load();
    assert_eq!(fresh.size(), 0);
    std::fs::write(directory.join(FILE_NAME), "{ not json").unwrap();
    let broken = store(&directory);
    broken.load();
    assert_eq!(broken.size(), 0);
    let _ = std::fs::remove_dir_all(&directory);
}

// case: drops an entry whose shape is wrong rather than handing it to the parser
#[test]
fn an_entry_of_the_wrong_shape_is_dropped() {
    let directory = root();
    let body = json!({
        "schemaVersion": 1,
        "binaries": [
            { "fingerprint": "good:1:1", "versionText": "1.0.0", "capabilityText": "help", "at": 1 },
            { "fingerprint": "bad:1:1", "versionText": 7, "capabilityText": "help", "at": 1 },
            { "fingerprint": "nohelp:1:1", "versionText": "1.0.0", "capabilityText": null, "at": 1 },
            { "fingerprint": "", "versionText": "1.0.0", "capabilityText": "help", "at": 1 },
            "not an entry"
        ]
    });
    std::fs::write(directory.join(FILE_NAME), body.to_string()).unwrap();
    let kept = store(&directory);
    kept.load();
    assert_eq!(kept.size(), 1);
    assert_eq!(kept.get("good:1:1").map(|f| f.version_text), Some("1.0.0".into()));
    let _ = std::fs::remove_dir_all(&directory);
}

// case: ignores a file written by a schema it does not know
#[test]
fn a_file_of_another_schema_is_ignored() {
    let directory = root();
    let body = json!({ "schemaVersion": 99, "binaries": [{ "fingerprint": "a:1:1", "versionText": "x", "capabilityText": "y", "at": 1 }] });
    std::fs::write(directory.join(FILE_NAME), body.to_string()).unwrap();
    let kept = store(&directory);
    kept.load();
    assert_eq!(kept.size(), 0);
    let _ = std::fs::remove_dir_all(&directory);
}

// case: keeps the newest when it is full, because those are the files on disk now
#[test]
fn a_full_store_keeps_the_newest() {
    let directory = root();
    let clock = Arc::new(AtomicU64::new(0));
    let ticks = clock.clone();
    let kept = RuntimeFactsStore::new(&directory, Box::new(move || ticks.fetch_add(1, Ordering::SeqCst) + 1));
    kept.load();
    for index in 0..MAX_REMEMBERED_BINARIES + 20 {
        kept.set(&format!("fp:{index}"), facts(&index.to_string(), "help"));
    }
    assert_eq!(kept.size(), MAX_REMEMBERED_BINARIES);
    // The first twenty were the oldest, so they are the ones that went.
    assert_eq!(kept.get("fp:0"), None);
    assert_eq!(kept.get("fp:19"), None);
    assert_eq!(kept.get("fp:20").map(|f| f.version_text), Some("20".into()));
    let written: Value = serde_json::from_str(&std::fs::read_to_string(directory.join(FILE_NAME)).unwrap()).unwrap();
    assert_eq!(written["binaries"].as_array().map(Vec::len), Some(MAX_REMEMBERED_BINARIES));
    // And the file read back is the newest two hundred, too.
    let again = store(&directory);
    again.load();
    assert_eq!((again.size(), again.get("fp:19"), again.get("fp:219").map(|f| f.version_text)), (MAX_REMEMBERED_BINARIES, None, Some("219".into())));
    let _ = std::fs::remove_dir_all(&directory);
}

// Not an Electron case of its own: a file holding more than the cap (written by hand or by an older build) is read
// only up to the cap, so the store never starts over it.
#[test]
fn a_file_longer_than_the_cap_is_read_only_up_to_it() {
    let directory = root();
    std::fs::create_dir_all(&directory).unwrap();
    let binaries: Vec<Value> = (0..MAX_REMEMBERED_BINARIES + 5)
        .map(|index| serde_json::json!({"fingerprint": format!("fp:{index}"), "versionText": index.to_string(), "capabilityText": "help", "at": index}))
        .collect();
    std::fs::write(directory.join(FILE_NAME), serde_json::json!({"schemaVersion": 1, "binaries": binaries}).to_string()).unwrap();
    let read = store(&directory);
    read.load();
    assert_eq!(read.size(), MAX_REMEMBERED_BINARIES);
    let _ = std::fs::remove_dir_all(&directory);
}

// case: does not rewrite the file when nothing changed
#[test]
fn the_same_facts_again_do_not_rewrite_the_file() {
    let directory = root();
    let kept = store(&directory);
    kept.load();
    kept.set("a:1:1", facts("1", "h"));
    assert!(std::fs::read_to_string(directory.join(FILE_NAME)).unwrap().contains("a:1:1"));
    // A marker in place of the file: any rewrite would replace it.
    std::fs::write(directory.join(FILE_NAME), "marker").unwrap();
    kept.set("a:1:1", facts("1", "h"));
    assert_eq!(std::fs::read_to_string(directory.join(FILE_NAME)).unwrap(), "marker");
    // The control: a change is written.
    kept.set("a:1:1", facts("2", "h"));
    assert!(std::fs::read_to_string(directory.join(FILE_NAME)).unwrap().contains("\"versionText\":\"2\""));
    let _ = std::fs::remove_dir_all(&directory);
}

// case: never throws when the directory it writes to is not there
#[test]
fn a_missing_folder_never_fails_and_memory_still_answers() {
    let gone = std::env::temp_dir().join("locust-facts-absent").join(format!("nope-{}", crate::invariants::unique_nanos()));
    let kept = store(&gone);
    kept.load();
    kept.set("a:1:1", facts("1", "h"));
    assert_eq!(kept.get("a:1:1").map(|f| f.version_text), Some("1".into()));
    assert!(!gone.exists());
}
