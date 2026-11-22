//! Carries apps/desktop/src/main/a-carried-conversation-keeps-its-route-and-reports-its-status.test.ts (Electron
//! ~/work/locust, 0.880): a follow-up an app sends keeps its conversation's own route -- agent, model, account and
//! effort -- not the teammate's saved one, and read_reply says where a turn stands beside its words.
use super::*;
use crate::carry_on::{Reason, carry_on_prompt, stopped_for};
use crate::door_tools::ToolResult;
use std::collections::{HashMap, HashSet};

const CRASH: &str = "OpenCode ended without a step that reported it had stopped.";

fn crash() -> Value {
    json!({"type": "run.failed", "payload": {"kind": "process-failed", "message": CRASH}})
}

/// The teammate the conversation belongs to, saved on another route: none of it may reach a follow-up.
fn mate(id: &str, folder: Option<&str>) -> Teammate {
    Teammate {
        id: id.into(),
        name: "Mate".into(),
        role: "Builder".into(),
        runtime: Some("codex".into()),
        model: Some("gpt-5.5".into()),
        account: Some("account_saved".into()),
        busy: false,
        runs_in: "ask".into(),
        folder: folder.map(str::to_owned),
    }
}

/// The fixture's Locust: one conversation, `root`, whose newest turn `tip` was carried on to Cursor's grok.
struct Fake {
    roster: Vec<(Teammate, Option<String>)>,
    turns: Mutex<HashMap<String, Turn>>,
    live: Mutex<HashSet<String>>,
    waiting: Mutex<HashSet<String>>,
    started: Mutex<Vec<StartRequest>>,
    open: &'static str,
    /// The host holds the carried turn's route (`followUpRoute`); otherwise only its record is known.
    holds: bool,
}

fn fixture() -> Fake {
    let tip = Turn {
        mission_id: "tip".into(),
        prompt: "Look at it".into(),
        runtime: Some("cursor".into()),
        model: Some("grok".into()),
        mode: Some("ask".into()),
        started_by: json!({"kind": "carry-on", "from": "antigravity"}),
        cwd: Some("/work/conversation".into()),
        events: vec![crash()],
        ..Default::default()
    };
    let root = Turn { mission_id: "root".into(), started_by: json!({"kind": "mcp"}), ..Default::default() };
    Fake {
        roster: vec![(mate("mate", None), Some("low".into()))],
        turns: Mutex::new(HashMap::from([("tip".to_owned(), tip), ("root".to_owned(), root)])),
        live: Mutex::default(),
        waiting: Mutex::default(),
        started: Mutex::default(),
        open: "/wrong",
        holds: true,
    }
}

impl Locust for Fake {
    fn teammates(&self) -> Result<Vec<(Teammate, Option<String>)>, String> {
        Ok(self.roster.clone())
    }
    fn busy(&self, _: &str) -> bool {
        false
    }
    fn start(&self, request: StartRequest) -> Result<Begun, String> {
        let next = Turn {
            mission_id: "next".into(),
            runtime: request.runtime.clone(),
            model: request.model.clone(),
            started_by: json!({"kind": "mcp"}),
            cwd: Some("/work/conversation".into()),
            ..Default::default()
        };
        self.turns.lock().unwrap().insert("next".into(), next);
        let begun = Begun { mission_id: "next".into(), mode: request.mode.as_str().into(), runtime: request.runtime.clone(), model: request.model.clone() };
        self.started.lock().unwrap().push(request);
        Ok(begun)
    }
    fn read(&self, mission: &str) -> Option<Turn> {
        self.turns.lock().unwrap().get(mission).cloned()
    }
    fn newest(&self, mission: &str) -> Option<String> {
        Some(if mission == "root" { "tip".into() } else { mission.into() })
    }
    fn owner(&self, _: &str) -> Option<String> {
        Some("mate".into())
    }
    fn live(&self, mission: &str) -> bool {
        self.live.lock().unwrap().contains(mission)
    }
    fn waiting(&self, mission: &str) -> bool {
        self.waiting.lock().unwrap().contains(mission)
    }
    fn cancel(&self, _: &str) -> bool {
        false
    }
    fn conversations(&self) -> Vec<Turn> {
        Vec::new()
    }
    fn check_folder(&self, path: &str) -> Result<PathBuf, String> {
        Ok(PathBuf::from(path))
    }
    fn folder(&self) -> Option<String> {
        Some(self.open.into())
    }
    fn own_mode(&self) -> bool {
        false
    }
    fn see_all(&self) -> bool {
        true
    }
    fn models(&self) -> Vec<Value> {
        vec![json!({"id": "grok", "runtime": "cursor", "displayName": "Grok", "description": "", "supportedEfforts": ["low", "high"]})]
    }
    fn carry_on(&self, _: &str, _: &str, _: Option<&str>, _: &crate::carry_on::CarryOnAlso) -> Result<(String, Option<String>), String> {
        Err("not here".into())
    }
    // index.ts `followUpRoute`: the route the carried turn ran on, as the host holds it.
    fn follow_up_route(&self, mission: &str) -> Option<Route> {
        (self.holds && mission == "tip").then(|| Route { runtime: "cursor".into(), model: Some("grok".into()), effort: Some("high".into()), account: Some("work".into()) })
    }
}

/// The routes door::Engine holds are the whole process's: a test that reads or bounds them holds them alone.
static HELD: Mutex<()> = Mutex::new(());

fn holding() -> std::sync::MutexGuard<'static, ()> {
    HELD.lock().unwrap_or_else(|e| e.into_inner())
}

fn reply(result: &ToolResult) -> Value {
    assert!(!result.is_error, "{}", result.text);
    serde_json::from_str(&result.text).unwrap()
}

// case: continues on the carried runtime and model with its account and effort, and reports the actual folder
#[test]
fn a_follow_up_continues_on_the_carried_route_with_its_account_and_effort_and_reports_the_actual_folder() {
    let tools = Tools::new(fixture());
    let result = tools.call("send_message", &json!({"conversation_id": "root", "message": "Continue."}));
    let sent = reply(&result);
    let started = tools.locust.started.lock().unwrap();
    let request = &started[0];
    assert_eq!(
        (request.runtime.as_deref(), request.model.as_deref(), request.effort.as_deref(), request.account.clone()),
        (Some("cursor"), Some("grok"), Some("high"), Some(Some("work".to_owned()))),
        "the conversation's own route, not the teammate's saved codex, gpt-5.5, low and account_saved"
    );
    assert_eq!(request.teammate.id, "mate");
    assert_eq!(request.follow_up_of.as_deref(), Some("tip"));
    assert_eq!((&sent["runtime"], &sent["model"], &sent["folder"]), (&json!("cursor"), &json!("grok"), &json!("/work/conversation")));
}

// A host that holds no route for the turn keeps what its record says: its agent and model, and no effort; door::Engine
// reads the account from the record too.
#[test]
fn a_follow_up_of_a_turn_no_route_is_held_for_keeps_its_recorded_agent_model_and_account() {
    let mut fake = fixture();
    fake.holds = false;
    let tools = Tools::new(fake);
    reply(&tools.call("send_message", &json!({"conversation_id": "root", "message": "Continue."})));
    let started = tools.locust.started.lock().unwrap();
    assert_eq!(
        (started[0].runtime.as_deref(), started[0].model.as_deref(), started[0].effort.as_deref(), started[0].account.clone()),
        (Some("cursor"), Some("grok"), None, Some(None))
    );
    let account = format!("account_{}", "0123456789abcdef".repeat(2));
    let records = [json!({"recordType": "mission.created", "metadata": {"missionId": "m1", "runtime": "codex", "model": "account-default",
        "resolvedRouteId": format!("codex:account-default-account:{account}")}})];
    let recorded = recorded_route(Turn::from_records(&records).unwrap());
    assert_eq!(recorded, Some(Route { runtime: "codex".into(), model: None, effort: None, account: Some(account) }));
    let records = [json!({"recordType": "mission.created", "metadata": {"missionId": "m2", "runtime": "codex", "model": "gpt-5.5",
        "resolvedRouteId": "codex:gpt-5.5-account:default"}})];
    assert_eq!(recorded_route(Turn::from_records(&records).unwrap()).unwrap().account, None, "the machine's own sign-in");
    // A held route wins over the record, its effort with it.
    let _held = holding();
    remember_route("held-turn", Route { runtime: "claude".into(), model: Some("opus".into()), effort: Some("max".into()), account: None });
    let record = || Turn::from_records(&records);
    assert_eq!(route_for("held-turn", record).and_then(|r| r.effort), Some("max".into()), "held wins over the record");
    assert_eq!(route_for("held-turn", record).map(|r| r.runtime), Some("claude".into()));
    let unheld = route_for("never-held-turn", record).unwrap();
    assert_eq!((unheld.runtime.as_str(), unheld.model.as_deref(), unheld.effort), ("codex", Some("gpt-5.5"), None), "else its record");
}

// door::Engine sends a follow-up's account as an explicit pick and its effort as the turn's, and holds what it ran on.
#[test]
fn the_engine_sends_a_kept_account_as_the_turns_own_and_holds_the_route_it_ran_on() {
    let request = StartRequest {
        teammate: mate("mate", None),
        prompt: "Continue.".into(),
        follow_up_of: Some("tip".into()),
        mode: crate::approval::Mode::Ask,
        folder: None,
        runtime: Some("cursor".into()),
        model: Some("grok".into()),
        effort: Some("high".into()),
        account: Some(None),
    };
    let _held = holding();
    let (job, origin, route) = job_of(request.clone(), Some("low".into()));
    assert_eq!(origin.account, Some(None), "the machine's own sign-in, as the conversation ran, not account_saved");
    assert_eq!((job.effort.as_deref(), job.model.as_deref(), origin.runtime.as_deref()), (Some("high"), Some("grok"), Some("cursor")));
    assert_eq!(route, Route { runtime: "cursor".into(), model: Some("grok".into()), effort: Some("high".into()), account: None });
    let ran = begun("engine-held-turn".into(), "ask".into(), route.clone());
    assert_eq!((ran.runtime.as_deref(), ran.model.as_deref()), (Some("cursor"), Some("grok")));
    assert_eq!(held_route("engine-held-turn"), Some(route), "held for the turn's own follow-up");
    // A new conversation follows the teammate's saved account.
    let new = StartRequest { follow_up_of: None, account: None, runtime: None, model: None, effort: None, ..request };
    let (job, origin, route) = job_of(new.clone(), Some("low".into()));
    assert_eq!(origin.account, None);
    assert_eq!((route.runtime.as_str(), route.account.as_deref()), ("codex", Some("account_saved")));
    // It runs at the teammate's saved effort (send_from fills it in), so that is the effort held for its follow-up.
    assert_eq!((job.effort, route.effort.as_deref()), (None, Some("low")));
    // On another agent the saved effort is not the turn's.
    let (_, _, route) = job_of(StartRequest { runtime: Some("cursor".into()), ..new }, Some("low".into()));
    assert_eq!(route.effort, None);
}

// A route is held for the newest 500 turns, the oldest going first.
#[test]
fn routes_are_held_for_the_newest_five_hundred_turns() {
    let _held = holding();
    let id = |n: usize| format!("bounded-{}-{n}", std::process::id());
    for n in 0..501 {
        remember_route(&id(n), Route { runtime: "codex".into(), effort: Some(format!("e{n}")), ..Default::default() });
    }
    assert_eq!(held_route(&id(0)), None);
    // Whatever other tests hold meanwhile, never more than 500.
    assert_eq!(ROUTE_OF_TURN.lock().unwrap().len(), 500);
    assert_eq!(held_route(&id(500)).and_then(|r| r.effort), Some("e500".into()));
}

// case: lists a teammate’s own full folder and the open full folder for one without its own
#[test]
fn lists_a_teammates_own_full_folder_and_the_open_full_folder_for_one_without_its_own() {
    let mut fake = fixture();
    fake.roster = vec![(mate("own", Some("/work/teammate")), None), (mate("default", None), None)];
    fake.open = "/work/open";
    let rows = reply(&Tools::new(fake).call("list_teammates", &json!({})));
    let folders: Vec<&Value> = rows.as_array().unwrap().iter().map(|r| &r["folder"]).collect();
    assert_eq!(folders, [&json!("/work/teammate"), &json!("/work/open")]);
}

// case: keeps readable text beside machine status for working, approval, done, failed and stopped turns
#[test]
fn keeps_readable_text_beside_machine_status_for_working_approval_done_failed_and_stopped_turns() {
    let tools = Tools::new(fixture());
    for (end, live, waiting, status, carry) in [
        ("run.failed", true, false, "working", false),
        ("run.failed", true, true, "waiting_for_person", false),
        ("run.completed", false, false, "done", false),
        ("run.failed", false, false, "failed", true),
        ("run.cancelled", false, false, "stopped", false),
    ] {
        let ended = if end == "run.failed" { crash() } else { json!({"type": end, "payload": {}}) };
        tools.locust.turns.lock().unwrap().get_mut("tip").unwrap().events = vec![ended];
        for (set, on) in [(&tools.locust.live, live), (&tools.locust.waiting, waiting)] {
            let mut set = set.lock().unwrap();
            if on { set.insert("tip".into()) } else { set.remove("tip") };
        }
        let result = tools.call("read_reply", &json!({"conversation_id": "root"}));
        assert!(!result.text.is_empty(), "{status}");
        assert_eq!(
            result.structured,
            Some(json!({"status": status, "message": result.text, "runtime": "cursor", "model": "grok", "folder": "/work/conversation", "can_carry_on": carry})),
            "{status}"
        );
    }
}

// case: offers a process crash after tools ran through the same card and door reason, without replaying uncertain actions
#[test]
fn offers_a_process_crash_after_tools_ran_through_the_same_card_and_door_reason_without_replaying_uncertain_actions() {
    let ran = vec![json!({"type": "tool.started", "payload": {}}), crash()];
    assert_eq!(stopped_for(Some("failed"), &ran), Some(Reason::Process));
    let refused = json!({"type": "run.failed", "payload": {"kind": "process-failed", "message": "connection refused"}});
    assert_eq!(stopped_for(Some("failed"), &[refused]), Some(Reason::Provider));
    let mismatch = json!({"type": "run.failed", "payload": {"kind": "protocol-mismatch", "message": "Invalid runtime record"}});
    assert_eq!(stopped_for(Some("failed"), &[mismatch]), Some(Reason::Process));
    // The door offers it as the card does.
    let tools = Tools::new(fixture());
    tools.locust.turns.lock().unwrap().get_mut("tip").unwrap().events = ran.clone();
    let read = tools.call("read_reply", &json!({"conversation_id": "root"}));
    assert_eq!(read.structured.as_ref().unwrap()["can_carry_on"], true);
    // And the next agent is told to reconcile, never to replay.
    assert!(carry_on_prompt("opencode", Reason::Process).contains("never retry an irreversible action"));
    for phase in ["completed", "cancelled"] {
        assert_eq!(stopped_for(Some(phase), &ran), None, "{phase}");
    }
    for (kind, message) in [("safety-blocked", "refused"), ("process-failed", "The mode does not allow shell, so OpenCode stopped when it tried to use it.")] {
        let events = [
            json!({"type": "route.limit_detected", "payload": {"kind": "quota-exhausted"}}),
            json!({"type": "run.failed", "payload": {"kind": kind, "message": message}}),
        ];
        assert_eq!(stopped_for(Some("failed"), &events), None, "{kind}");
    }
}

// The engine itself asks for the held route first: the effort a turn ran at lives nowhere else.
#[test]
fn the_engine_answers_a_follow_up_with_the_route_it_holds() {
    let root = std::env::temp_dir().join(format!("locust-door-route-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let store = Store::with_codex(root.join("native"), root.join("electron"), vec![]).unwrap();
    let engine = Engine { store, settings: Arc::new(Mutex::new(Settings::default())) };
    let _held = holding();
    let route = Route { runtime: "claude".into(), model: Some("opus".into()), effort: Some("high".into()), account: Some("work".into()) };
    remember_route("engine-asked-turn", route.clone());
    assert_eq!(engine.follow_up_route("engine-asked-turn"), Some(route));
    assert_eq!(engine.follow_up_route("engine-unknown-turn"), None, "neither held nor recorded");
    let _ = std::fs::remove_dir_all(&root);
}
