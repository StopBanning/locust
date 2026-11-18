//! What a phone may see and do, with a fake engine: nothing here starts a turn or answers a real card.

use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

fn mate(id: &str, name: &str) -> Teammate {
    Teammate {
        id: id.to_owned(),
        name: name.to_owned(),
        role: String::new(),
        runtime: Some("codex".to_owned()),
        model: None,
        account: None,
        busy: false,
        runs_in: String::new(),
        folder: None,
    }
}

fn request(kind: Kind, summary: &str, detail: &str) -> Request {
    Request {
        approval_id: "ap_1".to_owned(),
        mission_id: "m2".to_owned(),
        runtime: "codex".to_owned(),
        kind,
        summary: summary.to_owned(),
        detail: detail.to_owned(),
        cwd: None,
        requested_at: "2026-11-18T10:00:00.000Z".to_owned(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        claude_input: None,
        questions: Vec::new(),
    }
}

fn mission(id: &str, prompt: &str, parent: Option<&str>) -> PhoneMission {
    PhoneMission {
        mission_id: id.to_owned(),
        prompt: prompt.to_owned(),
        runtime: "codex".to_owned(),
        created_at: format!("2026-11-18T10:0{}:00.000Z", id.len()),
        last_updated_at: format!("2026-11-18T10:0{}:00.000Z", id.len()),
        phase: "completed".to_owned(),
        continues_from_mission_id: parent.map(str::to_owned),
        continues_from_reason: parent.map(|_| "follow-up".to_owned()),
        started_by_kind: None,
        events: Vec::new(),
    }
}

#[derive(Default)]
struct Fake {
    newest: Mutex<Option<String>>,
    running: Mutex<bool>,
    route: Mutex<Option<PhoneRoute>>,
    fails: Mutex<Option<String>>,
    started: Mutex<Vec<(String, Mode, String, String)>>,
    answered: Mutex<Vec<(String, bool)>>,
    waiting: Mutex<Vec<Request>>,
    mark: Mutex<String>,
    live: Mutex<Vec<String>>,
    reads: AtomicUsize,
    /// Pressed again while the first message is on its way.
    again: Mutex<Option<Arc<EnginePhoneCalls>>>,
    again_said: Mutex<Option<PhoneSent>>,
}

impl PhoneEngine for Fake {
    fn missions(&self) -> Vec<PhoneMission> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        vec![mission("m1", "Plan the trip", None), mission("m22", "And the hotel?", Some("m1"))]
    }
    fn live(&self) -> Vec<String> {
        self.live.lock().unwrap().clone()
    }
    fn change_mark(&self) -> String {
        self.mark.lock().unwrap().clone()
    }
    fn name_of(&self, _mission_id: &str) -> Option<String> {
        Some("Mira".to_owned())
    }
    fn waiting(&self) -> Vec<Request> {
        self.waiting.lock().unwrap().clone()
    }
    fn answer_card(&self, approval_id: &str, approve: bool) -> bool {
        self.answered.lock().unwrap().push((approval_id.to_owned(), approve));
        true
    }
    fn newest(&self, _mission_id: &str) -> Option<String> {
        self.newest.lock().unwrap().clone()
    }
    fn running(&self, _mission_id: &str) -> bool {
        *self.running.lock().unwrap()
    }
    fn route_of(&self, _newest: &str) -> Option<PhoneRoute> {
        self.route.lock().unwrap().clone()
    }
    fn start(&self, route: PhoneRoute, newest: &str, text: &str) -> Result<(), String> {
        if let Some(calls) = self.again.lock().unwrap().take() {
            *self.again_said.lock().unwrap() = Some(calls.send("m1", "twice"));
        }
        if let Some(message) = self.fails.lock().unwrap().clone() {
            return Err(message);
        }
        self.started.lock().unwrap().push((route.teammate.id, route.mode, newest.to_owned(), text.to_owned()));
        Ok(())
    }
}

fn calls_with(fake: Arc<Fake>, clock: Arc<AtomicI64>) -> Arc<EnginePhoneCalls> {
    Arc::new(EnginePhoneCalls::new(fake, Arc::new(move || clock.load(Ordering::SeqCst))))
}

fn ready() -> Arc<Fake> {
    let fake = Arc::new(Fake::default());
    *fake.newest.lock().unwrap() = Some("m22".to_owned());
    *fake.route.lock().unwrap() = Some(PhoneRoute { teammate: mate("t_mira", "Mira"), mode: Mode::AcceptEdits, runtime: None });
    fake
}

#[test]
fn sends_a_message_as_the_persons_next_turn_on_the_newest_turn_to_its_teammate_in_its_saved_mode() {
    let fake = ready();
    let calls = calls_with(fake.clone(), Arc::default());
    assert_eq!(calls.send("m1", "Book the second one"), PhoneSent::Ok);
    assert_eq!(
        *fake.started.lock().unwrap(),
        vec![("t_mira".to_owned(), Mode::AcceptEdits, "m22".to_owned(), "Book the second one".to_owned())]
    );
}

#[test]
fn sends_nothing_while_the_conversation_is_still_working_or_cannot_be_read() {
    let fake = ready();
    *fake.running.lock().unwrap() = true;
    let calls = calls_with(fake.clone(), Arc::default());
    assert_eq!(calls.send("m1", "hi"), PhoneSent::Err { message: STILL_WORKING.to_owned() });
    assert_eq!(STILL_WORKING, "This conversation is still working. Send when it finishes.");

    *fake.running.lock().unwrap() = false;
    *fake.route.lock().unwrap() = None;
    assert_eq!(calls.send("m1", "hi"), PhoneSent::Err { message: CANNOT_READ.to_owned() });
    *fake.newest.lock().unwrap() = None;
    assert_eq!(calls.send("m1", "hi"), PhoneSent::Err { message: CANNOT_READ.to_owned() });
    assert_eq!(CANNOT_READ, "This conversation could not be read in Locust. Continue it on the computer. Nothing was sent.");
    assert!(fake.started.lock().unwrap().is_empty());
}

#[test]
fn a_second_press_while_the_first_is_on_its_way_is_refused_and_a_failed_send_says_why() {
    let fake = ready();
    let calls = calls_with(fake.clone(), Arc::default());
    *fake.again.lock().unwrap() = Some(calls.clone());
    assert_eq!(calls.send("m1", "once"), PhoneSent::Ok);
    assert_eq!(
        *fake.again_said.lock().unwrap(),
        Some(PhoneSent::Err { message: "That message is already on its way.".to_owned() })
    );
    assert_eq!(fake.started.lock().unwrap().len(), 1);

    // Once it has gone, the next press is sent; a refused one says the engine's words.
    *fake.fails.lock().unwrap() = Some("Codex is not signed in.".to_owned());
    assert_eq!(calls.send("m1", "again"), PhoneSent::Err { message: "Codex is not signed in.".to_owned() });
    *fake.fails.lock().unwrap() = None;
    assert_eq!(calls.send("m1", "again"), PhoneSent::Ok);
}

#[test]
fn answers_a_card_as_approve_once_or_deny_through_the_engine() {
    let fake = ready();
    let calls = calls_with(fake.clone(), Arc::default());
    assert!(calls.answer_card("ap_1", true));
    assert!(calls.answer_card("ap_2", false));
    assert_eq!(*fake.answered.lock().unwrap(), vec![("ap_1".to_owned(), true), ("ap_2".to_owned(), false)]);
}

#[test]
fn shows_a_waiting_card_by_who_and_what_with_the_exact_line_it_allows_and_the_command_said_plainly() {
    let fake = ready();
    *fake.waiting.lock().unwrap() = vec![request(Kind::Command, "Run a command", "\n  ls -la src  \npwd")];
    let calls = calls_with(fake.clone(), Arc::default());
    let state = calls.state();
    assert_eq!(state.waiting.len(), 1);
    let card = &state.waiting[0];
    assert_eq!(
        (card.approval_id.as_str(), card.mission_id.as_str(), card.name.as_deref(), card.exact.as_deref()),
        ("ap_1", "m2", Some("Mira"), Some("ls -la src"))
    );
    assert!(card.plain.is_some(), "a command is said plainly");

    // The summary already says it: no exact line. A question has none, and no command words.
    assert_eq!(exact_line("Run ls -la src", "ls -la src"), None);
    let question = phone_waiting(&request(Kind::Question, "Which one?", "pick a or b"), None);
    assert_eq!((question.exact, question.plain), (None, None));
    // At most 200 characters (UTF-16 units, as JavaScript counts), then an ellipsis.
    let long = "x".repeat(250);
    assert_eq!(exact_line("Edit", &long), Some(format!("{}…", "x".repeat(200))));
}

#[test]
fn reads_the_conversations_once_per_change_a_turn_starting_or_ending_or_a_new_minute() {
    let fake = ready();
    let clock = Arc::new(AtomicI64::new(60_000 * 1000));
    let calls = calls_with(fake.clone(), clock.clone());
    let first = calls.state();
    assert_eq!(first.conversations.len(), 1, "one conversation of two turns");
    assert_eq!(first.conversations[0].title, "Plan the trip");
    calls.state();
    assert!(calls.turns("m22").is_some_and(|turns| turns.len() == 2));
    assert_eq!(fake.reads.load(Ordering::SeqCst), 1);
    *fake.mark.lock().unwrap() = "written".to_owned();
    calls.state();
    assert_eq!(fake.reads.load(Ordering::SeqCst), 2);
    *fake.live.lock().unwrap() = vec!["m22".to_owned()];
    calls.state();
    assert_eq!(fake.reads.load(Ordering::SeqCst), 3);
    clock.fetch_add(60_000, Ordering::SeqCst);
    calls.state();
    assert_eq!(fake.reads.load(Ordering::SeqCst), 4);
}

#[test]
fn makes_a_turn_from_its_ledger_with_its_phase_parent_and_who_started_it() {
    let created = |extra: Value| {
        let mut meta = json!({"missionId": "m2", "prompt": "And then?", "runtime": "claude", "createdAt": "2026-11-18T10:00:00.000Z"});
        meta.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        json!({"recordType": "mission.created", "occurredAt": "2026-11-18T10:00:00.000Z", "metadata": meta})
    };
    let event = |seq: u64, kind: &str| {
        json!({"recordType": "mission.event", "occurredAt": "2026-11-18T10:00:05.000Z", "event": {"sequence": seq, "type": kind, "payload": {}}})
    };
    let records = vec![
        created(json!({"continuesFrom": {"missionId": "m1", "reason": "follow-up"}})),
        event(1, "run.started"),
        event(2, "run.completed"),
    ];
    let turn = phone_mission_of(&records, false).unwrap();
    assert_eq!(
        (turn.mission_id.as_str(), turn.prompt.as_str(), turn.runtime.as_str(), turn.phase.as_str()),
        ("m2", "And then?", "claude", "completed")
    );
    assert_eq!(turn.continues_from_mission_id.as_deref(), Some("m1"));
    assert_eq!(turn.continues_from_reason.as_deref(), Some("follow-up"));
    assert_eq!(turn.last_updated_at, "2026-11-18T10:00:05.000Z");
    assert_eq!(turn.started_by_kind, None);
    assert_eq!(phone_mission_of(&records, true).unwrap().phase, "running");
    assert_eq!(phone_mission_of(&records[..2], false).unwrap().phase, "interrupted");

    let side = phone_mission_of(&[created(json!({"side": true}))], false).unwrap();
    assert_eq!(side.started_by_kind.as_deref(), Some("side"));
    let app = phone_mission_of(&[created(json!({"startedBy": {"kind": "mcp"}}))], false).unwrap();
    assert_eq!(app.started_by_kind.as_deref(), Some("mcp"));
}
