//! Electron `main/a-switch-brief-says-what-was-done.test.ts` (A2.11, reported #13): a runtime switch is told what
//! happened, in words. Its settled actions were the provider's item ids (`call_8f2c...`), and only the last turn was
//! carried. One test per Electron case, its values and words; `reconcile` is `reconcileMission`, run on the same events.

use crate::carry_on::{
    compose_handoff_prompt, how_turn_ended, omission_notice, reconcile, EarlierTurn, KEEP_ORDER, MAX_HANDOFF_PROMPT_LENGTH, NOTICE_BUDGET,
};
use serde_json::{json, Value};

fn checkpoint(overrides: Value) -> Value {
    let mut cp = json!({
        "missionId": "mission_prior",
        "epoch": 2,
        "reason": "route-switch",
        "resumeSafety": "safe",
        "safetyReason": "settled",
        "createdAt": "2026-09-05T03:00:00.000Z",
        "unsettledActions": [],
        "settledActions": [],
        "assistantSummary": ""
    });
    for (k, v) in overrides.as_object().cloned().unwrap_or_default() {
        cp[k] = v;
    }
    cp
}

fn reconciled(events: &[Value]) -> Value {
    reconcile(&json!({"missionId": "m", "runId": "r"}), events, false, "route-switch", 1, "2026-09-05T03:00:00.000Z")
}

fn names(cp: &Value) -> Vec<String> {
    cp["settledNames"].as_array().into_iter().flatten().filter_map(|n| n.as_str().map(str::to_owned)).collect()
}

fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

// ---- settled actions ------------------------------------------------------------------------------------------------

#[test]
fn settled_actions_are_named_by_what_they_did_from_the_ledgers_own_start_events() {
    let events = [
        json!({"id": "e1", "runId": "r", "missionId": "m", "sequence": 1, "type": "tool.started", "occurredAt": "x", "sourceAdapter": "codex", "payload": {"itemId": "call_8f2c", "toolKind": "command", "name": "npm test"}}),
        json!({"id": "e2", "runId": "r", "missionId": "m", "sequence": 2, "type": "tool.completed", "occurredAt": "x", "sourceAdapter": "codex", "payload": {"itemId": "call_8f2c"}}),
        // Its start was not kept: counted, never shown as an id.
        json!({"id": "e3", "runId": "r", "missionId": "m", "sequence": 3, "type": "tool.completed", "occurredAt": "x", "sourceAdapter": "codex", "payload": {"itemId": "call_orphan"}}),
    ];
    let cp = reconciled(&events);
    assert_eq!(names(&cp), ["command: npm test"]);
    let prompt = compose_handoff_prompt("Fix the tests.", &cp, "Codex CLI", Some("Carry on."), &[], None).unwrap().prompt;
    assert!(prompt.contains("- command: npm test"), "{prompt}");
    assert!(prompt.contains("- 1 other action whose details were not recorded"), "{prompt}");
    assert!(!prompt.contains("call_8f2c"));
    assert!(!prompt.contains("call_orphan"));
}

#[test]
fn settled_actions_from_a_checkpoint_written_before_names_existed_are_counted_not_listed_as_ids() {
    let cp = checkpoint(json!({"settledActions": ["call_a", "call_b"]}));
    let prompt = compose_handoff_prompt("Fix the tests.", &cp, "Codex CLI", None, &[], None).unwrap().prompt;
    assert!(prompt.contains("- 2 actions whose details were not recorded"), "{prompt}");
    assert!(!prompt.contains("call_a"));
}

// ---- settled actions, as Antigravity records them (0.567) ------------------------------------------------------------

/// Its adapter names a tool twice (`toolKind` and `name` both `run_command`) and keeps what it ran in `command`: the
/// shape of Colin's W7 ledger, whose brief to Codex read `run_command: run_command` forty times.
fn tool(sequence: u64, item: &str, kind: &str, command: Option<&str>) -> [Value; 2] {
    let mut started = json!({"itemId": item, "toolKind": kind, "name": kind, "phase": "started"});
    if let Some(c) = command {
        started["command"] = json!(c);
    }
    [
        json!({"id": format!("s{sequence}"), "runId": "r", "missionId": "m", "sequence": sequence, "type": "tool.started", "occurredAt": "x", "sourceAdapter": "antigravity", "payload": started}),
        json!({"id": format!("c{sequence}"), "runId": "r", "missionId": "m", "sequence": sequence + 1, "type": "tool.completed", "occurredAt": "x", "sourceAdapter": "antigravity", "payload": {"itemId": item, "toolKind": kind, "name": kind, "phase": "completed"}}),
    ]
}

#[test]
fn antigravity_actions_are_named_by_the_command_run_and_the_file_read_or_written_never_the_tool_twice() {
    let events: Vec<Value> = [
        tool(1, "tool_2", "run_command", Some("git status -sb")),
        tool(3, "tool_4", "view_file", Some("docs/PLAN.md")),
        tool(5, "tool_6", "write_to_file", Some("apps/desktop/src/shared/routine-inputs.ts")),
        tool(7, "tool_8", "manage_task", None),
    ]
    .into_iter()
    .flatten()
    .collect();
    let cp = reconciled(&events);
    assert_eq!(
        names(&cp),
        ["run_command: git status -sb", "view_file: docs/PLAN.md", "write_to_file: apps/desktop/src/shared/routine-inputs.ts", "manage_task"]
    );
    let prompt = compose_handoff_prompt("Build W7.", &cp, "Antigravity", Some("keep working"), &[], None).unwrap().prompt;
    assert!(prompt.contains("- write_to_file: apps/desktop/src/shared/routine-inputs.ts"), "{prompt}");
    assert!(!prompt.contains("run_command: run_command"));
}

#[test]
fn past_the_most_recent_forty_the_rest_are_said_to_be_earlier_not_unrecorded() {
    let events: Vec<Value> = (0..62u64).flat_map(|at| tool(at * 2 + 1, &format!("tool_{at}"), "run_command", Some(&format!("step {at}")))).collect();
    let cp = reconciled(&events);
    let listed = names(&cp);
    assert_eq!(listed.len(), 40);
    assert_eq!(listed.last().map(String::as_str), Some("run_command: step 61"));
    let prompt = compose_handoff_prompt("Build W7.", &cp, "Antigravity", Some("keep working"), &[], None).unwrap().prompt;
    assert!(prompt.contains("- and 22 earlier actions, not listed here"), "{prompt}");
    assert!(!prompt.contains("whose details were not recorded"));
}

// ---- the earlier turns ----------------------------------------------------------------------------------------------

fn earlier() -> Vec<EarlierTurn> {
    vec![
        EarlierTurn { asked: "Find why the build is slow.".into(), answered: Some("The TypeScript step re-checks every package on each build.".into()) },
        EarlierTurn { asked: "Make it incremental.".into(), answered: None },
    ]
}

#[test]
fn earlier_turns_are_carried_oldest_first_each_as_what_was_asked_and_how_it_was_answered() {
    let prompt = compose_handoff_prompt("Now add a cache.", &checkpoint(json!({})), "Claude Code", Some("Carry on."), &earlier(), None).unwrap().prompt;
    assert!(
        prompt.contains("Earlier in this conversation, oldest first:\n- Asked: \"Find why the build is slow.\" -- answered: \"The TypeScript step re-checks every package on each build.\"\n- Asked: \"Make it incremental.\" -- no reply was recorded."),
        "{prompt}"
    );
    assert!(prompt.find("Earlier in this conversation").unwrap() < prompt.find("The person now asks").unwrap());
}

#[test]
fn earlier_turns_are_clipped_not_pasted_whole() {
    let long = [EarlierTurn { asked: "x".repeat(2_000), answered: Some("y".repeat(2_000)) }];
    let prompt = compose_handoff_prompt("Go.", &checkpoint(json!({})), "Claude Code", None, &long, None).unwrap().prompt;
    assert!(utf16(&prompt) < 1_000, "{}", utf16(&prompt));
    assert!(prompt.contains('…'));
}

#[test]
fn earlier_turns_are_the_first_given_up_when_the_brief_is_long_and_never_push_out_the_persons_words() {
    let big: Vec<EarlierTurn> = (0..4).map(|_| EarlierTurn { asked: "a".repeat(240), answered: Some("b".repeat(360)) }).collect();
    let near_full = "n".repeat(MAX_HANDOFF_PROMPT_LENGTH - 1_200);
    let briefing =
        compose_handoff_prompt("Go.", &checkpoint(json!({"assistantSummary": "I changed tsconfig."})), "Claude Code", Some(&near_full), &big, None).expect("a briefing");
    // Since 0.671 the earlier turns give way in part: the newest that fit are kept, the rest counted.
    assert!(briefing.omitted.is_empty(), "{:?}", briefing.omitted);
    assert!(crate::invariants::regex(r"- \(\d+ earlier turns? (is|are) not listed here\)").is_match(&briefing.prompt), "{}", briefing.prompt);
    assert!(briefing.prompt.contains("I changed tsconfig."));
    assert!(briefing.prompt.contains(&format!("The person now asks:\n\n{near_full}")));
    assert!(utf16(&briefing.prompt) <= MAX_HANDOFF_PROMPT_LENGTH);
}

#[test]
fn earlier_turns_give_way_to_what_the_last_agent_said_when_only_one_of_the_two_fits() {
    let turn = [EarlierTurn { asked: "a".repeat(240), answered: Some("b".repeat(360)) }];
    let summary = "s".repeat(700);
    // Room for either the summary (~745) or the earlier turn (~680), not both.
    let next = "n".repeat(MAX_HANDOFF_PROMPT_LENGTH - NOTICE_BUDGET - 140 - 1_000);
    let briefing = compose_handoff_prompt("Go.", &checkpoint(json!({"assistantSummary": summary})), "Claude Code", Some(&next), &turn, None).expect("a briefing");
    assert_eq!(briefing.omitted, ["earlier"]);
    assert!(briefing.prompt.contains(&summary));
}

#[test]
fn the_notice_fits_its_reserve_with_every_optional_section_named() {
    assert!(utf16(&omission_notice(&KEEP_ORDER)) + 2 <= NOTICE_BUDGET);
    assert_eq!(KEEP_ORDER, ["unsettled", "settled", "summary", "earlier"]);
}

// ---- how the turn handed over ended (Colin, 2026-10-03) -------------------------------------------------------------

fn ended(kind: &str, payload: Value) -> Vec<Value> {
    vec![json!({"type": "run.started", "payload": {}}), json!({"type": kind, "payload": payload})]
}

#[test]
fn a_turn_cut_off_by_a_usage_limit_is_said_to_be_not_finished() {
    let said = how_turn_ended(&ended("run.failed", json!({"kind": "quota-exhausted", "message": "You have hit your usage limit."})));
    assert_eq!(said, Some("its usage limit was reached"));
    let prompt = compose_handoff_prompt("Answer Casper.", &checkpoint(json!({})), "Codex CLI", Some("Carry on."), &[], said).unwrap().prompt;
    assert!(prompt.contains("whose last turn ended before it finished: its usage limit was reached"), "{prompt}");
    assert!(!prompt.contains("which finished its last turn"));
}

#[test]
fn a_completed_turn_is_still_said_to_have_finished() {
    assert_eq!(how_turn_ended(&ended("run.completed", json!({}))), None);
    let prompt = compose_handoff_prompt("Answer Casper.", &checkpoint(json!({})), "Codex CLI", Some("Carry on."), &[], None).unwrap().prompt;
    assert!(prompt.contains("which finished its last turn"), "{prompt}");
    assert_eq!(how_turn_ended(&ended("run.cancelled", json!({}))), Some("the person stopped it"));
}
