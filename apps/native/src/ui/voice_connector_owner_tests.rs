use super::*;
use rune::{Env, Runtime};

struct Peer;
impl AudioPeer for Peer {
    fn offer(&self) -> &str {
        "v=0"
    }
    fn accept(&mut self, _: &str) -> Result<(), String> {
        Ok(())
    }
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..10 {
        rt.frame(0.05);
    }
}

#[test]
fn voice_connector_card_stays_off_other_threads() {
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        header: Teammate {
            id: "tm_ada".into(),
            name: "Ada".into(),
            role: "Engineer".into(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: Some("/test".into()),
        },
        conversation: Conversation {
            conversation_id: Some("m_typed".into()),
            ..Default::default()
        },
        items: vec![ledger::Item::User("Before the call".into())],
        dir: Default::default(),
        worker: None,
        snapshot: ledger::Snapshot {
            tip: "m_typed".into(),
            turn_metas: vec![ledger::Metadata {
                mission_id: "m_typed".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
        cold_tip: None,
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    let (_, started) = mpsc::channel();
    let (tx, events) = mpsc::channel();
    s.voice.call = Some(Talk {
        cancel: Arc::default(),
        target: conversation_draft_key(&s),
        conversation: Some("m_typed".into()),
        state: TalkState::Connected(Box::new(Peer), CallWindow { started, events }, true),
    });
    s.voice.words = Some(LiveWords::new(
        "tm_ada".into(),
        "Ada".into(),
        Some("m_typed".into()),
        1_000,
    ));

    let mut rt = Runtime::new(
        s,
        |s, _| {
            let p = Palette::new(true);
            col()
                .children(vec![crate::messages::message_column(
                    &s.ledger.as_ref().unwrap().items,
                    "Ada",
                    &p,
                    s,
                )])
                .children(notices(s, &p))
        },
        (1000.0, 1700.0),
        Env::new(true, 1.0),
    );
    tx.send(CallEvent::Asks {
        id: "ask_1".into(),
        question: locust_core::runtime::codex_voice::ConnectorQuestion {
            what: "locust probe: send note".into(),
            detail: "text: hello".into(),
            fields: vec![("text".into(), "hello".into())],
        },
    })
    .unwrap();
    rt.update(|s, tx| {
        poll_call(s, 1.0, tx);
    });
    settle(&mut rt);

    assert!(
        rt.find("voice-question-ask_1").is_some(),
        "the thread that owns the live call shows the connector approval card"
    );

    rt.update(|s, _| {
        s.ledger.as_mut().unwrap().conversation.conversation_id = Some("m_else".into());
    });
    settle(&mut rt);

    assert!(
        rt.find("voice-question-ask_1").is_none(),
        "another thread must not show the live call's connector approval card"
    );
    assert!(
        !rt.elements()
            .into_iter()
            .filter(|e| !e.exiting)
            .filter_map(|e| e.text)
            .any(|t| t.contains("Approval required")),
        "no connector approval text may leak onto another thread"
    );
}
