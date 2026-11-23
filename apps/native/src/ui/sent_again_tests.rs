//! `an-ended-run-offers-the-way-on.test.tsx`, drawn: the stopped card and Send again under the newest turn, the stopped
//! copy that gives way to the one sent again, the note under a kept one, the fresh-session note, and Continue on a run
//! the provider dropped. The logic's own cases are in core/src/sent_again_tests.rs.
use super::*;
use serde_json::{json, Value};

/// One turn: the person's words, then what the runtime recorded, read by the ledger's own reader.
fn turn(prompt: &str, adapter: &str, events: &[(&str, Value)]) -> Vec<ledger::Item> {
    let records: Vec<Value> = events
        .iter()
        .enumerate()
        .map(|(n, (kind, payload))| json!({ "event": { "id": format!("evt_{n}"), "runId": "run_1", "missionId": "m", "sequence": n + 1,
            "occurredAt": "2026-09-29T12:00:00.000Z", "sourceAdapter": adapter, "type": kind, "payload": payload } }))
        .collect();
    let mut transcript = ledger::Transcript::default();
    transcript.apply(records);
    std::iter::once(ledger::Item::User(prompt.into())).chain(transcript.items).collect()
}
fn stopped(prompt: &str, adapter: &str, before: &[(&str, Value)]) -> Vec<ledger::Item> {
    let mut events = vec![("run.started", json!({}))];
    events.extend(before.iter().cloned());
    events.push(("run.cancelled", json!({})));
    turn(prompt, adapter, &events)
}
fn said(prompt: &str, words: &str) -> Vec<ledger::Item> {
    turn(prompt, "claude", &[("run.started", json!({})), ("message.delta", json!({ "itemId": "m1", "text": words, "final": true })), ("run.completed", json!({}))])
}
fn read_tool() -> Vec<(&'static str, Value)> {
    vec![
        ("tool.started", json!({ "itemId": "c1", "toolKind": "file_read", "name": "Read", "command": "a.txt" })),
        ("tool.completed", json!({ "itemId": "c1", "toolKind": "file_read", "name": "Read", "command": "a.txt", "exitCode": 0 })),
    ]
}

struct Drawn {
    texts: Vec<String>,
    keys: Vec<String>,
}
impl Drawn {
    fn has(&self, words: &str) -> bool {
        self.texts.iter().any(|t| t.contains(words))
    }
    fn count(&self, words: &str) -> usize {
        self.texts.iter().filter(|t| t.contains(words)).count()
    }
    fn key(&self, key: &str) -> bool {
        self.keys.iter().any(|k| k == key)
    }
}

/// The thread drawn in a window: `turns` in order, on `runtime`, the newest one `cold` if so.
fn draw(turns: &[Vec<ledger::Item>], runtime: &str, cold: bool, busy_continue: bool) -> (rune::Runtime<State>, Drawn) {
    draw_with(turns, runtime, cold, busy_continue, |_| {})
}

/// The whole conversation view, as the person sees it: the failure card's offers stand under the thread there.
fn draw_whole(turns: &[Vec<ledger::Item>], runtime: &str, tweak: impl FnOnce(&mut State)) -> (rune::Runtime<State>, Drawn) {
    let (rt, _) = draw_with(turns, runtime, false, false, tweak);
    let mut rt = rune::Runtime::new(rt.state, |s, _| crate::thread_view::thread_view(s, &Palette::new(true)), (1200.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.05);
    let d = drawn(&rt);
    (rt, d)
}

fn drawn(rt: &rune::Runtime<State>) -> Drawn {
    let els = rt.elements();
    Drawn { texts: els.iter().filter_map(|e| e.text.clone()).collect(), keys: els.iter().filter_map(|e| e.key.clone()).collect() }
}

fn draw_with(turns: &[Vec<ledger::Item>], runtime: &str, cold: bool, busy_continue: bool, tweak: impl FnOnce(&mut State)) -> (rune::Runtime<State>, Drawn) {
    let items: Vec<ledger::Item> = turns.concat();
    let metas: Vec<ledger::Metadata> = turns
        .iter()
        .enumerate()
        .map(|(n, t)| ledger::Metadata {
            mission_id: format!("m_{n}"),
            prompt: match &t[0] { ledger::Item::User(words) => words.clone(), _ => String::new() },
            ..Default::default()
        })
        .collect();
    let tip = metas.last().map(|m| m.mission_id.clone()).unwrap_or_default();
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: cold.then(|| (tip.clone(), true)),
        header: Teammate { id: "tm_codec".into(), name: "Codec".into(), role: "Engineer".into(), runtime: Some(runtime.into()), model: None, account: None, busy: false, runs_in: "local".into(), folder: None },
        conversation: Conversation { conversation_id: Some("c_1".into()), turns: Vec::new() },
        items,
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot { running: false, tip, turn_metas: metas, busy_continue, ..Default::default() },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    tweak(&mut s);
    let mut rt = rune::Runtime::new(
        s,
        |s, _| col().w_full().child(message_column(&s.ledger.as_ref().unwrap().items, "Codec", &Palette::new(true), s)).children(crate::busy_continue::view(s, &Palette::new(true))),
        (800.0, 3000.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.frame(0.05);
    let d = drawn(&rt);
    (rt, d)
}

// case: offers Send again
#[test]
fn offers_send_again() {
    let (_, d) = draw(&[stopped("Summarize the turns", "claude", &[])], "claude", false, false);
    assert!(d.has("Stopped before it used any tools"), "{:?}", d.texts);
    assert!(d.has("Send again") && d.key("send-again-button"), "{:?}", d.texts);
    assert!(d.has("No tool had run, so sending it again cannot repeat anything."), "{:?}", d.texts);
    // The press sends the same words again; what is in the box stays.
    let (mut rt, _) = draw_with(&[stopped("Summarize the turns", "claude", &[])], "claude", false, false, |s| s.draft.replace_text("half a thought"));
    press(&mut rt, "send-again-button");
    let flight = rt.state.flight.as_ref().expect("Send again sent nothing");
    assert_eq!((flight.teammate_id.as_str(), flight.conversation_id.as_deref(), flight.message.as_deref()), ("tm_codec", Some("c_1"), Some("Summarize the turns")));
    assert_eq!(rt.state.draft.text, "half a thought");
}

// case: on OpenCode, which reports a tool only once it finishes, does not promise nothing ran (Sol, 0.532)
#[test]
fn on_opencode_which_reports_a_tool_only_once_it_finishes_does_not_promise_nothing_ran() {
    let (_, d) = draw(&[stopped("Summarize the turns", "opencode", &[])], "opencode", false, false);
    assert!(!d.has("Stopped before it used any tools"), "{:?}", d.texts);
    assert!(!d.has("cannot repeat anything"), "{:?}", d.texts);
    assert!(d.has("An unreported OpenCode command may have been running when stopped."), "{:?}", d.texts);
    // Said once: the card says to check the folder; the Send again line does not say it again (0.696).
    assert_eq!(d.count("check the folder"), 1, "{:?}", d.texts);
    assert!(!d.has("Look at the folder before sending it again"), "{:?}", d.texts);
    assert!(d.key("send-again-button"), "{:?}", d.texts);
}

// case: on OpenCode with something finished, never says no tool call was open (0.536 drive)
#[test]
fn on_opencode_with_something_finished_never_says_no_tool_call_was_open() {
    let (_, d) = draw(&[stopped("Summarize the turns", "opencode", &read_tool())], "opencode", false, false);
    assert!(!d.has("No tool call was open"), "{:?}", d.texts);
    assert!(d.has("An unreported OpenCode command may have been running when stopped."), "{:?}", d.texts);
}

// case: does not, when a tool had run
#[test]
fn does_not_when_a_tool_had_run() {
    let (_, d) = draw(&[stopped("Summarize the turns", "claude", &read_tool())], "claude", false, false);
    assert!(d.has("This run was stopped"), "{:?}", d.texts);
    assert!(!d.has("Stopped before it used any tools"), "{:?}", d.texts);
    assert!(!d.has("Send again"), "{:?}", d.texts);
}

// case: does not, when the run was not stopped
#[test]
fn does_not_when_the_run_was_not_stopped() {
    let (_, d) = draw(&[said("Summarize the turns", "Two turns.")], "claude", false, false);
    assert!(!d.has("Send again"), "{:?}", d.texts);
}

// case: says nothing of undoing changes that were never made (0.496)
#[test]
fn says_nothing_of_undoing_changes_that_were_never_made() {
    let (_, d) = draw(&[stopped("Summarize the turns", "claude", &[])], "claude", false, false);
    assert!(!d.has("undoing a change is yours to do"), "{:?}", d.texts);
    // A run that did something still says it.
    let started = [("tool.started", json!({ "itemId": "c1", "toolKind": "file_read", "name": "Read", "command": "a.txt" }))];
    let (_, d) = draw(&[stopped("Summarize the turns", "claude", &started)], "claude", false, false);
    assert!(d.has("undoing a change is yours to do"), "{:?}", d.texts);
}

// case: draws the message once: the stopped copy gives way to the one sent again
#[test]
fn draws_the_message_once_the_stopped_copy_gives_way_to_the_one_sent_again() {
    let (_, d) = draw(&[stopped("Summarize the turns", "claude", &[]), said("Summarize the turns", "Two turns.")], "claude", false, false);
    assert_eq!(d.count("Summarize the turns"), 1, "{:?}", d.texts);
    // The stopped copy's card went with it.
    assert!(!d.has("Stopped before it used any tools") && !d.has("Stopped before it replied"), "{:?}", d.texts);
    assert!(d.has("Two turns."), "{:?}", d.texts);
}

// case: and says no fresh session began when nothing before it is lost
#[test]
fn and_says_no_fresh_session_began_when_nothing_before_it_is_lost() {
    let (_, d) = draw(&[stopped("Summarize the turns", "claude", &[]), said("Summarize the turns", "Two turns.")], "claude", true, false);
    assert!(!d.has("A fresh session"), "{:?}", d.texts);
    // It still says so when a turn before it is on screen.
    let (_, d) = draw(&[said("Start here", "Started."), stopped("Summarize the turns", "claude", &[]), said("Summarize the turns", "Two turns.")], "claude", true, false);
    assert!(d.has("A fresh session"), "{:?}", d.texts);
}

// case: keeps a stopped turn that said something, or that was followed by other words
#[test]
fn keeps_a_stopped_turn_that_said_something_or_that_was_followed_by_other_words() {
    let spoke = stopped("Summarize the turns", "claude", &[("message.delta", json!({ "itemId": "m1", "text": "Starting", "final": true }))]);
    let (_, d) = draw(&[spoke, said("Summarize the turns", "Two turns.")], "claude", false, false);
    assert_eq!(d.count("Summarize the turns"), 2, "{:?}", d.texts);
    // Kept, it is not the newest turn: Send again is the newest turn's alone.
    assert!(!d.has("Send again"), "{:?}", d.texts);
    let (_, d) = draw(&[stopped("Summarize the turns", "claude", &[]), said("Something else", "Done.")], "claude", false, false);
    assert_eq!(d.count("Summarize the turns"), 1, "{:?}", d.texts);
    assert!(d.has("Something else"), "{:?}", d.texts);
}

// case: says a kept turn that was stopped before it replied was stopped, and only that one
#[test]
fn says_a_kept_turn_that_was_stopped_before_it_replied_was_stopped_and_only_that_one() {
    let (_, d) = draw(&[stopped("Summarize the turns", "claude", &[]), said("Something else", "Done.")], "claude", false, false);
    assert!(d.has("Stopped before it replied"), "{:?}", d.texts);
    // The kept turn's note is not the newest turn's receipt: no Send again under it.
    assert!(!d.has("Send again"), "{:?}", d.texts);
    let (_, d) = draw(&[stopped("Summarize the turns", "claude", &[]), said("Summarize the turns", "Two turns.")], "claude", false, false);
    assert!(!d.has("Stopped before it replied"), "{:?}", d.texts);
    let spoke = stopped("Summarize the turns", "claude", &[("message.delta", json!({ "itemId": "m1", "text": "Starting", "final": true }))]);
    let (_, d) = draw(&[spoke, said("Something else", "Done.")], "claude", false, false);
    assert!(!d.has("Stopped before it replied"), "{:?}", d.texts);
}

// case: offers Continue on the failure card, saying it was the provider
#[test]
fn offers_continue_on_the_failure_card_saying_it_was_the_provider() {
    let failed = json!({ "type": "run.failed", "payload": { "message": "Selected model is at capacity. Please try a different model." } });
    let offers = locust_core::failure_words::offers_continue([&failed], false);
    let (_, d) = draw(&[said("Summarize the turns", "Working on it.")], "claude", false, offers);
    assert!(d.has("The model's servers were busy: not your account, and nothing you did."), "{:?}", d.texts);
    assert!(d.key("busy-continue-action") && d.has("Continue"), "{:?}", d.texts);
}

// case: offers nothing of the kind without it
#[test]
fn offers_nothing_of_the_kind_without_it() {
    let failed = json!({ "type": "run.failed", "payload": { "message": "Error: ENOENT: no such file" } });
    let offers = locust_core::failure_words::offers_continue([&failed], false);
    let (_, d) = draw(&[said("Summarize the turns", "Working on it.")], "claude", false, offers);
    assert!(!d.has("servers were busy"), "{:?}", d.texts);
}

/// A turn on `runtime`/`model` that ended failed with `payload`, its snapshot read as the ledger reads it.
fn failed_turn(runtime: &str, model: &str, payload: Value, tweak: impl FnOnce(&mut State)) -> (rune::Runtime<State>, Drawn) {
    let events = [("run.started", json!({})), ("run.failed", payload)];
    let records: Vec<Value> = events.iter().map(|(kind, payload)| json!({ "type": kind, "payload": payload })).collect();
    let (next_model, sign_in) = (locust_core::failure_words::offers_next_model(&records, false), locust_core::failure_words::offers_sign_in(&records, false));
    let (on, runtime, model) = (runtime, runtime.to_owned(), model.to_owned());
    draw_whole(&[turn("Summarize the turns", on, &events)], on, move |s| {
        let pane = s.ledger.as_mut().unwrap();
        pane.snapshot.next_model = next_model;
        pane.snapshot.sign_in = sign_in;
        for meta in &mut pane.snapshot.turn_metas {
            meta.runtime = runtime.clone();
            meta.model = model.clone();
        }
        let models = ["opencode/glm-5.3-flash-free", "opencode/beam-free", "opencode/longcat-2.5-preview-free", "opencode/big-pickle"]
            .map(|id| locust_core::engine::Model { id: id.into(), name: id.into(), efforts: Vec::new() })
            .to_vec();
        s.models.insert("tm_codec".into(), vec![locust_core::engine::Models { teammate: None, runtime: "opencode".into(), model: None, effort: None, models }]);
        tweak(s);
    })
}

fn press(rt: &mut rune::Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..40 {
        rt.frame(0.05);
    }
}

const RATE_LIMITED: &str = r#"timestamp=2026-09-29T03:40:47.596Z level=ERROR message="stream error" error.error="AI_APICallError: Rate limit exceeded""#;

// case: offers the next model on the failure card
#[test]
fn offers_the_next_model_on_the_failure_card() {
    let payload = json!({ "kind": "unknown", "message": "OpenCode ended. The runtime reported that it is out of capacity right now.", "process": { "stderr": RATE_LIMITED } });
    let (mut rt, d) = failed_turn("opencode", "opencode/beam-free", payload, |s| s.draft.replace_text("half a thought"));
    // Rust's failure card is the error line under the turn (no "The run could not continue" heading): it is drawn.
    assert!(d.has("out of capacity right now"), "{:?}", d.texts);
    assert!(d.has("Switch to Longcat 2.5 Preview Free") && d.key("next-model-action"), "{:?}", d.texts);
    assert!(d.has("puts your message back in the chat box"), "{:?}", d.texts);
    assert!(!d.has("servers were busy"), "{:?}", d.texts);
    // The press moves the teammate's next turn there and puts the words back in the chat box; nothing is sent.
    press(&mut rt, "next-model-action");
    let s = &rt.state;
    assert_eq!(s.picks.get("tm_codec").map(|p| (p.runtime.as_deref(), p.model.as_str())), Some((Some("opencode"), "opencode/longcat-2.5-preview-free")));
    assert_eq!(s.draft.text, "Summarize the turns");
    assert!(s.flight.is_none() && !s.sending.is_pending());
    // A paid model, or one with no other free model listed, offers no next model.
    let paid = json!({ "kind": "unknown", "message": "OpenCode ended.", "process": { "stderr": RATE_LIMITED } });
    let (_, d) = failed_turn("opencode", "opencode/big-pickle", paid, |_| {});
    assert!(!d.key("next-model-action"), "{:?}", d.texts);
    // Not for a failure that is not its limit.
    let (_, d) = failed_turn("opencode", "opencode/beam-free", json!({ "kind": "unknown", "message": "OpenCode ended.", "process": { "stderr": "Error: ENOENT: no such file" } }), |_| {});
    assert!(!d.key("next-model-action") && !d.has("Switch to"), "{:?}", d.texts);
}

// case: offers that runtime's sign-in on the failure card
#[test]
fn offers_that_runtimes_sign_in_on_the_failure_card() {
    let payload = json!({ "kind": "authentication-failed", "message": "Your access token could not be refreshed. Please log out and sign in again." });
    let (mut rt, d) = failed_turn("codex", "gpt-5", payload, |_| {});
    assert!(d.has("is signed out. Sign in again, then send your message."), "{:?}", d.texts);
    assert!(d.has("Codex CLI is signed out"), "{:?}", d.texts);
    assert!(d.key("sign-in-action") && d.has("Sign in"), "{:?}", d.texts);
    // The press opens that runtime's own sign-in, and the card says to finish there.
    press(&mut rt, "sign-in-action");
    assert_eq!(rt.state.agents_signed_in.len(), 1, "{:?}", rt.state.agents_signed_in);
    assert!(rt.state.agents_signed_in[0].starts_with("codex"), "{:?}", rt.state.agents_signed_in);
    let d = drawn(&rt);
    assert!(!d.key("sign-in-action") && d.has(&locust_core::agents::sign_in_opened_line("codex", false)), "{:?}", d.texts);
}

// case: offers nothing of the kind otherwise
#[test]
fn offers_nothing_of_the_kind_otherwise() {
    let (_, d) = failed_turn("codex", "gpt-5", json!({ "kind": "unknown", "message": "The model returned an unexpected response." }), |_| {});
    assert!(d.has("The model returned an unexpected response."), "{:?}", d.texts);
    assert!(!d.has("is signed out") && !d.key("sign-in-action"), "{:?}", d.texts);
    // A runtime with no sign-in of its own (OpenCode's free models need no account) offers none, signed out or not.
    let (_, d) = failed_turn("opencode", "opencode/beam-free", json!({ "kind": "authentication-failed", "message": "Unauthorized." }), |_| {});
    assert!(!d.has("is signed out") && !d.key("sign-in-action"), "{:?}", d.texts);
}
