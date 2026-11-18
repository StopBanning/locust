//! Electron's `a-restored-queue-waits-for-send.test.ts` (Colin: "Never send it unasked after a relaunch: the person
//! presses Send."), the cases Rust builds: the queue's rules (`queue.rs`, Electron's `steering.ts`) over rows saved and
//! read back through the queue's file (`queue_store.rs`, the disk half of `createConversationQueue`). Electron's
//! values and words. The queue object itself (waiting on its first read, one Send at a time, the window's copy kept
//! until a write lands) is the window's, in `src/ui/composer_wiring.rs`, and is not checked here.
use super::*;
use crate::composer::queue_store::{read_queue, write_queue};
use locust_core::store::Store;

fn one() -> QueuedRow {
    QueuedRow { id: "q_one".into(), key: "run_one".into(), text: "Check it.".into(), origin: Origin::Person,
        attachments: vec!["notes.md".into()], issue: None, restored: false, held: None }
}
fn other() -> QueuedRow {
    QueuedRow { id: "q_other".into(), key: "run_other".into(), text: "Keep me.".into(), attachments: vec![], ..one() }
}

/// A store of its own in a fresh folder, with `saved` already on disk (`fixture(saved)`).
struct Disk {
    root: std::path::PathBuf,
    store: Store,
}
impl Disk {
    fn new(name: &str, saved: &[QueuedRow]) -> Disk {
        let root = std::env::temp_dir().join(format!("locust-restored-queue-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("electron")).unwrap();
        let store = Store::with_codex(root.join("native"), root.join("electron"), vec![]).unwrap();
        if !saved.is_empty() {
            write_queue(&store, saved).unwrap();
        }
        Disk { root, store }
    }
    fn read(&self) -> Vec<QueuedRow> {
        read_queue(&self.store).unwrap()
    }
    fn keys(&self) -> Vec<String> {
        self.read().into_iter().map(|r| r.key).collect()
    }
}
impl Drop for Disk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn verdict(running: bool, phase: Option<&str>, restored: bool, held: Option<&str>) -> QueuedVerdict {
    queued_verdict(VerdictInput { running, phase, on_screen: true, restored, held })
}

#[test]
fn restored_messages_wait_for_send_even_when_the_previous_turn_completed() {
    let disk = Disk::new("held", &[one(), other()]);
    let first = disk.read().remove(0);
    for phase in [Some("completed"), Some("interrupted"), Some("failed"), Some("cancelled"), Some("running"), None] {
        assert_eq!(verdict(phase == Some("running"), phase, first.restored, None),
            QueuedVerdict::Held { note: QUEUE_RESTORED_NOTE.into() }, "{phase:?}");
    }
}

#[test]
fn a_newly_queued_message_still_sends_when_its_turn_completes_in_the_same_session() {
    // Queued in this session: the row the window holds, not one read back from the file.
    assert_eq!(verdict(false, Some("completed"), one().restored, None), QueuedVerdict::Send);
}

#[test]
fn edit_and_discard_remove_only_their_conversation_durably() {
    let disk = Disk::new("discard", &[one(), other()]);
    let seen = disk.read();
    assert_eq!(queued_in(&seen, Some("run_one"))[0].attachments, ["notes.md"]);
    write_queue(&disk.store, &without_queue_of(&seen, Some("run_one"))).unwrap();
    assert_eq!(disk.keys(), ["run_other"]);
    write_queue(&disk.store, &without_queue_of(&disk.read(), Some("run_other"))).unwrap();
    assert_eq!(disk.read(), []);
}

#[test]
fn an_explicit_send_takes_the_restored_words_and_files_without_releasing_another_conversation() {
    let disk = Disk::new("send", &[one(), other()]);
    let (going, rest) = take_next(&disk.read(), "run_one");
    assert_eq!(going, Some(QueuedRow { restored: true, ..one() }));
    write_queue(&disk.store, &rest).unwrap();
    assert_eq!(disk.keys(), ["run_other"]);
}

#[test]
fn rekeyed_messages_reopen_under_the_acknowledged_run_instead_of_a_temporary_key() {
    let disk = Disk::new("rekeyed", &[]);
    write_queue(&disk.store, &[one()]).unwrap();
    write_queue(&disk.store, &requeued_rows(&disk.read(), "run_one", "run_named")).unwrap();
    let reopened = disk.read();
    assert!(queued_in(&reopened, Some("run_one")).is_empty());
    let named = queued_in(&reopened, Some("run_named"))[0];
    assert_eq!((named.text.as_str(), named.restored), ("Check it.", true));
}

#[test]
fn a_new_automatic_queue_row_cannot_absorb_a_restored_message() {
    let restored = QueuedRow { id: "restored".into(), restored: true, attachments: vec![], ..one() };
    assert_eq!(combine_queued(&[one(), restored]).len(), 2);
}

/// Electron's queue refuses the update and writes nothing; Rust's writer refuses to replace a file it cannot read.
#[test]
fn an_unreadable_queue_refuses_updates_instead_of_silently_replacing_saved_words() {
    let disk = Disk::new("unreadable", &[other()]);
    let path = crate::composer::queue_store::queue_path(&disk.store).unwrap();
    std::fs::write(&path, "{ not a queue").unwrap();
    assert!(read_queue(&disk.store).is_err());
    assert!(write_queue(&disk.store, &[one()]).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not a queue");
}

/// The case's disk half; that the window's copy keeps both rows is `composer_wiring.rs`'s.
#[test]
fn only_person_written_messages_are_persisted_rather_than_host_scheduling_intents() {
    let disk = Disk::new("person", &[]);
    write_queue(&disk.store, &[one(), QueuedRow { origin: Origin::Host, ..other() }]).unwrap();
    assert_eq!(disk.read(), [QueuedRow { restored: true, ..one() }]);
}

/// The case's verdict half: a row held by a refused Send stays held. That a refused Send holds it, with
/// "not sent — press Send to try again", is the window's (`composer_wiring.rs`), and Rust has no such words.
#[test]
fn a_held_row_waits_for_send_after_its_turn_completes() {
    assert_eq!(verdict(false, Some("completed"), false, Some("not sent — press Send to try again")),
        QueuedVerdict::Held { note: "not sent — press Send to try again".into() });
}
