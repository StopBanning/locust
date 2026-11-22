//! Electron's `main/other-apps-can-only-ask-locust-teammates.test.ts`, "other apps run routines through the routine
//! card's own path (0.704)", with its exact values: list_routines, run_routine and routine_status over a made-up Locust
//! whose routines and runner are the test's own.
use super::*;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct Routined {
    own: bool,
    saved: Vec<Value>,
    /// What the run path was asked for: the routine and its values.
    ran: Mutex<Vec<(String, Option<Value>)>>,
    /// The runner's own refusal, when it refuses.
    refuses: Option<&'static str>,
    steps: Mutex<Vec<(String, u64, u64, String)>>,
    last: Mutex<HashMap<String, String>>,
    read_asked: Mutex<Vec<String>>,
    /// A step's conversation went on: its newest turn, when it is not the step's own.
    went_on: Option<(&'static str, &'static str)>,
}

fn wren() -> Teammate {
    Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Custom".into(),
        runtime: Some("codex".into()),
        model: Some("model-test".into()),
        account: None,
        busy: false,
        runs_in: "auto".into(),
        folder: None,
    }
}

impl Locust for Routined {
    fn teammates(&self) -> Result<Vec<(Teammate, Option<String>)>, String> {
        Ok(vec![(wren(), None)])
    }
    fn busy(&self, _: &str) -> bool {
        false
    }
    fn start(&self, _: StartRequest) -> Result<Begun, String> {
        Err("no".into())
    }
    fn read(&self, mission: &str) -> Option<Turn> {
        self.read_asked.lock().unwrap().push(mission.to_owned());
        Some(Turn {
            mission_id: mission.into(),
            events: vec![
                json!({"type": "message.delta", "payload": {"itemId": "a", "operation": "append", "text": "The summary.", "final": true}}),
                json!({"type": "run.completed", "payload": {}}),
            ],
            ..Default::default()
        })
    }
    fn newest(&self, mission: &str) -> Option<String> {
        match self.went_on {
            Some((from, to)) if from == mission => Some(to.to_owned()),
            _ => Some(mission.to_owned()),
        }
    }
    fn owner(&self, _: &str) -> Option<String> {
        Some("tm_wren".into())
    }
    fn live(&self, _: &str) -> bool {
        false
    }
    fn waiting(&self, mission: &str) -> bool {
        mission == "mission_wait"
    }
    fn cancel(&self, _: &str) -> bool {
        false
    }
    fn conversations(&self) -> Vec<Turn> {
        Vec::new()
    }
    fn check_folder(&self, _: &str) -> Result<PathBuf, String> {
        Err("no".into())
    }
    fn folder(&self) -> Option<String> {
        None
    }
    fn own_mode(&self) -> bool {
        self.own
    }
    fn see_all(&self) -> bool {
        false
    }
    fn models(&self) -> Vec<Value> {
        Vec::new()
    }
    fn carry_on(&self, _: &str, _: &str, _: Option<&str>, _: &crate::carry_on::CarryOnAlso) -> Result<(String, Option<String>), String> {
        Err("no".into())
    }
    fn routines(&self) -> Option<Vec<Value>> {
        Some(self.saved.clone())
    }
    fn run_routine(&self, routine_id: &str, values: Option<&Value>) -> Result<String, String> {
        if let Some(why) = self.refuses {
            return Err(why.into());
        }
        self.ran.lock().unwrap().push((routine_id.to_owned(), values.cloned()));
        Ok("mission_r1".into())
    }
    fn routine_steps(&self) -> Vec<(String, u64, u64, String)> {
        self.steps.lock().unwrap().clone()
    }
    fn routine_last_step(&self, routine_id: &str) -> Option<String> {
        self.last.lock().unwrap().get(routine_id).cloned()
    }
}

/// The test's routine: Wren's weekly review, in Ask, with one text input.
fn routine(overrides: Value) -> Value {
    let mut r = json!({
        "routineId": "rt_review", "name": "Weekly review", "teammateId": "tm_wren",
        "route": {"runtime": "codex", "model": "model-test", "mode": "ask"},
        "steps": ["Read the changes since {{since}}.", "Write a summary."], "learnedFrom": [], "createdAt": "2026-10-08", "runs": 0,
        "inputs": [{"key": "since", "label": "Since when", "kind": "text", "required": true}]
    });
    for (k, v) in overrides.as_object().unwrap() {
        r[k] = v.clone();
    }
    r
}

fn tools(saved: Vec<Value>, own: bool) -> Tools<Routined> {
    Tools::new(Routined { own, saved, ..Default::default() })
}

fn listed(t: &Tools<Routined>) -> Vec<Value> {
    serde_json::from_str::<Value>(&t.call("list_routines", &json!({})).text).unwrap().as_array().unwrap().clone()
}

// case: other apps run routines through the routine card's own path (0.704) > lists each step's teammate and mode, the inputs, and whether it can run from here
#[test]
fn lists_each_steps_teammate_and_mode_the_inputs_and_whether_it_can_run_from_here() {
    let handed = routine(json!({"routineId": "rt_fix", "name": "Fix it", "route": {"runtime": "codex", "model": "m", "mode": "ask"}, "handOffs": [{}, {"teammateId": "tm_wren", "check": true}]}));
    let writes = routine(json!({"routineId": "rt_edit", "name": "Edit it", "route": {"runtime": "codex", "model": "m", "mode": "accept-edits"}}));
    let folder = routine(json!({"routineId": "rt_folder", "name": "Folder", "inputs": [{"key": "dir", "label": "Folder", "kind": "folder", "required": true}]}));
    let rows = listed(&tools(vec![routine(json!({})), handed, writes, folder], false));
    assert_eq!(
        rows[0],
        json!({"id": "rt_review", "name": "Weekly review", "steps": [{"teammate": "Wren", "mode": "ask"}, {"teammate": "Wren", "mode": "ask"}],
            "inputs": [{"key": "since", "label": "Since when", "kind": "text", "required": true}], "changes_files": false, "runs_from_here": true})
    );
    assert_eq!(rows[1]["steps"][1], json!({"teammate": "Wren", "mode": "ask", "checks_the_work": true}));
    assert_eq!((rows[2]["changes_files"].clone(), rows[2]["runs_from_here"].clone()), (json!(true), json!(false)));
    assert!(rows[2]["why_not"].as_str().unwrap().contains("Use each teammate's own mode"));
    assert_eq!(rows[3]["runs_from_here"], false);
    assert!(rows[3]["why_not"].as_str().unwrap().contains("folder"));
    // A step that waits for the person's OK says so, a teammate gone is said as gone, and an input's default and choices
    // are listed.
    let more = routine(json!({"teammateId": "tm_gone", "handOffs": [{}, {"teammateId": "tm_wren", "askFirst": true}],
        "inputs": [{"key": "tone", "label": "Tone", "kind": "choice", "required": false, "default": "short", "choices": ["short", "long"]}]}));
    let rows = listed(&tools(vec![more], true));
    assert_eq!(rows[0]["steps"][0], json!({"teammate": "a teammate no longer here", "mode": "ask"}));
    assert_eq!(rows[0]["steps"][1], json!({"teammate": "Wren", "mode": "auto", "waits_for_your_ok": true}));
    assert_eq!(rows[0]["inputs"][0], json!({"key": "tone", "label": "Tone", "kind": "choice", "required": false, "default": "short", "choices": ["short", "long"]}));
    // Wren's own Auto writes, but own mode is on here: it runs from here.
    assert_eq!((rows[0]["changes_files"].clone(), rows[0]["runs_from_here"].clone()), (json!(true), json!(true)));
    assert!(rows[0].get("why_not").is_none());
}

// case: other apps run routines through the routine card's own path (0.704) > runs a read-only routine with its values through the run path, by name
#[test]
fn runs_a_read_only_routine_with_its_values_through_the_run_path_by_name() {
    let t = tools(vec![routine(json!({}))], false);
    let said = t.call("run_routine", &json!({"routine": "weekly REVIEW", "values": {"since": "Monday"}}));
    assert!(!said.is_error, "{}", said.text);
    assert_eq!(serde_json::from_str::<Value>(&said.text).unwrap(), json!({"routine_id": "rt_review", "steps": 2, "first_conversation_id": "mission_r1"}));
    assert_eq!(*t.locust.ran.lock().unwrap(), vec![("rt_review".to_owned(), Some(json!({"since": "Monday"})))]);
    // No values at all is the runner's to judge, not a refusal here.
    t.call("run_routine", &json!({"routine": "rt_review"}));
    assert_eq!(t.locust.ran.lock().unwrap()[1], ("rt_review".to_owned(), None));
}

// case: other apps run routines through the routine card's own path (0.704) > refuses a routine that changes files while own mode is off -- including one whose HANDED step writes
#[test]
fn refuses_a_routine_that_changes_files_while_own_mode_is_off_including_one_whose_handed_step_writes() {
    // Wren's own route is Auto: a step handed to her runs in Auto, whatever the routine was saved with.
    let handed = routine(json!({"handOffs": [{}, {"teammateId": "tm_wren"}], "teammateId": "tm_other"}));
    let edits = routine(json!({"route": {"runtime": "codex", "model": "m", "mode": "accept-edits"}}));
    let said = tools(vec![edits.clone()], false).call("run_routine", &json!({"routine": "rt_review"}));
    assert!(said.is_error);
    assert_eq!(said.text, "\"Weekly review\" changes files (step 1 runs in Edit). Turn on \"Use each teammate's own mode\" in Locust's Settings to let other apps run it. Nothing was started.");
    let t = tools(vec![handed], false);
    let said = t.call("run_routine", &json!({"routine": "rt_review"}));
    assert_eq!(said.text, "\"Weekly review\" changes files (step 2 runs in Auto). Turn on \"Use each teammate's own mode\" in Locust's Settings to let other apps run it. Nothing was started.");
    assert!(t.locust.ran.lock().unwrap().is_empty());
    let on = tools(vec![edits], true);
    assert!(!on.call("run_routine", &json!({"routine": "rt_review"})).is_error);
    assert_eq!(on.locust.ran.lock().unwrap().len(), 1);
    // Every mode is named in Locust's own words.
    let each = |mode: &str| tools(vec![routine(json!({"route": {"mode": mode}}))], false).call("run_routine", &json!({"routine": "rt_review"})).text;
    assert!(each("approve-each").contains("(step 1 runs in Approve each action)"));
    assert!(!tools(vec![routine(json!({"route": {"mode": "plan"}}))], false).call("run_routine", &json!({"routine": "rt_review"})).is_error, "Plan changes nothing");
}

// case: other apps run routines through the routine card's own path (0.704) > never runs a routine that asks for a folder, and refuses odd values before the run path
#[test]
fn never_runs_a_routine_that_asks_for_a_folder_and_refuses_odd_values_before_the_run_path() {
    let t = tools(
        vec![routine(json!({"inputs": [{"key": "dir", "label": "Folder", "kind": "folder", "required": true}]})), routine(json!({"routineId": "rt_two", "name": "Two"}))],
        true,
    );
    assert_eq!(
        t.call("run_routine", &json!({"routine": "rt_review", "values": {"dir": "C:\\"}})).text,
        "\"Weekly review\" asks for a folder, which is chosen in Locust's own picker. Run it from Locust. Nothing was started."
    );
    let thirteen: serde_json::Map<String, Value> = (0..13).map(|i| (format!("k{i}"), json!("v"))).collect();
    for values in [json!(["x"]), json!("since=Monday"), json!({"since": 5}), Value::Object(thirteen), json!({"since": "x".repeat(4_001)}), json!({"": "x"})] {
        let said = t.call("run_routine", &json!({"routine": "rt_two", "values": values}));
        assert!(said.is_error && said.text.ends_with("Nothing was started."), "{values}: {}", said.text);
    }
    assert_eq!(t.call("run_routine", &json!({"routine": "rt_two", "values": "x"})).text, "values must be an object of input keys to text. Nothing was started.");
    assert_eq!(t.call("run_routine", &json!({"routine": "nobody"})).text, "That routine could not be identified. Use its id from list_routines. Nothing was started.");
    assert_eq!(t.call("run_routine", &json!({})).text, "Name a routine by its id or name from list_routines. Nothing was started.");
    assert!(t.locust.ran.lock().unwrap().is_empty());
    // Twelve keys of 4,000 characters each are within bounds.
    let twelve: serde_json::Map<String, Value> = (0..12).map(|i| (format!("k{i}"), json!("x".repeat(4_000)))).collect();
    assert!(!t.call("run_routine", &json!({"routine": "rt_two", "values": twelve})).is_error);
}

// case: other apps run routines through the routine card's own path (0.704) > never runs a paused routine, and lists it as paused (0.705)
#[test]
fn never_runs_a_paused_routine_and_lists_it_as_paused() {
    let t = tools(vec![routine(json!({"paused": true}))], true);
    let row = &listed(&t)[0];
    assert_eq!((row["paused"].clone(), row["runs_from_here"].clone(), row["why_not"].clone()), (json!(true), json!(false), json!("It is paused in Locust.")));
    let said = t.call("run_routine", &json!({"routine": "rt_review", "values": {"since": "Monday"}}));
    assert_eq!(said, ToolResult::refused("\"Weekly review\" is paused in Locust. Resume it there, or run it from Locust. Nothing was started."));
    assert!(t.locust.ran.lock().unwrap().is_empty());
}

// case: other apps run routines through the routine card's own path (0.704) > passes the runner's own refusal through, unchanged
#[test]
fn passes_the_runners_own_refusal_through_unchanged() {
    let why = "This routine is already running or waiting for review. Open its routine card before starting more work.";
    let t = Tools::new(Routined { saved: vec![routine(json!({}))], refuses: Some(why), ..Default::default() });
    assert_eq!(t.call("run_routine", &json!({"routine": "rt_review"})), ToolResult::refused(why));
    // Refused is not started: its status is not this server's to read.
    assert!(t.call("routine_status", &json!({"routine": "rt_review"})).text.starts_with("This server has not started that routine"));
}

// case: other apps run routines through the routine card's own path (0.704) > reads status only for routines this server started: the step, a waiting card, then the last step's answer
#[test]
fn reads_status_only_for_routines_this_server_started() {
    let t = tools(vec![routine(json!({}))], false);
    assert_eq!(
        t.call("routine_status", &json!({"routine": "rt_review"})),
        ToolResult::refused("This server has not started that routine since Locust opened. Its runs are in Locust.")
    );
    t.call("run_routine", &json!({"routine": "rt_review", "values": {"since": "Monday"}}));
    t.locust.steps.lock().unwrap().push(("rt_review".into(), 1, 2, "mission_r1".into()));
    assert_eq!(t.call("routine_status", &json!({"routine": "rt_review"})), ToolResult::ok("Still working: step 1 of 2."));
    *t.locust.steps.lock().unwrap() = vec![("rt_review".into(), 2, 2, "mission_wait".into())];
    assert_eq!(
        t.call("routine_status", &json!({"routine": "rt_review"})).text,
        "Waiting for the person to approve or deny an action in Locust's window (step 2 of 2). This app cannot answer it; routine_status again later."
    );
    t.locust.steps.lock().unwrap().clear();
    assert_eq!(t.call("routine_status", &json!({"routine": "rt_review"})), ToolResult::refused("The run ended, and Locust has no answer from it to give."));
    t.locust.last.lock().unwrap().insert("rt_review".into(), "mission_r2".into());
    assert_eq!(t.call("routine_status", &json!({"routine": "rt_review"})), ToolResult::ok("The summary."));
    assert_eq!(t.locust.read_asked.lock().unwrap().last().map(String::as_str), Some("mission_r2"));
    // Another routine's step is not this one's.
    t.locust.steps.lock().unwrap().push(("rt_other".into(), 1, 3, "mission_x".into()));
    assert_eq!(t.call("routine_status", &json!({"routine": "rt_review"})), ToolResult::ok("The summary."));
}

// case: other apps run routines through the routine card's own path (0.704) > says a held run, a failing check and changes still waiting for Keep or Discard, in Locust's words
#[test]
fn says_a_held_run_a_failing_check_and_changes_still_waiting_for_keep_or_discard() {
    let held = routine(json!({"execution": {"attemptId": "a", "status": "held", "step": 2, "of": 2, "reason": "Review the remaining steps."}}));
    let t = tools(vec![held], false);
    t.call("run_routine", &json!({"routine": "rt_review"}));
    assert_eq!(
        t.call("routine_status", &json!({"routine": "rt_review"})),
        ToolResult::refused("Stopped at step 2 of 2 and waiting for the person in Locust: Review the remaining steps.")
    );
    let no_reason = tools(vec![routine(json!({"execution": {"status": "held", "step": 1, "of": 2}}))], false);
    no_reason.call("run_routine", &json!({"routine": "rt_review"}));
    assert_eq!(no_reason.call("routine_status", &json!({"routine": "rt_review"})).text, "Stopped at step 1 of 2 and waiting for the person in Locust.");
    // An attempt still under way is working; an abandoned one is over.
    let going = tools(vec![routine(json!({"execution": {"status": "running", "step": 1, "of": 2}}))], false);
    going.call("run_routine", &json!({"routine": "rt_review"}));
    assert_eq!(going.call("routine_status", &json!({"routine": "rt_review"})), ToolResult::ok("Still working: step 1 of 2."));
    let dropped = tools(vec![routine(json!({"execution": {"status": "abandoned", "step": 1, "of": 2}}))], false);
    dropped.call("run_routine", &json!({"routine": "rt_review"}));
    assert_eq!(dropped.call("routine_status", &json!({"routine": "rt_review"})).text, "The run ended, and Locust has no answer from it to give.");
    let failing = tools(vec![routine(json!({"lastFailed": "The tests still fail."}))], false);
    failing.call("run_routine", &json!({"routine": "rt_review"}));
    assert_eq!(failing.call("routine_status", &json!({"routine": "rt_review"})), ToolResult::refused("The run ended with its check still failing: The tests still fail."));
    let staged = tools(vec![routine(json!({"staged": {}}))], false);
    staged.call("run_routine", &json!({"routine": "rt_review"}));
    staged.locust.last.lock().unwrap().insert("rt_review".into(), "mission_r2".into());
    let said = staged.call("routine_status", &json!({"routine": "rt_review"}));
    assert!(!said.is_error);
    assert_eq!(said.text, "Its changes are waiting in Locust under Routines for the person to Keep or Discard; nothing has reached the folder yet.\n\nThe summary.");
}

#[test]
fn a_locust_that_offers_no_routines_says_so_for_each_tool() {
    let fake = Routined::default();
    // The default trait methods: a door with no window offers none.
    struct Bare(Routined);
    impl Locust for Bare {
        fn teammates(&self) -> Result<Vec<(Teammate, Option<String>)>, String> { self.0.teammates() }
        fn busy(&self, t: &str) -> bool { self.0.busy(t) }
        fn start(&self, r: StartRequest) -> Result<Begun, String> { self.0.start(r) }
        fn read(&self, m: &str) -> Option<Turn> { self.0.read(m) }
        fn newest(&self, m: &str) -> Option<String> { self.0.newest(m) }
        fn owner(&self, m: &str) -> Option<String> { self.0.owner(m) }
        fn live(&self, m: &str) -> bool { self.0.live(m) }
        fn waiting(&self, m: &str) -> bool { self.0.waiting(m) }
        fn cancel(&self, m: &str) -> bool { self.0.cancel(m) }
        fn conversations(&self) -> Vec<Turn> { Vec::new() }
        fn check_folder(&self, p: &str) -> Result<PathBuf, String> { self.0.check_folder(p) }
        fn folder(&self) -> Option<String> { None }
        fn own_mode(&self) -> bool { true }
        fn see_all(&self) -> bool { false }
        fn models(&self) -> Vec<Value> { Vec::new() }
        fn carry_on(&self, m: &str, r: &str, model: Option<&str>, also: &crate::carry_on::CarryOnAlso) -> Result<(String, Option<String>), String> { self.0.carry_on(m, r, model, also) }
    }
    let t = Tools::new(Bare(fake));
    assert_eq!(t.call("list_routines", &json!({})), ToolResult::refused("This Locust does not offer routines to other apps."));
    assert_eq!(t.call("run_routine", &json!({"routine": "r"})), ToolResult::refused("This Locust does not offer routines to other apps. Nothing was started."));
    assert_eq!(t.call("routine_status", &json!({"routine": "r"})), ToolResult::refused("This Locust does not offer routines to other apps."));
}

#[test]
fn the_answer_is_read_from_the_newest_turn_of_the_last_steps_conversation() {
    let t = Tools::new(Routined { saved: vec![routine(json!({}))], went_on: Some(("mission_step", "mission_after")), ..Default::default() });
    t.call("run_routine", &json!({"routine": "rt_review"}));
    t.locust.last.lock().unwrap().insert("rt_review".into(), "mission_step".into());
    assert_eq!(t.call("routine_status", &json!({"routine": "rt_review"})), ToolResult::ok("The summary."));
    assert_eq!(t.locust.read_asked.lock().unwrap().last().map(String::as_str), Some("mission_after"));
}

#[test]
fn two_routines_of_one_name_are_named_by_id_and_a_step_handed_to_someone_gone_runs_in_ask() {
    let t = tools(vec![routine(json!({})), routine(json!({"routineId": "rt_second"}))], false);
    assert_eq!(
        t.call("run_routine", &json!({"routine": "Weekly review"})).text,
        "That routine could not be identified. Use its id from list_routines. Nothing was started."
    );
    assert!(!t.call("run_routine", &json!({"routine": "rt_second"})).is_error, "by id it is one");
    // A step handed to a teammate no longer on the roster runs in Ask: nothing it does changes files.
    let gone = tools(vec![routine(json!({"handOffs": [{}, {"teammateId": "tm_gone"}]}))], false);
    assert_eq!(listed(&gone)[0]["steps"][1], json!({"teammate": "a teammate no longer here", "mode": "ask"}));
    assert!(!gone.call("run_routine", &json!({"routine": "rt_review"})).is_error);
}
