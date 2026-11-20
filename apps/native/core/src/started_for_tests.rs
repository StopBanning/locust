//! Electron's `a-reply-is-shown-the-message-it-answers.test.ts` (2 cases): a run started to answer a message is shown
//! that message first, whatever waits before it.
use super::*;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wren() -> Party {
    Party { teammate_id: "tm_wren".into(), name: "Wren".into() }
}
fn booty() -> Party {
    Party { teammate_id: "tm_booty".into(), name: "Booty".into() }
}

/// Six notes that need nothing back, then a question: the question's id.
fn waiting_then_a_question(room: &Workroom) -> String {
    for n in 1..=6 {
        room.post(&wren(), "mission_wren", &booty(), &format!("Note {n}: the deploy finished. No further action is needed from you."), None).unwrap();
    }
    room.post(&wren(), "mission_wren", &booty(), "Booty, what does the lint step run?", None).unwrap().message_id
}

fn scratch() -> (Temp, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("locust-started-for-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&root).unwrap();
    (Temp(root.clone()), root)
}

// case: are the oldest, after the ones it was started for
#[test]
fn are_the_oldest_after_the_ones_it_was_started_for() {
    let (_t, root) = scratch();
    let room = Workroom::new(&root);
    let question = waiting_then_a_question(&room);
    let plain = room.unread("tm_booty", 5, &[]).unwrap();
    let heads: Vec<String> = plain.messages.iter().map(|m| m.text.chars().take(6).collect()).collect();
    assert_eq!(heads, ["Note 1", "Note 2", "Note 3", "Note 4", "Note 5"]);
    let asked = room.unread("tm_booty", 5, &[question.clone()]).unwrap();
    assert_eq!(asked.messages[0].message_id, question);
    assert_eq!(asked.messages.len(), 5);
    assert_eq!(asked.remaining, 2);
}

// case: reach the runtime: the relay names the message, and the run quotes it
#[test]
fn reach_the_runtime_the_run_quotes_the_message_it_was_started_for() {
    // The relay names the message (relay_tests: a start's relay.answering); send_from hands that list to unread as here.
    let (_t, root) = scratch();
    let room = Workroom::new(&root.join("room"));
    let question = waiting_then_a_question(&room);
    let shown = room.unread("tm_booty", crate::briefing::MAX_INBOUND_MESSAGES, &[question]).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::write(
        store.electron().join("teammates.json"),
        serde_json::json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_wren","name":"Wren","role":"Code & Migrations"},{"teammateId":"tm_booty","name":"Booty","role":"Custom"}],"missionOwners":{},"settings":{}}).to_string(),
    )
    .unwrap();
    let (sent, kept) = crate::people::turn_brief(&store, "tm_booty", &root.join("shop"), "Answer Wren.", None, Some("codex"), &shown.messages, shown.remaining).unwrap();
    assert!(sent.contains("Booty, what does the lint step run?"), "{sent}");
    assert_eq!(kept[0].text, "Booty, what does the lint step run?");
}
