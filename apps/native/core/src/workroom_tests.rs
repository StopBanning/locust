//! Electron's packages/mission-store/test/workroom.test.ts.
use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

const NOW: &str = "2026-09-01T16:00:00.000Z";

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch() -> Scratch {
    Scratch(std::env::temp_dir().join(format!("teammate-workroom-{}-{}", std::process::id(), crate::invariants::unique_nanos())))
}

fn atlas() -> Party {
    Party { teammate_id: "tm_atlas".into(), name: "Atlas".into() }
}
fn wren() -> Party {
    Party { teammate_id: "tm_wren".into(), name: "Wren".into() }
}

fn workroom_at(root: &Path) -> Workroom {
    let next = AtomicU32::new(0);
    Workroom::with(root, Box::new(|| NOW.to_owned()), Box::new(move || format!("id{}", next.fetch_add(1, Ordering::SeqCst) + 1)))
}

fn post_from_atlas(w: &Workroom, text: &str, mission: &str) -> Result<Message, String> {
    w.post(&atlas(), mission, &wren(), text, None)
}

fn append_raw(root: &Path, text: &str) {
    let mut f = std::fs::OpenOptions::new().append(true).create(true).open(root.join(FILE_NAME)).unwrap();
    f.write_all(text.as_bytes()).unwrap();
}

fn codes(s: &Snapshot) -> Vec<IssueCode> {
    s.issues.iter().map(|i| i.code).collect()
}

#[test]
fn counts_more_from_the_paths_that_are_paths() {
    let s = scratch();
    let w = workroom_at(&s.0);
    let mut paths: Vec<String> = (0..45).map(|at| format!("src/file{at}.ts")).collect();
    paths.extend((0..10).map(|_| "x".repeat(5_000)));
    let posted = w.post(&atlas(), "mission_a1", &wren(), "Changed these.", Some(&paths)).unwrap();
    let observed = posted.observed.unwrap();
    assert_eq!(observed.len(), 41);
    assert_eq!(observed.last().unwrap(), "+5 more");
}

#[test]
fn records_an_attributed_routed_message_and_reads_it_back() {
    let s = scratch();
    let w = workroom_at(&s.0);
    let posted = post_from_atlas(&w, "The build runs through pnpm check.", "mission_a1").unwrap();
    assert_eq!(
        posted,
        Message {
            message_id: "wm_id1".into(),
            sequence: 1,
            from: atlas(),
            from_mission: "mission_a1".into(),
            to: wren(),
            text: "The build runs through pnpm check.".into(),
            posted_at: NOW.into(),
            observed: None
        }
    );
    let snapshot = w.read();
    assert!(snapshot.issues.is_empty());
    assert_eq!(snapshot.messages, vec![posted]);
}

#[test]
fn shows_a_message_to_its_recipient_once_and_never_to_anyone_else() {
    let s = scratch();
    let w = workroom_at(&s.0);
    let first = post_from_atlas(&w, "first", "mission_a1").unwrap();
    let second = post_from_atlas(&w, "second", "mission_a1").unwrap();
    assert_eq!(w.unread("tm_atlas", 10, &[]).unwrap(), Unread::default());
    assert_eq!(w.unread("tm_wren", 10, &[]).unwrap(), Unread { messages: vec![first.clone(), second.clone()], remaining: 0 });
    w.mark_delivered(std::slice::from_ref(&first.message_id), "mission_w1").unwrap();
    assert_eq!(w.unread("tm_wren", 10, &[]).unwrap(), Unread { messages: vec![second], remaining: 0 });
    assert_eq!(w.read().deliveries, vec![Delivery { message_id: first.message_id, mission_id: "mission_w1".into(), delivered_at: NOW.into() }]);
}

#[test]
fn bounds_a_delivery_batch_oldest_first_and_says_how_many_still_wait() {
    let s = scratch();
    let w = workroom_at(&s.0);
    let first = post_from_atlas(&w, "one", "mission_a1").unwrap();
    post_from_atlas(&w, "two", "mission_a1").unwrap();
    let third = post_from_atlas(&w, "three", "mission_a1").unwrap();
    assert_eq!(w.unread("tm_wren", 1, &[]).unwrap(), Unread { messages: vec![first.clone()], remaining: 2 });
    // One asked for by id comes first whatever its age.
    assert_eq!(w.unread("tm_wren", 1, std::slice::from_ref(&third.message_id)).unwrap(), Unread { messages: vec![third], remaining: 2 });
}

#[test]
fn refuses_to_deliver_the_same_message_twice() {
    let s = scratch();
    let w = workroom_at(&s.0);
    let m = post_from_atlas(&w, "once", "mission_a1").unwrap();
    w.mark_delivered(std::slice::from_ref(&m.message_id), "mission_w1").unwrap();
    assert!(w.mark_delivered(std::slice::from_ref(&m.message_id), "mission_w2").unwrap_err().contains("already delivered"));
    assert_eq!(w.mark_delivered(&["wm_nope".into()], "mission_w2").unwrap_err(), "Unknown workroom message");
}

#[test]
fn posts_the_same_words_from_the_same_run_to_the_same_teammate_once() {
    let s = scratch();
    let w = workroom_at(&s.0);
    let first = post_from_atlas(&w, "The build runs through pnpm check.", "mission_a1").unwrap();
    let again = post_from_atlas(&w, "The build runs through pnpm check.", "mission_a1").unwrap();
    assert_eq!(again, first);
    assert_eq!(w.read().messages, vec![first]);
    post_from_atlas(&w, "Also: pnpm lint.", "mission_a1").unwrap();
    post_from_atlas(&w, "The build runs through pnpm check.", "mission_a2").unwrap();
    assert_eq!(w.read().messages.len(), 3);
    assert_eq!(workroom_at(&s.0).unread("tm_wren", 10, &[]).unwrap().messages.len(), 3);
}

#[test]
fn refuses_a_self_addressed_message_an_oversized_one_and_control_characters() {
    let s = scratch();
    let w = workroom_at(&s.0);
    assert!(w.post(&atlas(), "mission_a1", &atlas(), "note to self", None).unwrap_err().contains("cannot message itself"));
    assert!(post_from_atlas(&w, &"x".repeat(MAX_WORKROOM_MESSAGE_LENGTH + 1), "mission_a1").unwrap_err().contains("text is invalid"));
    assert!(post_from_atlas(&w, "bell\u{7}", "mission_a1").unwrap_err().contains("text is invalid"));
    assert!(post_from_atlas(&w, "   ", "mission_a1").unwrap_err().contains("text is invalid"));
    assert_eq!(post_from_atlas(&w, "line one\nline two", "mission_a1").unwrap().text, "line one\nline two");
}

#[test]
fn tolerates_a_truncated_tail_and_reports_it_keeping_every_complete_record() {
    let s = scratch();
    let w = workroom_at(&s.0);
    let kept = post_from_atlas(&w, "kept", "mission_a1").unwrap();
    append_raw(&s.0, r#"{"schemaVersion":1,"recordType":"workroom.message","sequ"#);
    let snapshot = w.read();
    assert_eq!(snapshot.messages, vec![kept]);
    assert_eq!(codes(&snapshot), vec![IssueCode::TruncatedTail]);
}

#[test]
fn repairs_a_torn_tail_and_carries_on() {
    let s = scratch();
    let w = workroom_at(&s.0);
    let kept = post_from_atlas(&w, "kept", "mission_a1").unwrap();
    append_raw(&s.0, r#"{"schemaVersion":1,"recordType":"workroom.message","sequ"#);
    let next = post_from_atlas(&w, "after the crash", "mission_a2").unwrap();
    let unread = w.unread("tm_wren", 10, &[]).unwrap();
    assert_eq!(unread.messages.iter().map(|m| m.text.as_str()).collect::<Vec<_>>(), vec!["kept", "after the crash"]);
    let snapshot = w.read();
    assert!(snapshot.issues.is_empty());
    assert_eq!(snapshot.messages, vec![kept, next]);
}

#[test]
fn stops_at_a_record_that_breaks_the_sequence_and_refuses_to_append_past_it() {
    let s = scratch();
    let w = workroom_at(&s.0);
    let kept = post_from_atlas(&w, "kept", "mission_a1").unwrap();
    // A well-formed message of its own, only out of sequence.
    let mut other = kept.clone();
    other.message_id = "wm_forged".into();
    let forged = json!({"schemaVersion": 1, "recordType": "workroom.message", "sequence": 7, "occurredAt": NOW, "message": message_json(&other)});
    append_raw(&s.0, &format!("{forged}\n"));
    let snapshot = w.read();
    assert_eq!(snapshot.messages, vec![kept]);
    assert_eq!(codes(&snapshot), vec![IssueCode::InvalidRecord]);
    assert_eq!(post_from_atlas(&w, "after the break", "mission_a1").unwrap_err(), "Workroom is unavailable");
}

#[test]
fn refuses_a_self_addressed_record_on_read_not_only_on_write() {
    let s = scratch();
    let w = workroom_at(&s.0);
    std::fs::create_dir_all(&s.0).unwrap();
    let record = json!({"schemaVersion": 1, "recordType": "workroom.message", "sequence": 1, "occurredAt": NOW, "message": {
        "messageId": "wm_self", "from": {"teammateId": "tm_atlas", "name": "Atlas", "missionId": "mission_a1"},
        "to": {"teammateId": "tm_atlas", "name": "Atlas"}, "text": "note to self", "postedAt": NOW}});
    std::fs::write(s.0.join(FILE_NAME), format!("{record}\n")).unwrap();
    let snapshot = w.read();
    assert!(snapshot.messages.is_empty());
    assert_eq!(codes(&snapshot), vec![IssueCode::InvalidRecord]);
    assert_eq!(w.unread("tm_atlas", 5, &[]).unwrap_err(), "Workroom is unavailable");
}

#[test]
fn refuses_a_delivery_record_for_a_message_the_file_does_not_hold() {
    let s = scratch();
    let w = workroom_at(&s.0);
    std::fs::create_dir_all(&s.0).unwrap();
    let record = json!({"schemaVersion": 1, "recordType": "workroom.delivery", "sequence": 1, "occurredAt": NOW,
        "delivery": {"messageId": "wm_ghost", "missionId": "mission_w1", "deliveredAt": NOW}});
    std::fs::write(s.0.join(FILE_NAME), format!("{record}\n")).unwrap();
    let snapshot = w.read();
    assert!(snapshot.deliveries.is_empty());
    assert_eq!(codes(&snapshot), vec![IssueCode::InvalidRecord]);
}

#[test]
fn refuses_a_schema_it_does_not_know_rather_than_guessing() {
    let s = scratch();
    let w = workroom_at(&s.0);
    std::fs::create_dir_all(&s.0).unwrap();
    std::fs::write(s.0.join(FILE_NAME), format!("{}\n", json!({"schemaVersion": 2, "recordType": "workroom.message", "sequence": 1, "occurredAt": NOW, "message": {}}))).unwrap();
    assert_eq!(codes(&w.read()), vec![IssueCode::UnsupportedSchema]);
}

#[test]
fn writes_each_record_with_a_version_a_sequence_and_one_line() {
    let s = scratch();
    let w = workroom_at(&s.0);
    let m = post_from_atlas(&w, "durable", "mission_a1").unwrap();
    w.mark_delivered(std::slice::from_ref(&m.message_id), "mission_w1").unwrap();
    let text = std::fs::read_to_string(s.0.join(FILE_NAME)).unwrap();
    let lines: Vec<Value> = text.trim_end().split('\n').map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 2);
    assert_eq!((lines[0]["recordType"].as_str(), lines[0]["sequence"].as_u64(), lines[0]["schemaVersion"].as_u64()), (Some("workroom.message"), Some(1), Some(1)));
    assert_eq!((lines[1]["recordType"].as_str(), lines[1]["sequence"].as_u64(), lines[1]["schemaVersion"].as_u64()), (Some("workroom.delivery"), Some(2), Some(1)));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(s.0.join(FILE_NAME)).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn reads_as_empty_not_broken_before_anyone_has_posted() {
    let s = scratch();
    let w = workroom_at(&s.0);
    assert_eq!(w.read(), Snapshot::default());
    assert_eq!(w.unread("tm_wren", 5, &[]).unwrap(), Unread::default());
}

#[test]
fn a_file_changed_by_another_writer_between_reading_and_appending_is_refused_not_overwritten() {
    let s = scratch();
    let w = workroom_at(&s.0);
    post_from_atlas(&w, "kept", "mission_a1").unwrap();
    // A link in the file's place is never followed.
    let elsewhere = s.0.join("elsewhere.jsonl");
    std::fs::rename(s.0.join(FILE_NAME), &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, s.0.join(FILE_NAME)).unwrap();
    assert!(post_from_atlas(&w, "through a link", "mission_a1").is_err());
    assert_eq!(std::fs::read_to_string(&elsewhere).unwrap().lines().count(), 1);
}
