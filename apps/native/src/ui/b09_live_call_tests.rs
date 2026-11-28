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
