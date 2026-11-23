//! the-storage-report-reads-only-what-changed.test.ts: each file's date is kept by its size and modified time. Proved
//! by what the report says, not by counting reads: a file overwritten with garbage of the same size, its modified time
//! put back, still reports its old date, which it can only do if it was not read again.
use super::*;
use serde_json::json;
use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};

const OLD: &str = "2026-08-01T10:00:00.000Z";
const RECENT: &str = "2026-09-20T10:00:00.000Z";

/// Every file's modified time pinned to a whole second before anything reads it, so it can be put back exactly.
fn stamp() -> SystemTime {
    UNIX_EPOCH + StdDuration::from_secs(1_789_948_800) // 2026-09-21T00:00:00Z
}

fn pin(path: &Path) {
    fs::File::options().write(true).open(path).unwrap().set_modified(stamp()).unwrap();
}

fn seeded() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "locust-storage-{}-{}",
        std::process::id(),
        crate::native::uuid_v4().unwrap_or_else(|_| "0".into())
    ));
    fs::create_dir_all(&root).unwrap();
    for (id, created) in [("mission_old", OLD), ("mission_recent", RECENT)] {
        let line = json!({
            "schemaVersion": 27,
            "recordType": "mission.created",
            "ledgerSequence": 1,
            "occurredAt": created,
            "metadata": {"missionId": id, "runId": format!("run_{id}"), "prompt": "Inspect the workspace without changing it.", "runtime": "codex", "model": "account-default", "createdAt": created}
        });
        let path = root.join(format!("{id}.jsonl"));
        fs::write(&path, format!("{line}\n")).unwrap();
        pin(&path);
    }
    root
}

/// Same byte count, different bytes, the modified time put back.
fn garble_keeping_stamp(path: &Path) {
    let before = fs::metadata(path).unwrap();
    fs::write(path, "x".repeat(before.len() as usize)).unwrap();
    pin(path);
    let after = fs::metadata(path).unwrap();
    assert_eq!(after.len(), before.len());
    assert_eq!(after.modified().unwrap(), before.modified().unwrap());
}

fn says(report: &StorageReport) -> (usize, usize, Option<&str>) {
    (report.mission_count, report.unreadable_count, report.oldest_updated_at.as_deref())
}

#[test]
fn does_not_read_an_unchanged_file_again() {
    // case: does not read an unchanged file again
    let root = seeded();
    let mut ledger = StorageReader::default();
    assert_eq!(says(&ledger.report(&root).unwrap()), (2, 0, Some(OLD)));
    garble_keeping_stamp(&root.join("mission_old.jsonl"));
    // Read again, the garbled file would be unreadable and the oldest date would jump to the recent mission.
    assert_eq!(says(&ledger.report(&root).unwrap()), (2, 0, Some(OLD)));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reads_a_file_again_when_it_changed_and_forgets_one_that_is_gone() {
    // case: reads a file again when it changed, and forgets one that is gone
    let root = seeded();
    let mut ledger = StorageReader::default();
    assert_eq!(ledger.report(&root).unwrap().oldest_updated_at.as_deref(), Some(OLD));
    // A different size: this one must be read again, and now has no date.
    fs::write(root.join("mission_old.jsonl"), "not a ledger\n").unwrap();
    assert_eq!(says(&ledger.report(&root).unwrap()), (2, 1, Some(RECENT)));
    fs::remove_file(root.join("mission_old.jsonl")).unwrap();
    assert_eq!(says(&ledger.report(&root).unwrap()), (1, 0, Some(RECENT)));
    assert_eq!(ledger.dates.len(), 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_fresh_ledger_object_reads_everything_so_nothing_stale_outlives_the_app() {
    // case: a fresh ledger object reads everything, so nothing stale outlives the app
    let root = seeded();
    StorageReader::default().report(&root).unwrap();
    garble_keeping_stamp(&root.join("mission_old.jsonl"));
    assert_eq!(says(&StorageReader::default().report(&root).unwrap()), (2, 1, Some(RECENT)));
    assert_eq!(says(&read_storage_report(&root).unwrap()), (2, 1, Some(RECENT)));
    let _ = fs::remove_dir_all(root);
}
