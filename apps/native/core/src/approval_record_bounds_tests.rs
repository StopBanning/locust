use super::*;

// case: is bounded by the host the way the reader accepts it
#[test]
fn a_card_answer_is_bounded_to_the_ledger_fields() {
    let mut request = Request {
        approval_id: "ap_1".into(), mission_id: "mission_1".into(), runtime: "codex".into(),
        kind: Kind::Command, summary: "x".repeat(5_000), detail: String::new(), cwd: None,
        requested_at: "2026-10-04T01:00:00.000Z".into(), always_covers: None, always_key: None,
        data_sent_says: None, reversible_says: None, patch: None, skippable: false, answer_in: None,
        claude_input: None, questions: Vec::new(),
    };
    let deny = |words: String| Answer::Decision {
        approval_id: "ap_1".into(), decision: Decision::Deny, reason: Some(words),
    };
    let bounded = record_of(&request, &deny(format!("  {}  ", "y".repeat(3_000)))).to_json();
    assert_eq!(bounded["asked"], "x".repeat(2_000));
    assert_eq!(bounded["words"], "y".repeat(1_000));
    crate::ledger_write::validate_approval(&bounded).unwrap();
    request.summary = "   ".into();
    let empty = record_of(&request, &deny("  ".into())).to_json();
    assert_eq!(empty["asked"], "(the card said nothing more)");
    assert!(empty.get("words").is_none());
    crate::ledger_write::validate_approval(&empty).unwrap();
}
