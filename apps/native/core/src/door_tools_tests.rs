//! The door's tools against a made-up Locust: the rules locust-mcp-tools.ts holds, in its words.
use super::*;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct Fake {
    own_mode: bool,
    see_all: bool,
    turns: Mutex<HashMap<String, Turn>>,
    newest: Mutex<HashMap<String, String>>,
    live: Mutex<HashSet<String>>,
    waiting: Mutex<HashSet<String>>,
    started: Mutex<Vec<StartRequest>>,
    /// The mode the host really runs a turn in, when it differs from the one asked for.
    host_mode: Option<&'static str>,
    /// The conversation's teammate was removed, or it never had one.
    no_owner: bool,
    /// How many times a waiting read paused, and after how many pauses the turn finishes (None: at the first look).
    pauses: Mutex<u64>,
    finish_after: Option<u64>,
    /// Locust's own Stop did not take.
    cancel_fails: bool,
    /// A roster and a model list of the test's own, in place of the usual ones.
    roster: Option<Vec<(Teammate, Option<String>)>>,
    model_list: Option<Vec<Value>>,
    /// Whose conversations are, when not Scout's.
    owner_id: Option<&'static str>,
    /// Each carry-on Locust was asked for: the conversation, the agent and the model.
    carried: Mutex<Vec<(String, String, Option<String>)>>,
    /// The folder Locust's carry-on stood the carried turn in, recorded on it as a carried turn records it.
    carried_folder: Option<&'static str>,
}

fn mate(id: &str, name: &str, mode: &str) -> Teammate {
    Teammate {
        id: id.into(),
        name: name.into(),
        role: "Builder".into(),
        runtime: Some("codex".into()),
        model: Some("gpt-5.5".into()),
        account: None,
        busy: false,
        runs_in: mode.into(),
        folder: None,
    }
}

fn turn(id: &str, by_app: bool, events: Vec<Value>) -> Turn {
    Turn {
        mission_id: id.into(),
        prompt: "Look at it".into(),
        runtime: Some("codex".into()),
        model: Some("gpt-5.5".into()),
        mode: Some("ask".into()),
        started_by: if by_app { json!({"kind": "mcp"}) } else { Value::Null },
        events,
        ..Default::default()
    }
}

fn ev(kind: &str, payload: Value) -> Value {
    json!({"type": kind, "payload": payload})
}

impl Locust for Fake {
    fn teammates(&self) -> Result<Vec<(Teammate, Option<String>)>, String> {
        if let Some(roster) = &self.roster {
            return Ok(roster.clone());
        }
        Ok(vec![(mate("tm_scout", "Scout", "auto"), Some("high".into())), (mate("tm_pip", "Pip", "accept-edits"), None)])
    }
    fn busy(&self, _: &str) -> bool {
        false
    }
    fn start(&self, request: StartRequest) -> Result<Begun, String> {
        let id = format!("m{}", self.started.lock().unwrap().len() + 1);
        let mode = self.host_mode.unwrap_or(request.mode.as_str()).to_owned();
        let mut t = turn(&id, true, vec![]);
        if let Some(f) = &request.folder {
            t.started_by = json!({"kind": "mcp", "folder": f.to_string_lossy()});
            t.cwd = Some(f.to_string_lossy().into_owned());
        }
        self.turns.lock().unwrap().insert(id.clone(), t);
        if let Some(prev) = &request.follow_up_of {
            self.newest.lock().unwrap().insert(prev.clone(), id.clone());
        }
        self.live.lock().unwrap().insert(id.clone());
        self.started.lock().unwrap().push(request);
        Ok(Begun { mission_id: id, mode, runtime: Some("codex".into()), model: Some(self.started.lock().unwrap().last().and_then(|r| r.model.clone()).unwrap_or_else(|| "gpt-5.5".into())) })
    }
    fn read(&self, mission: &str) -> Option<Turn> {
        self.turns.lock().unwrap().get(mission).cloned()
    }
    fn newest(&self, mission: &str) -> Option<String> {
        let mut at = mission.to_owned();
        while let Some(next) = self.newest.lock().unwrap().get(&at).cloned() {
            at = next;
        }
        Some(at)
    }
    fn owner(&self, _: &str) -> Option<String> {
        (!self.no_owner).then(|| self.owner_id.unwrap_or("tm_scout").into())
    }
    fn live(&self, mission: &str) -> bool {
        self.live.lock().unwrap().contains(mission)
    }
    fn waiting(&self, mission: &str) -> bool {
        self.waiting.lock().unwrap().contains(mission)
    }
    fn cancel(&self, mission: &str) -> bool {
        !self.cancel_fails && self.live.lock().unwrap().remove(mission)
    }
    fn conversations(&self) -> Vec<Turn> {
        self.turns.lock().unwrap().values().cloned().collect()
    }
    fn check_folder(&self, path: &str) -> Result<PathBuf, String> {
        if path.starts_with("/known") { Ok(PathBuf::from(path)) } else { Err(format!("{path} is not a folder Locust knows.")) }
    }
    fn folder(&self) -> Option<String> {
        Some("/known/open".into())
    }
    fn own_mode(&self) -> bool {
        self.own_mode
    }
    fn see_all(&self) -> bool {
        self.see_all
    }
    fn models(&self) -> Vec<Value> {
        if let Some(list) = &self.model_list {
            return list.clone();
        }
        vec![
            json!({"id": "account-default", "runtime": "codex", "displayName": "Account default", "description": "", "supportedEfforts": ["low", "high"]}),
            json!({"id": "gpt-5.5", "runtime": "codex", "displayName": "GPT-5.5", "description": "", "supportedEfforts": ["low", "high"]}),
            json!({"id": "gpt-4", "runtime": "codex", "displayName": "GPT-4", "description": "", "supportedEfforts": [], "older": true}),
            json!({"id": "opencode/free", "runtime": "opencode", "displayName": "free", "description": "", "supportedEfforts": []}),
            json!({"id": "own_local", "runtime": "claude", "displayName": "My local model", "description": "", "supportedEfforts": [], "own": true}),
        ]
    }
    fn carry_on(&self, mission: &str, runtime: &str, model: Option<&str>) -> Result<(String, Option<String>), String> {
        // The agent whose plan ran out cannot carry its own turn on (Locust's own carry-on refuses it).
        let ran = self.turns.lock().unwrap().get(mission).and_then(|t| t.runtime.clone());
        if ran.as_deref() == Some(runtime) {
            return Err(format!("{} is the agent whose plan ran out.", if runtime == "codex" { "Codex CLI" } else { runtime }));
        }
        self.carried.lock().unwrap().push((mission.to_owned(), runtime.to_owned(), model.map(str::to_owned)));
        let id = format!("{mission}-carried");
        let mut carried = turn(&id, false, vec![]);
        if let Some(f) = self.carried_folder {
            carried.started_by = json!({"kind": "carry-on", "from": ran.unwrap_or_default(), "folder": f});
        }
        self.turns.lock().unwrap().insert(id.clone(), carried);
        Ok((id, self.carried_folder.map(str::to_owned)))
    }
    fn pause(&self, _: u64) {
        // A waiting read finds the turn finished at its first look, unless told how many pauses it takes.
        let mut pauses = self.pauses.lock().unwrap();
        *pauses += 1;
        if self.finish_after.is_none_or(|n| *pauses >= n) {
            self.live.lock().unwrap().clear();
        }
    }
}

fn reply(r: &ToolResult) -> Value {
    serde_json::from_str(&r.text).unwrap_or(Value::Null)
}

#[test]
fn a_turn_from_an_app_runs_in_ask_unless_the_person_allows_each_teammates_mode() {
    let tools = Tools::new(Fake::default());
    let listed = reply(&tools.call("list_teammates", &json!({})));
    assert_eq!(listed[0]["runs_in"], "ask");
    assert_eq!(listed[0]["effort"], "high");
    assert_eq!(listed[0]["folder"], "/known/open");
    let started = tools.call("start_conversation", &json!({"teammate": "scout", "message": "hi"}));
    assert!(!started.is_error, "{}", started.text);
    assert_eq!(reply(&started)["mode"], "ask");
    assert_eq!(reply(&started)["folder"], "/known/open");

    let own = Tools::new(Fake { own_mode: true, ..Default::default() });
    assert_eq!(reply(&own.call("list_teammates", &json!({})))[0]["runs_in"], "auto");
    assert_eq!(reply(&own.call("start_conversation", &json!({"teammate": "Pip", "message": "hi"})))["mode"], "accept-edits");
    // An app may always step a turn down to Ask.
    let asked = own.call("start_conversation", &json!({"teammate": "Pip", "message": "hi", "read_only": true}));
    assert_eq!(reply(&asked)["mode"], "ask");
}

#[test]
fn naming_a_mode_or_folder_is_refused_in_words_while_the_switch_is_off() {
    let tools = Tools::new(Fake::default());
    let r = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "hi", "mode": "auto"}));
    assert!(r.is_error);
    assert!(r.text.starts_with("Choosing the mode or folder is off"), "{}", r.text);
    assert!(r.text.ends_with("Nothing was started."));
    assert!(tools.locust.started.lock().unwrap().is_empty());
}

#[test]
fn a_named_folder_is_checked_and_the_conversation_stays_there() {
    let tools = Tools::new(Fake { own_mode: true, ..Default::default() });
    let refused = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "hi", "folder": "/elsewhere"}));
    assert!(refused.is_error);
    assert_eq!(refused.text, "/elsewhere is not a folder Locust knows. Nothing was started.");
    let bad_mode = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "hi", "mode": "yolo"}));
    assert_eq!(bad_mode.text, "mode is one of: ask, plan, accept-edits, approve-each, auto. Nothing was started.");
    let started = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "hi", "folder": "/known/wt", "mode": "auto"}));
    assert_eq!(reply(&started)["folder"], "/known/wt");
    assert_eq!(reply(&started)["mode"], "auto");
    let id = reply(&started)["conversation_id"].as_str().unwrap().to_owned();
    tools.locust.live.lock().unwrap().clear();
    // The follow-up names nothing: it keeps the conversation's folder.
    let next = tools.call("send_message", &json!({"conversation_id": id, "message": "and then?"}));
    assert!(!next.is_error, "{}", next.text);
    assert_eq!(reply(&next)["conversation_id"], id.as_str());
    let second = tools.locust.started.lock().unwrap()[1].clone();
    assert_eq!(second.folder, Some(PathBuf::from("/known/wt")));
    assert_eq!(second.follow_up_of.as_deref(), Some(id.as_str()));
}

#[test]
fn the_reply_says_what_ran_when_the_host_held_a_turn_in_another_mode() {
    let tools = Tools::new(Fake { own_mode: true, host_mode: Some("ask"), ..Default::default() });
    let r = reply(&tools.call("start_conversation", &json!({"teammate": "Scout", "message": "review", "mode": "auto"})));
    assert_eq!(r["mode"], "ask");
    assert_eq!(r["mode_asked"], "auto");
}

#[test]
fn a_model_and_effort_for_one_turn_are_checked_against_what_the_agent_offers_here() {
    let tools = Tools::new(Fake::default());
    let r = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "hi", "model": "gpt-5.5", "effort": "high"}));
    assert!(!r.is_error, "{}", r.text);
    assert_eq!((reply(&r)["model"].as_str(), reply(&r)["effort"].as_str()), (Some("gpt-5.5"), Some("high")));
    let sent = tools.locust.started.lock().unwrap()[0].clone();
    assert_eq!((sent.model.as_deref(), sent.effort.as_deref()), (Some("gpt-5.5"), Some("high")));
    // A model alone keeps the teammate's saved effort when the model takes it (Scout saves high).
    assert_eq!(reply(&tools.call("start_conversation", &json!({"teammate": "Scout", "message": "hi", "model": "GPT-5.5"})))["effort"], "high");
    let unknown = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "hi", "model": "gpt-9"}));
    assert_eq!(unknown.text, "That model is not one codex offers in this Locust. Ask list_models for this teammate’s choices. Nothing was started.");
    let level = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "hi", "model": "gpt-5.5", "effort": "max"}));
    assert_eq!(level.text, "GPT-5.5 takes these efforts here: low, high. Nothing was started.");
    // Effort alone rides on the model the teammate runs on.
    let alone = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "hi", "effort": "low"}));
    assert_eq!((reply(&alone)["model"].as_str(), reply(&alone)["effort"].as_str()), (Some("gpt-5.5"), Some("low")));
}

#[test]
fn list_models_gives_a_teammates_choices_or_every_agent_without_older_versions() {
    let tools = Tools::new(Fake::default());
    let mine = reply(&tools.call("list_models", &json!({"teammate": "Scout"})));
    assert_eq!(mine["runtime"], "codex");
    assert_eq!(mine["effort"], "high");
    assert_eq!(mine["models"].as_array().unwrap().len(), 2, "older versions are left out");
    let every = reply(&tools.call("list_models", &json!({})));
    assert_eq!(every[1], json!({"runtime": "opencode", "models": [{"id": "opencode/free", "name": "free"}]}));
}

#[test]
fn read_reply_says_working_waiting_or_the_answer_with_warnings_after_it() {
    let fake = Fake::default();
    fake.turns.lock().unwrap().insert(
        "m9".into(),
        turn("m9", true, vec![ev("tool.started", json!({"name": "shell", "title": "Reading app.ts"}))]),
    );
    fake.live.lock().unwrap().insert("m9".into());
    let tools = Tools::new(fake);
    let working = tools.call("read_reply", &json!({"conversation_id": "m9"}));
    assert_eq!(working.text, "Still working: Reading app.ts");
    assert_eq!(working.structured.as_ref().unwrap()["status"], "working");
    tools.locust.waiting.lock().unwrap().insert("m9".into());
    let waiting = tools.call("read_reply", &json!({"conversation_id": "m9"}));
    assert!(waiting.text.starts_with("Waiting for the person to approve or deny"));
    assert_eq!(waiting.structured.as_ref().unwrap()["status"], "waiting_for_person");

    tools.locust.turns.lock().unwrap().insert(
        "m9".into(),
        turn(
            "m9",
            true,
            vec![
                ev("message.delta", json!({"itemId": "a", "operation": "append", "text": "I will read ", "final": false})),
                ev("message.delta", json!({"itemId": "a", "operation": "append", "text": "the files. <locust-memory>x</locust-memory>", "final": true})),
                ev("adapter.diagnostic", json!({"level": "warning", "message": "Locust refused a read outside the folder."})),
                ev("run.completed", json!({})),
            ],
        ),
    );
    tools.locust.waiting.lock().unwrap().clear();
    // Asked to wait: the turn ends during the wait, and the answer comes back.
    let done = tools.call("read_reply", &json!({"conversation_id": "m9", "wait_seconds": 5}));
    assert_eq!(done.text, "I will read the files. x\n\nLocust: Locust refused a read outside the folder.");
    assert_eq!(done.structured.as_ref().unwrap()["status"], "done");
}

#[test]
fn a_failed_turn_says_its_failure_and_an_unfinished_one_says_so() {
    let fake = Fake::default();
    fake.turns.lock().unwrap().insert("f".into(), turn("f", true, vec![ev("run.failed", json!({"message": "Codex is not signed in."}))]));
    fake.turns.lock().unwrap().insert("s".into(), turn("s", true, vec![ev("run.cancelled", json!({}))]));
    let tools = Tools::new(fake);
    let failed = tools.call("read_reply", &json!({"conversation_id": "f"}));
    assert!(failed.is_error);
    assert_eq!(failed.text, "Codex is not signed in.");
    assert_eq!(failed.structured.as_ref().unwrap()["status"], "failed");
    let stopped = tools.call("read_reply", &json!({"conversation_id": "s"}));
    assert_eq!(stopped.text, "The run was interrupted or stopped before it finished.");
    assert_eq!(stopped.structured.as_ref().unwrap()["status"], "stopped");
}

#[test]
fn the_persons_own_conversations_stay_theirs_unless_they_let_apps_see_them() {
    let fake = Fake::default();
    fake.turns.lock().unwrap().insert("mine".into(), turn("mine", false, vec![]));
    fake.live.lock().unwrap().insert("mine".into());
    let tools = Tools::new(fake);
    let unread = tools.call("read_reply", &json!({"conversation_id": "mine"}));
    assert!(unread.text.starts_with("That conversation could not be read"));
    let stop = tools.call("stop_turn", &json!({"conversation_id": "mine"}));
    assert!(stop.is_error);
    assert!(tools.locust.live("mine"));
    assert!(tools.call("list_conversations", &json!({})).text.starts_with("Off: this app sees only"));

    let open = Tools::new(Fake { see_all: true, ..Default::default() });
    open.locust.turns.lock().unwrap().insert("mine".into(), turn("mine", false, vec![]));
    open.locust.live.lock().unwrap().insert("mine".into());
    let rows = reply(&open.call("list_conversations", &json!({})));
    assert_eq!(rows[0]["started_by"], "the person");
    assert_eq!(rows[0]["teammate"], "Scout");
    assert_eq!(rows[0]["state"], "working");
    assert_eq!(open.call("stop_turn", &json!({"conversation_id": "mine"})).text, "Stopping it. What it already changed stays; the person can undo that turn in Locust.");
    assert_eq!(open.call("stop_turn", &json!({"conversation_id": "mine"})).text, "Nothing is running in that conversation now. Nothing was stopped.");
}

#[test]
fn a_message_is_not_queued_while_a_turn_works() {
    let fake = Fake::default();
    fake.turns.lock().unwrap().insert("m1".into(), turn("m1", true, vec![]));
    fake.live.lock().unwrap().insert("m1".into());
    let tools = Tools::new(fake);
    let r = tools.call("send_message", &json!({"conversation_id": "m1", "message": "more"}));
    assert_eq!(r.text, "Still working: waiting for the AI agent to answer.");
    assert!(tools.locust.started.lock().unwrap().is_empty());
}

#[test]
fn unknown_tools_bad_arguments_and_unoffered_features_are_said_plainly() {
    let tools = Tools::new(Fake::default());
    assert_eq!(tools.call("rm_rf", &json!({})).text, "That tool is not offered by Locust.");
    assert_eq!(tools.call("carry_on", &json!({})).text, "Give a conversation id from start_conversation or list_conversations.");
    assert!(tools.call("start_conversation", &json!({"teammate": "Scout"})).text.starts_with("Choose a teammate and give it a message"));
    assert!(tools.call("start_conversation", &json!({"teammate": "Nobody", "message": "x"})).text.starts_with("That teammate could not be identified"));
    assert_eq!(tools.call("run_routine", &json!({"routine": "r"})).text, "This Locust does not offer routines to other apps. Nothing was started.");
    assert_eq!(tools.call("list_background_runs", &json!({})).text, "[]");
    assert!(tools.call("read_reply", &json!("not an object")).is_error);
}

#[test]
fn locusts_own_tags_come_off_a_reply_but_not_out_of_code() {
    assert_eq!(unwrap_protocol_tags("Done.\n<locust-share to=\"Pip\">hello</locust-share>"), "Done.\nhello");
    assert_eq!(unwrap_protocol_tags("Use `<locust-share>` like this."), "Use `<locust-share>` like this.");
    assert_eq!(unwrap_protocol_tags("Cut off here\n<locu"), "Cut off here");
    assert_eq!(unwrap_protocol_tags("<locust-key name=\"WEATHER\">the key</locust-key>"), "Asks for the key WEATHER (below).");
}

#[test]
fn a_turn_is_read_from_its_ledger_records() {
    let records = vec![
        json!({"recordType": "mission.created", "occurredAt": "t0", "metadata": {"missionId": "m1", "prompt": "p", "runtime": "codex", "model": "account-default", "mode": "auto", "sandbox": "full-access", "startedBy": {"kind": "mcp", "folder": "/w"}, "command": {"cwd": "/w"}}}),
        json!({"recordType": "mission.event", "occurredAt": "t2", "event": {"sequence": 2, "type": "run.completed", "payload": {}}}),
        json!({"recordType": "mission.event", "occurredAt": "t1", "event": {"sequence": 1, "type": "message.delta", "payload": {"itemId": "a", "operation": "replace", "text": "hi", "final": true}}}),
    ];
    let t = Turn::from_records(&records).unwrap();
    assert_eq!(t.model, None);
    assert_eq!(t.mode.as_deref(), Some("auto"));
    assert_eq!(t.cwd.as_deref(), Some("/w"));
    assert_eq!(t.phase(), "completed");
    assert!(t.from_app());
    assert_eq!(conversation_folder(&t.started_by).as_deref(), Some("/w"));
    assert_eq!(answer_of(&t).text, "hi");
}

#[test]
fn a_turn_whose_plan_ran_out_is_offered_and_carried_on_to_an_agent_this_locust_can_run() {
    let fake = Fake::default();
    let ran_out = vec![
        ev("tool.started", json!({"itemId": "t1", "toolKind": "shell", "name": "shell"})),
        ev("route.limit_detected", json!({"kind": "quota-exhausted", "message": "Usage limit reached."})),
        ev("run.failed", json!({"kind": "quota-exhausted", "message": "Usage limit reached."})),
    ];
    fake.turns.lock().unwrap().insert("out".into(), turn("out", true, ran_out));
    let tools = Tools::new(fake);
    let read = tools.call("read_reply", &json!({"conversation_id": "out"}));
    assert_eq!(read.structured.as_ref().unwrap()["can_carry_on"], true);
    assert_eq!(reply(&tools.call("list_models", &json!({})))[1]["runtime"], "opencode");
    let unknown = tools.call("carry_on", &json!({"conversation_id": "out", "runtime": "gemini"}));
    assert_eq!(unknown.text, "Name an AI agent this Locust can run, as list_models with no teammate lists them: codex, opencode. Nothing was started.");
    let same = tools.call("carry_on", &json!({"conversation_id": "out", "runtime": "codex"}));
    assert_eq!(same.text, "Codex CLI is the agent whose plan ran out. Nothing was started.");
    let wrong = tools.call("carry_on", &json!({"conversation_id": "out", "runtime": "opencode", "model": "nope"}));
    assert_eq!(wrong.text, "opencode does not offer that model here. Use one from list_models. Nothing was started.");
    let carried = tools.call("carry_on", &json!({"conversation_id": "out", "runtime": "opencode", "model": "opencode/free"}));
    assert!(!carried.is_error, "{}", carried.text);
    let r = reply(&carried);
    assert_eq!((r["conversation_id"].as_str(), r["runtime"].as_str(), r["mode"].as_str()), (Some("out-carried"), Some("opencode"), Some("ask")));

    // A turn in a mode above Ask is carried on from here only with the person's switch on.
    let mut edits = turn("edits", true, vec![ev("run.failed", json!({"kind": "quota-exhausted"}))]);
    edits.mode = Some("accept-edits".into());
    tools.locust.turns.lock().unwrap().insert("edits".into(), edits);
    assert!(tools.call("carry_on", &json!({"conversation_id": "edits", "runtime": "opencode"})).text.starts_with("That turn ran in accept-edits, not Ask."));
}

// A question a turn ends on (an-app-continues-a-conversation-with-no-teammate.test.ts, 0.878), Electron's values.
const QUESTION: &str = "Batch 8 is done.\n\n<locust-ask>\nWhen Rust's command guard refuses a command, what should the record keep?\n- Keep it as Electron :: guard refusals leave no line, and every file stays readable by Electron\n- Allow \"command-guard\" in Rust :: the record shows guard refusals again\n</locust-ask>";

fn asking(fake: Fake, said: &str, phase: &str) -> Tools<Fake> {
    let mut events = vec![ev("message.delta", json!({"itemId": "m", "operation": "replace", "text": said, "final": true}))];
    if phase != "interrupted" {
        events.push(ev(&format!("run.{phase}"), json!({"message": "It broke."})));
    }
    let mut t = turn("tip", true, events);
    t.mode = Some("auto".into());
    fake.turns.lock().unwrap().insert("tip".into(), t);
    Tools::new(fake)
}

#[test]
fn a_question_a_turn_ends_on_reads_back_with_its_numbered_options() {
    let tools = asking(Fake::default(), QUESTION, "completed");
    let read = tools.call("read_reply", &json!({"conversation_id": "tip"}));
    let s = read.structured.unwrap();
    assert_eq!(s["status"], "done");
    assert_eq!(
        s["question"],
        json!({"text": "When Rust's command guard refuses a command, what should the record keep?", "options": [
            {"number": 1, "label": "Keep it as Electron", "note": "guard refusals leave no line, and every file stays readable by Electron"},
            {"number": 2, "label": "Allow \"command-guard\" in Rust", "note": "the record shows guard refusals again"}
        ]})
    );
}

#[test]
fn a_question_is_answered_by_number_with_exactly_what_the_cards_press_sends() {
    let tools = asking(Fake::default(), QUESTION, "completed");
    let r = tools.call("answer_question", &json!({"conversation_id": "tip", "choice": 1}));
    assert!(!r.is_error, "{}", r.text);
    let started = tools.locust.started.lock().unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].prompt, "Keep it as Electron (guard refusals leave no line, and every file stays readable by Electron). Continue with that.");
    assert_eq!(started[0].follow_up_of.as_deref(), Some("tip"));
    assert_eq!(started[0].mode, Mode::Ask);
    assert_eq!(started[0].teammate.id, "tm_scout");
    assert_eq!(reply(&r)["conversation_id"], "tip");
}

#[test]
fn a_question_is_answered_by_label_whatever_its_case_in_the_conversations_own_mode_when_allowed() {
    let tools = asking(Fake { own_mode: true, ..Default::default() }, QUESTION, "completed");
    tools.call("answer_question", &json!({"conversation_id": "tip", "choice": "allow \"command-guard\" in rust"}));
    let started = tools.locust.started.lock().unwrap();
    assert_eq!(started[0].prompt, "Allow \"command-guard\" in Rust (the record shows guard refusals again). Continue with that.");
    assert_eq!(started[0].mode, Mode::Auto);
}

#[test]
fn a_choice_it_does_not_offer_is_refused_naming_the_ones_it_does_and_nothing_is_sent() {
    let tools = asking(Fake::default(), QUESTION, "completed");
    for choice in [json!(3), json!(0), json!(-1), json!(1.5), json!("Maybe"), json!(null), json!(true)] {
        let r = tools.call("answer_question", &json!({"conversation_id": "tip", "choice": choice}));
        assert!(r.is_error);
        assert_eq!(r.text, "Choose one of: 1 Keep it as Electron; 2 Allow \"command-guard\" in Rust. Nothing was sent.", "{choice}");
    }
    assert!(tools.locust.started.lock().unwrap().is_empty());
}

#[test]
fn no_question_is_there_when_the_answer_asks_nothing_or_while_the_turn_still_works() {
    let plain = asking(Fake::default(), "All done.", "completed");
    assert!(plain.call("read_reply", &json!({"conversation_id": "tip"})).structured.unwrap().get("question").is_none());
    let asked = plain.call("answer_question", &json!({"conversation_id": "tip", "choice": 1}));
    assert!(asked.is_error);
    assert_eq!(asked.text, "Its newest answer asks no question with choices. Use send_message to reply in words. Nothing was sent.");
    let working = asking(Fake::default(), QUESTION, "completed");
    working.locust.live.lock().unwrap().insert("tip".into());
    let early = working.call("answer_question", &json!({"conversation_id": "tip", "choice": 1}));
    assert!(early.is_error);
    assert_eq!(early.text, "It is still working, so there is no question to answer yet. read_reply first. Nothing was sent.");
    assert!(working.call("read_reply", &json!({"conversation_id": "tip"})).structured.unwrap().get("question").is_none());
    assert!(working.locust.started.lock().unwrap().is_empty());
}

#[test]
fn no_question_is_offered_from_a_turn_that_failed_or_was_stopped_even_if_its_last_words_asked_one() {
    for phase in ["failed", "cancelled", "interrupted"] {
        let tools = asking(Fake::default(), QUESTION, phase);
        assert!(tools.call("read_reply", &json!({"conversation_id": "tip"})).structured.unwrap().get("question").is_none(), "{phase}");
        assert!(tools.call("answer_question", &json!({"conversation_id": "tip", "choice": 1})).is_error, "{phase}");
        assert!(tools.locust.started.lock().unwrap().is_empty(), "{phase}");
    }
}

#[test]
fn a_question_in_the_persons_own_conversation_is_out_of_reach_while_they_do_not_let_apps_see_it() {
    let fake = Fake::default();
    let mut own = turn("tip", false, vec![ev("message.delta", json!({"itemId": "m", "text": QUESTION, "final": true})), ev("run.completed", json!({}))]);
    own.mode = Some("ask".into());
    fake.turns.lock().unwrap().insert("tip".into(), own.clone());
    let tools = Tools::new(fake);
    let r = tools.call("answer_question", &json!({"conversation_id": "tip", "choice": 1}));
    assert!(r.is_error);
    assert_eq!(r.text, "That conversation could not be read from the local ledger. Use an id returned by this server.");
    assert!(tools.locust.started.lock().unwrap().is_empty());
    let seen = Tools::new(Fake { see_all: true, ..Default::default() });
    seen.locust.turns.lock().unwrap().insert("tip".into(), own);
    assert!(!seen.call("answer_question", &json!({"conversation_id": "tip", "choice": 2})).is_error);
}

// A conversation with no teammate (an-app-continues-a-conversation-with-no-teammate.test.ts, 0.878), Electron's values:
// the person's own, on claude/opus, recorded in Auto.
fn ownerless(fake: Fake, mode: Option<&str>) -> Tools<Fake> {
    let mut t = turn("tip", false, vec![ev("message.delta", json!({"itemId": "m", "operation": "replace", "text": QUESTION, "final": true})), ev("run.completed", json!({}))]);
    t.runtime = Some("claude".into());
    t.model = Some("opus".into());
    t.mode = mode.map(str::to_owned);
    fake.turns.lock().unwrap().insert("tip".into(), t);
    Tools::new(fake)
}

#[test]
fn a_conversation_with_no_teammate_is_continued_on_its_own_agent_and_model_belonging_to_nobody_in_ask() {
    let tools = ownerless(Fake { no_owner: true, see_all: true, ..Default::default() }, Some("auto"));
    let r = tools.call("send_message", &json!({"conversation_id": "tip", "message": "Carry on."}));
    assert!(!r.is_error, "{}", r.text);
    let started = tools.locust.started.lock().unwrap();
    assert_eq!(started.len(), 1);
    let request = &started[0];
    assert_eq!(request.prompt, "Carry on.");
    assert_eq!(request.follow_up_of.as_deref(), Some("tip"));
    assert_eq!((request.runtime.as_deref(), request.model.as_deref()), (Some("claude"), Some("opus")));
    assert_eq!(request.mode, Mode::Ask);
    assert!(crate::native::is_nobody(&request.teammate), "no teammate is named: {:?}", request.teammate);
    assert_eq!((request.teammate.runtime.as_deref(), request.teammate.model.as_deref()), (Some("claude"), Some("opus")));
}

#[test]
fn a_conversation_with_no_teammate_keeps_its_own_mode_when_the_person_lets_apps_use_each_teammates_own() {
    let tools = ownerless(Fake { no_owner: true, see_all: true, own_mode: true, ..Default::default() }, Some("auto"));
    tools.call("send_message", &json!({"conversation_id": "tip", "message": "Carry on."}));
    assert_eq!(tools.locust.started.lock().unwrap()[0].mode, Mode::Auto);
}

#[test]
fn a_conversation_with_no_teammate_runs_in_ask_when_it_never_recorded_a_mode_the_switch_on_or_not() {
    for own_mode in [true, false] {
        let tools = ownerless(Fake { no_owner: true, see_all: true, own_mode, ..Default::default() }, None);
        tools.call("send_message", &json!({"conversation_id": "tip", "message": "Carry on."}));
        assert_eq!(tools.locust.started.lock().unwrap()[0].mode, Mode::Ask, "own mode {own_mode}");
    }
}

#[test]
fn a_conversation_with_no_teammate_stays_out_of_reach_while_the_person_does_not_let_apps_see_their_conversations() {
    let tools = ownerless(Fake { no_owner: true, ..Default::default() }, Some("auto"));
    for (tool, args) in [("send_message", json!({"conversation_id": "tip", "message": "Carry on."})), ("answer_question", json!({"conversation_id": "tip", "choice": 1}))] {
        assert!(tools.call(tool, &args).is_error, "{tool}");
    }
    assert!(tools.locust.started.lock().unwrap().is_empty());
}

#[test]
fn a_conversation_that_has_a_teammate_is_still_continued_as_that_teammate() {
    let tools = ownerless(Fake { see_all: true, ..Default::default() }, Some("auto"));
    tools.call("send_message", &json!({"conversation_id": "tip", "message": "Carry on."}));
    assert_eq!(tools.locust.started.lock().unwrap()[0].teammate.id, "tm_scout");
}

#[test]
fn a_question_in_a_conversation_with_no_teammate_is_answered_by_number_belonging_to_nobody() {
    let tools = ownerless(Fake { no_owner: true, see_all: true, ..Default::default() }, Some("auto"));
    let r = tools.call("answer_question", &json!({"conversation_id": "tip", "choice": 1}));
    assert!(!r.is_error, "{}", r.text);
    let started = tools.locust.started.lock().unwrap();
    assert_eq!(started[0].prompt, "Keep it as Electron (guard refusals leave no line, and every file stays readable by Electron). Continue with that.");
    assert_eq!(started[0].mode, Mode::Ask);
    assert!(crate::native::is_nobody(&started[0].teammate));
}

#[test]
fn an_answer_already_starting_is_not_sent_twice() {
    let tools = asking(Fake::default(), QUESTION, "completed");
    tools.starting.lock().unwrap().insert("tip".into());
    let r = tools.call("answer_question", &json!({"conversation_id": "tip", "choice": 1}));
    assert!(r.is_error);
    assert_eq!(r.text, "Still working: starting the next turn. Nothing was sent.");
    assert!(tools.locust.started.lock().unwrap().is_empty());
    tools.starting.lock().unwrap().clear();
    assert!(!tools.call("answer_question", &json!({"conversation_id": "tip", "choice": 1})).is_error);
    assert!(tools.starting.lock().unwrap().is_empty(), "the claim is let go after the answer");
}

// Electron 0.879, `an-app-starts-a-conversation-with-no-teammate.test.ts`: an app names an agent instead of a teammate.
#[test]
fn an_app_starts_a_conversation_with_no_teammate_on_the_agent_it_names_in_ask() {
    let tools = Tools::new(Fake::default());
    let r = tools.call("start_conversation", &json!({"message": "Look at the login page.", "runtime": "opencode"}));
    assert!(!r.is_error, "{}", r.text);
    let started = tools.locust.started.lock().unwrap();
    let request = &started[0];
    assert!(crate::native::is_nobody(&request.teammate), "no teammate: {:?}", request.teammate.id);
    assert_eq!(request.prompt, "Look at the login page.");
    assert_eq!((request.runtime.as_deref(), request.model.as_deref(), request.effort.as_deref()), (Some("opencode"), None, None));
    assert_eq!(request.mode, Mode::Ask);
    assert_eq!(reply(&r)["mode"], "ask");
}

#[test]
fn a_conversation_with_no_teammate_takes_a_model_and_effort_its_agent_offers() {
    let tools = Tools::new(Fake::default());
    let r = tools.call("start_conversation", &json!({"message": "Think hard.", "runtime": "codex", "model": "GPT-5.5", "effort": "HIGH"}));
    assert!(!r.is_error, "{}", r.text);
    let request = tools.locust.started.lock().unwrap()[0].clone();
    assert_eq!((request.runtime.as_deref(), request.model.as_deref(), request.effort.as_deref()), (Some("codex"), Some("gpt-5.5"), Some("high")));
    assert_eq!((reply(&r)["model"].as_str(), reply(&r)["effort"].as_str()), (Some("gpt-5.5"), Some("high")));
}

#[test]
fn a_conversation_with_no_teammate_refuses_in_words_an_agent_model_or_effort_not_offered() {
    let tools = Tools::new(Fake::default());
    let agents = "Name a teammate, or an AI agent this Locust can run, as list_models with no teammate lists them: codex, opencode. Nothing was started.";
    // An agent only the person's own models run on is not one an app may name.
    for args in [json!({"message": "Go."}), json!({"message": "Go.", "runtime": "cursor"}), json!({"message": "Go.", "runtime": "claude"})] {
        let r = tools.call("start_conversation", &args);
        assert!(r.is_error);
        assert_eq!(r.text, agents);
    }
    let said = |args: Value| tools.call("start_conversation", &args).text;
    assert_eq!(
        said(json!({"message": "Go.", "runtime": "opencode", "model": "gpt-5.5"})),
        "That model is not one opencode offers in this Locust. Ask list_models with no teammate for its choices. Nothing was started."
    );
    assert_eq!(said(json!({"message": "Go.", "runtime": "codex", "model": "gpt-5.5", "effort": "max"})), "GPT-5.5 takes these efforts here: low, high. Nothing was started.");
    assert_eq!(
        said(json!({"message": "Go.", "runtime": "codex", "effort": "high"})),
        "With no teammate, codex runs on its account’s default model, whose efforts Locust cannot name: give a model too, from list_models. Nothing was started."
    );
    assert_eq!(said(json!({"message": "", "runtime": "codex"})), "Give a message of up to 8,000 characters. Nothing was started.");
    assert_eq!(said(json!({"message": "x".repeat(8001), "runtime": "codex"})), "Give a message of up to 8,000 characters. Nothing was started.");
    assert!(tools.locust.started.lock().unwrap().is_empty());
}

#[test]
fn a_conversation_with_no_teammate_runs_in_ask_with_the_switch_on_unless_a_mode_is_named() {
    let tools = Tools::new(Fake { own_mode: true, ..Default::default() });
    assert_eq!(reply(&tools.call("start_conversation", &json!({"message": "Go.", "runtime": "codex"})))["mode"], "ask");
    assert_eq!(reply(&tools.call("start_conversation", &json!({"message": "Build it.", "runtime": "codex", "mode": "accept-edits"})))["mode"], "accept-edits");
    assert_eq!(reply(&tools.call("start_conversation", &json!({"message": "Look.", "runtime": "codex", "mode": "auto", "read_only": true})))["mode"], "ask");
    let off = Tools::new(Fake::default());
    let r = off.call("start_conversation", &json!({"message": "Go.", "runtime": "codex", "mode": "auto"}));
    assert!(r.is_error && r.text.starts_with("Choosing the mode or folder is off"), "{}", r.text);
    assert!(off.locust.started.lock().unwrap().is_empty());
}

#[test]
fn a_named_teammate_is_still_asked_and_a_runtime_beside_it_is_ignored() {
    let tools = Tools::new(Fake::default());
    let r = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "Review this.", "runtime": "opencode"}));
    assert!(!r.is_error, "{}", r.text);
    let request = tools.locust.started.lock().unwrap()[0].clone();
    assert_eq!(request.teammate.id, "tm_scout");
    assert_eq!(request.runtime, None);
}

/// A run held up behind its provider (a free quota spent, OpenCode trying again on its own) says so, still as working,
/// and goes back to saying what it does once it moves again.
#[test]
fn a_run_held_up_by_its_provider_says_why_instead_of_only_still_working() {
    let busy = json!({"type": "adapter.diagnostic", "payload": {"level": "warning", "code": "opencode.provider_busy.runtime_error", "terminal": false,
        "message": "The model's provider answered \"Free usage exceeded, subscribe to Go\", and OpenCode is trying again on its own. To go on now, press Stop and pick another model."}});
    let bash = json!({"type": "tool.started", "payload": {"name": "bash"}});
    let fake = Fake::default();
    fake.turns.lock().unwrap().insert("m1".into(), turn("m1", true, vec![bash.clone(), busy]));
    fake.live.lock().unwrap().insert("m1".into());
    let tools = Tools::new(fake);
    let r = tools.call("read_reply", &json!({"conversation_id": "m1"}));
    assert!(r.text.starts_with("Still working, but held up: The model's provider answered \"Free usage exceeded"), "{}", r.text);
    assert!(r.text.contains("press Stop and pick another model"), "{}", r.text);
    // An app still reads it as a turn at work, with the reason as what it is doing.
    let said = crate::engine::classify_reply(crate::engine::ToolText { text: r.text.clone(), is_error: false });
    assert!(matches!(said, crate::engine::Reply::Working(ref why) if why.starts_with("but held up")), "{said:?}");
    // Once it moves again, it says what it is doing.
    let mut turns = tools.locust.turns.lock().unwrap();
    turns.get_mut("m1").unwrap().events.push(bash);
    drop(turns);
    assert_eq!(tools.call("read_reply", &json!({"conversation_id": "m1"})).text, "Still working: Using bash");
}

#[cfg(test)]
#[path = "an_app_stops_and_waits_tests.rs"]
mod an_app_stops_and_waits_tests;

#[cfg(test)]
#[path = "an_app_hears_why_tests.rs"]
mod an_app_hears_why_tests;

#[cfg(test)]
#[path = "an_app_picks_a_model_tests.rs"]
mod an_app_picks_a_model_tests;

#[cfg(test)]
#[path = "an_app_carries_on_tests.rs"]
mod an_app_carries_on_tests;

#[cfg(test)]
#[path = "an_app_started_turn_tests.rs"]
mod an_app_started_turn_tests;

#[cfg(test)]
#[path = "other_apps_ask_teammates_tests.rs"]
mod other_apps_ask_teammates_tests;
