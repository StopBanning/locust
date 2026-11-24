//! Electron's `packages/runtime-adapters/test/antigravity-waiting-is-said-as-waiting.test.ts`, case by case, through the
//! caller: each turn is a native Antigravity turn (`Antigravity::spawn` ... `next_event` to the end, agy_events_case_tests
//! `turn_of`) whose stand-in agy prints the records the case writes, in Full access (Electron's sandbox "full-access").
//! The helpers a list names are read by `agy_events::helpers_in`, which the normalizer reads each list with.
use super::agy_events_case_tests::turn_of;
use crate::approval::Mode;
use crate::runtime::Event;
use crate::runtime::agy_events::{REFUSED_SENTENCE, STALLED_SENTENCE, helpers_in};
use serde_json::{Value, json};

const CONVERSATION: &str = "ce799772-ae3c-426e-843b-a0d33fd982e4";

fn step(index: u32, body: Value) -> String {
    let mut update = json!({"conversation_id": CONVERSATION, "step_index": index, "state": "DONE"});
    update.as_object_mut().unwrap().extend(body.as_object().unwrap().clone());
    json!({"event": "step_update", "step_update": update}).to_string()
}

fn list() -> String {
    format!(
        "You have 1 active subagent(s):\n{}",
        json!([{"role":"Soak & Sweep Runner","type":"self","conversationId":"8115c8f0","state":"waiting_for_dependents","lastStepTime":"2026-10-24T07:18:51Z"}])
    )
}

fn wake(index: u32, seconds: f64, thinking: u64) -> String {
    step(
        index,
        json!({"step_type":"agent_response","duration_seconds":seconds,"usage":{"input_tokens":229_594 + index as u64,"output_tokens":thinking,"thinking_tokens":thinking,"cache_read_tokens":0,"total_tokens":229_594 + thinking}}),
    )
}

fn init(model: Option<&str>) -> String {
    let init = model.map(|m| json!({"model": m})).unwrap_or_else(|| json!({}));
    json!({"event":"init","conversation_id":CONVERSATION,"init":init}).to_string()
}

fn result(status: &str, response: &str, error: Option<&str>) -> String {
    let mut r = json!({"conversation_id":CONVERSATION,"status":status,"response":response});
    if let Some(e) = error {
        r["error"] = json!(e);
    }
    json!({"event":"result","result":r}).to_string()
}

fn run(lines: &[String]) -> Vec<Event> {
    turn_of(lines, 0, Mode::Auto)
}

fn is_thought(e: &Event) -> bool {
    e.kind == "step.completed" && e.payload["stepKind"] == "reasoning"
}

fn thoughts(events: &[Event]) -> Vec<&Value> {
    events.iter().filter(|e| is_thought(e)).map(|e| &e.payload).collect()
}

fn is_warning(e: &Event) -> bool {
    e.kind == "adapter.diagnostic" && e.payload["code"] == "antigravity.rereading"
}

fn warnings(events: &[Event]) -> Vec<String> {
    events.iter().filter(|e| is_warning(e)).map(|e| e.payload["message"].as_str().unwrap().to_owned()).collect()
}

/// Colin's turn, 2026-10-24, as agy 1.4.7 reported it.
fn turn() -> Vec<String> {
    vec![
        init(Some("gemini-4-argon")),
        step(622, json!({"step_type":"agent_response","duration_seconds":7.3,"usage":{"input_tokens":2_075,"output_tokens":55,"thinking_tokens":32,"cache_read_tokens":228_696}})),
        step(623, json!({"step_type":"tool","tool_name":"manage_subagents","duration_seconds":0.1,"tool_info":{"name":"manage_subagents","parameters":{"Action":"list"},"output":list()}})),
        wake(624, 309.3, 98),
        wake(627, 357.3, 2_147),
        wake(629, 338.6, 1_252),
        wake(631, 320.2, 902),
        wake(633, 340.0, 3_006),
        wake(635, 352.1, 4_493),
        result(
            "ERROR",
            "Waiting for task-570 to complete.",
            Some("agent executor error: generating and executing: request failed: stream idle timeout (5m0s): context deadline exceeded"),
        ),
    ]
}

#[test]
fn reads_the_helpers_a_list_names() {
    // case: reads the helpers a list names
    assert_eq!(
        helpers_in(&list()),
        [json!({"role":"Soak & Sweep Runner","state":"waiting_for_dependents","lastStepAt":"2026-10-24T07:18:51Z"})]
    );
    assert!(helpers_in("You have no active subagents.").is_empty());
    assert!(helpers_in("not a list [ at all").is_empty());
}

#[test]
fn says_each_long_wait_after_the_list_as_a_wait_on_that_helper_at_the_length_agy_timed() {
    // case: says each long wait after the list as a wait on that helper, at the length agy timed
    let events = run(&turn());
    let all = thoughts(&events);
    assert_eq!(all.len(), 7);
    assert!(all[0].get("message").is_none());
    let rows = &all[1..];
    let seconds: Vec<i64> = rows.iter().map(|r| (r["durationMs"].as_f64().unwrap() / 1000.0).round() as i64).collect();
    assert_eq!(seconds, [309, 357, 339, 320, 340, 352]);
    for row in rows {
        let message = row["message"].as_str().unwrap();
        assert!(message.starts_with("**Waiting on its helper Soak & Sweep Runner**"), "{message}");
        assert!(message.contains("waiting for dependents, with no step since"), "{message}");
    }
}

#[test]
fn says_once_at_the_third_that_the_whole_conversation_is_being_read_again_and_what_it_has_cost() {
    // case: says, once, at the third, that the whole conversation is being read again and what it has cost
    let events = run(&turn());
    let said = warnings(&events);
    assert_eq!(said.len(), 1);
    assert!(
        said[0].contains("read this whole conversation again 3 times this turn, about 230,000 tokens each, with none of it reused: 691,000 tokens of your quota so far"),
        "{}",
        said[0]
    );
    assert!(said[0].contains("press Stop"));
    let at = events.iter().position(is_warning).unwrap();
    assert_eq!(events[..at].iter().filter(|e| is_thought(e)).count(), 4);
}

#[test]
fn says_the_timeout_that_ended_it_in_words() {
    // case: says the timeout that ended it in words
    let events = run(&turn());
    let failed = events.iter().find(|e| e.kind == "run.failed").unwrap();
    assert_eq!(failed.payload["message"], STALLED_SENTENCE);
    assert_eq!(
        STALLED_SENTENCE,
        "Antigravity's model stopped answering for five minutes, so Antigravity ended the turn. What it did so far is kept. Antigravity says this can be tried again: send \"continue\"."
    );
}

#[test]
fn says_googles_refusal_of_a_long_turn_in_words_with_what_to_do_0842() {
    // case: says Google's refusal of a long turn in words, with what to do (0.842)
    let events = run(&[
        init(Some("gemini-3.8-flash")),
        step(1, json!({"step_type":"tool","tool_name":"run_command","tool_info":{"name":"run_command","parameters":{"CommandLine":"git worktree add -b drums-adt D:\\work\\locust-wt\\drums-adt 39db1a3"},"output":"Preparing worktree"}})),
        result("ERROR", "I will create the git worktree.", Some("INVALID_ARGUMENT (code 400): Request contains an invalid argument.")),
    ]);
    let refused = events.iter().find(|e| e.kind == "run.failed").unwrap();
    assert_eq!(refused.payload["message"], REFUSED_SENTENCE);
    assert_eq!(
        REFUSED_SENTENCE,
        "Google refused this turn's next request as invalid, after it had already worked; what it did so far is kept. Send \"carry on\" to go on from there. If Google refuses again, start a new conversation and say what is already done."
    );
}

#[test]
fn keeps_the_last_list_through_a_call_that_is_not_one_and_lets_it_go_once_no_helper_is_left() {
    // case: keeps the last list through a call that is not one, and lets it go once no helper is left
    let manage = |index: u32, output: &str| {
        step(index, json!({"step_type":"tool","tool_name":"manage_subagents","tool_info":{"name":"manage_subagents","parameters":{"Action":"send"},"output":output}}))
    };
    let events = run(&[
        init(None),
        manage(1, &list()),
        manage(2, "Sent your message to Soak & Sweep Runner."),
        wake(3, 300.0, 50),
        manage(4, "You have no active subagents."),
        wake(5, 300.0, 50),
        result("SUCCESS", "Done.", None),
    ]);
    let rows = thoughts(&events);
    assert!(rows[0]["message"].as_str().unwrap().starts_with("**Waiting on its helper Soak & Sweep Runner**"));
    assert!(rows[1].get("message").is_none());
}

#[test]
fn says_nothing_of_the_kind_for_a_helper_that_finished_or_for_steps_that_reused_the_conversation() {
    // case: says nothing of the kind for a helper that finished, or for steps that reused the conversation
    let finished = list().replace("waiting_for_dependents", "completed");
    let cached = |index: u32| {
        step(index, json!({"step_type":"agent_response","duration_seconds":40,"usage":{"input_tokens":229_594,"output_tokens":500,"thinking_tokens":500,"cache_read_tokens":228_000}}))
    };
    let events = run(&[
        init(None),
        step(1, json!({"step_type":"tool","tool_name":"manage_subagents","tool_info":{"name":"manage_subagents","parameters":{"Action":"list"},"output":finished}})),
        cached(2),
        cached(3),
        cached(4),
        cached(5),
        result("SUCCESS", "Done.", None),
    ]);
    assert!(thoughts(&events).iter().all(|row| row.get("message").is_none()));
    assert!(warnings(&events).is_empty());
}

/// The same reading with no process in between, so load cannot hide it: agy's lines go straight into the normalizer
/// antigravity.rs feeds (`AgyNormalizer::accept_line`), and the wait is said with the helper that list named, in
/// Electron's words (agy-events.ts: "**Waiting on its helper ${role}**\nIts list says the helper is ${state}").
#[test]
fn the_wait_is_said_with_the_helper_the_normalizer_read_from_the_list() {
    use crate::runtime::agy_events::AgyNormalizer;
    for (role, state, said) in [
        ("Soak & Sweep Runner", "waiting_for_dependents", "waiting for dependents"),
        ("Docs Checker", "running", "running"),
    ] {
        let listed = format!("You have 1 active subagent(s):\n{}", json!([{"role": role, "state": state}]));
        let mut normal = AgyNormalizer::default().with_sandbox("full-access");
        let mut events = vec![];
        for line in [
            init(None),
            step(1, json!({"step_type":"tool","tool_name":"manage_subagents","tool_info":{"name":"manage_subagents","parameters":{"Action":"list"},"output":listed}})),
            wake(2, 309.3, 98),
        ] {
            events.extend(normal.accept_line(&line));
        }
        let rows = thoughts(&events);
        assert_eq!(rows.len(), 1, "{role}");
        assert_eq!(rows[0]["durationMs"], 309_300, "{role}");
        assert_eq!(
            rows[0]["message"],
            format!("**Waiting on its helper {role}**\nIts list says the helper is {said}."),
            "{role}"
        );
    }
}
