//! Electron's `an-app-stops-a-turn-it-started.test.ts` and `an-app-waits-for-the-answer.test.ts`, through the door's
//! own tools against the fake Locust.
use super::*;

/// A conversation an app began, its newest turn the app's too, still running.
fn running(by_app: bool, fake: Fake) -> Tools<Fake> {
    fake.turns.lock().unwrap().insert("mission_app".into(), turn("mission_app", by_app, vec![]));
    fake.live.lock().unwrap().insert("mission_app".into());
    Tools::new(fake)
}

// ---- an-app-stops-a-turn-it-started.test.ts ----

#[test]
fn stops_the_running_turn_by_its_own_run_id_and_says_what_is_kept() {
    let tools = running(true, Fake::default());
    let r = tools.call("stop_turn", &json!({"conversation_id": "mission_app"}));
    assert!(!r.is_error);
    assert!(r.text.starts_with("Stopping it.") && r.text.contains("undo that turn in Locust"), "{}", r.text);
    assert!(!tools.locust.live("mission_app"), "the run itself was stopped");
}

#[test]
fn never_stops_a_turn_the_person_started_even_in_a_conversation_the_app_began() {
    // The app began it; the person went on in Locust's window, and that newest turn is running.
    let fake = Fake::default();
    fake.turns.lock().unwrap().insert("mission_app".into(), turn("mission_app", true, vec![]));
    fake.turns.lock().unwrap().insert("mission_person".into(), turn("mission_person", false, vec![]));
    fake.newest.lock().unwrap().insert("mission_app".into(), "mission_person".into());
    fake.live.lock().unwrap().insert("mission_person".into());
    let tools = Tools::new(fake);
    let r = tools.call("stop_turn", &json!({"conversation_id": "mission_app"}));
    assert!(r.is_error, "{}", r.text);
    // Nor a conversation the app did not start at all.
    assert!(tools.call("stop_turn", &json!({"conversation_id": "mission_person"})).is_error);
    assert!(tools.locust.live("mission_person"), "the person's turn still runs");
}

#[test]
fn says_so_when_nothing_is_running_or_when_locust_could_not_stop_it() {
    let idle = running(true, Fake::default());
    idle.locust.live.lock().unwrap().clear();
    assert!(idle.call("stop_turn", &json!({"conversation_id": "mission_app"})).text.starts_with("Nothing is running"));
    let stuck = running(true, Fake { cancel_fails: true, ..Default::default() });
    let r = stuck.call("stop_turn", &json!({"conversation_id": "mission_app"}));
    assert!(r.is_error);
    assert!(r.text.contains("can stop it in Locust"), "{}", r.text);
}

// ---- an-app-waits-for-the-answer.test.ts ----

/// A turn that finishes with "All set." after `pauses` half-second looks.
fn answering(pauses: u64) -> Tools<Fake> {
    let fake = Fake { finish_after: Some(pauses), ..Default::default() };
    let done = vec![ev("message.delta", json!({"itemId": "m", "text": "All set.", "final": true})), ev("run.completed", json!({}))];
    fake.turns.lock().unwrap().insert("mission_1".into(), turn("mission_1", true, done));
    fake.live.lock().unwrap().insert("mission_1".into());
    Tools::new(fake)
}

#[test]
fn waits_for_the_turn_to_finish_and_gives_its_answer_in_the_same_call() {
    let tools = answering(3);
    assert_eq!(tools.call("read_reply", &json!({"conversation_id": "mission_1", "wait_seconds": 10})).text, "All set.");
    assert_eq!(*tools.locust.pauses.lock().unwrap(), 3);
}

#[test]
fn says_it_is_still_working_once_the_wait_runs_out_never_longer_than_the_limit() {
    let tools = answering(u64::MAX);
    assert!(tools.call("read_reply", &json!({"conversation_id": "mission_1", "wait_seconds": 600})).text.starts_with("Still working"));
    assert_eq!(*tools.locust.pauses.lock().unwrap(), crate::door_tools::WAIT_LIMIT_S * 2);
}

#[test]
fn does_not_wait_at_all_unless_asked_or_while_a_card_waits_for_the_person() {
    let asked = answering(u64::MAX);
    assert!(asked.call("read_reply", &json!({"conversation_id": "mission_1"})).text.starts_with("Still working"));
    assert!(asked.call("read_reply", &json!({"conversation_id": "mission_1", "wait_seconds": -5})).text.starts_with("Still working"));
    assert_eq!(*asked.locust.pauses.lock().unwrap(), 0);
    let card = answering(u64::MAX);
    card.locust.waiting.lock().unwrap().insert("mission_1".into());
    assert!(card.call("read_reply", &json!({"conversation_id": "mission_1", "wait_seconds": 20})).text.starts_with("Waiting for the person"));
    assert_eq!(*card.locust.pauses.lock().unwrap(), 0);
}

/// Electron reads its main file for the wiring; so does this. The Rust door is never built without Stop (there is no
/// server variant without the window's own), so the half of the case about a server without it has nothing to hold.
#[test]
fn is_not_offered_by_a_server_without_the_windows_stop_and_the_windows_stop_is_what_locust_passes() {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let door = std::fs::read_to_string(here.join("src/door.rs")).unwrap();
    let window = std::fs::read_to_string(here.join("../src/main.rs")).unwrap();
    let stop = "crate::engine_api::api().stop(mission)";
    assert!(door.contains(&format!("fn cancel(&self, mission: &str) -> bool {{\n        {stop}.is_ok()")), "the door's Stop is the engine's");
    assert!(window.contains("locust_core::engine_api::api().stop(&id)"), "the window's Stop is the engine's too");
    assert!(Tools::new(Fake::default()).call("stop_turn", &json!({})).is_error, "stop_turn is offered, and asks for an id");
}
