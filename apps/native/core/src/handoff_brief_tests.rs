//! Electron `main/handoff.test.ts`: the briefing a continuation starts with, and what it says happened. One test per
//! Electron case, Electron's values and words.

use crate::carry_on::{compose_handoff_prompt, Briefing, EarlierTurn, MAX_HANDOFF_PROMPT_LENGTH};
use serde_json::{json, Value};

/// The test file's `checkpoint()`: a settled route switch that read the README.
fn checkpoint() -> Value {
    json!({
        "missionId": "mission_prior",
        "epoch": 2,
        "reason": "route-switch",
        "resumeSafety": "safe",
        "safetyReason": "settled",
        "createdAt": "2026-09-05T03:00:00.000Z",
        "unsettledActions": [],
        "settledActions": ["read README.md"],
        "assistantSummary": "I read the README and found two files to change."
    })
}

fn with(mut base: Value, key: &str, value: Value) -> Value {
    base[key] = value;
    base
}

fn brief(task: &str, checkpoint: &Value, from: &str, next: Option<&str>) -> Option<Briefing> {
    compose_handoff_prompt(task, checkpoint, from, next, &[], None)
}

fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

#[test]
fn a_briefing_carries_the_persons_next_instruction_last_as_the_latest_word() {
    let briefing = brief("check the google stock price", &checkpoint(), "Codex CLI", Some("now do it in euros")).expect("a briefing");
    let prompt = &briefing.prompt;
    assert!(prompt.contains("another agent (Codex CLI)"), "{prompt}");
    assert!(prompt.contains("check the google stock price"), "{prompt}");
    assert!(prompt.ends_with("The person now asks:\n\nnow do it in euros"), "{prompt}");
    assert!(prompt.find("I read the README").unwrap() < prompt.find("The person now asks").unwrap(), "{prompt}");
}

#[test]
fn a_briefing_with_no_next_instruction_is_the_live_handoffs_own_and_a_blank_one_counts_as_none() {
    let rescue = brief("check the google stock price", &checkpoint(), "Codex CLI", None).expect("a briefing");
    assert!(!rescue.prompt.contains("The person now asks"), "{}", rescue.prompt);
    assert_eq!(brief("check the google stock price", &checkpoint(), "Codex CLI", Some("   ")).unwrap().prompt, rescue.prompt);
}

#[test]
fn a_briefing_never_drops_the_next_instruction_to_make_room_it_refuses_instead() {
    let huge = "x".repeat(MAX_HANDOFF_PROMPT_LENGTH);
    let squeezed = brief("short task", &with(checkpoint(), "assistantSummary", json!(huge)), "Codex CLI", Some("and then this"))
        .expect("the summary gives way");
    assert!(squeezed.omitted.contains(&"summary".to_owned()), "{:?}", squeezed.omitted);
    assert!(squeezed.prompt.contains("and then this"));

    assert_eq!(brief("short task", &checkpoint(), "Codex CLI", Some(&huge)), None);
}

#[test]
fn a_long_original_task_is_clipped_to_carry_the_new_words_whole_and_says_how_much_it_cut() {
    let task = format!("Spec start. {} Spec end.", "a".repeat(3_900));
    let reply = format!("Now do this. {} The end.", "b".repeat(3_900));
    let carried = brief(&task, &checkpoint(), "Codex CLI", Some(&reply)).expect("clipped, not refused");
    assert!(utf16(&carried.prompt) <= MAX_HANDOFF_PROMPT_LENGTH, "{}", utf16(&carried.prompt));
    assert!(carried.prompt.contains(&reply));
    assert!(carried.prompt.contains("Spec start."));
    assert!(!carried.prompt.contains("Spec end."));
    let notice = "[The original task continues for ";
    let at = carried.prompt.find(notice).expect("the clip notice");
    let rest = &carried.prompt[at + notice.len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    assert!(!digits.is_empty(), "{rest}");
    assert!(
        rest[digits.len()..].starts_with(" more characters that did not fit here. Ask the person if the part above is not enough.]"),
        "{rest}"
    );
    assert!(carried.task_clipped);
}

#[test]
fn a_briefing_keeps_the_newest_finished_steps_that_fit_and_counts_the_earlier_ones_it_leaves_out() {
    let names: Vec<String> =
        (1..=120).map(|i| format!("Ran npm test, pass {i} of the suite with a long name to take room")).collect();
    let long = with(with(checkpoint(), "settledActions", json!(names)), "settledNames", json!(names));
    let carried = brief(&format!("Fix it. {}", "t".repeat(3_000)), &long, "Cursor", Some(&format!("Carry on. {}", "n".repeat(1_500))))
        .expect("a briefing");
    assert!(utf16(&carried.prompt) <= MAX_HANDOFF_PROMPT_LENGTH, "{}", utf16(&carried.prompt));
    assert!(!carried.omitted.contains(&"settled".to_owned()), "{:?}", carried.omitted);
    assert!(carried.kept.contains(&"settled".to_owned()), "{:?}", carried.kept);
    assert!(carried.prompt.contains("pass 120 of the suite"));
    assert!(!carried.prompt.contains("pass 1 of the suite"));
    let marker = "- and ";
    let line = carried.prompt.lines().find(|l| l.starts_with(marker) && l.ends_with(" earlier actions, not listed here"));
    let count = line.map(|l| &l[marker.len()..l.len() - " earlier actions, not listed here".len()]);
    assert!(count.is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())), "{}", carried.prompt);
}

#[test]
fn a_briefing_names_the_sections_it_carried_in_reading_order_and_only_those() {
    let earlier = [EarlierTurn { asked: "Start".into(), answered: Some("Started".into()) }];
    let whole = compose_handoff_prompt("Add a chart", &checkpoint(), "Claude Code", Some("Now make it blue"), &earlier, None).unwrap();
    assert_eq!(whole.kept, ["task", "earlier", "settled", "summary"]);
    assert!(whole.omitted.is_empty(), "{:?}", whole.omitted);
    assert!(!whole.task_clipped);
    let squeezed = brief(
        "Add a chart",
        &with(checkpoint(), "assistantSummary", json!("s".repeat(7_000))),
        "Claude Code",
        Some(&format!("Now {}", "b".repeat(1_500))),
    )
    .unwrap();
    assert!(squeezed.omitted.contains(&"summary".to_owned()), "{:?}", squeezed.omitted);
    assert!(!squeezed.kept.contains(&"summary".to_owned()), "{:?}", squeezed.kept);
    assert!(!squeezed.prompt.contains("What the previous agent said it had done"));
}

#[test]
fn after_a_clean_finish_and_a_reply_the_brief_says_the_conversation_was_taken_over_not_stopped_partway() {
    let clean = brief("Add a chart", &checkpoint(), "Claude Code", Some("Now make it blue")).unwrap();
    assert!(
        clean.prompt.contains(
            "taking over this conversation from another agent (Claude Code), which finished its last turn. The last thing it was asked was:"
        ),
        "{}",
        clean.prompt
    );
    assert!(!clean.prompt.contains("stopped partway"));
}

#[test]
fn after_a_stop_with_work_in_flight_the_brief_still_says_it_stopped_partway() {
    let in_flight = with(checkpoint(), "unsettledActions", json!([{"toolKind": "shell", "name": "npm test"}]));
    let stopped = brief("Add a chart", &in_flight, "Claude Code", Some("Carry on")).unwrap();
    assert!(stopped.prompt.contains("started and stopped partway through"), "{}", stopped.prompt);
}
