//! The window's half of Electron's `attention-reader.test.ts` and `a-waiting-notification-opens-its-conversation.test.ts`:
//! what `poll` shows for a card or a turn that ended is named for the turn's owner in the roster and for its
//! conversation as the sidebar titles it, and opens that turn.
use super::*;
use serde_json::json;

struct Gone(std::path::PathBuf);

impl Drop for Gone {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn mate(id: &str, name: &str) -> crate::Teammate {
    crate::Teammate { id: id.into(), name: name.into(), role: "Custom".into(), model: None, runtime: Some("codex".into()), folder: None, runs_in: "ask".into(), account: None, busy: false }
}

/// A window on the native engine whose team file names each turn's owner and its conversation's title.
fn window(label: &str) -> (State, Gone) {
    let root = std::env::temp_dir().join(format!("locust-attention-watch-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [], "missionOwners": {"approval": "tm_sage", "question": "tm_wren"}, "missionTitles": {"approval": "Check storage", "question": "Choose storage"}, "settings": {}}),
        )
        .unwrap();
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.data = Some(store);
    s.teammates = vec![mate("tm_sage", "Sage"), mate("tm_wren", "Wren")];
    (s, Gone(root))
}

// case: names the teammate and conversation and opens exactly the card that raised it
#[test]
fn names_the_teammate_and_conversation_and_opens_exactly_the_card_that_raised_it() {
    let (s, _gone) = window("card");
    let card = locust_core::approval::Request {
        approval_id: "ap_1".into(),
        mission_id: "approval".into(),
        runtime: "codex".into(),
        kind: locust_core::approval::Kind::Command,
        summary: "Run a command".into(),
        detail: String::new(),
        cwd: None,
        requested_at: String::new(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        claude_input: None,
        skippable: false,
        answer_in: None,
        questions: Vec::new(),
    };
    let notice = card_notice(&s, &card);
    assert_eq!((notice.title.as_str(), notice.opens.as_deref()), ("Sage needs your approval · Check storage", Some("approval")));
}

// case: a decision card in a completed reply is a question from the teammate
#[test]
fn a_decision_card_in_a_completed_reply_is_a_question_from_the_teammate() {
    let (s, _gone) = window("ended");
    let ask = "I can do this two ways.\n<locust-ask>\nWhich database?\n- Postgres :: what we run today\n- SQLite :: no server\n</locust-ask>";
    let turn = native::TurnRecords {
        path: std::path::PathBuf::new(),
        metadata: json!({"missionId": "question", "runtime": "claude"}),
        events: vec![
            json!({"type": "message.delta", "sourceAdapter": "claude", "payload": {"itemId": "m1", "operation": "append", "text": ask, "final": true}}),
            json!({"type": "run.completed", "sourceAdapter": "claude", "payload": {}}),
        ],
        checkpoints: 0,
        incomplete: false,
    };
    let notice = ended_turn_notice(&s, "question", &turn).unwrap();
    assert_eq!(
        (notice.title.as_str(), notice.body.as_str(), notice.opens.as_deref()),
        ("Wren is asking you something · Choose storage", "Which database? · the run waits for your answer.", Some("question"))
    );
}
