//! Every decision Locust can send Codex, checked against Codex 0.172.0's own generated response schemas (checked in
//! under `tests/fixtures/codex-0.172.0/`). Locust sent `"reject"` for Deny, a value in no schema of Codex's, and the
//! legacy replies a bare `"denied"`: a Codex that moves fails a test here instead of a person's turn.
use super::*;
use crate::approval::Decision;

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

#[test]
fn the_validator_tells_codexs_decisions_from_others() {
    // The check itself: what 0.172 takes passes, and what Locust used to send does not.
    let command = decision_schema(V2[0]);
    assert!(fits(&json!("decline"), &command));
    assert!(fits(&json!({"acceptWithExecpolicyAmendment": {"execpolicy_amendment": ["ls"]}}), &command));
    assert!(!fits(&json!("reject"), &command));
    assert!(!fits(&json!({"acceptWithExecpolicyAmendment": {}}), &command));
    let legacy = decision_schema(LEGACY[0]);
    assert!(fits(&json!("abort"), &legacy));
    assert!(fits(&json!({"denied": {"rejection": "no"}}), &legacy));
    assert!(!fits(&json!("denied"), &legacy));
    assert!(!fits(&json!({"denied": {}}), &legacy));
    assert!(!fits(&json!({"denied": {"rejection": "no"}, "extra": 1}), &legacy));
}

#[test]
fn every_answer_a_person_gives_codex_is_in_its_0_172_schemas() {
    for file in V2 {
        let schema = decision_schema(file);
        for decision in every_decision() {
            let answer = crate::approval::protocol_answer_for(&crate::approval::Answer::Decision {
                approval_id: "ap_1".into(),
                decision: decision.clone(),
                reason: Some("because".into()),
            });
            assert_eq!(answer.as_object().map(|o| o.len()), Some(1), "only `decision`: {answer}");
            assert!(fits(&answer["decision"], &schema), "{file} does not take {answer} for {decision:?}");
        }
        assert!(fits(&crate::approval::codex_refusal()["decision"], &schema), "{file} does not take Locust's own refusal");
    }
}

#[test]
fn deny_declines_and_lets_the_turn_carry_on() {
    // decline, not cancel: the turn goes on without the action, so a reason the person gave can still be steered into
    // it as its next input (`take_denied_saying` → `turn/steer`). Cancel would interrupt the turn it is steered into.
    assert_eq!(crate::approval::protocol_decision_for(&Decision::Deny), "decline");
}

#[test]
fn the_legacy_refusal_is_in_codexs_0_172_legacy_schemas() {
    let refusal = crate::approval::codex_legacy_refusal();
    assert_eq!(refusal.as_object().map(|o| o.len()), Some(1), "only `decision`: {refusal}");
    for file in LEGACY {
        assert!(fits(&refusal["decision"], &decision_schema(file)), "{file} does not take {refusal}");
    }
    // The denied variant, not abort: the agent tries something else rather than stopping until the next message.
    assert!(refusal["decision"]["denied"]["rejection"].as_str().is_some_and(|s| !s.is_empty()));
}

/// A fake app server that asks, in turn, each request in `asks` (method, thread), says back each answer as an agent
/// message, and ends the turn.
fn asking_server(asks: &[(&str, &str)]) -> Command {
    let mut script = String::from(
        "import json, sys\n\
         def send(m): print(json.dumps(m), flush=True)\n\
         inp = open(0, 'rb', buffering=0)\n\
         def line(): return inp.readline().decode()\n",
    );
    for (n, (method, thread)) in asks.iter().enumerate() {
        let id = n + 1;
        script.push_str(&format!(
            "send({{'id':{id},'method':'{method}','params':{{'threadId':'{thread}','conversationId':'{thread}','itemId':'i{id}','callId':'c{id}','command':['rm','-rf','build'],'cwd':'/tmp','changes':{{}}}}}})\n\
             send({{'method':'item/completed','params':{{'threadId':'t1','item':{{'id':'said_{id}','type':'agentMessage','text':line().strip()}}}}}})\n"
        ));
    }
    script.push_str("send({'method':'turn/completed','params':{'threadId':'t1','turn':{'id':'turn_1','status':'completed'}}})\nline()\n");
    let mut command = Command::new("python3");
    command.args(["-c", &script]);
    command
}

/// The results Locust sent the server, by request id.
fn answers_from(mut runtime: Codex) -> Vec<Value> {
    let mut sent = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let Some(ev) = runtime.next_event().unwrap() else { continue };
        if ev.kind == "message.delta" {
            sent.push(serde_json::from_str::<Value>(ev.payload["text"].as_str().unwrap()).unwrap());
        }
        if matches!(ev.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled") {
            break;
        }
    }
    sent
}

#[test]
fn what_codex_is_sent_on_the_wire_without_a_card_is_in_its_0_172_schemas() {
    // Each answer Locust gives without asking the person: a legacy request (both kinds), another thread's approval, and
    // an approval outside Approve each. Each is checked against the schema of the request it answers.
    let asks = [
        ("execCommandApproval", "t1"),
        ("applyPatchApproval", "t1"),
        ("item/commandExecution/requestApproval", "other"),
        ("item/fileChange/requestApproval", "other"),
        ("item/commandExecution/requestApproval", "t1"),
        ("item/fileChange/requestApproval", "t1"),
    ];
    let schemas = [LEGACY[0], LEGACY[1], V2[0], V2[1], V2[0], V2[1]];
    let mut runtime = Codex::spawn(&mut asking_server(&asks)).unwrap();
    runtime.normal.thread = Some("t1".into());
    runtime.mode = Mode::Ask;
    let sent = answers_from(runtime);
    assert_eq!(sent.len(), asks.len(), "{sent:?}");
    for (n, (reply, file)) in sent.iter().zip(schemas).enumerate() {
        assert_eq!(reply["id"], json!(n + 1), "{reply}");
        assert!(fits(&reply["result"]["decision"], &decision_schema(file)), "{} answered {reply}, not in {file}", asks[n].0);
    }
}

#[test]
fn what_codex_is_sent_on_the_wire_for_a_persons_deny_is_in_its_0_172_schemas() {
    for (method, file) in [("item/commandExecution/requestApproval", V2[0]), ("item/fileChange/requestApproval", V2[1])] {
        let mut runtime = Codex::spawn(&mut asking_server(&[(method, "t1")])).unwrap();
        let channel = Channel::new();
        runtime.normal.thread = Some("t1".into());
        runtime.mode = Mode::ApproveEach;
        runtime.run_id = "run_1".into();
        runtime.mission_id = "mission_1".into();
        runtime.approvals = Some(channel.clone());
        let decider = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if let Some(card) = channel.waiting().first() {
                    channel
                        .decide(crate::approval::Answer::Decision { approval_id: card.approval_id.clone(), decision: Decision::Deny, reason: None })
                        .unwrap();
                    return true;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            false
        });
        let sent = answers_from(runtime);
        assert!(decider.join().unwrap(), "{method}: no card was shown");
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert!(fits(&sent[0]["result"]["decision"], &decision_schema(file)), "{method} Deny answered {}, not in {file}", sent[0]);
        assert_eq!(sent[0]["result"], json!({"decision": "decline"}));
    }
}
