//! Refusing Always on a card that must be asked about every time names the card's actual button ("Allow once"),
//! never "Approve once", and leaves the card waiting for the person's decision.
use super::*;

#[test]
fn always_refusal_on_asks_every_time_card_names_allow_once() {
    let ch = Channel::new();
    let (tx, rx) = mpsc::sync_channel(1);
    let req = Request {
        approval_id: "ap_browser_every_time".into(),
        mission_id: "m_browser_every_time".into(),
        runtime: "browser".into(),
        kind: Kind::Connector,
        summary: "Submit the form".into(),
        detail: "Click Submit".into(),
        cwd: None,
        requested_at: now_iso(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false,
        answer_in: None,
        claude_input: None,
        questions: Vec::new(),
    };
    assert!(asks_every_time(&req));
    crate::invariants::lock(&ch.inner).pending.insert(
        req.approval_id.clone(),
        Pending {
            request: req,
            tx,
            run_id: "run_browser_every_time".into(),
            item_id: None,
            steers_reason: false,
        },
    );

    let err = ch
        .decide(Answer::Decision {
            approval_id: "ap_browser_every_time".into(),
            decision: Decision::ApproveAlways,
            reason: None,
        })
        .unwrap_err();
    assert_eq!(
        err,
        "This action must be asked about every time. Choose Allow once or Deny."
    );
    // The card stays pending so the person can still choose Allow once or Deny.
    assert_eq!(ch.waiting().len(), 1);
    assert!(rx.try_recv().is_err());

    ch.decide(Answer::Decision {
        approval_id: "ap_browser_every_time".into(),
        decision: Decision::ApproveOnce,
        reason: None,
    })
    .unwrap();
    assert!(matches!(
        rx.try_recv(),
        Ok(Answer::Decision {
            decision: Decision::ApproveOnce,
            ..
        })
    ));
}
