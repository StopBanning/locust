//! A resume refusal stays visible as an alert on its mission even if the recovery offer clears.
use super::*;

fn render(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, crate::view, (1100.0, 780.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.05);
    rt
}

#[test]
fn resume_refusal_stays_visible_when_offer_is_gone() {
    let mut s = State::default();
    crate::scenes::recovery_scene(&mut s);
    let tip = s.ledger.as_ref().unwrap().snapshot.tip.clone();
    s.ledger.as_mut().unwrap().snapshot.recovery = None;
    let refusal = Arc::new(OnceLock::new());
    let _ = refusal.set("The mission could not be reconciled, so nothing was resumed.".into());
    s.turn_undo.resume_attempt = Some(Attempt {
        mission: tip,
        refusal,
    });

    let mut rt = render(s);
    assert!(
        rt.find("resume-refusal").is_some(),
        "the refusal alert must still appear when the recovery offer is gone"
    );
    let alert = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("resume-refusal"))
        .expect("resume-refusal element");
    assert_eq!(alert.role, "alert");
    assert_eq!(
        alert.text.as_deref(),
        Some("Not resumed: The mission could not be reconciled, so nothing was resumed.")
    );

    // Moving to a different mission tip clears the refusal alert.
    rt.update(|s, _| s.ledger.as_mut().unwrap().snapshot.tip = "another_mission".into());
    rt.frame(0.1);
    assert!(rt.find("resume-refusal").is_none());
}
