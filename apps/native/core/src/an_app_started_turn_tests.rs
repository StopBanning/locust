//! Electron's `main/an-app-started-opencode-turn-works-where-it-was-sent.test.ts`, the door's half: the door hands the
//! host the mode and folder an app named and replies with what really ran; a carried-on conversation's folder is named
//! and kept for the app's next message. The host's half (Scout's teammate runs unsealed, the carry keeps the folder) is
//! in `native/an_app_started_turn_tests.rs`.
use super::*;

const FOLDER: &str = "/known/review";

// case: hands the host the mode and folder the app named, and replies with them when they ran
#[test]
fn hands_the_host_the_mode_and_folder_the_app_named_and_replies_with_them_when_they_ran() {
    let tools = Tools::new(Fake { own_mode: true, ..Default::default() });
    let r = reply(&tools.call("start_conversation", &json!({"teammate": "Scout", "message": "Review M1.", "mode": "auto", "folder": FOLDER})));
    let sent = tools.locust.started.lock().unwrap()[0].clone();
    assert_eq!((sent.teammate.id.as_str(), sent.mode), ("tm_scout", Mode::Auto));
    assert_eq!(sent.folder, Some(PathBuf::from(FOLDER)));
    assert_eq!((r["conversation_id"].as_str(), r["mode"].as_str(), r["folder"].as_str()), (Some("m1"), Some("auto"), Some(FOLDER)));
    assert_eq!(r.get("mode_asked"), None);
}

// case: replies with the mode the host really ran, and names the one asked for, when they differ
#[test]
fn replies_with_the_mode_the_host_really_ran_and_names_the_one_asked_for_when_they_differ() {
    let tools = Tools::new(Fake { own_mode: true, host_mode: Some("ask"), ..Default::default() });
    let said = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "Review M1.", "mode": "auto", "folder": FOLDER}));
    assert!(!said.is_error, "{}", said.text);
    let r = reply(&said);
    assert_eq!((r["conversation_id"].as_str(), r["mode"].as_str(), r["mode_asked"].as_str(), r["folder"].as_str()), (Some("m1"), Some("ask"), Some("auto"), Some(FOLDER)));
}

// case: the door's carry_on names the folder, and the app's next message stays there
#[test]
fn the_doors_carry_on_names_the_folder_and_the_apps_next_message_stays_there() {
    let fake = Fake { own_mode: true, see_all: true, carried_folder: Some(FOLDER), ..Default::default() };
    let mut ran_out = turn("mission_1", true, vec![ev("route.limit_detected", json!({"kind": "quota-exhausted"}))]);
    ran_out.runtime = Some("antigravity".into());
    ran_out.mode = Some("auto".into());
    ran_out.started_by = json!({"kind": "mcp", "folder": FOLDER});
    fake.turns.lock().unwrap().insert("mission_1".into(), ran_out);
    let tools = Tools::new(fake);
    let said = tools.call("carry_on", &json!({"conversation_id": "mission_1", "runtime": "codex", "model": "gpt-5.5"}));
    assert!(!said.is_error, "{}", said.text);
    let r = reply(&said);
    assert_eq!((r["conversation_id"].as_str(), r["folder"].as_str()), (Some("mission_1-carried"), Some(FOLDER)));
    let next = tools.call("send_message", &json!({"conversation_id": "mission_1-carried", "message": "Finish the review."}));
    assert!(!next.is_error, "{}", next.text);
    let sent = tools.locust.started.lock().unwrap().last().cloned().unwrap();
    assert_eq!(sent.follow_up_of.as_deref(), Some("mission_1-carried"));
    assert_eq!(sent.folder, Some(PathBuf::from(FOLDER)));
}
