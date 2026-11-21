//! Electron's `main/an-app-started-opencode-turn-works-where-it-was-sent.test.ts`, the host's half: an app's own work
//! sent to Scout's teammate runs in the mode and folder it was sent with, not sealed in Ask; a turn carried on stays in
//! the folder an app named. The door's half (its replies) is in `door_tools`' `an_app_started_turn_tests.rs`.
use super::provider_carry_on::{carried_from, stopped, team};
use super::*;
use crate::plan_carry_on_tests::quota;
use crate::scout::host::asks_scout;

const SCOUT: &str = "tm_scout";

// case: is not taken for a question to Scout, even on Scout's own teammate
#[test]
fn is_not_taken_for_a_question_to_scout_even_on_scouts_own_teammate() {
    // The person's own turn and a voice call still ask Scout what it saw, sealed.
    assert!(asks_scout(SCOUT, Some(SCOUT), None));
    assert!(asks_scout(SCOUT, Some(SCOUT), Some(&json!({"kind": "mcp", "via": "voice"}))));
    // An app's own work, with or without a folder named, runs as any teammate's turn does.
    assert!(!asks_scout(SCOUT, Some(SCOUT), Some(&json!({"kind": "mcp"}))));
    assert!(!asks_scout(SCOUT, Some(SCOUT), Some(&json!({"kind": "mcp", "folder": "/work/review"}))));
    // Another teammate, or no Scout set up, is never a question to Scout.
    assert!(!asks_scout("tm_wren", Some(SCOUT), None));
    assert!(!asks_scout(SCOUT, None, None));
}

/// Scout on, on `tm_scout`, with a look log; Auto allowed in this workspace.
fn scout_on(t: &Temp) {
    std::fs::create_dir_all(t.0.join("native")).unwrap();
    std::fs::write(t.0.join("native/scout.json"), json!({"settings": {"on": true, "teammateId": SCOUT}}).to_string()).unwrap();
    std::fs::write(
        t.0.join("native").join(crate::scout::store::SCOUT_LOG_FILE),
        json!([{"at": "2026-10-22T12:00:00.000Z", "looks": 1, "raised": [], "aside": [{"key": "pr:9", "said": "PR #9 minor lint", "why": "told"}], "off": []}]).to_string(),
    )
    .unwrap();
    let settings = crate::workspace_settings::WorkspaceSettings { auto_mode: true, ..Default::default() };
    crate::workspace_settings::save(&t.store(), &settings).unwrap();
}

/// The host's half of Electron's "hands the host the mode and folder the app named": the door's turn with Scout's
/// teammate really runs in Auto in the named folder, unsealed (the fake refuses a sealed config for `auto`).
// case: hands the host the mode and folder the app named, and replies with them when they ran
#[test]
fn an_app_turn_sent_to_scouts_teammate_runs_in_the_mode_and_folder_it_was_sent_with() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("auto"));
    scout_on(&t);
    let folder = t.0.join("work");
    let mut req = job(&t, None, "Synthetic prompt");
    req.teammate.id = SCOUT.into();
    req.teammate.name = "Scout".into();
    let origin = Origin {
        mode: Some(Mode::Auto),
        folder: Some(folder.clone()),
        started_by: Some(json!({"kind": "mcp", "folder": folder.to_string_lossy()})),
        ..Origin::default()
    };
    let started = send_from(req, origin).unwrap();
    finished(&started.conversation_id);
    let recs = records(&path_of(&t, &started.conversation_id));
    assert_eq!(recs[0]["metadata"]["mode"], "auto");
    assert_eq!(recs[0]["metadata"]["sandbox"], "full-access");
    assert_eq!(recs[0]["metadata"]["startedBy"], json!({"kind": "mcp", "folder": folder.to_string_lossy()}));
    assert!(kinds(&path_of(&t, &started.conversation_id)).contains(&"run.completed".to_owned()), "{recs:?}");
    // A voice call through the door is still a question to Scout: no slot is given up for it.
    assert!(crate::scout::host::asking_scout_slot(&t.store(), SCOUT, Some(&json!({"kind": "mcp", "via": "voice"}))).is_some());
    assert!(crate::scout::host::asking_scout_slot(&t.store(), SCOUT, Some(&json!({"kind": "mcp"}))).is_none());
}

/// A stopped turn whose plan ran out, started as `started_by` records it.
fn ran_out_started_by(t: &Temp, id: &str, started_by: Option<Value>) {
    stopped(t, id, "codex", &[quota()]);
    if let Some(by) = started_by {
        let path = path_of(t, id);
        let text = fs::read_to_string(&path).unwrap();
        let mut lines: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        lines[0]["metadata"]["startedBy"] = by;
        let out: String = lines.iter().map(|l| format!("{l}\n")).collect();
        fs::write(&path, out).unwrap();
    }
}

// case: stays in the folder an app named for the conversation, not the teammate's own
#[test]
fn stays_in_the_folder_an_app_named_for_the_conversation_not_the_teammates_own() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    let review = t.0.join("review");
    fs::create_dir_all(&review).unwrap();
    crate::folders::use_folder(&t.store(), &review, "2026-11-20T00:00:00.000Z").unwrap();
    let named = crate::door::check_folder(&t.store(), &review.to_string_lossy()).unwrap().to_string_lossy().into_owned();
    ran_out_started_by(&t, "mission_app", Some(json!({"kind": "mcp", "folder": named})));
    let (id, folder, _) = carry("mission_app", "claude", None).unwrap();
    finished(&id);
    assert_eq!(folder.as_deref(), Some(named.as_str()));
    let started = carried_from(&t, "mission_app");
    assert_eq!(started[0]["startedBy"], json!({"kind": "carry-on", "from": "codex", "folder": named}));
    assert_eq!(started[0]["command"]["cwd"], json!(named));
    // A carried turn carried on again keeps it: the carried turn records the folder too.
    ran_out_started_by(&t, "mission_again", Some(json!({"kind": "carry-on", "from": "claude", "folder": named})));
    let (again, folder, _) = carry("mission_again", "claude", None).unwrap();
    finished(&again);
    assert_eq!(folder.as_deref(), Some(named.as_str()));
    assert_eq!(carried_from(&t, "mission_again")[0]["startedBy"]["folder"], json!(named));
    // A conversation nobody sent to a folder stands where it always did.
    for (i, by) in [None, Some(json!({"kind": "mcp"})), Some(json!({"kind": "routine", "routineId": "r", "step": 1}))].into_iter().enumerate() {
        let id = format!("mission_plain_{i}");
        ran_out_started_by(&t, &id, by);
        let (next, folder, _) = carry(&id, "claude", None).unwrap();
        finished(&next);
        assert_eq!(folder, None);
        assert_eq!(carried_from(&t, &id)[0]["startedBy"].get("folder"), None);
    }
}

// case: reads the folder from an app turn or a carried turn, and from nothing else
#[test]
fn reads_the_folder_from_an_app_turn_or_a_carried_turn_and_from_nothing_else() {
    use crate::door_tools::conversation_folder;
    let folder = "/work/review";
    assert_eq!(conversation_folder(&json!({"kind": "mcp", "folder": folder})).as_deref(), Some(folder));
    assert_eq!(conversation_folder(&json!({"kind": "carry-on", "folder": folder})).as_deref(), Some(folder));
    assert_eq!(conversation_folder(&json!({"kind": "mcp"})), None);
    assert_eq!(conversation_folder(&json!({"kind": "mcp", "folder": ""})), None);
    assert_eq!(conversation_folder(&json!({"kind": "window", "folder": folder})), None);
    assert_eq!(conversation_folder(&Value::Null), None);
}
