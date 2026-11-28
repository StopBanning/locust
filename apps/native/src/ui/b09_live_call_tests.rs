use super::*;
use rune::{Env, Runtime};

struct Peer;
impl AudioPeer for Peer {
    fn offer(&self) -> &str { "v=0" }
    fn accept(&mut self, _: &str) -> Result<(), String> { Ok(()) }
}

fn fixture() -> (State, mpsc::Sender<CallEvent>) {
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        header: Teammate { id: "tm_ada".into(), name: "Ada".into(), role: "Engineer".into(), runtime: Some("codex".into()), model: None, account: None, busy: false, runs_in: "ask".into(), folder: Some("/test".into()) },
        conversation: Conversation { conversation_id: Some("m_typed".into()), ..Default::default() },
        items: vec![ledger::Item::User("Before the call".into())],
        dir: Default::default(), worker: None,
        snapshot: ledger::Snapshot { tip: "m_typed".into(), turn_metas: vec![ledger::Metadata { mission_id: "m_typed".into(), ..Default::default() }], ..Default::default() },
        cold_tip: None, due: 0., loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.,
    });
    let (_, started) = mpsc::channel();
    let (tx, events) = mpsc::channel();
    s.voice.call = Some(Talk { cancel: Arc::default(), target: conversation_draft_key(&s),
        conversation: Some("m_typed".into()), state: TalkState::Connected(Box::new(Peer), CallWindow { started, events }, true) });
    s.voice.words = Some(LiveWords::new("tm_ada".into(), "Ada".into(), Some("m_typed".into()), 1_000));
    (s, tx)
}

fn thread(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, |s, _| crate::messages::message_column(
        &s.ledger.as_ref().unwrap().items, "Ada", &Palette::new(true), s), (1000., 1700.), Env::new(true, 1.));
    settle(&mut rt);
    rt
}
fn settle(rt: &mut Runtime<State>) { for _ in 0..10 { rt.frame(0.05); } }
fn poll(rt: &mut Runtime<State>) {
    rt.update(|s, tx| { poll_call(s, 1., tx); });
    settle(rt);
}
fn words(rt: &Runtime<State>) -> String {
    let mut all = rt.elements();
    all.sort_by(|a,b| a.rect[1].total_cmp(&b.rect[1]));
    all.into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

// case: live: opens with its divider and your words and theirs, each marked as said
#[test]
fn b09_call_words_live_divider_and_spoken_lines() {
    let (s, tx) = fixture();
    let mut rt = thread(s);
    tx.send(CallEvent::Said("you", "Ada, check the readme for the secret ingredient.".into())).unwrap();
    tx.send(CallEvent::Said("voice", "Let me check.".into())).unwrap();
    tx.send(CallEvent::Partial("voice", "Still looking".into())).unwrap();
    poll(&mut rt);
    let said = words(&rt);
    assert!(said.to_ascii_lowercase().contains("call with ada"), "{said}");
    assert!(said.contains("Ada, check the readme for the secret ingredient.") && said.contains("Let me check."), "{said}");
    assert_eq!(said.matches("· Said").count(), 2, "{said}");
    assert!(said.contains("Still looking") && !said.contains("CALL ENDED") && !said.contains("Show what was said"), "{said}");
    rt.update(|s, _| s.ledger.as_mut().unwrap().conversation.conversation_id = Some("m_else".into()));
    settle(&mut rt);
    assert!(!words(&rt).contains("Let me check."), "another thread cannot show this call");
}

#[path = "b09_call_wire_tests.rs"]
mod b09_call_wire_tests;

fn ask_question(fields: Vec<(String, String)>) -> CallEvent {
    CallEvent::Asks { id: "ask_1".into(), question: locust_core::runtime::codex_voice::ConnectorQuestion {
        what: "locust probe: send note".into(), detail: "text: hello".into(), fields,
    }}
}

// case: is an approval card under what was said: the action, each value, what leaves, and Allow once or Deny
#[test]
fn b09_call_words_connector_is_under_its_spoken_line() {
    let (s, tx) = fixture();
    let mut rt = thread(s);
    tx.send(CallEvent::Said("you", "Yes, send it.".into())).unwrap();
    tx.send(ask_question(vec![("text".into(), "hello".into())])).unwrap();
    poll(&mut rt);
    let said = words(&rt);
    assert!(said.find("Yes, send it.").unwrap() < said.find("Approval required").unwrap(), "{said}");
    for expected in ["On the call with Ada", "locust probe: send note", "Text", "hello", "The input above, to the service the connector reaches. It acts there, not on this machine.", "Unknown — a connector acts on the service it reaches, and Locust cannot undo what happens there.", "Allow once", "Deny", "Saying yes on the call does not allow it"] {
        assert!(said.contains(expected), "{expected}: {said}");
    }
}

// case: says what it would do when Codex named no values, and asks nothing once the call has ended
#[test]
fn b09_call_words_connector_keeps_detail_and_leaves_when_the_call_ends() {
    let (s, tx) = fixture();
    let mut rt = thread(s);
    tx.send(ask_question(Vec::new())).unwrap();
    poll(&mut rt);
    assert!(words(&rt).contains("text: hello"));
    tx.send(CallEvent::Ended(None)).unwrap();
    poll(&mut rt);
    assert!(!words(&rt).contains("Approval required"));
}

// case: answers through the call that is on, and only through it
#[test]
fn b09_call_words_connector_routes_only_the_current_pending_answer() {
    let (s, tx) = fixture();
    let mut rt = thread(s);
    tx.send(ask_question(Vec::new())).unwrap();
    poll(&mut rt);
    let now = std::time::Instant::now();
    let mut answers = Vec::new();
    rt.update(|s, _| {
        assert!(!forward_answer(s, "ask_1", true, now, |id, a| answers.push((id,a))), "fresh card does not allow");
        assert!(!forward_answer(s, "not_asked", true, now + Duration::from_secs(1), |id, a| answers.push((id,a))));
    });
    rt.update(|s, _| {
        s.ledger.as_mut().unwrap().conversation.conversation_id = Some("m_else".into());
        assert!(!forward_answer(s, "ask_1", true, now + Duration::from_secs(1), |id, a| answers.push((id,a))), "other thread cannot answer");
        s.ledger.as_mut().unwrap().conversation.conversation_id = Some("m_typed".into());
        assert!(forward_answer(s, "ask_1", false, now + Duration::from_secs(1), |id, a| answers.push((id,a))));
        assert!(!forward_answer(s, "ask_1", true, now + Duration::from_secs(1), |id, a| answers.push((id,a))), "already sent once");
    });
    tx.send(CallEvent::Ended(None)).unwrap();
    poll(&mut rt);
    rt.update(|s, _| { assert!(!forward_answer(s, "ask_1", true, now + Duration::from_secs(1), |id,a| answers.push((id,a)))); });
    assert_eq!(answers, [("ask_1".to_owned(), false)]);
}

// case: is never kept with the call
#[test]
fn b09_call_words_connector_is_not_in_the_saved_transcript() {
    let (s, tx) = fixture();
    let mut rt = thread(s);
    tx.send(CallEvent::Said("you", "Yes, send it.".into())).unwrap();
    tx.send(ask_question(Vec::new())).unwrap();
    poll(&mut rt);
    let live = rt.state.voice.words.take().unwrap();
    let kept = live.transcript.finish(9_000);
    assert!(kept.get("asks").is_none());
    assert!(!kept.to_string().contains("text: hello"));
    assert_eq!(kept["lines"][0]["text"], "Yes, send it.");
}

fn sidebar_fixture() -> (Runtime<State>, mpsc::Sender<CallEvent>) {
    let (mut s, events) = fixture();
    s.demo = true;
    for (id, parent, at) in [
        ("m_typed", None, "2026-10-11T04:00:00.000Z"),
        ("m_asked", Some("m_typed"), "2026-10-11T05:00:00.000Z"),
        ("m_other", None, "2026-10-11T05:00:00.000Z")
    ] {
        s.recent.push(ConversationRow { id:id.into(), title:format!("Conversation {id}"), teammate:"Wren".into(),
            started_by:"native".into(), state:"completed".into(), updated:at.into() });
        s.ledger_summaries.insert(id.into(), ledger::Summary { mission_id:id.into(), parent:parent.map(Into::into),
            created_at:at.into(), ..Default::default() });
    }
    let mut rt = Runtime::new(s, |s, _| crate::sidebar::sidebar(s, &Palette::new(true)), (320., 900.), Env::new(true, 1.));
    settle(&mut rt);
    (rt, events)
}
fn sidebar_marks(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting && e.text.as_deref() == Some("on a call"))
        .filter_map(|e| e.key).collect()
}

// case: says "on a call" on the row whose turns the call is among, and on no other
#[test]
fn b09_call_words_sidebar_marks_only_the_calls_conversation() {
    let (rt, _events) = sidebar_fixture();
    assert_eq!(sidebar_marks(&rt), ["conversation-on-call-m_asked"]);
}

// case: is gone once the call has ended, and was never there without one
#[test]
fn b09_call_words_sidebar_drops_the_mark_at_call_end() {
    let (mut rt, events) = sidebar_fixture();
    assert_eq!(sidebar_marks(&rt), ["conversation-on-call-m_asked"]);
    events.send(CallEvent::Ended(None)).unwrap();
    poll(&mut rt);
    assert!(sidebar_marks(&rt).is_empty(), "ended call must no longer mark the row");
    let (mut never, _events) = sidebar_fixture();
    never.update(|s, _| s.voice.call = None);
    settle(&mut never);
    assert!(sidebar_marks(&never).is_empty());
}

// case: keys the call by where it is, not by what is being said
#[test]
fn b09_call_words_sidebar_identity_does_not_follow_spoken_text() {
    let (mut rt, events) = sidebar_fixture();
    let before = sidebar_marks(&rt);
    events.send(CallEvent::Partial("you", "and one more thing".into())).unwrap();
    events.send(CallEvent::Said("you", "Hi".into())).unwrap();
    poll(&mut rt);
    assert_eq!(sidebar_marks(&rt), before);
    assert_eq!(before, ["conversation-on-call-m_asked"]);
}
