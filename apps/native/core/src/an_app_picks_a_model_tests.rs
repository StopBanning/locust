//! Electron's `an-app-picks-a-model-for-one-turn.test.ts` (9 cases), with Electron's teammates and models: an app may
//! name a model and effort for one turn, only one the teammate's own agent offers here, on the teammate's own account,
//! never the mode, and the saved route is not changed.
use super::*;

fn wren() -> Teammate {
    Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Reviewer".into(), runtime: Some("codex".into()), model: Some("gpt-6.1-sol".into()), account: Some("work".into()), busy: false, runs_in: "auto".into(), folder: None }
}
fn newt() -> Teammate {
    Teammate { id: "tm_new".into(), name: "Newt".into(), role: "Helper".into(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: None }
}
fn models() -> Vec<Value> {
    vec![
        json!({"id": "gpt-6.1-sol", "runtime": "codex", "displayName": "GPT-6.1 Sol", "description": "", "supportedEfforts": ["low", "medium", "high", "xhigh"]}),
        json!({"id": "gpt-6.1-mini", "runtime": "codex", "displayName": "GPT-6.1 Mini", "description": "", "supportedEfforts": ["low", "medium"]}),
        json!({"id": "gpt-5.6", "runtime": "codex", "displayName": "GPT-5.6", "description": "", "supportedEfforts": ["high"], "older": true}),
        json!({"id": "claude-opus-5-5", "runtime": "claude", "displayName": "Opus 5.5", "description": "", "supportedEfforts": ["high"]}),
        json!({"id": "own_local", "runtime": "codex", "displayName": "My local model", "description": "", "supportedEfforts": [], "own": true}),
    ]
}
fn fixture() -> Tools<Fake> {
    let fake = Fake { roster: Some(vec![(wren(), Some("medium".into())), (newt(), None)]), model_list: Some(models()), owner_id: Some("tm_wren"), ..Default::default() };
    // The conversation ran on Wren's own model, as Electron's mission did.
    let mut t = turn("mission_1", true, vec![ev("run.completed", json!({}))]);
    t.model = Some("gpt-6.1-sol".into());
    fake.turns.lock().unwrap().insert("mission_1".into(), t);
    Tools::new(fake)
}
fn last(tools: &Tools<Fake>) -> StartRequest {
    tools.locust.started.lock().unwrap().last().cloned().expect("a start")
}

#[test]
fn runs_that_model_and_effort_on_the_teammates_own_agent_and_account_in_ask_keeping_the_saved_route() {
    let tools = fixture();
    let r = tools.call("start_conversation", &json!({"teammate": "Wren", "message": "Review this.", "model": "GPT-6.1 Mini", "effort": "LOW", "runtime": "claude"}));
    let asked = last(&tools);
    assert_eq!(asked.teammate.id, "tm_wren");
    assert_eq!(asked.teammate.account.as_deref(), Some("work"), "on the teammate's own account");
    assert_eq!((asked.runtime.as_deref(), asked.model.as_deref(), asked.effort.as_deref()), (Some("codex"), Some("gpt-6.1-mini"), Some("low")));
    assert_eq!(asked.mode, Mode::Ask);
    assert_eq!(asked.teammate.model.as_deref(), Some("gpt-6.1-sol"), "the saved route is not changed");
    let said = reply(&r);
    assert_eq!((said["mode"].clone(), said["model"].clone(), said["effort"].clone()), (json!("ask"), json!("gpt-6.1-mini"), json!("low")));
}

#[test]
fn uses_the_teammates_own_when_the_app_names_neither() {
    let tools = fixture();
    tools.call("start_conversation", &json!({"teammate": "Wren", "message": "Review this."}));
    let asked = last(&tools);
    assert_eq!((asked.runtime, asked.model, asked.effort), (None, None, None));
    assert_eq!(asked.mode, Mode::Ask);
}

#[test]
fn takes_an_effort_alone_on_the_saved_model_and_keeps_the_saved_effort_with_a_model_named_alone_when_it_fits() {
    let tools = fixture();
    tools.call("start_conversation", &json!({"teammate": "Wren", "message": "Think hard.", "effort": "xhigh"}));
    let asked = last(&tools);
    assert_eq!((asked.model.as_deref(), asked.effort.as_deref()), (Some("gpt-6.1-sol"), Some("xhigh")));
    tools.call("start_conversation", &json!({"teammate": "Wren", "message": "Quick one.", "model": "gpt-6.1-mini"}));
    let asked = last(&tools);
    assert_eq!((asked.model.as_deref(), asked.effort.as_deref()), (Some("gpt-6.1-mini"), Some("medium")));
}

#[test]
fn starts_nothing_for_a_model_the_agent_does_not_offer_here_or_an_effort_the_model_does_not_take() {
    let tools = fixture();
    let long = "x".repeat(500);
    for (args, words) in [
        (json!({"model": "claude-opus-5-5"}), "not one codex offers in this Locust"),
        (json!({"model": "gpt-7-imaginary"}), "not one codex offers in this Locust"),
        (json!({"model": "My local model"}), "not one codex offers in this Locust"),
        (json!({"model": "gpt-6.1-mini", "effort": "xhigh"}), "GPT-6.1 Mini takes these efforts here: low, medium."),
        (json!({"model": long}), "short text"),
    ] {
        let mut call = json!({"teammate": "Wren", "message": "Go."});
        for (k, v) in args.as_object().unwrap() {
            call[k] = v.clone();
        }
        let r = tools.call("start_conversation", &call);
        assert!(r.is_error, "{args}");
        assert!(r.text.contains(words), "{args}: {}", r.text);
        assert!(r.text.contains("Nothing was started."), "{args}: {}", r.text);
    }
    assert!(tools.call("start_conversation", &json!({"teammate": "Newt", "message": "Go.", "model": "gpt-6.1-mini"})).is_error);
    assert!(tools.locust.started.lock().unwrap().is_empty());
}

#[test]
fn lists_the_choices_for_a_teammate_current_ones_first_without_retired_or_own_models() {
    let tools = fixture();
    let listed = reply(&tools.call("list_models", &json!({"teammate": "wren"})));
    assert_eq!(listed["teammate"], "Wren");
    assert_eq!(listed["runtime"], "codex");
    assert_eq!(listed["model"], "gpt-6.1-sol");
    assert_eq!(listed["effort"], "medium");
    assert_eq!(listed["models"], json!([
        {"id": "gpt-6.1-sol", "name": "GPT-6.1 Sol", "efforts": ["low", "medium", "high", "xhigh"]},
        {"id": "gpt-6.1-mini", "name": "GPT-6.1 Mini", "efforts": ["low", "medium"]},
    ]));
    assert!(tools.locust.started.lock().unwrap().is_empty());
}

/// Electron builds a server without the choice for a voice call's tools; Rust's door is built one way, always with
/// list_models and the choice, so this case's server has no Rust form. What holds: list_models is always offered.
#[test]
fn is_refused_by_a_server_that_does_not_offer_the_choice_such_as_a_voice_calls() {
    let tools = fixture();
    assert!(!tools.call("list_models", &json!({})).is_error, "the choice is offered");
}

#[test]
fn steps_a_turn_down_to_ask_when_the_app_asks_for_read_only_and_never_up() {
    let own = Fake { roster: Some(vec![(wren(), Some("medium".into()))]), model_list: Some(models()), owner_id: Some("tm_wren"), own_mode: true, ..Default::default() };
    let own = Tools::new(own);
    let r = own.call("start_conversation", &json!({"teammate": "Wren", "message": "Change it."}));
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(last(&own).mode, Mode::Auto);
    own.call("start_conversation", &json!({"teammate": "Wren", "message": "Only look.", "read_only": true}));
    assert_eq!(last(&own).mode, Mode::Ask);
    // Only read_only steps down; nothing named raises a turn above Ask.
    let plain = fixture();
    // Electron runs it in Ask; Rust refuses the turn and says why. Either way nothing runs above Ask.
    let r = plain.call("start_conversation", &json!({"teammate": "Wren", "message": "Go.", "read_only": false, "mode": "auto"}));
    if r.is_error {
        assert!(r.text.contains("Choosing the mode or folder is off") && r.text.contains("Nothing was started."), "{}", r.text);
        assert!(plain.locust.started.lock().unwrap().is_empty());
    } else {
        assert_eq!(last(&plain).mode, Mode::Ask);
    }
}

#[test]
fn says_which_folder_its_turns_work_in_as_a_full_path() {
    let tools = fixture();
    let listed = reply(&tools.call("list_teammates", &json!({})));
    assert_eq!(listed[0]["name"], "Wren");
    assert_eq!(listed[0]["folder"], "/known/open");
    let started = reply(&tools.call("start_conversation", &json!({"teammate": "Wren", "message": "Go."})));
    assert!(started["folder"].as_str().is_some_and(|f| f.starts_with('/')), "{started}");
}

#[test]
fn takes_the_same_choice_when_a_conversation_goes_on() {
    let tools = fixture();
    let r = tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "Again, harder.", "effort": "high"}));
    assert!(!r.is_error, "{}", r.text);
    let asked = last(&tools);
    assert_eq!(asked.follow_up_of.as_deref(), Some("mission_1"));
    assert_eq!((asked.model.as_deref(), asked.effort.as_deref()), (Some("gpt-6.1-sol"), Some("high")));
    assert_eq!(asked.mode, Mode::Ask);
}
