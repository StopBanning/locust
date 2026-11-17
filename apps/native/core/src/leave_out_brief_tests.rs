//! Electron `main/a-person-can-leave-out-part-of-a-brief.test.ts` (0.527): a reply on another runtime is briefed from
//! sections, and the person may leave out the earlier messages, the finished steps and the last reply. The task, the
//! person's own words and the steps that never reported back may not go. One test per Electron case, its values and
//! words. The two bound cases at 3,773 and 3,800 are Electron's own answers (`composeHandoffPrompt`, run from this
//! tree), so the notice's charge against the budget is pinned.

use crate::carry_on::{chosen_leave_out, compose_handoff_prompt_with, EarlierTurn, MAX_HANDOFF_PROMPT_LENGTH};
use serde_json::{json, Value};

fn checkpoint() -> Value {
    json!({
        "missionId": "mission_prior",
        "epoch": 2,
        "reason": "route-switch",
        "resumeSafety": "safe",
        "safetyReason": "settled",
        "createdAt": "2026-10-01T17:00:00.000Z",
        "unsettledActions": [],
        "settledActions": ["write src/app.ts"],
        "settledNames": ["write src/app.ts"],
        "assistantSummary": "I moved the session check after the cookie is set."
    })
}

fn earlier() -> Vec<EarlierTurn> {
    vec![EarlierTurn { asked: "Fix the login redirect loop".into(), answered: Some("Fixed by moving the session check.".into()) }]
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn the_last_reply_is_left_out_when_asked_and_the_brief_says_the_person_chose_it() {
    let briefing = compose_handoff_prompt_with(
        "Fix the login redirect loop",
        &checkpoint(),
        "Codex CLI",
        Some("Now do the signup form"),
        &earlier(),
        None,
        &names(&["summary"]),
    )
    .expect("a briefing");
    assert!(!briefing.prompt.contains("I moved the session check after the cookie is set."), "{}", briefing.prompt);
    assert!(briefing.prompt.contains("Now do the signup form"));
    assert!(briefing.prompt.ends_with("\n\n(The person chose to leave out its last reply.)"), "{}", briefing.prompt);
    assert_eq!(briefing.kept, names(&["task", "earlier", "settled"]));
    assert_eq!(briefing.left_out_by_you, names(&["summary"]));
    assert!(briefing.omitted.is_empty());
}

#[test]
fn every_section_it_offers_can_be_left_out_at_once_and_the_task_and_the_reply_stay() {
    let briefing = compose_handoff_prompt_with(
        "Fix the login redirect loop",
        &checkpoint(),
        "Codex CLI",
        Some("Now do the signup form"),
        &earlier(),
        None,
        &names(&["earlier", "settled", "summary"]),
    )
    .expect("a briefing");
    assert_eq!(briefing.kept, names(&["task"]));
    assert_eq!(briefing.left_out_by_you, names(&["earlier", "settled", "summary"]));
    assert!(briefing.prompt.contains("Fix the login redirect loop"));
    assert!(briefing.prompt.contains("Now do the signup form"));
    // Electron's words for all three, in reading order whatever order they were sent in.
    let reordered =
        compose_handoff_prompt_with("Fix it", &checkpoint(), "Codex CLI", Some("Again"), &[EarlierTurn { asked: "a".into(), answered: Some("b".into()) }], None, &names(&["summary", "earlier", "settled"]))
            .unwrap();
    assert_eq!(reordered.left_out_by_you, names(&["earlier", "settled", "summary"]));
    assert!(
        reordered.prompt.ends_with("Again\n\n(The person chose to leave out the earlier messages, the steps it finished and its last reply.)"),
        "{}",
        reordered.prompt
    );
}

#[test]
fn the_task_the_reply_and_the_steps_that_never_reported_back_are_never_left_out_whatever_is_sent() {
    let mut unsettled = checkpoint();
    unsettled["unsettledActions"] = json!([{"toolKind": "command", "name": "npm run migrate"}]);
    let briefing = compose_handoff_prompt_with(
        "Fix the login redirect loop",
        &unsettled,
        "Codex CLI",
        Some("Now do the signup form"),
        &earlier(),
        None,
        &names(&["task", "next", "unsettled"]),
    )
    .expect("a briefing");
    assert!(briefing.left_out_by_you.is_empty());
    assert!(briefing.prompt.contains("npm run migrate"));
    assert!(briefing.prompt.contains("Fix the login redirect loop"));
    assert!(briefing.prompt.contains("Now do the signup form"));
    assert!(!briefing.prompt.contains("chose to leave out"), "{}", briefing.prompt);
}

#[test]
fn only_what_was_there_to_leave_out_is_named() {
    let mut no_summary = checkpoint();
    no_summary["assistantSummary"] = json!("");
    let briefing = compose_handoff_prompt_with("Fix it", &no_summary, "Codex CLI", Some("Again"), &[], None, &names(&["summary", "earlier"])).unwrap();
    assert!(briefing.left_out_by_you.is_empty());
    assert!(!briefing.prompt.contains("chose to leave out"), "{}", briefing.prompt);
}

#[test]
fn the_bound_still_holds_when_sections_are_left_out_by_choice_and_others_must_go_to_fit() {
    let long = "x".repeat(3_000);
    let turns: Vec<EarlierTurn> = (0..30).map(|i| EarlierTurn { asked: format!("{i} {long}"), answered: Some(long.clone()) }).collect();
    let mut cp = checkpoint();
    cp["assistantSummary"] = json!(long);
    let briefing = compose_handoff_prompt_with("Fix it", &cp, "Codex CLI", Some(&"y".repeat(3_900)), &turns, None, &names(&["settled"])).expect("a briefing");
    assert!(briefing.prompt.encode_utf16().count() <= MAX_HANDOFF_PROMPT_LENGTH);
    assert_eq!(briefing.left_out_by_you, names(&["settled"]));
}

#[test]
fn a_section_that_did_not_fit_is_named_first_and_the_persons_choice_last() {
    // Electron's answer: a 6,000-character reply leaves no room for a 3,000-character last reply, so it is left out
    // for room, and the person's own choice is said after that, at 6,357 characters in all.
    let mut cp = checkpoint();
    cp["assistantSummary"] = json!("x".repeat(3_000));
    let turns = [EarlierTurn { asked: "a".into(), answered: Some("b".into()) }];
    let briefing = compose_handoff_prompt_with("Fix it", &cp, "Codex CLI", Some(&"y".repeat(6_000)), &turns, None, &names(&["settled"])).expect("a briefing");
    assert_eq!(briefing.kept, names(&["task", "earlier"]));
    assert_eq!(briefing.omitted, names(&["summary"]));
    assert_eq!(briefing.prompt.encode_utf16().count(), 6_357);
    assert!(
        briefing.prompt.ends_with("yy\n\n(Some handoff detail did not fit and was left out: summary.)\n\n(The person chose to leave out the steps it finished.)"),
        "{}",
        briefing.prompt
    );
}

#[test]
fn the_persons_notice_is_charged_against_the_bound_so_the_task_gives_way_for_it() {
    // Electron's answers: a 3,900-character task beside a reply of 3,773 fits whole, at exactly 7,880; a reply of
    // 3,800 clips the task, because the notice's 49 characters are held back first.
    let mut cp = checkpoint();
    cp["settledActions"] = json!([]);
    cp["settledNames"] = json!([]);
    let task = "t".repeat(3_900);
    let at = |n: usize| compose_handoff_prompt_with(&task, &cp, "Codex CLI", Some(&"n".repeat(n)), &[], None, &names(&["summary"])).expect("a briefing");
    let whole = at(3_773);
    assert!(!whole.task_clipped);
    assert_eq!(whole.prompt.encode_utf16().count(), 7_880);
    let clipped = at(3_800);
    assert!(clipped.task_clipped, "the notice's room comes out of the task");
    assert_eq!(clipped.prompt.encode_utf16().count(), 7_879);
    assert!(clipped.prompt.ends_with("\n\n(The person chose to leave out its last reply.)"));
}

#[test]
fn what_the_window_sent_is_kept_to_the_sections_that_may_be_left_out() {
    assert_eq!(chosen_leave_out(&json!(["summary", "task", "unsettled", "next", "earlier", 7, null])), names(&["earlier", "summary"]));
    assert!(chosen_leave_out(&json!("summary")).is_empty());
    assert!(chosen_leave_out(&Value::Null).is_empty());
}

#[test]
fn the_engine_wire_keeps_only_what_may_be_left_out() {
    // `chosenLeaveOut` at Electron's index.ts, where the window's payload comes in.
    let origin = crate::engine_process::origin_from(&json!({"leftOutByYou": ["task", "summary", "next"]})).unwrap();
    assert_eq!(origin.left_out_by_you, names(&["summary"]));
}
