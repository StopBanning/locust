//! Electron a-codex-deny-is-in-codexs-schema.test.ts, using the real channel and a fake app-server.
use super::codex_stand_in::{stand_in, to_the_end};
use super::*;
use crate::approval::{Answer, Decision, MAX_PENDING};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/codex-0.172.0");

/// The decision type a response schema names: `properties.decision`'s `$ref`, resolved in its own definitions.
fn decision_schema(file: &str) -> Value {
    let path = format!("{FIXTURES}/{file}.json");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let reference = schema["properties"]["decision"]["$ref"].as_str().expect("decision is a $ref");
    let name = reference.strip_prefix("#/definitions/").expect("a local definition");
    schema["definitions"][name].clone()
}

const V2: [&str; 2] = ["CommandExecutionRequestApprovalResponse", "FileChangeRequestApprovalResponse"];
const LEGACY: [&str; 2] = ["ExecCommandApprovalResponse", "ApplyPatchApprovalResponse"];

fn type_fits(value: &Value, ty: Option<&str>) -> bool {
    match ty {
        Some("string") => value.is_string(),
        Some("array") => value.is_array(),
        Some("object") => value.is_object(),
        _ => true,
    }
}

/// Whether `decision` is one of the schema's `oneOf` variants: a string in a variant's `enum`, or an object with exactly
/// the one key a variant requires (it says `additionalProperties: false`), carrying that key's required fields.
fn fits(decision: &Value, schema: &Value) -> bool {
    schema["oneOf"].as_array().expect("a oneOf").iter().any(|variant| {
        if let Some(words) = variant["enum"].as_array() {
            return words.contains(decision);
        }
        let (Some(given), Some(props)) = (decision.as_object(), variant["properties"].as_object()) else { return false };
        let required: Vec<&str> = variant["required"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if !required.iter().all(|key| given.contains_key(*key)) || !given.keys().all(|key| props.contains_key(key)) {
            return false;
        }
        given.iter().all(|(key, inner)| {
            let shape = &props[key];
            type_fits(inner, shape["type"].as_str())
                && shape["required"].as_array().into_iter().flatten().filter_map(Value::as_str).all(|field| {
                    inner.get(field).is_some_and(|v| type_fits(v, shape["properties"][field]["type"].as_str()))
                })
        })
    })
}

/// Every answer the person can give: a new variant of `Decision` fails to compile here until it is listed.
fn every_decision() -> Vec<Decision> {
    let all = vec![Decision::ApproveOnce, Decision::ApproveAlways, Decision::Deny];
    for d in &all {
        match d {
            Decision::ApproveOnce | Decision::ApproveAlways | Decision::Deny => {}
        }
    }
    all
}

const COMMAND: &str = "item/commandExecution/requestApproval";
const PATCH: &str = "item/fileChange/requestApproval";
const QUESTION: &str = "item/tool/requestUserInput";
const LEGACY_METHODS: [&str; 2] = ["execCommandApproval", "applyPatchApproval"];
const MISSION: &str = "deny-parity";
const REASON: &str = "Check the fixture first.";

fn params(method: &str) -> Value {
    let common = json!({"threadId":"thread_9","turnId":"turn_1","itemId":"i1"});
    match method {
        QUESTION => json!({"threadId":"thread_9","turnId":"turn_1","itemId":"i1","isBlocking":true,
            "questions":[{"id":"q","header":"Test","question":"Continue?","options":null}]}),
        COMMAND => json!({"threadId":"thread_9","turnId":"turn_1","itemId":"i1","command":"echo fixture","cwd":"/work"}),
        PATCH => common,
        _ => json!({"conversationId":"thread_9","callId":"call_1","command":["echo","fixture"],"cwd":"/work"}),
    }
}

fn takes(answer: &Value, file: &str) {
    assert_eq!(answer.as_object().map(|body| body.len()), Some(1), "{answer}");
    assert!(fits(&answer["decision"], &decision_schema(file)), "{file}: {answer}");
}

fn answer_request(
    method: &str,
    approvals: Option<Arc<Channel>>,
    mode: Mode,
) -> Value {
    let (mut codex, told) = stand_in(json!({
        "answer": [{"send":{"method":"turn/completed","params":{"threadId":"thread_9",
            "turn":{"id":"turn_1","status":"completed"}}}}]
    }));
    codex.normal.thread = Some("thread_9".into());
    codex.normal.turn = Some("turn_1".into());
    codex.mission_id = MISSION.into();
    codex.run_id = "turn_1".into();
    codex.mode = mode;
    codex.approvals = approvals;
    codex.notification(json!({"jsonrpc":"2.0","id":900,"method":method,"params":params(method)})).unwrap();
    to_the_end(&mut codex);
    let replies = told.replies();
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_eq!(replies[0]["id"], 900);
    replies[0].clone()
}

fn with_card(method: &str, answer: impl FnOnce(&Channel, String) + Send + 'static) -> Value {
    let channel = Channel::new();
    let asker = channel.clone();
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            if let Some(card) = asker.waiting().first() {
                answer(&asker, card.approval_id.clone());
                break;
            }
            if Instant::now() >= deadline {
                asker.release(MISSION);
                panic!("the runtime did not show a card");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let answer = answer_request(method, Some(channel.clone()), Mode::ApproveEach);
    worker.join().unwrap();
    assert!(channel.waiting().is_empty());
    assert!(answer.get("error").is_none(), "{answer}");
    answer["result"].clone()
}

fn unanswered(answer: &Value) {
    // Native preserves the question ID. An empty array still answers nothing and authorizes nothing.
    assert_eq!(answer, &json!({"answers":{"q":{"answers":[]}}}));
}

// case: passes what Codex 0.172 takes and fails what Locust used to send
#[test]
fn the_pinned_schemas_reject_the_old_deny_and_bare_legacy_denial() {
    let command = decision_schema(V2[0]);
    assert!(fits(&json!("decline"), &command));
    assert!(fits(&json!({"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["echo"]}}), &command));
    for file in V2 {
        assert!(!fits(&json!("reject"), &decision_schema(file)));
    }
    assert!(!fits(&json!({"acceptWithExecpolicyAmendment":{}}), &command));
    assert!(fits(&json!("abort"), &decision_schema(LEGACY[0])));
    assert!(fits(&json!({"denied":{"rejection":"no"}}), &decision_schema(LEGACY[1])));
    for answer in [json!("denied"), json!({"denied":{}}), json!({"denied":{"rejection":"no"},"extra":1})] {
        assert!(!fits(&answer, &decision_schema(LEGACY[0])), "{answer}");
    }
}

// case: is in both v2 schemas, for every answer
#[test]
fn a_persons_every_decision_validates_against_both_current_schemas() {
    for file in V2 {
        for decision in every_decision() {
            takes(&json!({"decision":crate::approval::protocol_decision_for(&decision)}), file);
            takes(&crate::approval::protocol_answer_for(&Answer::Decision {
                approval_id: "ap_1".into(), decision, reason: None,
            }), file);
        }
    }
}

// case: is decline for Deny, so the turn carries on and a reason can still be steered into it
#[test]
fn a_deny_uses_decline_not_cancel() {
    assert_eq!(crate::approval::protocol_decision_for(&Decision::Deny), "decline");
}

// case: is what a Deny on the card sends, with a reason or without
#[test]
fn a_deny_card_sends_a_schema_valid_decline_with_or_without_a_reason() {
    for (method, file) in [(COMMAND, V2[0]), (PATCH, V2[1])] {
        for reason in [None, Some(REASON)] {
            let result = with_card(method, move |channel, id| {
                channel.decide(Answer::Decision {
                    approval_id: id, decision: Decision::Deny, reason: reason.map(str::to_owned)
                }).unwrap();
            });
            takes(&result, file);
            assert_eq!(result, json!({"decision":"decline"}));
        }
    }
}

// case: is in the schema of the method that asked, v2 and legacy
#[test]
fn each_kind_of_host_refusal_fits_the_method_that_asked() {
    for file in V2 { takes(&crate::approval::codex_refusal(), file); }
    for file in LEGACY { takes(&crate::approval::codex_legacy_refusal(), file); }
}

// case: refuses a legacy request it never puts on a card in the legacy shape
#[test]
fn active_card_host_does_not_turn_either_legacy_request_into_a_card() {
    for (method, file) in LEGACY_METHODS.into_iter().zip(LEGACY) {
        let channel = Channel::new();
        let answer = answer_request(method, Some(channel.clone()), Mode::ApproveEach);
        takes(&answer["result"], file);
        assert!(channel.waiting().is_empty());
    }
}

// case: releases a card a finished run was still asking with a decision Codex takes
#[test]
fn release_of_the_pending_command_sends_a_valid_refusal() {
    let result = with_card(COMMAND, |channel, _| channel.release(MISSION));
    takes(&result, V2[0]);
    assert_eq!(result, json!({"decision":"decline"}));
}

// case: is what a run with no one to ask sends on the wire, for every kind of approval
#[test]
fn with_no_card_host_each_of_the_four_approval_methods_gets_its_own_shape() {
    for (method, file) in [(COMMAND, V2[0]), (PATCH, V2[1]), (LEGACY_METHODS[0], LEGACY[0]), (LEGACY_METHODS[1], LEGACY[1])] {
        let answer = answer_request(method, None, Mode::Ask);
        takes(&answer["result"], file);
    }
}

// case: releases a pending question with no answers when the run ends
#[test]
fn releasing_the_question_returns_no_answers_on_the_wire() {
    unanswered(&with_card(QUESTION, |channel, _| channel.release(MISSION)));
}

// case: answers a question with no answers when given an authorization decision
#[test]
fn giving_an_authorization_to_a_question_returns_no_answers() {
    unanswered(&with_card(QUESTION, |channel, id| {
        assert!(channel.decide(Answer::Decision { approval_id: id, decision: Decision::ApproveOnce, reason: None }).is_err());
    }));
}

// case: declines an authorization when given answers intended for a question
#[test]
fn giving_question_answers_to_an_authorization_does_not_allow_it() {
    let result = with_card(COMMAND, |channel, id| {
        let answers = vec![("q".into(), vec!["yes".into()])];
        assert!(channel.decide(Answer::Answers { approval_id: id, answers }).is_err());
    });
    assert_eq!(result, json!({"decision":"decline"}));
}

// case: answers an overflowed question without an authorization decision
#[test]
fn a_question_over_the_native_pending_limit_gets_no_answers_on_the_wire() {
    let channel = Channel::new();
    let workers: Vec<_> = (0..MAX_PENDING).map(|id| {
        let channel = channel.clone();
        std::thread::spawn(move || channel.ask_codex("turn_1", MISSION, QUESTION, &params(QUESTION), json!(id), &|| false))
    }).collect();
    let deadline = Instant::now() + Duration::from_secs(12);
    while channel.waiting().len() != MAX_PENDING && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let filled = channel.waiting().len() == MAX_PENDING;
    let result = filled.then(|| answer_request(QUESTION, Some(channel.clone()), Mode::ApproveEach));
    channel.release(MISSION);
    for worker in workers { worker.join().unwrap().unwrap(); }
    assert!(filled, "the approvals reached their native bound");
    unanswered(&result.unwrap()["result"]);
    assert!(channel.waiting().is_empty());
}

// case: reports an unknown request as method not found
#[test]
fn unknown_request_gets_method_not_found_and_no_card() {
    let channel = Channel::new();
    let answer = answer_request("unknown/request", Some(channel.clone()), Mode::ApproveEach);
    assert_eq!(answer["error"]["code"], -32601);
    assert!(answer.get("result").is_none());
    assert!(channel.waiting().is_empty());
}
