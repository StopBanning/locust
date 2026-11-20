//! Electron's `test/handoff.test.ts` (8 cases), with Electron's values: the brief one AI agent hands the next when a
//! turn stops. Rust's composer also carries earlier turns (`earlier`), which Electron's file never passes; none here.
use super::*;
use serde_json::{Value, json};

fn checkpoint(overrides: Value) -> Value {
    let mut base = json!({
        "schemaVersion": 1, "missionId": "mission_a", "runId": "run_a", "epoch": 1, "reason": "route-switch",
        "reconciledThroughSequence": 12, "settledActions": [], "unsettledActions": [], "resumeSafety": "safe",
        "safetyReason": "Nothing was in flight.", "transcriptDigest": "digest", "assistantSummary": "",
        "createdAt": "2026-09-01T00:00:00.000Z"
    });
    for (k, v) in overrides.as_object().cloned().unwrap_or_default() {
        base[k] = v;
    }
    base
}

fn unsettled(name: &str) -> Value {
    json!({"itemId": format!("item_{name}"), "toolKind": "command", "name": name, "startedAt": "2026-09-01T00:00:00.000Z", "startedAtSequence": 4})
}

fn compose(task: &str, checkpoint: &Value) -> Option<Briefing> {
    compose_handoff_prompt(task, checkpoint, "Codex", None, &[], None)
}

/// Electron measures in UTF-16 units.
fn len(s: &str) -> usize {
    s.encode_utf16().count()
}

#[test]
fn carries_the_original_task_and_names_the_runtime_that_stopped() {
    let brief = compose("Refactor the parser.", &checkpoint(json!({}))).unwrap();
    assert!(brief.prompt.contains("Refactor the parser."));
    assert!(brief.prompt.contains("Codex"));
    assert!(brief.omitted.is_empty());
}

#[test]
fn tells_the_new_runtime_to_verify_actions_that_never_reported_back() {
    let brief = compose("Refactor the parser.", &checkpoint(json!({"resumeSafety": "approval-required", "unsettledActions": [unsettled("rm -rf build")]}))).unwrap();
    assert!(brief.prompt.contains("rm -rf build"));
    assert!(brief.prompt.to_lowercase().contains("unknown"), "{}", brief.prompt);
    assert!(brief.prompt.to_lowercase().contains("before you redo or build on any of them"), "{}", brief.prompt);
}

#[test]
fn separates_what_finished_from_what_did_not() {
    let brief = compose("Task.", &checkpoint(json!({"settledActions": ["call_1"], "settledNames": ["read src/index.ts"], "unsettledActions": [unsettled("write src/index.ts")]}))).unwrap();
    let (unsettled_at, settled_at) = (brief.prompt.find("write src/index.ts").unwrap(), brief.prompt.find("read src/index.ts").unwrap());
    assert!(unsettled_at < settled_at, "the unsettled half comes first");
}

#[test]
fn drops_the_summary_before_the_unsettled_actions_when_space_runs_out() {
    let brief = compose("Task.", &checkpoint(json!({
        "assistantSummary": "x".repeat(4_000),
        "settledActions": ["call_1"],
        "settledNames": [format!("settled {}", "y".repeat(3_800))],
        "unsettledActions": [unsettled("the dangerous one")]
    }))).expect("a brief");
    assert!(brief.prompt.contains("the dangerous one"));
    assert!(brief.omitted.iter().any(|o| o == "summary"), "{:?}", brief.omitted);
    assert!(len(&brief.prompt) <= MAX_HANDOFF_PROMPT_LENGTH);
}

#[test]
fn says_so_when_detail_was_left_out_rather_than_reading_as_complete() {
    let brief = compose("Task.", &checkpoint(json!({"assistantSummary": "x".repeat(7_999)}))).unwrap();
    assert_eq!(brief.omitted, vec!["summary".to_owned()]);
    assert!(brief.prompt.contains("did not fit and was left out"), "{}", brief.prompt);
}

#[test]
fn reserves_enough_room_for_the_longest_possible_omission_notice() {
    let brief = compose(&"a".repeat(MAX_HANDOFF_PROMPT_LENGTH - 400), &checkpoint(json!({
        "assistantSummary": "x".repeat(3_000),
        "settledActions": ["call_1"],
        "settledNames": [format!("settled {}", "z".repeat(300))],
        "unsettledActions": [unsettled("unsettled thing")]
    }))).unwrap();
    assert_eq!(brief.omitted, vec!["unsettled".to_owned(), "settled".to_owned(), "summary".to_owned()]);
    assert!(len(&brief.prompt) <= MAX_HANDOFF_PROMPT_LENGTH);
}

#[test]
fn reserves_room_for_the_longest_notice_every_optional_section_could_produce() {
    // Every optional section this composer can drop, Rust's "earlier" included.
    let worst = omission_notice(&KEEP_ORDER);
    assert!(len(&worst) + 2 <= NOTICE_BUDGET, "{} > {NOTICE_BUDGET}: {worst}", len(&worst) + 2);
}

#[test]
fn refuses_rather_than_truncating_a_request_too_long_to_carry() {
    assert!(compose(&"a".repeat(MAX_HANDOFF_PROMPT_LENGTH), &checkpoint(json!({}))).is_none());
}
