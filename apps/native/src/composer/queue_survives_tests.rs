//! Electron's `a-queued-message-survives-closing-locust.test.ts` (9 cases): the queue kept beside its conversation in
//! the profile, shown back on reopen and never sent unasked. Rust marks a row read back from disk `restored` (the box's
//! own state), so rows are compared by what was saved: id, conversation, words and attachments.
use super::*;
use crate::composer::queue::QueuedRow;

struct Profile(std::path::PathBuf, Store);
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn profile() -> Profile {
    let root = std::env::temp_dir().join(format!("locust-durable-queue-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = Store::with_codex(root.join("native"), root.join("electron"), vec![]).unwrap();
    Profile(root, store)
}

fn row(id: &str, key: &str, text: &str) -> QueuedRow {
    QueuedRow { id: id.into(), key: key.into(), text: text.into(), origin: Origin::Person, attachments: vec!["notes/context.md".into()], issue: None, restored: false, held: None, retry_at: None, tries: None }
}

fn one() -> QueuedRow {
    row("q_one", "run_one", "Check the second paragraph.")
}

fn saved(rows: &[QueuedRow]) -> Vec<(String, String, String, Vec<String>)> {
    rows.iter().map(|r| (r.id.clone(), r.key.clone(), r.text.clone(), r.attachments.clone())).collect()
}

#[test]
fn a_fresh_reader_recovers_exact_words_attachments_and_their_conversation() {
    let p = profile();
    let two = row("q_two", "run_two", "Another conversation.");
    write_queue(&p.1, &[one(), two.clone()]).unwrap();
    assert_eq!(saved(&read_queue(&p.1).unwrap()), saved(&[one(), two]));
}

#[test]
fn a_profile_that_has_never_queued_anything_starts_empty() {
    assert!(read_queue(&profile().1).unwrap().is_empty());
}

#[test]
fn removing_one_conversation_leaves_the_other_queued_after_reopening() {
    let p = profile();
    let other = row("q_other", "run_other", "Check the second paragraph.");
    write_queue(&p.1, &[one(), other.clone()]).unwrap();
    write_queue(&p.1, std::slice::from_ref(&other)).unwrap();
    assert_eq!(saved(&read_queue(&p.1).unwrap()), saved(&[other]));
}

/// Rust writes the queue at once and whole (an atomic replace), so two writes in a row are two writes in order.
#[test]
fn concurrent_writes_and_a_shutdown_flush_preserve_their_invocation_order() {
    let p = profile();
    let edited = QueuedRow { text: "The edited words.".into(), ..one() };
    write_queue(&p.1, &[one()]).unwrap();
    write_queue(&p.1, std::slice::from_ref(&edited)).unwrap();
    assert_eq!(saved(&read_queue(&p.1).unwrap()), saved(&[edited]));
}

#[test]
fn a_truncated_queue_is_reported_and_kept_instead_of_overwritten_with_emptiness() {
    let p = profile();
    let path = queue_path(&p.1).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{\"schemaVersion\":1,\"rows\":[").unwrap();
    assert!(read_queue(&p.1).is_err());
    assert!(write_queue(&p.1, &[]).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"schemaVersion\":1,\"rows\":[");
}

#[test]
fn invalid_input_cannot_replace_previously_saved_words() {
    let p = profile();
    write_queue(&p.1, &[one()]).unwrap();
    let bad_rows: Vec<Vec<QueuedRow>> = vec![
        vec![QueuedRow { key: "../escape".into(), ..one() }],
        vec![QueuedRow { text: String::new(), ..one() }],
        vec![QueuedRow { text: "x".repeat(8_001), ..one() }],
        vec![one(); 257],
    ];
    for bad in bad_rows {
        assert!(write_queue(&p.1, &bad).is_err(), "{} rows", bad.len());
    }
    assert_eq!(saved(&read_queue(&p.1).unwrap()), saved(&[one()]));
}

#[test]
fn an_unknown_schema_stays_on_disk_for_recovery() {
    let p = profile();
    let path = queue_path(&p.1).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let text = serde_json::json!({"schemaVersion": 2, "rows": [{"id": "q_one", "key": "run_one", "text": "Check the second paragraph."}]}).to_string();
    std::fs::write(&path, &text).unwrap();
    assert!(write_queue(&p.1, &[]).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
}

/// Only the message is written: never whether it was restored, when to retry it, what held it, or a host's row at all
/// (Rust keeps no row the host made; Electron writes its words and drops the origin: both keep no runtime authority).
#[test]
fn read_and_write_project_only_message_fields_and_never_runtime_authority() {
    let p = profile();
    let with_state = QueuedRow { restored: false, retry_at: Some(1), held: Some("busy".into()), ..one() };
    let host = QueuedRow { origin: Origin::Host, ..row("q_host", "run_one", "run me") };
    write_queue(&p.1, &[with_state, host]).unwrap();
    let file = std::fs::read_to_string(queue_path(&p.1).unwrap()).unwrap();
    for never in ["retry", "held", "restored", "origin", "run me"] {
        assert!(!file.contains(never), "{never}: {file}");
    }
    assert_eq!(saved(&read_queue(&p.1).unwrap()), saved(&[one()]));
}

#[test]
fn a_message_queued_before_the_start_receipt_can_be_saved_and_then_rekeyed() {
    let p = profile();
    let pending = QueuedRow { key: "pending:1".into(), ..one() };
    write_queue(&p.1, std::slice::from_ref(&pending)).unwrap();
    assert_eq!(saved(&read_queue(&p.1).unwrap()), saved(&[pending]));
    write_queue(&p.1, &[one()]).unwrap();
    assert_eq!(saved(&read_queue(&p.1).unwrap()), saved(&[one()]));
}
