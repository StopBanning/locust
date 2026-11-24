//! Electron's `packages/runtime-adapters/test/app-server.test.ts`, case by case. Electron's JSON-RPC client is a class of
//! its own; Rust's is the Codex transport itself (the reader thread, `request`, `notification`, `send`), so each case is
//! carried on a Codex turn whose `codex app-server` is the stand-in (codex_stand_in.rs), through the host's calls
//! (`start`, `next_event`, `steer`, `stop`). Where Electron's client goes on after a line it must drop for its size, a
//! Codex run does not: codex-app-server-run.ts `lose`s the run on `buffer-overflow` and `line-too-long`, and so does
//! Rust, so those two cases carry the run's behaviour. Fixed 24 Nov: a blank line or one that was not JSON ended the
//! run ("Codex sent malformed JSON-RPC"); an answer to no request was dropped without a word; an unanswered request
//! said "Codex timed out answering .." rather than Electron's words.
use super::codex_stand_in::*;
use super::*;

fn delta(text: &str) -> String {
    format!("{}\n", json!({"jsonrpc": "2.0", "method": "item/agentMessage/delta", "params": {"threadId": "thread_9", "itemId": "m1", "delta": text}}))
}

fn completed() -> String {
    format!("{}\n", json!({"jsonrpc": "2.0", "method": "turn/completed", "params": {"threadId": "thread_9"}}))
}

/// A turn whose server, after answering turn/start, writes `steps`.
fn turn_writing(steps: Vec<Value>) -> (Codex, Told) {
    let mut all = vec![json!({"result": {"turn": {"id": "turn_1"}}})];
    all.extend(steps);
    let (mut codex, told) = stand_in(json!({"turn/start": all}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    (codex, told)
}

fn texts(events: &[Event]) -> Vec<&str> {
    of(events, "message.delta").iter().map(|e| e.payload["text"].as_str().unwrap()).collect()
}

// case: reassembles messages split across arbitrary chunks
#[test]
fn reassembles_messages_split_across_chunks() {
    let whole = format!("{}{}", delta("Caching"), completed());
    let (head, rest) = whole.split_at(27);
    let (middle, tail) = rest.split_at(rest.len() - 40);
    let (mut codex, _told) = turn_writing(vec![
        json!({"raw": head}),
        json!({"sleep": 0.1}),
        json!({"raw": middle}),
        json!({"sleep": 0.1}),
        json!({"raw": tail}),
    ]);
    let events = to_the_end(&mut codex);
    assert_eq!(kinds(&events), ["message.delta", "run.completed"]);
    assert_eq!(texts(&events), ["Caching"]);
}

// case: ignores blank lines and keeps a trailing partial buffered
#[test]
fn ignores_blank_lines_and_keeps_a_partial_buffered() {
    let line = delta("a");
    let (mut codex, _told) = turn_writing(vec![
        json!({"raw": "\n\n"}),
        json!({"raw": line.trim_end()}),
        json!({"sleep": 1.0}),
        json!({"raw": "\n"}),
        json!({"raw": completed()}),
    ]);
    let early = events_until_end(&mut codex, Duration::from_millis(300));
    assert!(early.is_empty(), "nothing yet: {:?}", kinds(&early));
    let events = to_the_end(&mut codex);
    assert_eq!(kinds(&events), ["message.delta", "run.completed"]);
}

// case: reports an unparsable line instead of throwing
#[test]
fn reports_an_unparsable_line_and_keeps_going() {
    let (mut codex, _told) = turn_writing(vec![json!({"raw": "{not json\n"}), json!({"raw": delta("a")}), json!({"raw": completed()})]);
    let events = to_the_end(&mut codex);
    assert_eq!(kinds(&events), ["adapter.diagnostic", "message.delta", "run.completed"]);
    assert_eq!(events[0].payload["code"], "codex.malformed_json");
    assert_eq!(events[0].payload["terminal"], false);
}

// case: drops a buffer that grows without a newline rather than growing forever
#[test]
fn a_buffer_without_a_newline_does_not_grow_forever() {
    let (mut codex, _told) = turn_writing(vec![json!({"raw": "x".repeat(8 * 1024 * 1024 + 64)}), json!({"sleep": 5})]);
    let end = to_the_end(&mut codex).pop().unwrap();
    assert_eq!(end.kind, "run.failed");
    assert_eq!(end.payload["message"], "Codex JSON-RPC line exceeds 8 MiB");
    assert_eq!(end.payload["process"]["outputLimitExceeded"], true);
}

// case: drops an oversized single line but keeps the connection usable
#[test]
fn an_oversized_line_ends_the_run_and_says_why() {
    let long = format!("{}\n", json!({"jsonrpc": "2.0", "method": "x".repeat(8 * 1024 * 1024)}));
    let (mut codex, _told) = turn_writing(vec![json!({"raw": long}), json!({"raw": completed()})]);
    let end = to_the_end(&mut codex).pop().unwrap();
    assert_eq!(end.kind, "run.failed", "a Codex run is lost on a line too long (codex-app-server-run.ts `lose`)");
    assert_eq!(end.payload["process"]["stderr"], "Codex JSON-RPC line exceeds 8 MiB");
}

// case: resolves a request with its own response
#[test]
fn resolves_a_request_with_its_own_response() {
    let (mut codex, _told) = stand_in(json!({"thread/start": [{"result": {"thread": {"id": "thread_own"}}}]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    assert_eq!(codex.normal.thread.as_deref(), Some("thread_own"));
}

// case: matches responses to the right request when they arrive out of order
#[test]
fn matches_responses_to_the_right_request() {
    // A stale answer to another id arrives first; the request still takes its own.
    let (mut codex, _told) = stand_in(json!({"thread/start": [
        {"send": {"jsonrpc": "2.0", "id": 77, "result": {"thread": {"id": "not_mine"}}}},
        {"result": {"thread": {"id": "thread_mine"}}},
    ]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    assert_eq!(codex.normal.thread.as_deref(), Some("thread_mine"));
}

// case: rejects with the server's message when the response is an error
#[test]
fn rejects_with_the_servers_message() {
    let (mut codex, _told) = stand_in(json!({"thread/start": [{"error": "no such thread"}]}));
    assert_eq!(codex.start(&turn_in(Mode::AcceptEdits)).unwrap_err().to_string(), "no such thread");
}

// case: reports a response nobody asked for rather than dropping it silently
#[test]
fn reports_a_response_nobody_asked_for() {
    let (mut codex, _told) = turn_writing(vec![json!({"send": {"jsonrpc": "2.0", "id": 999, "result": {}}}), json!({"raw": completed()})]);
    let events = to_the_end(&mut codex);
    assert_eq!(kinds(&events), ["adapter.diagnostic", "run.completed"]);
    assert_eq!(events[0].payload["message"], "A response arrived for an unknown request.");
}

// case: times out a request the server never answers
#[test]
fn times_out_a_request_never_answered() {
    let (mut codex, _told) = stand_in(json!({"turn/start": [{"silent": true}]}));
    codex.answer_within = Duration::from_millis(300);
    let refused = codex.start(&turn_in(Mode::AcceptEdits)).unwrap_err().to_string();
    assert!(refused.contains("did not answer"), "{refused}");
    assert_eq!(refused, "The app server did not answer turn/start in time.");
}

// case: refuses to queue beyond its in-flight limit
#[test]
fn refuses_to_queue_beyond_its_limit() {
    // A server that never reads: the pipe fills, then the bounded queue, and the run is stopped and says why rather than
    // hold more and more.
    let (mut codex, _told) = stand_in(json!({"$start": [{"sleep": 30}]}));
    let big = json!({"jsonrpc": "2.0", "method": "x", "params": {"pad": "x".repeat(64 * 1024)}});
    let mut refused = None;
    for _ in 0..400 {
        if let Err(e) = codex.send(big.clone()) {
            refused = Some(e.to_string());
            break;
        }
    }
    let refused = refused.expect("the queue took everything");
    assert!(refused.starts_with("Codex stdin queue unavailable; terminating run"), "{refused}");
    assert_eq!(to_the_end(&mut codex).pop().unwrap().kind, "run.failed");
}

// case: fails every in-flight request on dispose instead of hanging
#[test]
fn fails_an_in_flight_request_on_dispose() {
    let (codex, _told) = stand_in(json!({"turn/start": [{"silent": true}]}));
    let flag = Arc::new(AtomicBool::new(false));
    let mut codex = codex.with_cancel(flag.clone());
    let setter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        flag.store(true, Ordering::Relaxed);
    });
    let begun = Instant::now();
    let refused = codex.start(&turn_in(Mode::AcceptEdits)).unwrap_err().to_string();
    setter.join().unwrap();
    assert_eq!(refused, "Interrupted before the turn started");
    assert!(begun.elapsed() < Duration::from_secs(20));
}

// case: answers a server request with the handler's result
#[test]
fn answers_a_server_request_with_the_handlers_result() {
    let (mut codex, told) = stand_in(json!({"turn/start": [
        {"result": {"turn": {"id": "turn_1"}}},
        {"send": {"jsonrpc": "2.0", "id": "srv-1", "method": "item/commandExecution/requestApproval", "params": {"command": "echo hello", "threadId": "thread_9", "itemId": "c1"}}},
    ]}));
    let channel = Channel::new();
    codex.start(&Turn { approvals: Some(channel.clone()), ..turn_in(Mode::ApproveEach) }).unwrap();
    let decider = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Some(card) = channel.waiting().first() {
                channel
                    .decide(crate::approval::Answer::Decision {
                        approval_id: card.approval_id.clone(),
                        decision: crate::approval::Decision::ApproveOnce,
                        reason: None,
                    })
                    .unwrap();
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while told.replies().is_empty() && Instant::now() < deadline {
        let _ = codex.next_event().unwrap();
    }
    assert!(decider.join().unwrap(), "no card was raised");
    let reply = told.replies().pop().expect("the request was never answered");
    assert_eq!(reply["id"], "srv-1");
    assert_eq!(reply["result"]["decision"], "accept");
}

// case: still answers when the handler throws
#[test]
fn still_answers_when_there_is_no_one_to_ask() {
    // Electron's handler throws "no UI attached"; here Approve each has no card to put it on.
    let (mut codex, told) = stand_in(json!({"turn/start": [
        {"result": {"turn": {"id": "turn_1"}}},
        {"send": {"jsonrpc": "2.0", "id": 7, "method": "item/fileChange/requestApproval", "params": {"threadId": "thread_9"}}},
    ]}));
    codex.start(&turn_in(Mode::ApproveEach)).unwrap();
    let events = events_until_end(&mut codex, Duration::from_millis(800));
    let reply = told.replies().pop().expect("the request was never answered");
    assert_eq!(reply["id"], 7);
    assert_eq!(reply["result"]["decision"], "decline");
    assert!(of(&events, "adapter.diagnostic").iter().any(|e| e.payload["evidence"]["runtimeEventType"] == "item/fileChange/requestApproval"));
}

// case: does not confuse a server request with a response
#[test]
fn does_not_confuse_a_server_request_with_a_response() {
    // While initialize (id 1) waits, the server asks a question under the same id; the request takes only its answer.
    let (mut codex, told) = stand_in(json!({"initialize": [
        {"send": {"jsonrpc": "2.0", "id": 1, "method": "item/tool/requestUserInput", "params": {"questions": []}}},
        {"result": {"userAgent": "codex"}},
    ]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    assert_eq!(codex.normal.thread.as_deref(), Some("thread_9"));
    assert!(told.replies().iter().any(|r| r["id"] == 1 && r.get("result").is_some()), "the question was answered too");
}

// case: writes newline-delimited JSON-RPC with an incrementing id
#[test]
fn writes_newline_delimited_json_rpc_with_an_incrementing_id() {
    let (mut codex, told) = stand_in(json!({}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    assert!(told.lines().iter().all(|l| l.ends_with('\n') && !l[..l.len() - 1].contains('\n')));
    let written = told.written();
    assert_eq!(written[0]["jsonrpc"], "2.0");
    assert_eq!(written[0]["method"], "initialize");
    assert_eq!(written[0]["params"]["clientInfo"]["name"], "locust");
    let ids: Vec<u64> = written.iter().filter_map(|m| m["id"].as_u64()).collect();
    assert!(ids.len() >= 3 && ids.windows(2).all(|w| w[1] == w[0] + 1), "{ids:?}");
}

// case: sends notifications without an id
#[test]
fn sends_notifications_without_an_id() {
    let (mut codex, told) = stand_in(json!({}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    let initialized = told.written().into_iter().find(|m| m["method"] == "initialized").unwrap();
    assert_eq!(initialized["jsonrpc"], "2.0");
    assert!(initialized.get("id").is_none());
}

// case: goes quiet after dispose
#[test]
fn goes_quiet_after_the_run_ends() {
    let (mut codex, told) = turn_writing(vec![json!({"raw": completed()}), json!({"raw": delta("late")})]);
    assert_eq!(to_the_end(&mut codex).last().unwrap().kind, "run.completed");
    let written = told.written().len();
    assert!(events_until_end(&mut codex, Duration::from_millis(300)).is_empty(), "nothing is read after the end");
    assert!(!codex.steer("late"));
    assert_eq!(told.written().len(), written, "nothing is written after the end");
}
