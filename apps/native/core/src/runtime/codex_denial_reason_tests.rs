//! A denial's reason on a Codex app server, end to end (Electron's index.ts `onDeniedSaying`, codex-app-server-run.ts
//! `steer`): the reply is the plain reject, and the reason goes to the running turn as `turn/steer`, in the person's
//! words. The cases are in `approval_denial_reason_tests.rs`; this is the wire they ride.
use super::*;

/// A fake app server: asks to run `rm -rf build`, then says back, as agent messages, every line Locust sends it until
/// the turn ends (read a byte at a time, so a line already sent is never hidden in a buffer from `select`). `steer` is how it answers a `turn/steer`: `ok`, or `refuse`.
fn echoing_server(steer: &str) -> Command {
    let script = format!(
        "import json, sys\n\
         def send(m): print(json.dumps(m), flush=True)\n\
         inp = open(0, 'rb', buffering=0)\n\
         def line(): return inp.readline().decode()\n\
         send({{'id':9,'method':'item/commandExecution/requestApproval','params':{{'threadId':'t1','itemId':'i1','command':'rm -rf build','cwd':'/tmp'}}}})\n\
         answer = line()\n\
         send({{'method':'item/completed','params':{{'threadId':'t1','item':{{'id':'said_1','type':'agentMessage','text':answer.strip()}}}}}})\n\
         import select\n\
         ready, _, _ = select.select([inp], [], [], 1.0)\n\
         if ready:\n\
         \x20   steered = line()\n\
         \x20   send({{'method':'item/completed','params':{{'threadId':'t1','item':{{'id':'said_2','type':'agentMessage','text':steered.strip()}}}}}})\n\
         \x20   ask = json.loads(steered)\n\
         \x20   send({{'id':ask['id'],'result':{{}}}} if '{steer}' == 'ok' else {{'id':ask['id'],'error':{{'code':-32600,'message':'no turn'}}}})\n\
         send({{'method':'turn/completed','params':{{'threadId':'t1','turn':{{'id':'turn_1','status':'completed'}}}}}})\n\
         line()\n"
    );
    let mut command = Command::new("python3");
    command.args(["-c", &script]);
    command
}

/// What the server was sent, in order, and every diagnostic the run said, the person denying with `reason`.
fn run_denying(steer: &str, turn: Option<&str>, reason: Option<&'static str>) -> (Vec<Value>, Vec<String>) {
    let mut runtime = Codex::spawn(&mut echoing_server(steer)).unwrap();
    let channel = Channel::new();
    runtime.normal.thread = Some("t1".into());
    runtime.normal.turn = turn.map(str::to_owned);
    runtime.mode = Mode::ApproveEach;
    runtime.run_id = "run_1".into();
    runtime.mission_id = "mission_1".into();
    runtime.approvals = Some(channel.clone());
    let ch = channel.clone();
    let decider = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(card) = ch.waiting().first() {
                ch.decide(crate::approval::Answer::Decision {
                    approval_id: card.approval_id.clone(),
                    decision: crate::approval::Decision::Deny,
                    reason: reason.map(str::to_owned),
                })
                .unwrap();
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let (mut sent, mut said) = (Vec::new(), Vec::new());
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let Some(ev) = runtime.next_event().unwrap() else { continue };
        if ev.kind == "message.delta" {
            sent.push(serde_json::from_str(ev.payload["text"].as_str().unwrap()).unwrap());
        }
        if ev.kind == "adapter.diagnostic" {
            said.push(ev.payload["message"].as_str().unwrap_or("").to_owned());
        }
        if matches!(ev.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled") {
            break;
        }
    }
    decider.join().unwrap();
    (sent, said)
}

const NOT_PASSED_ON: &str = "Your reason was not passed on: the run could not take a message just then. Say it in your next message.";

#[test]
fn codex_is_sent_the_plain_reject_and_then_the_reason_as_the_turns_next_input() {
    let (sent, said) = run_denying("ok", Some("turn_1"), Some("use the build script instead"));
    assert_eq!(sent[0], json!({"jsonrpc": "2.0", "id": 9, "result": {"decision": "reject"}}));
    assert_eq!(sent[1]["method"], "turn/steer");
    assert_eq!(
        sent[1]["params"],
        json!({"threadId": "t1", "expectedTurnId": "turn_1", "input": [{"type": "text", "text": "I declined that. use the build script instead"}]})
    );
    assert_eq!(sent.len(), 2);
    assert!(!said.iter().any(|m| m == NOT_PASSED_ON), "{said:?}");
}

#[test]
fn codex_is_steered_nothing_after_a_denial_without_a_reason() {
    let (sent, said) = run_denying("ok", Some("turn_1"), None);
    assert_eq!(sent, vec![json!({"jsonrpc": "2.0", "id": 9, "result": {"decision": "reject"}})]);
    assert!(!said.iter().any(|m| m == NOT_PASSED_ON), "{said:?}");
}

#[test]
fn a_reason_the_run_could_not_take_is_said_so() {
    // No turn to steer yet: nothing is sent, and the person is told.
    let (sent, said) = run_denying("ok", None, Some("use the build script instead"));
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert!(said.iter().any(|m| m == NOT_PASSED_ON), "{said:?}");
    // The server refused the steer: told too.
    let (sent, said) = run_denying("refuse", Some("turn_1"), Some("use the build script instead"));
    assert_eq!(sent[1]["method"], "turn/steer");
    assert!(said.iter().any(|m| m == NOT_PASSED_ON), "{said:?}");
}
