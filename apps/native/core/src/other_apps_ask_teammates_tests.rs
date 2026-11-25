//! Electron's `other-apps-can-only-ask-locust-teammates.test.ts`, its teammate cases, through the door's own tools
//! against the fake Locust. Its routine cases are in `door_routines_tests.rs`. The races are run against a Locust whose
//! read, roster or start is held on a channel while a second send arrives from another thread.
use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

/// The fake Locust, with a read, roster lookup or start that can be held, a switch that can be flipped, and a start
/// that can refuse.
#[derive(Default)]
struct Gated {
    fake: Fake,
    own: AtomicBool,
    read_gate: Mutex<Option<Receiver<()>>>,
    team_gate: Mutex<Option<Receiver<()>>>,
    start_gate: Mutex<Option<Receiver<()>>>,
    /// The host's refusal, and whether it is for the next start only.
    start_error: Mutex<Option<(String, bool)>>,
    reads: AtomicUsize,
    lookups: AtomicUsize,
    starts: AtomicUsize,
    asked: Mutex<Vec<StartRequest>>,
    checked: Mutex<Vec<String>>,
}

fn hold(slot: &Mutex<Option<Receiver<()>>>) {
    let gate = slot.lock().unwrap().take();
    if let Some(gate) = gate {
        let _ = gate.recv();
    }
}

fn gate(slot: &Mutex<Option<Receiver<()>>>) -> Sender<()> {
    let (tx, rx) = channel();
    *slot.lock().unwrap() = Some(rx);
    tx
}

impl Locust for Gated {
    fn teammates(&self) -> Result<Vec<(Teammate, Option<String>)>, String> {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        hold(&self.team_gate);
        self.fake.teammates()
    }
    fn busy(&self, teammate: &str) -> bool {
        self.fake.busy(teammate)
    }
    fn start(&self, request: StartRequest) -> Result<Begun, String> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        self.asked.lock().unwrap().push(request.clone());
        hold(&self.start_gate);
        let refusal = {
            let mut held = self.start_error.lock().unwrap();
            let refusal = held.clone();
            if held.as_ref().is_some_and(|(_, once)| *once) {
                *held = None;
            }
            refusal
        };
        if let Some((why, _)) = refusal {
            return Err(why);
        }
        self.fake.start(request)
    }
    fn read(&self, mission: &str) -> Option<Turn> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        hold(&self.read_gate);
        self.fake.read(mission)
    }
    fn newest(&self, mission: &str) -> Option<String> {
        self.fake.newest(mission)
    }
    fn owner(&self, mission: &str) -> Option<String> {
        self.fake.owner(mission)
    }
    fn live(&self, mission: &str) -> bool {
        self.fake.live(mission)
    }
    fn waiting(&self, mission: &str) -> bool {
        self.fake.waiting(mission)
    }
    fn cancel(&self, mission: &str) -> bool {
        self.fake.cancel(mission)
    }
    fn conversations(&self) -> Vec<Turn> {
        self.fake.conversations()
    }
    fn check_folder(&self, path: &str) -> Result<PathBuf, String> {
        self.checked.lock().unwrap().push(path.to_owned());
        self.fake.check_folder(path)
    }
    fn folder(&self) -> Option<String> {
        self.fake.folder()
    }
    fn own_mode(&self) -> bool {
        self.own.load(Ordering::SeqCst)
    }
    fn see_all(&self) -> bool {
        self.fake.see_all()
    }
    fn models(&self) -> Vec<Value> {
        self.fake.models()
    }
    fn carry_on(&self, mission: &str, runtime: &str, model: Option<&str>, also: &crate::carry_on::CarryOnAlso) -> Result<(String, Option<String>), String> {
        self.fake.carry_on(mission, runtime, model, also)
    }
    fn pause(&self, ms: u64) {
        self.fake.pause(ms)
    }
}

fn tools_with(fake: Fake, own: bool) -> Arc<Tools<Gated>> {
    let gated = Gated { fake, ..Default::default() };
    gated.own.store(own, Ordering::SeqCst);
    Arc::new(Tools::new(gated))
}

fn tools(own: bool) -> Arc<Tools<Gated>> {
    tools_with(Fake::default(), own)
}

/// mission_1 as Electron's fixture has it: an app started it, it finished, and it recorded no mode.
fn mission_1(tools: &Tools<Gated>) {
    let mut t = turn("mission_1", true, vec![ev("run.completed", json!({}))]);
    t.mode = None;
    tools.locust.fake.turns.lock().unwrap().insert("mission_1".into(), t);
}

fn asked(tools: &Tools<Gated>) -> Vec<StartRequest> {
    tools.locust.asked.lock().unwrap().clone()
}

fn last(tools: &Tools<Gated>) -> StartRequest {
    asked(tools).last().cloned().expect("a start")
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(std::time::Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A send from another app, on its own thread.
fn send_from_another_thread(tools: &Arc<Tools<Gated>>, message: &str) -> std::thread::JoinHandle<ToolResult> {
    let (tools, message) = (tools.clone(), message.to_owned());
    std::thread::spawn(move || tools.call("send_message", &json!({"conversation_id": "mission_1", "message": message})))
}

const STARTING: &str = "Still working: starting the next turn.";

// case: starts a writing teammate in Ask, ignores a caller route, and keeps the saved route
#[test]
fn a_writing_teammate_starts_in_ask_on_its_saved_route_whatever_route_the_app_names() {
    let tools = tools(false);
    let r = tools.call("start_conversation", &json!({"teammate": "sCoUt", "message": "Review this.", "runtime": "claude"}));
    assert!(!r.is_error, "{}", r.text);
    let request = last(&tools);
    assert_eq!((request.teammate.id.as_str(), request.prompt.as_str(), request.mode), ("tm_scout", "Review this.", Mode::Ask));
    assert_eq!((request.follow_up_of, request.folder), (None, None));
    assert_eq!((request.runtime, request.model, request.effort, request.account), (None, None, None, None), "the saved route");
    assert_eq!((reply(&r)["conversation_id"].as_str(), reply(&r)["mode"].as_str()), (Some("m1"), Some("ask")));
    assert_eq!(tools.locust.fake.teammates().unwrap()[0].0.runs_in, "auto", "the teammate's saved mode is left as it was");
}

// case: refuses a mode or folder in words while the switch is off, and starts nothing (0.869)
#[test]
fn a_mode_or_folder_is_refused_in_words_while_the_switch_is_off() {
    let tools = tools(false);
    for args in [json!({"mode": "auto"}), json!({"folder": "/known/tmp"}), json!({"mode": "ask"})] {
        let mut call = json!({"teammate": "Scout", "message": "Go."});
        for (k, v) in args.as_object().unwrap() {
            call[k] = v.clone();
        }
        let r = tools.call("start_conversation", &call);
        assert!(r.is_error, "{args}");
        assert!(r.text.contains("Use each teammate's own mode") && r.text.ends_with("Nothing was started."), "{}", r.text);
    }
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 0);
}

// case: uses an exact teammate id and reports the host monthly-limit sentence verbatim
#[test]
fn an_exact_teammate_id_is_used_and_the_hosts_monthly_limit_is_said_word_for_word() {
    let tools = tools(false);
    let sentence = "Scout has reached the $10 monthly limit you set. Nothing was started.";
    *tools.locust.start_error.lock().unwrap() = Some((sentence.into(), false));
    let r = tools.call("start_conversation", &json!({"teammate": "tm_scout", "message": "Review this."}));
    assert_eq!((r.is_error, r.text.as_str()), (true, sentence));
    assert_eq!(last(&tools).teammate.id, "tm_scout");
}

// case: lists public teammate identity, route and busy state, but never the stores or keys
#[test]
fn the_list_gives_who_a_teammate_is_its_route_and_whether_it_is_busy_and_nothing_else() {
    let tools = tools(false);
    let listed = reply(&tools.call("list_teammates", &json!({})));
    let first = listed[0].as_object().unwrap();
    let keys: Vec<&str> = first.keys().map(String::as_str).collect();
    // Rust says the folder a turn works in too; nothing else.
    assert_eq!(keys, vec!["id", "name", "role", "folder", "runtime", "model", "effort", "busy", "runs_in"]);
    assert_eq!(
        listed[0],
        json!({"id": "tm_scout", "name": "Scout", "role": "Builder", "folder": "/known/open", "runtime": "codex", "model": "gpt-5.5", "effort": "high", "busy": false, "runs_in": "ask"})
    );
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 0);
}

// case: continues the newest turn in Ask and returns the stable conversation id
#[test]
fn a_message_continues_the_newest_turn_in_ask_under_the_conversations_own_id() {
    let tools = tools(false);
    mission_1(&tools);
    tools.locust.fake.turns.lock().unwrap().insert("mission_2".into(), turn("mission_2", true, vec![ev("run.completed", json!({}))]));
    tools.locust.fake.newest.lock().unwrap().insert("mission_1".into(), "mission_2".into());
    let r = tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "And this?"}));
    assert_eq!((reply(&r)["conversation_id"].as_str(), reply(&r)["mode"].as_str()), (Some("mission_1"), Some("ask")));
    let request = last(&tools);
    assert_eq!((request.teammate.id.as_str(), request.prompt.as_str()), ("tm_scout", "And this?"));
    assert_eq!((request.follow_up_of.as_deref(), request.mode), (Some("mission_2"), Mode::Ask));
}

// case: reports activity and does not queue a message while the turn is running
#[test]
fn a_running_turn_says_what_it_is_doing_and_a_message_is_not_queued() {
    let tools = tools(false);
    let working = turn("mission_1", true, vec![ev("tool.started", json!({"name": "Read", "title": "Reading the source"}))]);
    tools.locust.fake.turns.lock().unwrap().insert("mission_1".into(), working);
    tools.locust.fake.live.lock().unwrap().insert("mission_1".into());
    assert_eq!(tools.call("read_reply", &json!({"conversation_id": "mission_1"})).text, "Still working: Reading the source");
    assert_eq!(tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "next"})).text, "Still working: Reading the source");
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 0);
}

// case: returns the newest complete reply rather than an earlier answer or partial text
#[test]
fn the_reply_is_the_newest_whole_answer_not_an_earlier_one_or_a_part() {
    let tools = tools(false);
    let events = vec![
        ev("message.delta", json!({"itemId": "a", "operation": "append", "text": "earlier", "final": true})),
        ev("message.delta", json!({"itemId": "b", "operation": "append", "text": "new ", "final": false})),
        ev("message.delta", json!({"itemId": "b", "operation": "append", "text": "answer", "final": true})),
        ev("run.completed", json!({})),
    ];
    tools.locust.fake.turns.lock().unwrap().insert("mission_1".into(), turn("mission_1", true, events));
    let r = tools.call("read_reply", &json!({"conversation_id": "mission_1"}));
    assert_eq!((r.is_error, r.text.as_str()), (false, "new answer"));
}

// case: returns the host failure in Locust words, not a previous successful answer
#[test]
fn a_failure_is_said_in_locusts_words_not_as_an_earlier_answer() {
    let tools = tools(false);
    let events = vec![
        ev("message.delta", json!({"itemId": "a", "operation": "append", "text": "An earlier answer.", "final": true})),
        ev("run.failed", json!({"message": "The run could not save its record. Nothing was retried."})),
    ];
    tools.locust.fake.turns.lock().unwrap().insert("mission_1".into(), turn("mission_1", true, events));
    let r = tools.call("read_reply", &json!({"conversation_id": "mission_1"}));
    assert_eq!((r.is_error, r.text.as_str()), (true, "The run could not save its record. Nothing was retried."));
}

// case: lists background turns without starting or stopping anything
#[test]
fn background_turns_are_listed_without_starting_or_stopping_anything() {
    let tools = tools(false);
    mission_1(&tools);
    tools.locust.fake.live.lock().unwrap().insert("mission_1".into());
    let listed = tools.call("list_background_runs", &json!({}));
    assert_eq!((listed.is_error, listed.text.as_str()), (false, "[]"));
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 0);
    assert!(tools.locust.live("mission_1"), "nothing was stopped");
}

// case: refuses invalid messages, unknown teammates, unknown tools and non-server conversations
#[test]
fn bad_messages_unknown_teammates_unknown_tools_and_the_persons_conversations_are_refused() {
    let tools = tools(false);
    for args in [
        json!({"teammate": "Scout", "message": ""}),
        json!({"teammate": "Scout", "message": "x".repeat(8001)}),
        json!({"teammate": "missing", "message": "hi"}),
    ] {
        assert!(tools.call("start_conversation", &args).is_error, "{args}");
    }
    assert!(tools.call("delete_files", &json!({})).is_error);
    tools.locust.fake.turns.lock().unwrap().insert("person_1".into(), turn("person_1", false, vec![ev("run.completed", json!({}))]));
    assert!(tools.call("send_message", &json!({"conversation_id": "person_1", "message": "hi"})).is_error);
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 0);
}

// case: claims a send before the first asynchronous ledger read, refusing simultaneous sends
#[test]
fn a_send_is_claimed_before_its_first_read_so_a_second_one_waits() {
    let tools = tools(false);
    mission_1(&tools);
    let release = gate(&tools.locust.read_gate);
    let first = send_from_another_thread(&tools, "first");
    wait_for("the first read", || tools.locust.reads.load(Ordering::SeqCst) == 1);
    let second = tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "second"}));
    assert_eq!(second.text, STARTING);
    release.send(()).unwrap();
    assert!(!first.join().unwrap().is_error);
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 1);
}

// case: keeps a conversation claimed until its next turn has started
#[test]
fn a_conversation_stays_claimed_until_its_next_turn_has_started() {
    let tools = tools(false);
    mission_1(&tools);
    let release = gate(&tools.locust.start_gate);
    let first = send_from_another_thread(&tools, "first");
    wait_for("the start", || tools.locust.starts.load(Ordering::SeqCst) == 1);
    let second = tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "second"}));
    let poll = tools.call("read_reply", &json!({"conversation_id": "mission_1"}));
    release.send(()).unwrap();
    let first = first.join().unwrap();
    assert_eq!((reply(&first)["conversation_id"].as_str(), reply(&first)["mode"].as_str()), (Some("mission_1"), Some("ask")));
    assert_eq!(second.text, STARTING);
    assert_eq!(poll.text, STARTING);
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 1);
    // That turn ends; the next send starts the one after it.
    tools.locust.fake.live.lock().unwrap().clear();
    let after = tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "after startup"}));
    assert!(!after.is_error, "{}", after.text);
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 2);
}

// case: keeps a conversation claimed while looking up its next turn mode
#[test]
fn a_conversation_stays_claimed_while_its_next_turns_mode_is_looked_up() {
    let tools = tools(true);
    mission_1(&tools);
    let release = gate(&tools.locust.team_gate);
    let first = send_from_another_thread(&tools, "first");
    wait_for("the roster lookup", || tools.locust.lookups.load(Ordering::SeqCst) == 1);
    let second = tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "second"}));
    assert_eq!(tools.locust.lookups.load(Ordering::SeqCst), 1, "the second send looked nothing up");
    release.send(()).unwrap();
    let first = first.join().unwrap();
    assert_eq!((reply(&first)["conversation_id"].as_str(), reply(&first)["mode"].as_str()), (Some("mission_1"), Some("auto")));
    assert_eq!(second.text, STARTING);
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 1);
}

// case: releases a conversation claim when starting its next turn rejects
#[test]
fn a_claim_is_let_go_when_the_next_turn_cannot_start() {
    let tools = tools(false);
    mission_1(&tools);
    *tools.locust.start_error.lock().unwrap() = Some(("start unavailable".into(), true));
    let release = gate(&tools.locust.start_gate);
    let first = send_from_another_thread(&tools, "first");
    wait_for("the start", || tools.locust.starts.load(Ordering::SeqCst) == 1);
    let second = tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "second"}));
    release.send(()).unwrap();
    let first = first.join().unwrap();
    assert_eq!((first.is_error, first.text.as_str()), (true, "start unavailable"));
    assert_eq!(second.text, STARTING);
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 1);
    let after = tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "after failure"}));
    assert_eq!((reply(&after)["conversation_id"].as_str(), reply(&after)["mode"].as_str()), (Some("mission_1"), Some("ask")));
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 2);
}

// case: starts the teammate in the mode saved in Locust when the app names none
#[test]
fn with_the_switch_on_a_teammate_starts_in_its_saved_mode() {
    let tools = tools(true);
    let r = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "Fix it."}));
    assert_eq!(last(&tools).mode, Mode::Auto);
    assert_eq!(last(&tools).runtime, None, "the saved route");
    assert_eq!((reply(&r)["conversation_id"].as_str(), reply(&r)["mode"].as_str()), (Some("m1"), Some("auto")));
}

// case: starts it in the mode the app names, and says so (0.869)
#[test]
fn with_the_switch_on_a_turn_starts_in_the_mode_the_app_names() {
    let tools = tools(true);
    for mode in ["ask", "plan", "accept-edits", "approve-each", "auto"] {
        let r = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "Fix it.", "mode": mode}));
        assert_eq!(reply(&r)["mode"], mode);
        assert_eq!(last(&tools).mode.as_str(), mode);
    }
    // read_only still steps it down; a mode Locust has not got is refused.
    tools.call("start_conversation", &json!({"teammate": "Scout", "message": "Look.", "mode": "auto", "read_only": true}));
    assert_eq!(last(&tools).mode, Mode::Ask);
    let before = tools.locust.starts.load(Ordering::SeqCst);
    assert!(tools.call("start_conversation", &json!({"teammate": "Scout", "message": "Go.", "mode": "yolo"})).is_error);
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), before);
}

// case: continues in the CONVERSATION's mode, not the teammate's saved one (0.869)
#[test]
fn a_follow_up_keeps_the_conversations_mode_not_the_teammates_saved_one() {
    // Pip is saved in Edit; the conversation was set to Auto.
    let tools = tools_with(Fake { owner_id: Some("tm_pip"), ..Default::default() }, true);
    let mut t = turn("mission_1", true, vec![ev("run.completed", json!({}))]);
    t.mode = Some("auto".into());
    tools.locust.fake.turns.lock().unwrap().insert("mission_1".into(), t.clone());
    let r = tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "next"}));
    assert_eq!((reply(&r)["conversation_id"].as_str(), reply(&r)["mode"].as_str()), (Some("mission_1"), Some("auto")));
    assert_eq!(last(&tools).teammate.id, "tm_pip");
    // And a mode named for the next turn wins over both.
    tools.locust.fake.live.lock().unwrap().clear();
    tools.locust.fake.newest.lock().unwrap().clear();
    tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "look only", "mode": "plan"}));
    let request = last(&tools);
    assert_eq!((request.prompt.as_str(), request.follow_up_of.as_deref(), request.mode), ("look only", Some("mission_1"), Mode::Plan));
}

// case: works in the folder the app names, records it, and the conversation stays there (0.869)
#[test]
fn a_named_folder_is_worked_in_recorded_and_kept_by_the_conversation() {
    let tools = tools(true);
    let r = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "Build it.", "mode": "auto", "folder": "/known/wt/a"}));
    assert_eq!((reply(&r)["conversation_id"].as_str(), reply(&r)["mode"].as_str(), reply(&r)["folder"].as_str()), (Some("m1"), Some("auto"), Some("/known/wt/a")));
    assert_eq!((last(&tools).mode, last(&tools).folder), (Mode::Auto, Some(PathBuf::from("/known/wt/a"))));
    // The follow-up, with no folder named, stands in the conversation's folder, checked again.
    tools.locust.fake.turns.lock().unwrap().get_mut("m1").unwrap().mode = Some("auto".into());
    tools.locust.fake.live.lock().unwrap().clear();
    tools.locust.checked.lock().unwrap().clear();
    tools.call("send_message", &json!({"conversation_id": "m1", "message": "Now test it."}));
    let request = last(&tools);
    assert_eq!((request.prompt.as_str(), request.follow_up_of.as_deref(), request.mode), ("Now test it.", Some("m1"), Mode::Auto));
    assert_eq!(request.folder, Some(PathBuf::from("/known/wt/a")));
    assert_eq!(tools.locust.checked.lock().unwrap().last().map(String::as_str), Some("/known/wt/a"));
    // A folder Locust does not know is refused, in the host's words, and nothing starts.
    let calls = tools.locust.starts.load(Ordering::SeqCst);
    let refused = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "Go.", "folder": "/etc"}));
    assert_eq!((refused.is_error, refused.text.as_str()), (true, "/etc is not a folder Locust knows. Nothing was started."));
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), calls);
}

// case: reads the switch on every turn: turned off, the next turn is Ask
#[test]
fn the_switch_is_read_at_every_turn_so_off_makes_the_next_one_ask() {
    let tools = tools(true);
    tools.call("start_conversation", &json!({"teammate": "Scout", "message": "one"}));
    assert_eq!(last(&tools).mode, Mode::Auto);
    tools.locust.fake.turns.lock().unwrap().get_mut("m1").unwrap().mode = Some("auto".into());
    tools.locust.fake.live.lock().unwrap().clear();
    tools.locust.own.store(false, Ordering::SeqCst);
    tools.call("send_message", &json!({"conversation_id": "m1", "message": "two"}));
    let request = last(&tools);
    assert_eq!((request.prompt.as_str(), request.follow_up_of.as_deref(), request.mode), ("two", Some("m1"), Mode::Ask));
}

// case: continues in the owner's saved mode, and a teammate with no route is Ask
#[test]
fn a_follow_up_takes_its_owners_saved_mode_and_a_teammate_with_no_route_is_ask() {
    let tools = tools(true);
    mission_1(&tools);
    tools.call("send_message", &json!({"conversation_id": "mission_1", "message": "next"}));
    let request = last(&tools);
    assert_eq!((request.teammate.id.as_str(), request.follow_up_of.as_deref(), request.mode), ("tm_scout", Some("mission_1"), Mode::Auto));
    let unrouted = tools_with(Fake { roster: Some(vec![(mate("tm_scout", "Scout", ""), None)]), ..Default::default() }, true);
    unrouted.call("start_conversation", &json!({"teammate": "Scout", "message": "hi"}));
    assert_eq!(last(&unrouted).mode, Mode::Ask);
}

// case: lists the mode a turn would run in
#[test]
fn the_list_says_the_mode_a_turn_would_run_in() {
    let tools = tools(true);
    assert_eq!(reply(&tools.call("list_teammates", &json!({})))[0]["runs_in"], "auto");
}

// case: passes the host's Auto refusal through as the error, unchanged
#[test]
fn the_hosts_auto_refusal_is_the_error_word_for_word() {
    let tools = tools(true);
    let sentence = "Auto mode is switched off for this workspace. Turn it on in Settings to let a run work outside the workspace folder. Nothing was recorded.";
    *tools.locust.start_error.lock().unwrap() = Some((sentence.into(), false));
    let r = tools.call("start_conversation", &json!({"teammate": "Scout", "message": "go"}));
    assert_eq!((r.is_error, r.text.as_str()), (true, sentence));
}

// case: says a card is waiting in Locust's window and offers no way to answer it
#[test]
fn a_waiting_card_is_said_to_be_in_locusts_window_and_no_tool_answers_it() {
    let tools = tools(true);
    mission_1(&tools);
    tools.locust.fake.live.lock().unwrap().insert("mission_1".into());
    tools.locust.fake.waiting.lock().unwrap().insert("mission_1".into());
    let r = tools.call("read_reply", &json!({"conversation_id": "mission_1"}));
    assert!(r.text.starts_with("Waiting for the person to approve or deny an action in Locust's window. This app cannot answer it"), "{}", r.text);
    for tool in ["approve", "answer_approval", "decide"] {
        assert!(tools.call(tool, &json!({"conversation_id": "mission_1", "decision": "approve-once"})).is_error, "{tool}");
    }
    assert_eq!(tools.locust.starts.load(Ordering::SeqCst), 0);
}

// case: refuses a mode on a server whose apps may not choose (a voice call), even with the switch on
#[test]
fn a_voice_call_is_refused_a_mode_even_with_the_switch_on() {
    let gated = Gated::default();
    gated.own.store(true, Ordering::SeqCst);
    let scope = Arc::new(crate::door_tools::VoiceScope::new(std::env::temp_dir(), None));
    let on_call = Tools::new(gated).with_voice(scope);
    for args in [json!({"mode": "auto"}), json!({"folder": "/known/tmp"})] {
        let mut call = json!({"teammate": "Scout", "message": "Fix it."});
        for (k, v) in args.as_object().unwrap() {
            call[k] = v.clone();
        }
        let r = on_call.call("start_conversation", &call);
        assert!(r.is_error, "{args}");
        assert_eq!(r.text, "The call uses the teammate’s own agent, model, mode and folder. Nothing was started.");
    }
    assert_eq!(on_call.locust.starts.load(Ordering::SeqCst), 0);
    // The same server, not on a call, takes the mode: the refusal is the call's, not the switch's.
    let tools = tools(true);
    assert!(!tools.call("start_conversation", &json!({"teammate": "Scout", "message": "Fix it.", "mode": "auto"})).is_error);
}
