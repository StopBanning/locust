//! Electron's `an-old-message-says-it-is-old.test.ts` (3 cases): a teammate's message that waited says how long, in
//! the unit that reads, beside the message, against the time now.
use super::*;
use crate::workroom::{Message, Party};

const POSTED: &str = "2026-09-24T09:00:00.000Z";

fn at(minutes: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(POSTED).unwrap().with_timezone(&chrono::Utc) + chrono::Duration::minutes(minutes)
}

fn waiting(posted_at: &str) -> Message {
    Message {
        message_id: "wm_1".into(),
        sequence: 1,
        from: Party { teammate_id: "tm_atlas".into(), name: "Atlas".into() },
        from_mission: "mission_a".into(),
        to: Party { teammate_id: "tm_wren".into(), name: "Wren".into() },
        text: "The tests pass on main.".into(),
        posted_at: posted_at.into(),
        observed: None,
    }
}

fn atlas() -> PeerEntry {
    PeerEntry { teammate_id: "tm_atlas".into(), name: "Atlas".into(), role: "Research & Briefs".into() }
}

// case: is said once it is half an hour old, in the unit that reads
#[test]
fn is_said_once_it_is_half_an_hour_old_in_the_unit_that_reads() {
    assert_eq!(message_age(POSTED, at(10)), None);
    assert_eq!(message_age(POSTED, at(45)).as_deref(), Some("45 minutes ago"));
    assert_eq!(message_age(POSTED, at(60)).as_deref(), Some("an hour ago"));
    assert_eq!(message_age(POSTED, at(180)).as_deref(), Some("3 hours ago"));
    assert_eq!(message_age(POSTED, at(26 * 60)).as_deref(), Some("a day ago"));
    assert_eq!(message_age(POSTED, at(3 * 24 * 60)).as_deref(), Some("3 days ago"));
    // A clock that moved backwards says nothing rather than something false.
    assert_eq!(message_age(POSTED, at(-90)), None);
}

// case: goes beside the message it describes, with what to do about it
#[test]
fn goes_beside_the_message_it_describes_with_what_to_do_about_it() {
    let late = inbound_section(&[waiting(POSTED)], 0, &[atlas()], Some(at(180)));
    assert!(late.contains(&format!("- Atlas (Research & Briefs), {POSTED} (sent 3 hours ago: check it still holds before acting on it):\n  The tests pass on main.")), "{late}");
    let fresh = inbound_section(&[waiting(POSTED)], 0, &[atlas()], Some(at(5)));
    assert!(fresh.contains(&format!("- Atlas (Research & Briefs), {POSTED}:\n  The tests pass on main.")), "{fresh}");
}

// case: is read against the time now when the host prepares a run
#[test]
fn is_read_against_the_time_now_when_the_host_prepares_a_run() {
    let root = std::env::temp_dir().join(format!("locust-old-message-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::write(
        store.electron().join("teammates.json"),
        serde_json::json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_wren","name":"Wren","role":"Code & Migrations"},{"teammateId":"tm_atlas","name":"Atlas","role":"Research & Briefs"}],"missionOwners":{},"settings":{}}).to_string(),
    )
    .unwrap();
    let three_hours_ago = (chrono::Utc::now() - chrono::Duration::hours(3)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let (briefed, _) = crate::people::turn_brief(&store, "tm_wren", &root.join("shop"), "Go on.", None, Some("claude"), &[waiting(&three_hours_ago)], 0).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert!(briefed.contains("(sent 3 hours ago: check it still holds before acting on it):\n  The tests pass on main."), "{briefed}");
}
