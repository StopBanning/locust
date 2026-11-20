//! Electron's `an-app-hears-why-a-turn-stopped-short.test.ts` (3 cases), with Electron's events and words. Rust reads
//! a turn as finished from its `run.completed` event, so each ends with one; Electron's fixture sets the phase instead.
use super::*;

fn said(mut events: Vec<Value>) -> String {
    events.push(ev("run.completed", json!({})));
    let fake = Fake::default();
    fake.turns.lock().unwrap().insert("mission_1".into(), turn("mission_1", true, events));
    Tools::new(fake).call("read_reply", &json!({"conversation_id": "mission_1"})).text
}

fn answer(text: &str) -> Value {
    ev("message.delta", json!({"itemId": "m1", "operation": "append", "text": text, "final": true}))
}

fn note(level: &str, message: &str) -> Value {
    ev("adapter.diagnostic", json!({"level": level, "code": "x", "message": message, "terminal": false}))
}

#[test]
fn gets_the_warning_locust_recorded_after_the_last_words() {
    let refused = "Antigravity was not allowed to run a command in this mode. Edit lets it change files but not run commands; Auto lets it do both.";
    assert_eq!(
        said(vec![answer("I will read the three files to begin the review."), note("warning", refused), note("info", "Started in 2.2 s.")]),
        format!("I will read the three files to begin the review.\n\nLocust: {refused}")
    );
}

#[test]
fn gets_the_answer_alone_when_nothing_was_refused_and_no_warning_from_before_the_answer() {
    assert_eq!(said(vec![note("warning", "An earlier hiccup."), answer("All set."), note("info", "Usage 50%.")]), "All set.");
}

#[test]
fn finds_the_end_of_an_answer_that_arrived_in_pieces() {
    let pieces = vec![
        ev("message.delta", json!({"itemId": "m1", "operation": "append", "text": "Half ", "final": false})),
        note("warning", "Too early to count."),
        ev("message.delta", json!({"itemId": "m1", "operation": "append", "text": "and whole.", "final": true})),
        note("error", "Then it stopped."),
    ];
    assert_eq!(said(pieces), "Half and whole.\n\nLocust: Then it stopped.");
}
