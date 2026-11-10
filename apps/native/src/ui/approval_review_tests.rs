use super::*;
#[test]
fn approval_preview_matches_its_teammate_and_fresh_cards_ignore_approval() {
    let mut s = State::default();
    approval_scene_dark(&mut s);
    assert_eq!(
        s.pending_approval.as_ref().unwrap().runtime,
        s.ledger
            .as_ref()
            .unwrap()
            .header
            .runtime
            .as_deref()
            .unwrap()
    );
    s.approval_shown = std::time::Instant::now();
    decide_pending(&mut s, approval::Decision::ApproveOnce);
    assert!(s.pending_approval.is_some());
    assert!(s.approval_notice.is_none());
    assert!(!approval::card_note(s.pending_approval.as_ref().unwrap()).contains("Keys"));
}
