//! Pressing Resume from checkpoint while another send or flight is already in progress must be ignored rather than
//! replacing the in-flight send or starting a duplicate resume.
use super::*;

struct Release(Option<std::sync::mpsc::Sender<()>>);
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(());
        }
    }
}

#[test]
fn resume_button_is_ignored_while_a_send_or_flight_is_in_progress() {
    let mut s = State::default();
    crate::scenes::recovery_scene(&mut s);
    s.engine = Some(Kind::Native);
    let tip = s.ledger.as_ref().unwrap().snapshot.tip.clone();
    let header = s.ledger.as_ref().unwrap().header.clone();
    s.ledger.as_mut().unwrap().snapshot.turn_metas.push(ledger::Metadata {
        mission_id: tip.clone(),
        runtime: "codex".into(),
        ..Default::default()
    });
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let release = Release(Some(tx));
    s.sending.request((), move |()| {
        let _ = rx.recv();
        Err("first send still pending".into())
    });
    assert!(s.sending.is_pending());
    assert!(s.flight.is_none());

    let mut rt = rune::Runtime::new(s, crate::view, (1100.0, 780.0), rune::Env::new(true, 1.0));
    for _ in 0..40 {
        rt.frame(0.05);
    }
    let (x, y) = rt
        .find("resume-from-checkpoint")
        .expect("resume button must be drawn when flight is cleared");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);

    assert!(
        rt.state.flight.is_none(),
        "pressing Resume while a send is pending must not start a new flight"
    );
    assert!(
        rt.state.turn_undo.resume_attempt.is_none(),
        "pressing Resume while a send is pending must not record a resume attempt"
    );
    assert_ne!(rt.state.status, "Sending…");

    // Release the pending send and also verify an active flight blocks a duplicate resume press.
    rt.state.sending.cancel();
    drop(release);
    rt.state.flight = Some(Flight {
        teammate_id: header.id.clone(),
        conversation_id: Some(tip.clone()),
        header: Some(header),
        after_tip: None,
        message: None,
    });
    resume(&mut rt.state, tip, "codex".into());
    assert!(
        rt.state.turn_undo.resume_attempt.is_none(),
        "a resume while a flight is active must not overwrite state or start another send"
    );
    assert!(!rt.state.sending.is_pending());
}
