//! An editor's session controls only its own turn (a-cli-session-controls-only-its-own-turn.test.ts), over a fake
//! Locust whose roster and start can be held on a channel, and a clock the test moves.
use super::*;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::mpsc::{Receiver, Sender, channel};

fn team() -> Vec<CliTeammate> {
    vec![CliTeammate { id: "mate-one".into(), name: "One".into(), runtime: Some("codex".into()), model: Some("test".into()), runs_in: "ask".into() }]
}

fn folder() -> String {
    std::env::temp_dir().to_string_lossy().into_owned()
}

fn begun() -> CliBegun {
    CliBegun { mission_id: "mission-one".into(), run_id: "run-one".into() }
}

fn card(id: &str) -> CliCard {
    CliCard {
        approval_id: id.into(),
        mission_id: "mission-one".into(),
        run_id: "run-one".into(),
        kind: "command".into(),
        summary: "Check".into(),
        detail: "echo check".into(),
        cwd: None,
        answer_in: None,
    }
}

/// As a new Locust has it: "Let editors answer cards for their own turns" is off.
#[derive(Default)]
struct Fake {
    lookups: AtomicUsize,
    /// A roster of the test's own, in place of One alone.
    team: Mutex<Option<Vec<CliTeammate>>>,
    /// Holds the roster lookups after the first (the session's open) until something is sent.
    team_gate: Mutex<Option<Receiver<()>>>,
    workspace: Mutex<Option<String>>,
    starts: Mutex<Vec<CliStart>>,
    sink: Mutex<Option<CliSink>>,
    /// Holds a start until its answer is sent.
    start_gate: Mutex<Option<Receiver<Result<CliBegun, String>>>>,
    start_answer: Mutex<Option<Result<CliBegun, String>>>,
    cancels: Mutex<Vec<String>>,
    answers: Mutex<Vec<(String, bool)>>,
    opened: AtomicUsize,
    approvals: Mutex<Vec<CliCard>>,
    editors: AtomicBool,
}

impl CliHost for Fake {
    fn teammates(&self) -> Result<Vec<CliTeammate>, String> {
        if self.lookups.fetch_add(1, Ordering::SeqCst) > 0 {
            let gate = lock(&self.team_gate).take();
            if let Some(gate) = gate {
                let _ = gate.recv();
            }
        }
        Ok(lock(&self.team).clone().unwrap_or_else(team))
    }
    fn workspace(&self) -> Option<String> {
        lock(&self.workspace).clone()
    }
    fn start(&self, request: CliStart, update: CliSink) -> Result<CliBegun, String> {
        *lock(&self.sink) = Some(update);
        lock(&self.starts).push(request);
        let gate = lock(&self.start_gate).take();
        if let Some(gate) = gate {
            return gate.recv().unwrap_or_else(|_| Err("gone".into()));
        }
        lock(&self.start_answer).clone().unwrap_or_else(|| Ok(begun()))
    }
    fn cancel(&self, run_id: &str) {
        lock(&self.cancels).push(run_id.to_owned());
    }
    fn approvals(&self) -> Vec<CliCard> {
        lock(&self.approvals).clone()
    }
    fn answer(&self, approval_id: &str, approve: bool) -> bool {
        lock(&self.answers).push((approval_id.to_owned(), approve));
        true
    }
    fn editor_answers(&self) -> bool {
        self.editors.load(Ordering::SeqCst)
    }
    fn opened(&self) {
        self.opened.fetch_add(1, Ordering::SeqCst);
    }
    fn status(&self) -> Result<Value, String> {
        Ok(json!({"workspace": folder(), "turns": []}))
    }
}

struct F {
    fake: Arc<Fake>,
    host: CliSessions,
    clock: Arc<AtomicU64>,
}

fn fixture_with(lease_ms: u64, start_timeout_ms: u64, setup: impl FnOnce(&Fake)) -> F {
    let fake = Fake::default();
    *lock(&fake.workspace) = Some(folder());
    setup(&fake);
    let fake = Arc::new(fake);
    let clock = Arc::new(AtomicU64::new(1_000));
    let now = clock.clone();
    let options = CliOptions { now: Arc::new(move || now.load(Ordering::SeqCst)), lease_ms, start_timeout_ms };
    F { host: CliSessions::new(fake.clone(), options), fake, clock }
}

fn fixture(setup: impl FnOnce(&Fake)) -> F {
    fixture_with(LEASE_MS, START_TIMEOUT_MS, setup)
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

impl F {
    fn call(&self, name: &str, args: Value) -> Result<Value, String> {
        let result = self.host.call(name, &args);
        if result.is_error {
            return Err(result.text);
        }
        Ok(serde_json::from_str(&result.text).unwrap())
    }
    fn open(&self) -> Value {
        self.call("cli/open", json!({})).unwrap()
    }
    fn with(&self, session: &Value, more: Value) -> Value {
        let mut args = json!({"sessionId": session["sessionId"], "key": session["key"]});
        for (k, v) in more.as_object().unwrap() {
            args[k] = v.clone();
        }
        args
    }
    fn prompt(&self, session: &Value, request: &str, message: &str) -> Result<Value, String> {
        self.call("cli/prompt", self.with(session, json!({"requestId": request, "message": message})))
    }
    fn poll(&self, session: &Value, after: u64) -> Value {
        self.call("cli/poll", self.with(session, json!({"after": after}))).unwrap()
    }
    fn starts(&self) -> Vec<CliStart> {
        lock(&self.fake.starts).clone()
    }
    fn cancels(&self) -> Vec<String> {
        lock(&self.fake.cancels).clone()
    }
    /// The turn has started and its start's answer is in (a fake start answers at once).
    fn running(&self, session: &Value) {
        wait_for("the turn to run", || self.poll(session, 0)["turn"]["phase"] == "running");
    }
    /// What Electron's fixture emits: the run's ids, and one event.
    fn emit(&self, kind: &str, payload: Value) {
        let sink = lock(&self.fake.sink).clone().expect("a start");
        sink(CliUpdate::Run { mission_id: "mission-one".into(), run_id: "run-one".into() });
        sink(CliUpdate::Event(json!({"type": kind, "payload": payload})));
    }
    fn advance(&self, ms: u64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }
}

fn gate<T>(slot: &Mutex<Option<Receiver<T>>>) -> Sender<T> {
    let (tx, rx) = channel();
    *lock(slot) = Some(rx);
    tx
}

// case: opens without starting a turn and ignores client route or mode overrides
#[test]
fn a_session_opens_idle_and_ignores_the_clients_route_and_mode() {
    let f = fixture(|_| {});
    let session = f.open();
    assert!(f.starts().is_empty());
    let args = f.with(&session, json!({"requestId": "one", "message": "Hello", "mode": "auto", "runtime": "other", "runId": "not-owned"}));
    f.call("cli/prompt", args).unwrap();
    wait_for("a start", || f.starts().len() == 1);
    assert_eq!(
        f.starts()[0],
        CliStart { prompt: "Hello".into(), teammate_id: "mate-one".into(), mode: "ask".into(), follow_up_of: None }
    );
}

// case: claims before asynchronous lookup and does not replay a duplicate request
#[test]
fn a_prompt_is_claimed_before_the_roster_lookup_and_a_duplicate_is_not_replayed() {
    let f = fixture(|_| {});
    let release = gate(&f.fake.team_gate);
    let session = f.open();
    f.prompt(&session, "one", "First").unwrap();
    f.prompt(&session, "one", "First").unwrap();
    let busy = f.prompt(&session, "two", "Second").unwrap_err();
    assert!(busy.contains("still working"), "{busy}");
    release.send(()).unwrap();
    wait_for("a start", || f.starts().len() == 1);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(f.starts().len(), 1);
}

// case: rejects a wrong capability for reading, cancellation and decisions
#[test]
fn a_wrong_key_reads_cancels_and_decides_nothing() {
    let f = fixture(|fake| fake.editors.store(true, Ordering::SeqCst));
    let session = f.open();
    for name in ["cli/poll", "cli/cancel", "cli/decide", "cli/prompt"] {
        let args = json!({"sessionId": session["sessionId"], "key": "b".repeat(64), "approvalId": "other", "approve": true, "after": 0, "requestId": "one", "message": "Hi"});
        let refused = f.call(name, args).unwrap_err();
        assert!(refused.contains("no longer connected"), "{name}: {refused}");
    }
    // A key of the wrong shape, and the right key under another session's id, are refused too.
    let odd = json!({"sessionId": session["sessionId"], "key": "B".repeat(64), "after": 0});
    assert!(f.call("cli/poll", odd).unwrap_err().contains("no longer connected"));
    let other = f.open();
    let swapped = json!({"sessionId": other["sessionId"], "key": session["key"], "after": 0});
    assert!(f.call("cli/poll", swapped).unwrap_err().contains("no longer connected"));
    assert!(f.cancels().is_empty());
    assert!(lock(&f.fake.answers).is_empty());
    assert!(f.starts().is_empty());
}

// case: leaves every card in Locust until the person lets editors answer
#[test]
fn every_card_waits_in_locust_until_the_person_lets_editors_answer() {
    let f = fixture(|_| {});
    let session = f.open();
    assert_eq!(f.fake.opened.load(Ordering::SeqCst), 1);
    f.prompt(&session, "one", "Check").unwrap();
    f.running(&session);
    f.emit("run.started", json!({}));
    lock(&f.fake.approvals).push(card("own"));
    let poll = f.poll(&session, 0);
    let cards: Vec<(String, bool)> = poll["approvals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["approvalId"].as_str().unwrap().to_owned(), a["answerInLocust"].as_bool().unwrap()))
        .collect();
    assert_eq!(cards, vec![("own".to_owned(), true)]);
    let refused = f.call("cli/decide", f.with(&session, json!({"approvalId": "own", "approve": true}))).unwrap_err();
    assert!(refused.contains("Let editors answer cards for their own turns"), "{refused}");
    assert!(lock(&f.fake.answers).is_empty());
}

// case: answers only its own current action, never another run or a question
#[test]
fn an_editor_answers_only_its_own_current_action() {
    let f = fixture(|fake| fake.editors.store(true, Ordering::SeqCst));
    let session = f.open();
    f.prompt(&session, "one", "Check").unwrap();
    f.running(&session);
    f.emit("run.started", json!({}));
    let own = card("own");
    let foreign = CliCard { approval_id: "foreign".into(), run_id: "another-run".into(), ..own.clone() };
    let foreign_mission = CliCard { approval_id: "foreign-mission".into(), mission_id: "another-mission".into(), ..own.clone() };
    let question = CliCard { approval_id: "question".into(), kind: "question".into(), detail: "secret".into(), ..own.clone() };
    let elsewhere = CliCard { approval_id: "elsewhere".into(), answer_in: Some("Antigravity".into()), ..own.clone() };
    lock(&f.fake.approvals).extend([own, foreign, foreign_mission, question, elsewhere]);
    let poll = f.poll(&session, 0);
    let listed: Vec<&str> = poll["approvals"].as_array().unwrap().iter().map(|a| a["approvalId"].as_str().unwrap()).collect();
    assert_eq!(listed, vec!["own", "question", "elsewhere"]);
    let flags: Vec<bool> = poll["approvals"].as_array().unwrap().iter().map(|a| a["answerInLocust"].as_bool().unwrap()).collect();
    assert_eq!(flags, vec![false, true, true]);
    assert_eq!(poll["approvals"][1]["detail"], "", "a question's words never reach the editor");
    for id in ["foreign", "foreign-mission", "question", "elsewhere"] {
        let refused = f.call("cli/decide", f.with(&session, json!({"approvalId": id, "approve": true}))).unwrap_err();
        assert!(refused.contains("not waiting"), "{id}: {refused}");
    }
    let odd = f.call("cli/decide", f.with(&session, json!({"approvalId": "own", "approve": "yes"}))).unwrap_err();
    assert!(odd.contains("not waiting"), "{odd}");
    assert_eq!(f.call("cli/decide", f.with(&session, json!({"approvalId": "own", "approve": true}))).unwrap(), json!({"ok": true}));
    assert_eq!(*lock(&f.fake.answers), vec![("own".to_owned(), true)]);
}

// case: cancels a late start using its returned run id, not a client-supplied one
#[test]
fn a_late_start_is_cancelled_by_its_own_run_id() {
    let f = fixture(|_| {});
    let started = gate(&f.fake.start_gate);
    let session = f.open();
    f.prompt(&session, "one", "Start").unwrap();
    wait_for("a start", || f.starts().len() == 1);
    f.call("cli/cancel", f.with(&session, json!({"runId": "do-not-stop"}))).unwrap();
    started.send(Ok(begun())).unwrap();
    wait_for("the cancel", || f.cancels().contains(&"run-one".to_owned()));
    assert!(!f.cancels().contains(&"do-not-stop".to_owned()));
    assert_eq!(f.poll(&session, 0)["turn"]["phase"], "cancelled");
}

// case: does not start when a cancellation arrives before its prompt request
#[test]
fn a_cancel_that_arrives_first_keeps_its_prompt_from_starting() {
    let f = fixture(|_| {});
    let session = f.open();
    f.call("cli/cancel", f.with(&session, json!({"requestId": "one"}))).unwrap();
    f.prompt(&session, "one", "Do not start").unwrap();
    assert_eq!(f.poll(&session, 0)["turn"]["phase"], "cancelled");
    std::thread::sleep(Duration::from_millis(50));
    assert!(f.starts().is_empty());
}

// case: does not replay an older request id after another turn completed
#[test]
fn an_older_request_id_is_not_replayed() {
    let f = fixture(|_| {});
    let session = f.open();
    f.prompt(&session, "one", "First").unwrap();
    f.running(&session);
    f.emit("run.completed", json!({}));
    f.prompt(&session, "two", "Second").unwrap();
    wait_for("the second start", || f.starts().len() == 2);
    f.running(&session);
    f.emit("run.completed", json!({}));
    assert!(f.prompt(&session, "one", "First").unwrap_err().contains("not sent again"));
    assert!(f.prompt(&session, "two", "Different").unwrap_err().contains("not sent again"));
    assert_eq!(f.prompt(&session, "two", "Second").unwrap(), json!({"ok": true}), "the same request again is the same turn");
    assert_eq!(f.starts().len(), 2);
}

// case: fails a hung startup and cancels its late result without replaying it
#[test]
fn a_hung_start_fails_and_its_late_result_is_cancelled() {
    let f = fixture_with(10_000, 100, |_| {});
    let started = gate(&f.fake.start_gate);
    let session = f.open();
    f.prompt(&session, "one", "Start").unwrap();
    wait_for("a start", || f.starts().len() == 1);
    f.advance(99);
    assert_eq!(f.poll(&session, 0)["turn"]["phase"], "starting", "not before its deadline");
    f.advance(2);
    let turn = f.poll(&session, 0)["turn"].clone();
    assert_eq!(turn["phase"], "failed");
    assert!(turn["message"].as_str().unwrap().contains("starting in time"), "{turn}");
    started.send(Ok(begun())).unwrap();
    wait_for("the cancel", || f.cancels().contains(&"run-one".to_owned()));
    assert_eq!(f.starts().len(), 1);
    assert_eq!(f.poll(&session, 0)["turn"]["phase"], "failed");
}

// case: does not start after close while the roster lookup is pending
#[test]
fn a_closed_session_does_not_start_after_its_roster_lookup() {
    let f = fixture(|_| {});
    let release = gate(&f.fake.team_gate);
    let session = f.open();
    f.prompt(&session, "one", "Do not start").unwrap();
    f.call("cli/close", f.with(&session, json!({}))).unwrap();
    release.send(()).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(f.starts().is_empty());
    let refused = f.call("cli/poll", f.with(&session, json!({"after": 0}))).unwrap_err();
    assert!(refused.contains("no longer connected"), "{refused}");
}

// case: stops abandoned work when its client lease expires
#[test]
fn an_abandoned_session_stops_its_work_when_its_lease_runs_out() {
    let f = fixture_with(100, START_TIMEOUT_MS, |_| {});
    let session = f.open();
    f.prompt(&session, "one", "Start").unwrap();
    f.running(&session);
    f.advance(99);
    f.host.sweep();
    assert!(f.cancels().is_empty(), "the lease still holds");
    f.call("cli/keepalive", f.with(&session, json!({}))).unwrap();
    f.advance(99);
    f.host.sweep();
    assert!(f.cancels().is_empty(), "a keepalive renews the lease");
    f.advance(2);
    f.host.sweep();
    assert_eq!(f.cancels(), vec!["run-one".to_owned()]);
    assert!(f.call("cli/keepalive", f.with(&session, json!({}))).unwrap_err().contains("no longer connected"));
}

// case: continues only its own previous mission after completion
#[test]
fn a_session_continues_only_its_own_previous_turn() {
    let f = fixture(|_| {});
    let session = f.open();
    f.call("cli/prompt", f.with(&session, json!({"requestId": "one", "message": "First", "followUpOf": "foreign"}))).unwrap();
    f.running(&session);
    f.emit("run.completed", json!({}));
    f.call("cli/prompt", f.with(&session, json!({"requestId": "two", "message": "Second", "followUpOf": "foreign"}))).unwrap();
    wait_for("the second start", || f.starts().len() == 2);
    assert_eq!(f.starts()[0].follow_up_of, None);
    assert_eq!(f.starts()[1].follow_up_of.as_deref(), Some("mission-one"));
}

// case: holds the next turn until a cancelled runtime actually reports its end
#[test]
fn the_next_turn_waits_for_a_cancelled_runtime_to_end() {
    let f = fixture(|_| {});
    let session = f.open();
    f.prompt(&session, "one", "First").unwrap();
    f.running(&session);
    f.call("cli/cancel", f.with(&session, json!({}))).unwrap();
    assert_eq!(f.cancels(), vec!["run-one".to_owned()]);
    assert!(f.prompt(&session, "two", "Second").unwrap_err().contains("still working"));
    f.emit("run.cancelled", json!({}));
    f.prompt(&session, "two", "Second").unwrap();
    wait_for("the second start", || f.starts().len() == 2);
}

// case: streams only message text and frees acknowledged output
#[test]
fn only_message_text_is_streamed_and_read_output_is_freed() {
    let f = fixture(|_| {});
    let session = f.open();
    f.prompt(&session, "one", "Hello").unwrap();
    f.running(&session);
    f.emit("message.delta", json!({"itemId": "a", "operation": "append", "text": "Hello", "final": false}));
    f.emit("step.started", json!({"stepKind": "reasoning", "message": "not sent to the editor"}));
    let first = f.poll(&session, 0);
    assert_eq!(first["events"], json!([{"sequence": 1, "itemId": "a", "operation": "append", "text": "Hello"}]));
    assert_eq!(f.poll(&session, 0)["events"].as_array().unwrap().len(), 1, "unread output is kept");
    f.emit("run.completed", json!({}));
    let second = f.poll(&session, first["cursor"].as_u64().unwrap());
    assert_eq!(second["events"], json!([]));
    assert_eq!(second["turn"]["phase"], "completed");
    assert!(f.call("cli/poll", f.with(&session, json!({"after": 0}))).unwrap_err().contains("cursor"), "read output cannot be read again");
    assert!(f.call("cli/poll", f.with(&session, json!({"after": 99}))).unwrap_err().contains("cursor"));
}

// case: reports a startup refusal and permits a later explicit attempt
#[test]
fn a_refused_start_is_reported_and_a_later_attempt_may_start() {
    let f = fixture(|fake| *lock(&fake.start_answer) = Some(Err("Monthly limit reached. Change the limit in Locust.".into())));
    let session = f.open();
    f.prompt(&session, "one", "Start").unwrap();
    wait_for("the refusal", || f.poll(&session, 0)["turn"]["phase"] == "failed");
    assert_eq!(f.poll(&session, 0)["turn"]["message"], "Monthly limit reached. Change the limit in Locust.");
    f.prompt(&session, "two", "Try explicitly").unwrap();
    wait_for("the second start", || f.starts().len() == 2);
}

// case: does not silently use another folder or an ambiguous teammate
#[test]
fn another_folder_or_an_unknown_teammate_opens_nothing() {
    let f = fixture(|_| {});
    assert!(f.call("cli/open", json!({"cwd": "/not-the-open-project"})).unwrap_err().contains("folder"));
    assert!(f.call("cli/open", json!({"cwd": 7})).unwrap_err().contains("folder"));
    assert!(f.call("cli/open", json!({"teammate": "missing"})).unwrap_err().contains("One"));
    assert!(f.call("cli/open", json!({"cwd": format!("{}/./", folder()), "teammate": "ONE"})).is_ok(), "the same folder, the name in any case");
    assert!(f.starts().is_empty());
    *lock(&f.fake.workspace) = None;
    assert!(f.call("cli/open", json!({})).unwrap_err().contains("Choose a folder"));
}

// case: rechecks the chosen folder before starting a later prompt
#[test]
fn the_folder_is_checked_again_before_a_prompt_starts() {
    let f = fixture(|_| {});
    let session = f.open();
    *lock(&f.fake.workspace) = Some("/different-project".into());
    f.prompt(&session, "one", "Do not use another folder").unwrap();
    wait_for("the refusal", || f.poll(&session, 0)["turn"]["phase"] == "failed");
    assert!(f.poll(&session, 0)["turn"]["message"].as_str().unwrap().contains("folder open in Locust changed"));
    assert!(f.starts().is_empty());
}

// case: stops rather than silently dropping a slow editor’s output
#[test]
fn a_slow_editor_stops_the_turn_rather_than_losing_output() {
    let f = fixture(|_| {});
    let session = f.open();
    f.prompt(&session, "one", "Start").unwrap();
    f.running(&session);
    f.emit("message.delta", json!({"itemId": "a", "operation": "append", "text": "x".repeat(2 * 1024 * 1024)}));
    assert_eq!(f.poll(&session, 0)["turn"]["phase"], "running", "2 MiB exactly is kept");
    f.emit("message.delta", json!({"itemId": "a", "operation": "append", "text": "x"}));
    let turn = f.poll(&session, 0)["turn"].clone();
    assert_eq!(turn["phase"], "failed");
    assert!(turn["message"].as_str().unwrap().contains("did not keep up"), "{turn}");
    assert_eq!(f.cancels(), vec!["run-one".to_owned()]);
}

/// The other cap: 8,000 unread pieces, however small.
#[test]
fn eight_thousand_unread_pieces_stop_the_turn_too() {
    let f = fixture(|_| {});
    let session = f.open();
    f.prompt(&session, "one", "Start").unwrap();
    f.running(&session);
    let sink = lock(&f.fake.sink).clone().unwrap();
    for _ in 0..8_000 {
        sink(CliUpdate::Event(json!({"type": "message.delta", "payload": {"itemId": "a", "operation": "append", "text": "x"}})));
    }
    assert_eq!(f.poll(&session, 0)["turn"]["phase"], "running");
    sink(CliUpdate::Event(json!({"type": "message.delta", "payload": {"itemId": "a", "operation": "append", "text": "x"}})));
    assert_eq!(f.poll(&session, 0)["turn"]["phase"], "failed");
    assert_eq!(f.cancels(), vec!["run-one".to_owned()]);
}

// case: cancels only owned work on app shutdown, including a late start
#[test]
fn closing_locust_cancels_only_owned_work_including_a_late_start() {
    let f = fixture(|_| {});
    let finish = gate(&f.fake.start_gate);
    let session = f.open();
    f.prompt(&session, "one", "Start").unwrap();
    wait_for("a start", || f.starts().len() == 1);
    f.host.dispose();
    finish.send(Ok(begun())).unwrap();
    wait_for("the cancel", || !f.cancels().is_empty());
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(f.cancels(), vec!["run-one".to_owned()]);
    assert!(f.call("cli/open", json!({})).unwrap_err().contains("closing"));
}

/// The `locust` command's client (packages/cli client.ts) reads these shapes: a session, a poll, and `{ok}`.
#[test]
fn the_answers_have_the_shapes_the_locust_command_reads() {
    let f = fixture(|_| {});
    let session = f.open();
    let keys: Vec<&str> = session.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, vec!["sessionId", "key", "teammate", "workspace"]);
    assert_eq!(session["teammate"], json!({"id": "mate-one", "name": "One", "runtime": "codex", "model": "test", "runs_in": "ask"}));
    assert!(is_key(session["key"].as_str().unwrap()));
    let id = session["sessionId"].as_str().unwrap();
    assert_eq!(id.len(), 36);
    assert_eq!(id.matches('-').count(), 4);
    let idle = f.poll(&session, 0);
    assert_eq!(idle, json!({"events": [], "cursor": 0, "approvals": []}), "no turn yet, and no null in its place");
    f.prompt(&session, "one", "Hi").unwrap();
    f.running(&session);
    assert_eq!(f.poll(&session, 0)["turn"], json!({"requestId": "one", "phase": "running", "missionId": "mission-one", "runId": "run-one"}));
    assert_eq!(f.call("cli/status", json!({})).unwrap()["turns"], json!([]));
    assert!(f.call("cli/nothing", f.with(&session, json!({}))).unwrap_err().contains("not offered by Locust"));
    assert!(f.prompt(&session, "two", " ").unwrap_err().contains("8,000"));
    assert!(f.prompt(&session, "two", &"x".repeat(8_001)).unwrap_err().contains("8,000"));
    assert!(f.prompt(&session, &"r".repeat(101), "Hi").unwrap_err().contains("8,000"));
}

/// The bridge another app runs never lists an editor's names, and refuses them.
#[test]
fn the_bridge_lists_no_editor_command() {
    let tools = crate::door::tool_list();
    assert!(tools.as_array().unwrap().iter().all(|t| !t["name"].as_str().unwrap().starts_with("cli/")));
}

/// A stop that lands before the run's id is known stops the run as soon as its id arrives.
#[test]
fn a_stop_before_the_run_id_is_known_stops_the_run_when_it_arrives() {
    let f = fixture(|_| {});
    let started = gate(&f.fake.start_gate);
    let session = f.open();
    f.prompt(&session, "one", "Start").unwrap();
    wait_for("a start", || f.starts().len() == 1);
    f.call("cli/cancel", f.with(&session, json!({}))).unwrap();
    assert!(f.cancels().is_empty());
    let sink = lock(&f.fake.sink).clone().unwrap();
    sink(CliUpdate::Run { mission_id: "mission-one".into(), run_id: "run-one".into() });
    assert_eq!(f.cancels(), vec!["run-one".to_owned()]);
    sink(CliUpdate::Run { mission_id: "mission-one".into(), run_id: "run-one".into() });
    assert_eq!(f.cancels().len(), 1, "once");
    started.send(Ok(begun())).unwrap();
}

/// The run's transport failing fails the turn in its own words and stops it.
#[test]
fn a_transport_failure_fails_the_turn_and_stops_it() {
    let f = fixture(|_| {});
    let session = f.open();
    f.prompt(&session, "one", "Start").unwrap();
    f.running(&session);
    let sink = lock(&f.fake.sink).clone().unwrap();
    sink(CliUpdate::Error("The ledger could not be written.".into()));
    let turn = f.poll(&session, 0)["turn"].clone();
    assert_eq!((turn["phase"].as_str(), turn["message"].as_str()), (Some("failed"), Some("The ledger could not be written.")));
    assert_eq!(f.cancels(), vec!["run-one".to_owned()]);
}

/// Closing Locust stops a running turn of a session, and the sessions are gone.
#[test]
fn closing_locust_stops_a_running_turn() {
    let f = fixture(|_| {});
    let session = f.open();
    f.prompt(&session, "one", "Start").unwrap();
    f.running(&session);
    f.host.dispose();
    assert_eq!(f.cancels(), vec!["run-one".to_owned()]);
}

/// An earlier turn's late updates are not this turn's: they are ignored, and say nobody listens.
#[test]
fn an_earlier_turns_late_updates_are_ignored() {
    let f = fixture(|_| {});
    let session = f.open();
    f.prompt(&session, "one", "First").unwrap();
    f.running(&session);
    f.emit("run.completed", json!({}));
    let old = lock(&f.fake.sink).clone().unwrap();
    f.prompt(&session, "two", "Second").unwrap();
    wait_for("the second start", || f.starts().len() == 2);
    f.running(&session);
    let listening = old(CliUpdate::Event(json!({"type": "message.delta", "payload": {"itemId": "a", "operation": "append", "text": "late"}})));
    assert!(!listening);
    assert_eq!(f.poll(&session, 0)["events"], json!([]));
    let now = lock(&f.fake.sink).clone().unwrap();
    assert!(now(CliUpdate::Ping), "the current turn is listened to");
    f.emit("run.completed", json!({}));
    assert!(!now(CliUpdate::Ping), "and no longer once it ended");
}

/// With no teammate named, a session takes the roster's first, and only that one.
#[test]
fn a_session_with_no_teammate_named_takes_the_first() {
    let two = vec![team()[0].clone(), CliTeammate { id: "mate-two".into(), name: "Two".into(), ..team()[0].clone() }];
    let f = fixture(|fake| *lock(&fake.team) = Some(two));
    assert_eq!(f.open()["teammate"]["id"], "mate-one");
    assert_eq!(f.call("cli/open", json!({"teammate": "two"})).unwrap()["teammate"]["id"], "mate-two");
}
