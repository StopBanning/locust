//! Electron's `main/an-app-picks-up-a-turn-locust-closed-under.test.ts` (0.882), the door half:
//! listing and reading a cut-off turn, carrying it on with `cut_off: true`, and passing a `message` through `carry_on`.
use super::*;
use crate::carry_on::CarryOnAlso;

fn started_ev() -> Value {
    json!({"type": "run.started", "payload": {}})
}

fn lane_models() -> Vec<Value> {
    vec![
        json!({"id": "gpt-6.2-sol", "runtime": "codex", "displayName": "gpt-6.2-sol", "description": "", "supportedEfforts": []}),
        json!({"id": "opus", "runtime": "claude", "displayName": "opus", "description": "", "supportedEfforts": []}),
    ]
}

fn lane_door(events: Vec<Value>, live: bool) -> Tools<Fake> {
    let mut t = turn("mission_lane", true, events);
    t.prompt = "Port the rows".into();
    t.runtime = Some("codex".into());
    t.model = Some("gpt-6.2-sol".into());
    t.mode = Some("ask".into());
    t.started_by = json!({"kind": "mcp", "folder": "/work/lane"});
    t.updated = "2026-11-22T05:14:00.000Z".into();
    let fake = Fake {
        see_all: true,
        roster: Some(vec![]),
        model_list: Some(lane_models()),
        ..Default::default()
    };
    fake.turns.lock().unwrap().insert("mission_lane".into(), t);
    if live {
        fake.live.lock().unwrap().insert("mission_lane".into());
    }
    Tools::new(fake)
}

// case: lists it as stopped when Locust closed, offers carry_on, and says so in read_reply
#[test]
fn lists_it_as_stopped_when_locust_closed_offers_carry_on_and_says_so_in_read_reply() {
    let f = lane_door(vec![started_ev()], false);
    let rows = reply(&f.call("list_conversations", &json!({})));
    let row = &rows[0];
    assert_eq!(row["state"], "stopped: Locust closed while it was working");
    assert_eq!(row["can_carry_on"], true);

    let r = f.call("read_reply", &json!({"conversation_id": "mission_lane"}));
    assert!(r.text.starts_with("Locust closed while this turn was working"), "{}", r.text);
    assert_eq!(r.structured.as_ref().unwrap()["can_carry_on"], true);
    assert_eq!(r.structured.as_ref().unwrap()["message"], r.text);

    // Still running, it is working, not cut off.
    let live = lane_door(vec![started_ev()], true);
    let live_rows = reply(&live.call("list_conversations", &json!({})));
    assert_eq!(live_rows[0]["state"], "working");
    assert!(live_rows[0].get("can_carry_on").is_none());
}

// case: carries it on through Locust’s carry-on, saying it was cut off
#[test]
fn carries_it_on_through_locusts_carry_on_saying_it_was_cut_off() {
    let f = lane_door(vec![started_ev()], false);
    let r = f.call("carry_on", &json!({"conversation_id": "mission_lane", "runtime": "claude", "model": "opus"}));
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(
        f.locust.carried.lock().unwrap().as_slice(),
        [("mission_lane".to_owned(), "claude".to_owned(), Some("opus".to_owned()), CarryOnAlso { cut_off: true, message: None })]
    );
}

// case: passes the message through the door, and puts it after the carry-on words of a turn that ran out
#[test]
fn passes_the_message_through_the_door() {
    let f = lane_door(vec![started_ev(), ev("run.completed", json!({}))], false);
    let r = f.call("carry_on", &json!({"conversation_id": "mission_lane", "runtime": "claude", "message": "Review it."}));
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(
        f.locust.carried.lock().unwrap().as_slice(),
        [("mission_lane".to_owned(), "claude".to_owned(), None, CarryOnAlso { cut_off: false, message: Some("Review it.".into()) })]
    );
    // An invalid message (blank or over 8,000 chars) is refused in words and starts nothing.
    let blank = f.call("carry_on", &json!({"conversation_id": "mission_lane", "runtime": "claude", "message": "   "}));
    assert!(blank.is_error && blank.text.contains("8,000"), "{}", blank.text);
    let long = f.call("carry_on", &json!({"conversation_id": "mission_lane", "runtime": "claude", "message": "x".repeat(8_001)}));
    assert!(long.is_error && long.text.contains("8,000"), "{}", long.text);
    assert_eq!(f.locust.carried.lock().unwrap().len(), 1);
}
