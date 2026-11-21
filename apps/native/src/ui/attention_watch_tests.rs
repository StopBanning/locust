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

fn card(id: &str, mission: &str) -> locust_core::approval::Request {
    locust_core::approval::Request {
        approval_id: id.into(),
        mission_id: mission.into(),
        runtime: "codex".into(),
        kind: locust_core::approval::Kind::FileChange,
        summary: "Change 2 files".into(),
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
    }
}
type Shown = Vec<(String, String, Option<String>)>;
fn recorder(shown: &mut Shown) -> impl FnMut(&str, &str, Option<&str>) + '_ {
    |t: &str, b: &str, o: Option<&str>| shown.push((t.to_owned(), b.to_owned(), o.map(str::to_owned)))
}
fn turn(id: &str, events: Vec<serde_json::Value>) -> native::TurnRecords {
    native::TurnRecords {
        path: std::path::PathBuf::new(),
        metadata: json!({"missionId": id, "runtime": "claude"}),
        events,
        checkpoints: 0,
        incomplete: false,
    }
}

// case: when an approval earns a notification > shows the notification and brings the window forward on click
#[test]
fn shows_the_notification_and_brings_the_window_forward_on_click() {
    let (mut s, _gone) = window("click");
    let mut shown = Shown::new();
    say_card(&mut s, &card("ap_1", "approval"), true, &mut recorder(&mut shown));
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].0, "Sage needs your approval · Check storage");
    // The click: macOS brings Locust forward, and the window opens the card's own conversation.
    assert_eq!(shown[0].2.as_deref(), Some("approval"));
    // A card is said once, however many polls see it waiting.
    say_card(&mut s, &card("ap_1", "approval"), true, &mut recorder(&mut shown));
    assert_eq!(shown.len(), 1);
}

// case: when an approval earns a notification > stays quiet while the window has attention
#[test]
fn stays_quiet_while_the_window_has_attention() {
    let (mut s, _gone) = window("quiet");
    let mut shown = Shown::new();
    say_card(&mut s, &card("ap_2", "approval"), false, &mut recorder(&mut shown));
    assert!(shown.is_empty());
    // Seen while in front is not said later either: the person saw the card in the window.
    say_card(&mut s, &card("ap_2", "approval"), true, &mut recorder(&mut shown));
    assert!(shown.is_empty());
}

// case: the two other ways a run ends waiting for a person > both are shown at once when the window is elsewhere, and not at all when it has attention
#[test]
fn both_are_shown_at_once_when_the_window_is_elsewhere_and_not_at_all_when_it_has_attention() {
    let (s, _gone) = window("both");
    let ask = "Two ways.\n<locust-ask>\nWhich database?\n- Postgres :: today\n- SQLite :: no server\n</locust-ask>";
    let asked = turn("question", vec![
        json!({"type": "message.delta", "sourceAdapter": "claude", "payload": {"itemId": "m1", "operation": "append", "text": ask, "final": true}}),
        json!({"type": "run.completed", "sourceAdapter": "claude", "payload": {}}),
    ]);
    let limited = turn("question", vec![json!({"type": "route.limit_detected", "sourceAdapter": "claude", "payload": {"kind": "quota-exhausted"}})]);
    let mut shown = Shown::new();
    assert!(!say_ended(&s, "question", &asked, true, &mut recorder(&mut shown)), "a question is said with its finish");
    assert!(say_ended(&s, "question", &limited, true, &mut recorder(&mut shown)), "a limit is said instead of the finish");
    let titles: Vec<&str> = shown.iter().map(|(t, _, _)| t.as_str()).collect();
    assert_eq!(titles, ["Wren is asking you something · Choose storage", "Wren hit the Claude Code limit"]);
    let before = shown.len();
    say_ended(&s, "question", &asked, false, &mut recorder(&mut shown));
    assert!(say_ended(&s, "question", &limited, false, &mut recorder(&mut shown)), "still a limit: no finish toast either");
    assert_eq!(shown.len(), before);
}

// case: a finished run, told once > says nothing while the person is looking at the window
#[test]
fn says_nothing_while_the_person_is_looking_at_the_window() {
    let (mut s, _gone) = window("looking");
    let now = std::time::Instant::now();
    s.finishes = vec![("Wren finished".into(), "Done.".into())];
    s.finish_ids = vec!["m1".into()];
    s.finishes_due = Some(now);
    let mut shown = Shown::new();
    say_finishes(&mut s, now, false, &mut recorder(&mut shown));
    assert!(shown.is_empty());
    assert!(s.finishes.is_empty() && s.finishes_due.is_none(), "let go, not said when the person looks away");
    // Elsewhere, and only once due: the burst is one toast, and a click opens the newest.
    s.finishes = vec![("Wren finished".into(), "Moved the port. · took 4 min".into()), ("Juno stopped".into(), "Not signed in. · after 2 min".into())];
    s.finish_ids = vec!["m1".into(), "m2".into()];
    s.finishes_due = Some(now + std::time::Duration::from_millis(locust_core::attention::FINISH_WINDOW_MS));
    say_finishes(&mut s, now, true, &mut recorder(&mut shown));
    assert!(shown.is_empty(), "not before its four seconds");
    say_finishes(&mut s, now + std::time::Duration::from_millis(locust_core::attention::FINISH_WINDOW_MS), true, &mut recorder(&mut shown));
    assert_eq!(shown, vec![("2 teammates are done".to_owned(), "Wren finished: Moved the port. · took 4 min\nJuno stopped: Not signed in. · after 2 min".to_owned(), Some("m2".to_owned()))]);
}
