//! Electron `main/route-at-start.test.ts`, case by case: who decides a run's route at the moment it starts. A pick made
//! for this teammate's conversation, then the route the host keeps for the teammate (its effort included), then what
//! was asked or the defaults. The first cases also go through the app's own start (`native::send_from`), where the
//! organization policy's model refusal names the runtime and model the turn would have started on.
use super::*;
use crate::engine::Teammate;
use crate::store::Store;
use serde_json::json;

fn saved() -> TeammateRoute {
    TeammateRoute { runtime: "codex".into(), model: "gpt-5.6-luna".into(), mode: "accept-edits".into(), account: None, effort: Some("low".into()) }
}

fn input(runtime: Option<&str>, model: Option<&str>) -> StartRouteInput {
    StartRouteInput { runtime: runtime.map(str::to_owned), model: model.map(str::to_owned), ..Default::default() }
}

fn fallback() -> StartRouteInput {
    input(Some("opencode"), Some("account-default"))
}

fn start(runtime: &str, model: Option<&str>, effort: Option<&str>) -> StartRoute {
    StartRoute { runtime: runtime.into(), model: model.map(str::to_owned), effort: effort.map(str::to_owned), account: None }
}

fn picker(runtime: &str, model: &str) -> PickerRoute {
    PickerRoute { runtime: runtime.into(), model: model.into(), account: None }
}

fn choices() -> HashMap<String, PickerRoute> {
    HashMap::from([("mission_a".to_owned(), picker("opencode", "account-default")), ("new:tm_gem".to_owned(), picker("opencode", "account-default"))])
}

fn saved_picker() -> PickerRoute {
    picker("codex", "gpt-5.6-luna")
}

// ---- Through the app's own start: the policy refuses every model, saying which runtime and model it would have run.

struct Started {
    root: std::path::PathBuf,
    _hook: crate::native::hooks::Guard,
}
impl Drop for Started {
    fn drop(&mut self) {
        crate::org_policy::use_policy_on_this_thread(None);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn host(team: serde_json::Value) -> Started {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "locust-route-at-start-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::write(root.join("native").join("teammates.json"), team.to_string()).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let hook = crate::native::hooks::install(store, None);
    let mut policy = serde_json::Map::new();
    policy.insert("allowedModels".into(), json!({ "codex": ["none"], "opencode": ["none"], "kilo": ["none"], "cursor": ["none"] }));
    crate::org_policy::use_policy_on_this_thread(Some(crate::org_policy::PolicyState {
        path: "/etc/locust/policy.json".into(),
        present: true,
        policy,
        error: None,
    }));
    Started { root, _hook: hook }
}

fn team_with(route: serde_json::Value) -> serde_json::Value {
    json!({ "schemaVersion": 1, "teammates": [{ "teammateId": "tm_wren", "name": "Wren", "route": route }], "missionOwners": {}, "settings": {} })
}

/// The window's copy of a teammate, which may be behind what the host keeps.
fn window_copy(id: &str, runtime: &str, model: &str) -> Teammate {
    Teammate {
        id: id.into(),
        name: "Wren".into(),
        role: String::new(),
        runtime: Some(runtime.into()),
        model: Some(model.into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    }
}

/// What the app's start was refused with: the policy's sentence names the route the turn resolved to.
fn started_on(teammate: Teammate, picked: Option<&str>) -> String {
    let job = crate::native::Send { teammate, conversation: None, prompt: "hello".into(), model: picked.map(str::to_owned), effort: None };
    match crate::native::send_from(job, crate::native::Origin::default()) {
        Err(why) => why,
        Ok(_) => panic!("the probe policy should have refused the start"),
    }
}

#[test]
fn honours_an_explicit_picker_override_for_this_teammate() {
    // case: honours an explicit picker override for this teammate
    let asked = StartRouteInput { route_override_for: Some("tm_wren".into()), ..fallback() };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved())))), start("opencode", Some("account-default"), None));
    // The app's start: a model picked in the box for this conversation is the route.
    let _h = host(team_with(json!({ "runtime": "codex", "model": "gpt-5.6-luna", "mode": "accept-edits", "effort": "low" })));
    let why = started_on(window_copy("tm_wren", "opencode", "account-default"), Some("account-default"));
    assert!(why.contains("The opencode model account-default is not allowed"), "{why}");
}

#[test]
fn uses_the_host_saved_route_over_discovery_default_selections_including_saved_effort() {
    // case: uses the host saved route over discovery/default selections, including saved effort
    assert_eq!(route_at_start(&fallback(), Some(("tm_wren", Some(&saved())))), start("codex", Some("gpt-5.6-luna"), Some("low")));
    // The app's start: the window's copy says OpenCode, the host keeps Codex, and the turn starts on Codex.
    let _h = host(team_with(json!({ "runtime": "codex", "model": "gpt-5.6-luna", "mode": "accept-edits", "effort": "low" })));
    let why = started_on(window_copy("tm_wren", "opencode", "account-default"), None);
    assert!(why.contains("The codex model gpt-5.6-luna is not allowed"), "{why}");
}

#[test]
fn leaves_nobodys_mission_on_the_composer_route_regardless_of_old_picker_intent() {
    // case: leaves nobody's mission on the composer route, regardless of old picker intent
    let asked = StartRouteInput { effort: Some("high".into()), route_override_for: Some("tm_wren".into()), ..fallback() };
    assert_eq!(route_at_start(&asked, None), start("opencode", Some("account-default"), Some("high")));
    // The app's start: a conversation with nobody keeps what it was asked to run on.
    let _h = host(team_with(json!({ "runtime": "codex", "model": "gpt-5.6-luna", "mode": "accept-edits", "effort": "low" })));
    let why = started_on(window_copy("", "opencode", "account-default"), None);
    assert!(why.contains("The opencode model account-default is not allowed"), "{why}");
}

#[test]
fn does_not_mistake_another_teammates_picker_change_for_this_teammates_choice() {
    // case: does not mistake another teammate's picker change for this teammate's choice
    let asked = StartRouteInput { route_override_for: Some("tm_gem".into()), ..fallback() };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved())))).runtime, "codex");
}

#[test]
fn preserves_account_default_as_an_intentional_picker_selection() {
    // case: preserves account-default as an intentional picker selection
    let asked = StartRouteInput { route_override_for: Some("tm_wren".into()), ..input(Some("codex"), Some("account-default")) };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved())))), start("codex", Some("account-default"), None));
}

#[test]
fn does_not_promote_a_seeded_account_default_to_picker_intent() {
    // case: does not promote a seeded account-default to picker intent
    let asked = input(Some("codex"), Some("account-default"));
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved())))).model.as_deref(), Some("gpt-5.6-luna"));
}

#[test]
fn keeps_existing_defaults_for_a_new_teammate_without_a_saved_route() {
    // case: keeps existing defaults for a new teammate without a saved route
    assert_eq!(route_at_start(&fallback(), Some(("tm_new", None))), start("opencode", Some("account-default"), None));
    assert_eq!(route_at_start(&StartRouteInput::default(), None), start("codex", None, None));
    // The app's start: a teammate the host keeps no route for starts on what the window had.
    let _h = host(team_with(json!({ "runtime": "codex", "model": "gpt-5.6-luna", "mode": "accept-edits", "effort": "low" })));
    let why = started_on(window_copy("tm_new", "opencode", "account-default"), None);
    assert!(why.contains("The opencode model account-default is not allowed"), "{why}");
}

#[test]
fn keeps_effort_changes_when_the_request_is_based_on_the_saved_picker_identity() {
    // case: keeps effort changes when the request is based on the saved picker identity
    let asked = StartRouteInput { effort: Some("high".into()), ..input(Some("codex"), Some("gpt-5.6-luna")) };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved())))), start("codex", Some("gpt-5.6-luna"), Some("high")));
}

#[test]
fn preserves_concrete_effort_variants_without_declaring_them_a_new_picker_choice() {
    // case: preserves concrete effort variants without declaring them a new picker choice
    let cursor = TeammateRoute { runtime: "cursor".into(), model: "grok-low".into(), ..saved() };
    let asked = StartRouteInput { model_choice: Some("grok-low".into()), ..input(Some("cursor"), Some("grok-high")) };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&cursor)))), start("cursor", Some("grok-high"), None));
}

#[test]
fn cannot_carry_a_stale_effort_or_model_transform_across_a_different_saved_route() {
    // case: cannot carry a stale effort or model transform across a different saved route
    let asked = StartRouteInput {
        model_choice: Some("grok-low".into()),
        effort: Some("high".into()),
        ..input(Some("cursor"), Some("grok-high"))
    };
    assert_eq!(route_at_start(&asked, Some(("tm_wren", Some(&saved())))), start("codex", Some("gpt-5.6-luna"), Some("low")));
}

#[test]
fn returns_no_permission_mode_for_the_caller_to_accidentally_inherit() {
    // case: returns no permission mode for the caller to accidentally inherit
    // A StartRoute has no mode to inherit: written out whole, it is runtime, model, effort and account, and nothing of
    // the saved route's Accept edits.
    let StartRoute { runtime, model, effort, account } = route_at_start(&fallback(), Some(("tm_wren", Some(&saved()))));
    assert_eq!((runtime.as_str(), model.as_deref(), effort.as_deref(), account), ("codex", Some("gpt-5.6-luna"), Some("low"), None));
}

#[test]
fn a_new_chat_with_a_teammate_starts_on_their_saved_model() {
    // case: a new chat with a teammate starts on their saved model
    let key = pick_key_for(None, Some("tm_wren"));
    let shown = composer_route_for(&picker("opencode", "account-default"), Some(("tm_wren", Some(&saved()))), &choices(), Some(&key), None);
    assert_eq!(shown, saved_picker());
}

#[test]
fn a_pick_made_in_a_conversation_is_shown_there_again() {
    // case: a pick made in a conversation is shown there again
    let shown = composer_route_for(&saved_picker(), Some(("tm_wren", Some(&saved()))), &choices(), Some("mission_a"), None);
    assert_eq!(shown, picker("opencode", "account-default"));
}

#[test]
fn a_conversation_shows_what_it_ran_on_not_the_saved_model_of_its_teammate() {
    // case: a conversation shows what it ran on, not the saved model of its teammate
    let ran = picker("opencode", "opencode/free");
    let shown = composer_route_for(&picker("opencode", "account-default"), Some(("tm_wren", Some(&saved()))), &choices(), Some("mission_b"), Some(&ran));
    assert_eq!(shown, ran);
}

#[test]
fn a_pick_for_a_new_chat_with_one_teammate_is_not_for_another() {
    // case: a pick for a new chat with one teammate is not for another
    let gem = pick_key_for(None, Some("tm_gem"));
    assert_eq!(
        composer_route_for(&picker("opencode", "account-default"), Some(("tm_gem", Some(&saved()))), &choices(), Some(&gem), None),
        picker("opencode", "account-default")
    );
    let wren = pick_key_for(None, Some("tm_wren"));
    assert_eq!(
        composer_route_for(&picker("opencode", "account-default"), Some(("tm_wren", Some(&saved()))), &choices(), Some(&wren), None),
        saved_picker()
    );
}

#[test]
fn home_does_not_inherit_a_teammate_override() {
    // case: home does not inherit a teammate override
    let home = picker("claude", "account-default");
    let key = pick_key_for(None, None);
    assert_eq!(key, "new");
    assert_eq!(composer_route_for(&home, None, &choices(), Some(&key), None), home);
}

#[test]
fn shows_a_kilo_teammate_saved_on_the_account_default_as_auto_free() {
    // case: shows a Kilo teammate saved on the account default as Auto Free
    let kilo = TeammateRoute { runtime: "kilo".into(), model: "account-default".into(), mode: "ask".into(), account: None, effort: None };
    let key = pick_key_for(None, Some("tm_wren"));
    let shown = composer_route_for(&picker("opencode", "account-default"), Some(("tm_wren", Some(&kilo))), &HashMap::new(), Some(&key), None);
    assert_eq!((shown.runtime.as_str(), shown.model.as_str()), ("kilo", "kilo/kilo-auto/free"));
    let shown = composer_route_for(&picker("kilo", "account-default"), None, &HashMap::new(), Some("new"), None);
    assert_eq!((shown.runtime.as_str(), shown.model.as_str()), ("kilo", "kilo/kilo-auto/free"));
    let started = route_at_start(&input(Some("kilo"), Some("account-default")), Some(("tm_wren", Some(&kilo))));
    assert_eq!((started.runtime.as_str(), started.model.as_deref()), ("kilo", Some("kilo/kilo-auto/free")));
    // The app's start: the saved account default goes out as Auto Free.
    let _h = host(team_with(json!({ "runtime": "kilo", "model": "account-default", "mode": "ask" })));
    let why = started_on(window_copy("tm_wren", "opencode", "account-default"), None);
    assert!(why.contains("The kilo model kilo/kilo-auto/free is not allowed"), "{why}");
}

// A route saved without a model is not a saved route (Electron's teammate store requires one): the start keeps what
// was asked, never an empty model.
#[test]
fn a_route_saved_without_a_model_is_not_a_saved_route() {
    let team = json!({"teammates": [{"teammateId": "tm_codec", "route": {"runtime": "codex", "mode": "accept-edits"}}]});
    assert!(saved_route_in(&team, "tm_codec").is_none());
    let with_model = json!({"teammates": [{"teammateId": "tm_codec", "route": {"runtime": "codex", "model": "gpt-5.6-luna"}}]});
    assert_eq!(saved_route_in(&with_model, "tm_codec").unwrap().model, "gpt-5.6-luna");
}
