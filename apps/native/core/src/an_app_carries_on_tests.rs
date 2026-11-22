//! Electron's `an-app-carries-on-a-conversation-that-ran-out.test.ts` (7 cases), with Electron's turns and models: an
//! app sees the person's conversations only while they let it, and carries on a turn that ran out of plan to another
//! agent through Locust's own carry-on, in the stopped turn's mode, never above what an app may run.
use super::*;

fn wren() -> Teammate {
    Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Reviewer".into(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: None }
}

fn ran_out(id: &str, by_app: bool, mode: &str) -> Turn {
    let mut t = turn(id, by_app, vec![
        ev("route.limit_detected", json!({"kind": "quota-exhausted"})),
        ev("run.failed", json!({"kind": "quota-exhausted", "message": "Usage limit reached."})),
    ]);
    t.prompt = "Fix the drums in the mix".into();
    t.runtime = Some("claude".into());
    t.model = Some("claude-opus-5-5".into());
    t.mode = Some(mode.into());
    t.updated = "2026-10-29T15:00:00.000Z".into();
    t
}

fn models() -> Vec<Value> {
    let model = |id: &str, runtime: &str, own: bool| json!({"id": id, "runtime": runtime, "displayName": id.to_uppercase(), "description": "", "supportedEfforts": [], "own": own});
    vec![model("claude-opus-5-5", "claude", false), model("gpt-6.1-sol", "codex", false), model("my-local", "codex", true)]
}

fn fixture(see_all: bool, own_mode: bool, turns: Vec<Turn>) -> Tools<Fake> {
    let fake = Fake { see_all, own_mode, roster: Some(vec![(wren(), None)]), model_list: Some(models()), owner_id: Some("tm_wren"), ..Default::default() };
    for t in turns {
        fake.turns.lock().unwrap().insert(t.mission_id.clone(), t);
    }
    Tools::new(fake)
}

fn both() -> Vec<Turn> {
    vec![ran_out("mission_person", false, "ask"), ran_out("mission_app", true, "ask")]
}

#[test]
fn lists_the_persons_conversations_only_while_they_let_it_saying_which_ran_out() {
    let off = fixture(false, false, both()).call("list_conversations", &json!({}));
    assert!(off.text.starts_with("Off: ") && off.text.contains("Let apps see your conversations"), "{}", off.text);
    let rows = reply(&fixture(true, false, both()).call("list_conversations", &json!({})));
    let person = rows.as_array().unwrap().iter().find(|r| r["conversation_id"] == "mission_person").unwrap();
    assert_eq!(person["teammate"], "Wren");
    // Rust names a conversation by its words; Electron by the team's saved title ("Fix the drums").
    assert_eq!(person["title"], "Fix the drums in the mix");
    assert_eq!(person["started_by"], "the person");
    assert_eq!((person["runtime"].clone(), person["model"].clone()), (json!("claude"), json!("claude-opus-5-5")));
    assert_eq!(person["state"], "stopped: its plan ran out");
    assert_eq!(person["can_carry_on"], true);
    assert_eq!(person["updated"], "2026-10-29T15:00:00.000Z");
    assert_eq!(person["folder"], Value::Null);
}

#[test]
fn carries_one_on_to_another_agent_and_model_through_locusts_own_carry_on() {
    let tools = fixture(true, false, both());
    let r = tools.call("carry_on", &json!({"conversation_id": "mission_person", "runtime": "codex", "model": "GPT-6.1-SOL"}));
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(
        tools.locust.carried.lock().unwrap().as_slice(),
        [("mission_person".to_owned(), "codex".to_owned(), Some("gpt-6.1-sol".to_owned()), crate::carry_on::CarryOnAlso::default())]
    );
    let said = reply(&r);
    assert_eq!((said["runtime"].clone(), said["model"].clone(), said["mode"].clone()), (json!("codex"), json!("gpt-6.1-sol"), json!("ask")));
}

#[test]
fn reports_the_carried_turns_full_working_path_even_when_no_folder_was_named_by_the_app() {
    let mut old = ran_out("mission_person", false, "ask");
    old.cwd = Some("/work/old".into());
    let tools = fixture(true, false, vec![old]);
    // The carried turn records no folder of its own here, so it works in the folder Locust has open: a full path.
    let said = reply(&tools.call("carry_on", &json!({"conversation_id": "mission_person", "runtime": "codex"})));
    assert_eq!(said["folder"], "/known/open", "{said}");
    let rows = reply(&tools.call("list_conversations", &json!({})));
    let listed = rows.as_array().unwrap().iter().find(|r| r["conversation_id"] == "mission_person").unwrap();
    assert_eq!(listed["folder"], "/work/old");
}

#[test]
fn carries_on_its_own_conversations_with_the_switch_off_and_never_the_persons() {
    let tools = fixture(false, false, both());
    assert!(!tools.call("carry_on", &json!({"conversation_id": "mission_app", "runtime": "codex"})).is_error);
    assert!(tools.call("carry_on", &json!({"conversation_id": "mission_person", "runtime": "codex"})).is_error);
    assert!(tools.call("read_reply", &json!({"conversation_id": "mission_person"})).is_error);
    assert_eq!(tools.locust.carried.lock().unwrap().len(), 1);
}

#[test]
fn never_carries_a_turn_above_ask_unless_each_teammates_own_mode_is_on() {
    let off = fixture(true, false, vec![ran_out("mission_auto", false, "auto")]);
    let r = off.call("carry_on", &json!({"conversation_id": "mission_auto", "runtime": "codex"}));
    assert!(r.text.contains("ran in auto, not Ask"), "{}", r.text);
    assert!(off.locust.carried.lock().unwrap().is_empty());
    let own = fixture(true, true, vec![ran_out("mission_auto", false, "auto")]);
    assert!(!own.call("carry_on", &json!({"conversation_id": "mission_auto", "runtime": "codex"})).is_error);
    assert_eq!(own.locust.carried.lock().unwrap().len(), 1);
}

#[test]
fn starts_nothing_for_an_agent_or_model_this_locust_cannot_run_or_an_own_model() {
    let tools = fixture(true, false, both());
    assert!(tools.call("carry_on", &json!({"conversation_id": "mission_person", "runtime": "gemini"})).text.contains("claude, codex"));
    assert!(tools.call("carry_on", &json!({"conversation_id": "mission_person", "runtime": "codex", "model": "my-local"})).is_error);
    assert!(tools.locust.carried.lock().unwrap().is_empty());
    assert_eq!(reply(&tools.call("list_models", &json!({}))), json!([
        {"runtime": "claude", "models": [{"id": "claude-opus-5-5", "name": "CLAUDE-OPUS-5-5"}]},
        {"runtime": "codex", "models": [{"id": "gpt-6.1-sol", "name": "GPT-6.1-SOL"}]},
    ]));
}

/// Electron reads its main file for the wiring; so does this: the door's carry-on is the engine's own.
#[test]
fn is_wired_to_the_persons_switch_and_locusts_own_carry_on() {
    let door = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/door.rs")).unwrap();
    assert!(door.contains("crate::engine_api::api().carry_with(mission, runtime, model, also)"), "the door passes carry_on to Locust");
    assert!(door.contains("fn see_all(&self) -> bool"), "and asks the person's switch");
}
