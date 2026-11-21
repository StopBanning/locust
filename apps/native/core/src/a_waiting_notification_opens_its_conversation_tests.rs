//! Electron's `main/a-waiting-notification-opens-its-conversation.test.ts` ("a waiting notification"), one Rust test
//! for each of its two cases, with its names and words; and conversationAttentionTitle (index.ts), which gives the
//! conversation its name, under the second.
use super::*;
use serde_json::{Value, json};

// case: names the teammate and conversation and opens exactly the card that raised it
#[test]
fn names_the_teammate_and_conversation_and_opens_exactly_the_card_that_raised_it() {
    let asked = ended_notice("question", &[ask_reply("Which database?"), completed()], "claude", Some("Wren"), || "Choose storage".into()).unwrap();
    let approval = approval_notice("approval", false, "Run a command", Some("Sage"), Some("Check storage"));
    assert_eq!([asked.title.as_str(), approval.title.as_str()], ["Wren is asking you something · Choose storage", "Sage needs your approval · Check storage"]);
    assert_eq!([asked.opens.as_deref(), approval.opens.as_deref()], [Some("question"), Some("approval")]);
    // A question card is named the same way, and with no conversation the title is the bare one.
    assert_eq!(approval_notice("q", true, "Pick one", Some("Sage"), Some("Check storage")).title, "Sage has a question · Check storage");
    assert_eq!(decision_text(Some("Wren"), "Which database?", None).0, "Wren is asking you something");
    // The conversation's name is cut at 100 characters.
    let long = "x".repeat(130);
    assert_eq!(decision_text(Some("Wren"), "Which database?", Some(&long)).0, format!("Wren is asking you something · {}", "x".repeat(100)));
}

fn ask_reply(question: &str) -> Value {
    let text = format!("<locust-ask>\n{question}\n- SQLite :: local\n- Postgres :: server\n</locust-ask>");
    json!({"type": "message.delta", "sourceAdapter": "codex", "payload": {"itemId": "one", "operation": "append", "final": true, "text": text}})
}

fn completed() -> Value {
    json!({"type": "run.completed", "sourceAdapter": "codex", "payload": {}})
}

fn turn(prompt: &str, continues: Option<&str>) -> Value {
    match continues {
        Some(parent) => json!({"prompt": prompt, "continuesFrom": {"missionId": parent, "checkpointEpoch": 1, "reason": "follow-up"}}),
        None => json!({"prompt": prompt}),
    }
}

// case: carries a recorded decision destination and the chosen conversation name into attention
#[test]
fn carries_a_recorded_decision_destination_and_the_chosen_conversation_name_into_attention() {
    // Electron's record is completed with no event saying so; a native record always ends with one.
    let events = [ask_reply("Which database?"), completed()];
    assert_eq!(
        ended_notice("decision-turn", &events, "codex", None, || "Storage".into()),
        Some(Notice {
            title: "A teammate is asking you something · Storage".into(),
            body: "Which database? · the run waits for your answer.".into(),
            opens: Some("decision-turn".into()),
            limit: false,
        })
    );

    // The name: walking back from the turn, the oldest titled turn's title wins.
    let record_of = |id: &str| match id {
        "leaf" => Some(turn("Third", Some("mid"))),
        "mid" => Some(turn("Second", Some("root"))),
        "root" => Some(turn("First  question\n  asked", None)),
        "loop-a" => Some(turn("A", Some("loop-b"))),
        "loop-b" => Some(turn("B", Some("loop-a"))),
        "orphan" => Some(turn("Orphan", Some("gone"))),
        _ => None,
    };
    // A walk that never stops is a failure, not a hang: no walk reads more than its 500 steps' records.
    let reads = std::cell::Cell::new(0usize);
    let records = |id: &str| {
        reads.set(reads.get() + 1);
        assert!(reads.get() < 2_000, "the walk back did not stop");
        record_of(id)
    };
    let titles = json!({"leaf": "Leaf title", "mid": "Storage"});
    assert_eq!(conversation_title("leaf", &titles, records), "Storage");
    assert_eq!(conversation_title("leaf", &json!({"leaf": "Leaf title"}), records), "Leaf title");
    // Untitled: the first turn's words on one line.
    assert_eq!(conversation_title("leaf", &json!({}), records), "First question asked");
    // A turn whose parent is not on this computer names itself by its own words; a loop is stopped.
    assert_eq!(conversation_title("orphan", &json!({}), records), "Orphan");
    assert_eq!(conversation_title("loop-a", &json!({"loop-a": "A title", "loop-b": "Looped"}), records), "A title");
    // Nothing known: "Conversation". A long name is cut at 100.
    assert_eq!(conversation_title("nowhere", &json!({}), records), "Conversation");
    assert_eq!(conversation_title("nowhere", &json!({"nowhere": "y".repeat(140)}), records), "y".repeat(100));
}
