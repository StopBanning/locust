//! Editor protocol cases from packages/cli/test/an-editor-can-talk-to-a-locust-teammate.test.ts.
use super::*;

struct Door {
    calls: Mutex<Vec<(String, Value)>>,
    replies: Mutex<Vec<Value>>,
    next: Mutex<u64>,
}

fn team() -> Value {
    json!([{"id": "tm_codec", "name": "Codec", "runtime": "codex", "model": "gpt-6", "runs_in": "ask"},
        {"id": "tm_pip", "name": "Pip", "runtime": "claude", "model": null, "runs_in": "ask"}])
}

fn approval(kind: &str, answer_in_locust: bool) -> Value {
    json!({"approvalId":"card-1", "kind":kind, "summary":"Run the check", "detail":"A file changed", "answerInLocust":answer_in_locust})
}

fn status(phase: &str, events: Value, approvals: Value) -> Value {
    json!({"turn":{"phase":phase},"events":events,"cursor":events.as_array().unwrap().len(),"approvals":approvals})
}

fn event(sequence: u64, operation: &str, text: &str) -> Value {
    json!({"sequence":sequence,"itemId":"reply","operation":operation,"text":text})
}

fn ok(text: &str, state: Value) -> Value {
    match state["status"].as_str() {
        Some("working") => status("running", json!([]), json!([])),
        Some("waiting_for_person") => status("running", json!([]), json!([approval("file-change", true)])),
        _ => status("completed", json!([event(1, "replace", text)]), json!([])),
    }
}

fn door(replies: Vec<Value>) -> Arc<Door> {
    Arc::new(Door { calls: Mutex::new(vec![]), replies: Mutex::new(replies), next: Mutex::new(0) })
}

impl Door {
    fn call(&self, name: &str, args: Value) -> Result<Value, LocalError> {
        self.calls.lock().unwrap().push((name.to_owned(), args.clone()));
        match name {
            "list_teammates" => Ok(team()),
            "cli/open" => {
                let rows = team();
                let asked = args["teammate"].as_str().unwrap_or("tm_codec");
                let teammate = rows.as_array().unwrap().iter().find(|t| t["id"] == asked ||
                    t["name"].as_str().unwrap().eq_ignore_ascii_case(asked)).unwrap();
                let mut next = self.next.lock().unwrap();
                *next += 1;
                Ok(json!({"sessionId":format!("fake-{next}"),"key":"secret-fake-key","teammate":teammate,"workspace":args["cwd"]}))
            }
            "cli/prompt" | "cli/cancel" | "cli/close" => Ok(json!({"ok":true})),
            "cli/decide" => {
                *self.replies.lock().unwrap() = vec![status("completed", json!([]), json!([]))];
                Ok(json!({"ok":true}))
            }
            "cli/poll" => {
                let request = self.calls.lock().unwrap().iter().rev()
                    .find(|(n, a)| n == "cli/prompt" && a["sessionId"] == args["sessionId"])
                    .unwrap().1["requestId"].clone();
                let mut replies = self.replies.lock().unwrap();
                let mut reply = if replies.len() > 1 { replies.remove(0) } else { replies[0].clone() };
                if reply["turn"].get("requestId").is_none() {
                    reply["turn"]["requestId"] = request;
                }
                Ok(reply)
            }
            other => Err(LocalError(format!("Unexpected door request: {other}"))),
        }
    }
}

fn agent(d: &Arc<Door>, teammate: Option<&str>) -> (Arc<AcpAgent>, Arc<Mutex<Vec<Value>>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let (d2, s2) = (d.clone(), sent.clone());
    let client = LocustClient::for_test(Arc::new(move |name, args| d2.call(name, args)));
    let out: Out = Arc::new(move |message| s2.lock().unwrap().push(message));
    (Arc::new(AcpAgent::new(client, out, teammate.map(str::to_owned)).quick()), sent)
}

fn rpc(id: u64, method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

fn answer(sent: &Arc<Mutex<Vec<Value>>>, id: u64) -> Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(m) = sent.lock().unwrap().iter().find(|m| m["id"] == id) {
            return m.clone();
        }
        assert!(std::time::Instant::now() < deadline, "no answer to {id}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn chunks(sent: &Arc<Mutex<Vec<Value>>>) -> String {
    sent.lock().unwrap().iter().filter(|m| m["method"] == "session/update").filter_map(|m| m["params"]["update"]["content"]["text"].as_str().map(str::to_owned)).collect()
}

fn names(d: &Arc<Door>) -> Vec<String> {
    d.calls.lock().unwrap().iter().map(|(n, _)| n.clone()).collect()
}

fn open(a: &Arc<AcpAgent>, sent: &Arc<Mutex<Vec<Value>>>) -> String {
    a.accept(&rpc(1, "initialize", json!({"protocolVersion": 1})));
    a.accept(&rpc(2, "session/new", json!({"cwd": "/work/app", "mcpServers": []})));
    answer(sent, 2)["result"]["sessionId"].as_str().unwrap().to_owned()
}

#[test]
// case: offers text only, no editor files or terminal, and starts nothing during setup
fn offers_text_only_no_editor_files_or_terminal_and_starts_nothing_during_setup() {
    let d = door(vec![ok("Done.", json!({"status": "done"}))]);
    let (a, sent) = agent(&d, None);
    a.accept(&rpc(1, "initialize", json!({"protocolVersion": 1})));
    let init = answer(&sent, 1)["result"].clone();
    assert_eq!(init["protocolVersion"], 1);
    assert_eq!(init["agentCapabilities"]["promptCapabilities"], json!({"image": false, "audio": false, "embeddedContext": false}));
    assert_eq!(init["agentCapabilities"]["loadSession"], false);
    assert_eq!(init["agentInfo"]["name"], "locust");
    a.accept(&rpc(2, "session/new", json!({"cwd": "/work/app", "mcpServers": []})));
    assert!(answer(&sent, 2)["result"]["sessionId"].is_string());
    assert_eq!(names(&d), vec!["list_teammates", "cli/open"], "setting up started nothing");
    a.accept(&rpc(3, "fs/read_text_file", json!({})));
    assert_eq!(answer(&sent, 3)["error"]["message"], "Locust does not offer that ACP method. It offers no editor filesystem or terminal operations.");
    a.accept(&rpc(4, "initialize", json!({"protocolVersion": 1})));
    assert_eq!(answer(&sent, 4)["error"]["message"], "This editor connection is already initialized.");
}

#[test]
// case: selects a teammate by flag or config option before sending
fn selects_a_teammate_by_flag_or_config_option_before_sending() {
    let d = door(vec![ok("Done.", json!({"status": "done"}))]);
    let (a, sent) = agent(&d, Some("pip"));
    let session = open(&a, &sent);
    let options = answer(&sent, 2)["result"]["configOptions"][0].clone();
    assert_eq!(options["currentValue"], "tm_pip");
    assert_eq!(options["options"][1]["name"], "Pip (claude / model not set)");
    a.accept(&rpc(3, "session/set_config_option", json!({"sessionId": session, "configId": "teammate", "value": "tm_codec"})));
    assert_eq!(answer(&sent, 3)["result"]["configOptions"][0]["currentValue"], "tm_codec");
    a.accept(&rpc(4, "session/set_config_option", json!({"sessionId": session, "configId": "teammate", "value": "tm_nobody"})));
    assert_eq!(answer(&sent, 4)["error"]["message"], "Choose a teammate from this session\u{2019}s Teammate option.");
    a.accept(&rpc(5, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "Hi"}]})));
    answer(&sent, 5);
    let calls = d.calls.lock().unwrap().clone();
    let selected = calls.iter().rev().find(|(n, _)| n == "cli/open").unwrap();
    assert_eq!((selected.1["teammate"].as_str(), selected.1["cwd"].as_str()), (Some("tm_codec"), Some("/work/app")));
    let started = calls.iter().find(|(n, _)| n == "cli/prompt").unwrap();
    assert_eq!(started.1["sessionId"], "fake-2");
    a.accept(&rpc(6, "session/set_config_option", json!({"sessionId": session, "configId": "teammate", "value": "tm_pip"})));
    assert_eq!(answer(&sent, 6)["error"]["message"], "Create a new editor session to switch teammates after a conversation has started.");
}

#[test]
// case: streams a reply and keeps the conversation for a second prompt
fn streams_a_reply_and_keeps_the_conversation_for_a_second_prompt() {
    let d = door(vec![status("running", json!([event(1, "append", "The tests ")]), json!([])),
        status("completed", json!([event(2, "replace", "The tests pass.")]), json!([])),
        status("completed", json!([event(1, "replace", "Lint passes.")]), json!([]))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    a.accept(&rpc(3, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "Run the tests"}]})));
    assert_eq!(answer(&sent, 3)["result"]["stopReason"], "end_turn");
    assert!(chunks(&sent).contains("The tests pass."));
    a.accept(&rpc(4, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "And lint?"}]})));
    answer(&sent, 4);
    let calls = d.calls.lock().unwrap();
    let sent_to: Vec<_> = calls.iter().filter(|(n, _)| n == "cli/prompt").map(|(_, a)| a["sessionId"].as_str().unwrap()).collect();
    assert_eq!(sent_to, ["fake-1", "fake-1"]);
    assert_eq!(chunks(&sent), "The tests pass.Lint passes.");
}

#[test]
// case: leaves an approval in Locust while the person has not let editors answer
fn leaves_an_approval_in_locust_and_says_so_once() {
    let waiting = ok("Waiting for the person to approve or deny an action in Locust's window.", json!({"status": "waiting_for_person"}));
    let d = door(vec![waiting.clone(), waiting, ok("Done after approval.", json!({"status": "done"}))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    a.accept(&rpc(3, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "Delete the build folder"}]})));
    answer(&sent, 3);
    let said = chunks(&sent);
    assert_eq!(said.matches("Waiting for you in Locust:").count(), 1, "{said}");
    assert!(!sent.lock().unwrap().iter().any(|m| m["method"] == "session/request_permission"));
    assert!(!names(&d).contains(&"cli/decide".to_owned()));
}

#[test]
fn cancels_a_turn_and_stops_it_in_locust() {
    let d = door(vec![ok("Still working.", json!({"status": "working"}))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    a.accept(&rpc(3, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "Long job"}]})));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !names(&d).contains(&"cli/poll".to_owned()) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    a.accept(&json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": session}}).to_string());
    assert_eq!(answer(&sent, 3)["result"]["stopReason"], "cancelled");
    assert!(names(&d).contains(&"cli/cancel".to_owned()));
}

#[test]
// case: rejects concurrent prompts instead of starting two turns
fn rejects_concurrent_prompts_instead_of_starting_two_turns() {
    let d = door(vec![ok("Still working.", json!({"status": "working"}))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    a.accept(&rpc(3, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "One"}]})));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !names(&d).contains(&"cli/prompt".to_owned()) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    a.accept(&rpc(4, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "Two"}]})));
    assert_eq!(answer(&sent, 4)["error"]["message"], "This session is still working. Cancel the turn or wait for its reply.");
    assert_eq!(names(&d).iter().filter(|n| *n == "cli/prompt").count(), 1);
    a.accept(&json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": session}}).to_string());
    answer(&sent, 3);
}

#[test]
// case: rejects files, editor MCP programs and unknown sessions before starting
fn rejects_files_editor_mcp_programs_and_unknown_sessions_before_starting() {
    let d = door(vec![ok("Done.", json!({"status": "done"}))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    a.accept(&rpc(3, "session/new", json!({"cwd": "/work/app", "mcpServers": [{"name": "x"}]})));
    assert!(answer(&sent, 3)["error"]["message"].as_str().unwrap().starts_with("Use an absolute cwd and an empty mcpServers list."));
    a.accept(&rpc(4, "session/new", json!({"cwd": "relative/path", "mcpServers": []})));
    assert!(answer(&sent, 4)["error"].is_object());
    a.accept(&rpc(5, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "image", "data": "x"}]})));
    assert_eq!(answer(&sent, 5)["error"]["message"], "Locust accepts text prompts only. Send text, not a file, image, audio, or embedded resource.");
    a.accept(&rpc(6, "session/prompt", json!({"sessionId": "nope", "prompt": [{"type": "text", "text": "x"}]})));
    assert_eq!(answer(&sent, 6)["error"]["message"], NOT_OPEN);
    a.accept(&rpc(7, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "x".repeat(8_001)}]})));
    assert_eq!(answer(&sent, 7)["error"]["message"], "Write a text prompt of up to 8,000 characters. Nothing was sent.");
    assert!(!names(&d).contains(&"cli/prompt".to_owned()));
}

#[test]
// case: rejects malformed JSON and methods without sending hidden turns
fn rejects_malformed_json_and_methods_without_sending_hidden_turns() {
    let d = door(vec![ok("Done.", json!({"status": "done"}))]);
    let (a, sent) = agent(&d, None);
    a.accept("{not json");
    assert_eq!(sent.lock().unwrap()[0]["error"]["code"], -32700);
    a.accept(&json!({"jsonrpc": "1.0", "id": 1, "method": "initialize"}).to_string());
    assert_eq!(sent.lock().unwrap()[1]["error"]["message"], "Use a JSON-RPC 2.0 object.");
    a.accept(&rpc(2, "session/new", json!({"cwd": "/w", "mcpServers": []})));
    assert_eq!(answer(&sent, 2)["error"]["message"], "Initialize this ACP connection before creating a session.");
    a.accept(&rpc(3, "initialize", json!({"protocolVersion": 2})));
    assert_eq!(answer(&sent, 3)["error"]["message"], "Locust speaks Agent Client Protocol version 1. Update the editor\u{2019}s ACP client.");
    // A prompt with no id to answer is never a turn.
    a.accept(&json!({"jsonrpc": "2.0", "method": "session/prompt", "params": {"sessionId": "x", "prompt": [{"type": "text", "text": "hidden"}]}}).to_string());
    std::thread::sleep(Duration::from_millis(30));
    assert!(names(&d).is_empty());
}

#[test]
fn a_refusal_from_the_door_is_said_in_its_own_words() {
    let d = door(vec![]);
    let sent = Arc::new(Mutex::new(Vec::new()));
    let s2 = sent.clone();
    let client = LocustClient::for_test(Arc::new(move |name, args| match name {
        "cli/prompt" => Err(LocalError("That folder is not one Locust knows. Nothing was started.".into())),
        _ => d.call(name, args),
    }));
    let a = Arc::new(AcpAgent::new(client, Arc::new(move |m: Value| s2.lock().unwrap().push(m)), None).quick());
    let session = open(&a, &sent);
    a.accept(&rpc(3, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "Hi"}]})));
    assert_eq!(answer(&sent, 3)["error"]["message"], "That folder is not one Locust knows. Nothing was started.");
}

fn find_message(sent: &Mutex<Vec<Value>>, method: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(message) = sent.lock().unwrap().iter().find(|m| m["method"] == method) {
            return message.clone();
        }
        assert!(Instant::now() < deadline, "no {method}");
        std::thread::yield_now();
    }
}

fn ask(a: &Arc<AcpAgent>, session: &str) {
    a.accept(&rpc(3, "session/prompt", json!({"sessionId":session,"prompt":[{"type":"text","text":"Check it"}]})));
}

// case: maps permission outcome %s to at most one once-only decision
#[test]
fn maps_permission_outcome_to_at_most_one_once_only_decision() {
    for option in ["allow", "deny", "not-an-option", "cancelled"] {
        let d = door(vec![status("running", json!([]), json!([approval("file-change", false)]))]);
        let (a, sent) = agent(&d, None);
        let session = open(&a, &sent);
        ask(&a, &session);
        let request = find_message(&sent, "session/request_permission");
        assert_eq!(request["params"]["sessionId"], session);
        assert_eq!(request["params"]["toolCall"]["toolCallId"], "card-1");
        assert_eq!(request["params"]["toolCall"]["kind"], "edit");
        assert_eq!(request["params"]["options"], json!([
            {"optionId":"allow","kind":"allow_once","name":"Allow once"},
            {"optionId":"deny","kind":"reject_once","name":"Deny"}
        ]));
        assert!(!request.to_string().contains("secret-fake-key"));
        let outcome = if option == "cancelled" { json!({"outcome":"cancelled"}) } else { json!({"outcome":"selected","optionId":option}) };
        let reply = json!({"jsonrpc":"2.0","id":request["id"],"result":{"outcome":outcome}}).to_string();
        a.accept(&reply);
        a.accept(&reply);
        assert_eq!(answer(&sent, 3)["result"]["stopReason"], "end_turn");
        let calls = d.calls.lock().unwrap();
        let decisions: Vec<_> = calls.iter().filter(|(name, _)| name == "cli/decide").map(|(_, a)| a.clone()).collect();
        assert_eq!(decisions, [json!({"sessionId":"fake-1","key":"secret-fake-key","approvalId":"card-1","approve":option == "allow"})]);
    }
}

// case: cancels while a permission is open and ignores its late Allow
#[test]
fn cancels_while_a_permission_is_open_and_ignores_its_late_allow() {
    let d = door(vec![status("running", json!([]), json!([approval("file-change", false)]))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    ask(&a, &session);
    let request = find_message(&sent, "session/request_permission");
    a.accept(&json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":session}}).to_string());
    assert_eq!(answer(&sent, 3)["result"]["stopReason"], "cancelled");
    a.accept(&json!({"jsonrpc":"2.0","id":request["id"],"result":{"outcome":{"outcome":"selected","optionId":"allow"}}}).to_string());
    let calls = d.calls.lock().unwrap();
    let cancels: Vec<_> = calls.iter().filter(|(name, _)| name == "cli/cancel").collect();
    assert_eq!(cancels.len(), 1);
    let started = calls.iter().find(|(name, _)| name == "cli/prompt").unwrap();
    assert_eq!(cancels[0].1["requestId"], started.1["requestId"]);
    assert!(!calls.iter().any(|(name, _)| name == "cli/decide"));
}

// case: does not turn a question into permission to act
#[test]
fn does_not_turn_a_question_into_permission_to_act() {
    // A question stays local even if its reply omitted the answer-in-Locust flag.
    let d = door(vec![status("running", json!([]), json!([approval("question", false)])),
        status("completed", json!([]), json!([]))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    ask(&a, &session);
    assert_eq!(answer(&sent, 3)["result"]["stopReason"], "end_turn");
    assert!(chunks(&sent).contains("Waiting for you in Locust: Run the check"));
    assert!(!sent.lock().unwrap().iter().any(|m| m["method"] == "session/request_permission"));
    assert!(!names(&d).contains(&"cli/decide".into()));
}

#[test]
fn a_stale_poll_cannot_offer_another_prompts_card() {
    let mut stale = status("running", json!([]), json!([approval("file-change", false)]));
    stale["turn"]["requestId"] = json!("previous-request");
    let d = door(vec![stale, status("completed", json!([]), json!([]))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    ask(&a, &session);
    assert!(answer(&sent, 3)["error"]["message"].as_str().unwrap().contains("unreadable"));
    assert!(!sent.lock().unwrap().iter().any(|m| m["method"] == "session/request_permission"));
    assert!(!names(&d).contains(&"cli/decide".into()));
}

#[test]
fn a_prompt_is_reserved_before_the_next_cancel_line() {
    let d = door(vec![status("running", json!([]), json!([]))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    ask(&a, &session);
    a.accept(&json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":session}}).to_string());
    assert_eq!(answer(&sent, 3)["result"]["stopReason"], "cancelled");
    let calls = d.calls.lock().unwrap();
    let started = calls.iter().find(|(n, _)| n == "cli/prompt");
    if let Some((_, started)) = started {
        assert!(calls.iter().any(|(n, a)| n == "cli/cancel" && a["requestId"] == started["requestId"]));
    }
}

// case: speaks newline-framed JSON on real stdio and stops its turn on EOF
#[test]
fn speaks_newline_framed_json_on_real_stdio_and_stops_its_turn_on_eof() {
    use std::io::{BufRead, BufReader, Write};

    let (in_read, mut in_write) = std::io::pipe().unwrap();
    let (out_read, out_write) = std::io::pipe().unwrap();
    let d = door(vec![status("running", json!([]), json!([]))]);
    let d2 = d.clone();
    let client = LocustClient::for_test(Arc::new(move |name, args| d2.call(name, args)));
    let worker = std::thread::spawn(move || acp_stdio(client, None, BufReader::new(in_read), out_write));
    let (out_tx, out_rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(out_read).lines() {
            out_tx.send(line.unwrap()).unwrap();
        }
    });
    let recv = |id| {
        let line = out_rx.recv_timeout(Duration::from_secs(10)).expect("ACP reply line");
        assert!(!line.contains("secret-fake-key"));
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["jsonrpc"], "2.0");
        assert_eq!(reply["id"], id);
        reply
    };
    writeln!(in_write, "{}", rpc(1, "initialize", json!({"protocolVersion": 1}))).unwrap();
    assert_eq!(recv(1)["result"]["protocolVersion"], 1);
    writeln!(in_write, "{}", rpc(2, "session/new", json!({"cwd":"/work/app","mcpServers":[]}))).unwrap();
    let session = recv(2)["result"]["sessionId"].as_str().unwrap().to_owned();
    writeln!(in_write, "{}", rpc(3, "session/prompt", json!({"sessionId":session,"prompt":[{"type":"text","text":"Wait"}]}))).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !names(&d).contains(&"cli/poll".into()) {
        assert!(Instant::now() < deadline, "ACP did not reach its active turn");
        std::thread::yield_now();
    }
    drop(in_write);
    worker.join().unwrap();
    reader.join().unwrap();
    let called = names(&d);
    let cancel = called.iter().position(|n| n == "cli/cancel").expect("EOF cancels");
    let close = called.iter().position(|n| n == "cli/close").expect("EOF closes");
    assert!(cancel < close);
    for line in out_rx.try_iter() {
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["jsonrpc"], "2.0");
        assert!(!line.contains("secret-fake-key"));
    }
}

/// Drives the production HTTP door and session guard, with only the engine behind them replaced.
#[test]
fn grant_first_then_an_editor_can_answer_only_its_own_edit_once() {
    use crate::cli_sessions::{CliBegun, CliCard, CliHost, CliSessions, CliSink, CliStart, CliTeammate, CliUpdate};
    use crate::door::{Caller, Door as HttpDoor, Settings};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Engine {
        settings: Arc<Mutex<Settings>>,
        starts: AtomicUsize,
        pending: Mutex<Option<(String, CliSink)>>,
        decisions: Mutex<Vec<(String, bool)>>,
    }

    impl CliHost for Engine {
        fn teammates(&self) -> Result<Vec<CliTeammate>, String> {
            Ok(vec![CliTeammate { id: "tm_codec".into(), name: "Codec".into(), runtime: Some("codex".into()),
                model: Some("gpt-6".into()), runs_in: "ask".into() }])
        }
        fn workspace(&self) -> Option<String> { Some("/work/app".into()) }
        fn start(&self, request: CliStart, update: CliSink) -> Result<CliBegun, String> {
            assert_eq!(request.mode, "ask");
            assert_eq!(request.teammate_id, "tm_codec");
            let i = self.starts.fetch_add(1, Ordering::SeqCst);
            let mission_id = format!("mission-{i}");
            let run_id = format!("run-{i}");
            *self.pending.lock().unwrap() = Some((mission_id.clone(), update));
            Ok(CliBegun { mission_id, run_id })
        }
        fn cancel(&self, _: &str) {}
        fn approvals(&self) -> Vec<CliCard> {
            let held = self.pending.lock().unwrap();
            let Some((mission_id, _)) = held.as_ref() else { return Vec::new(); };
            let own = CliCard { approval_id: format!("{mission_id}-edit"), mission_id: mission_id.clone(),
                run_id: mission_id.replace("mission-", "run-"), kind: "file-change".into(),
                summary: "Review generated note".into(), detail: "notes.txt".into(),
                cwd: Some("/work/app".into()), answer_in: None };
            let foreign = CliCard { approval_id: "foreign-edit".into(), mission_id: "foreign".into(), ..own.clone() };
            vec![foreign, own]
        }
        fn answer(&self, id: &str, approve: bool) -> bool {
            self.decisions.lock().unwrap().push((id.into(), approve));
            self.finish();
            true
        }
        fn editor_answers(&self) -> bool { self.settings.lock().unwrap().editor_answers }
        fn status(&self) -> Result<Value, String> { Ok(json!({"turns":[]})) }
    }

    impl Engine {
        fn finish(&self) {
            if let Some((_, sink)) = self.pending.lock().unwrap().take() {
                sink(CliUpdate::Event(json!({"type":"run.completed","payload":{}})));
            }
        }
    }

    let root = std::env::temp_dir().join(format!("locust-acp-http-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    struct Temp(std::path::PathBuf);
    impl Drop for Temp { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    let _tmp = Temp(root.clone());
    let engine = Arc::new(Engine::default());
    let sessions = Arc::new(CliSessions::live(engine.clone()));
    let called = Arc::new(Mutex::new(Vec::<String>::new()));
    let called2 = called.clone();
    let call: Caller = Arc::new(move |name, args| {
        called2.lock().unwrap().push(name.into());
        if name == "list_teammates" {
            crate::door_tools::ToolResult::ok(team().to_string()).to_json()
        } else {
            sessions.call(name, args).to_json()
        }
    });
    let door = HttpDoor::with(root.clone(), "fake", engine.settings.clone(), call);
    assert!(door.set_enabled(true).settings.enabled, "the test needs its own loopback door");
    let sent = Arc::new(Mutex::new(Vec::new()));
    let sent2 = sent.clone();
    let agent = Arc::new(AcpAgent::new(LocustClient::new(root), Arc::new(move |m| sent2.lock().unwrap().push(m)), None));
    let session = open(&agent, &sent);

    ask(&agent, &session);
    find_message(&sent, "session/update");
    assert!(chunks(&sent).contains("Waiting for you in Locust: Review generated note"));
    assert!(!sent.lock().unwrap().iter().any(|m| m["method"] == "session/request_permission"));
    assert!(engine.decisions.lock().unwrap().is_empty());
    engine.finish();
    assert_eq!(answer(&sent, 3)["result"]["stopReason"], "end_turn");

    assert!(door.set_editor_answers(true).settings.editor_answers);
    agent.accept(&rpc(4, "session/prompt", json!({"sessionId":session,"prompt":[{"type":"text","text":"Check again"}]})));
    let permission = find_message(&sent, "session/request_permission");
    assert_eq!(permission["params"]["toolCall"]["toolCallId"], "mission-1-edit");
    assert!(!sent.lock().unwrap().iter().any(|m| m.to_string().contains("foreign-edit")));
    let allow = json!({"jsonrpc":"2.0","id":permission["id"],"result":{"outcome":{"outcome":"selected","optionId":"allow"}}}).to_string();
    agent.accept(&allow);
    agent.accept(&allow);
    assert_eq!(answer(&sent, 4)["result"]["stopReason"], "end_turn");
    assert_eq!(*engine.decisions.lock().unwrap(), [("mission-1-edit".to_string(), true)]);
    assert_eq!(called.lock().unwrap().iter().filter(|n| *n == "cli/decide").count(), 1);
    agent.close();
}

fn run_permission_prompt(a: &AcpAgent, session: &str) {
    let prompt = a.prepare_prompt(&json!({"sessionId":session,"prompt":[{"type":"text","text":"Check it"}]}))
        .unwrap_or_else(|RpcError(_, words)| panic!("{words}"));
    let result = a.prompt(prompt).unwrap_or_else(|RpcError(_, words)| panic!("{words}"));
    assert_eq!(result["stopReason"], "end_turn");
}

#[test]
fn an_expired_editor_permission_is_denied_at_the_door() {
    let d = door(vec![status("completed", json!([]), json!([approval("file-change", false)]))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    a.lock().permissions.insert("locust-permission-1".into(), Permission {
        editor_session: session.clone(), approval: "card-1".into(),
        expires: Instant::now() - Duration::from_secs(1), answered: None,
    });

    run_permission_prompt(&a, &session);

    let calls = d.calls.lock().unwrap();
    let decisions: Vec<_> = calls.iter().filter(|(name, _)| name == "cli/decide").map(|(_, args)| args.clone()).collect();
    assert_eq!(decisions, [json!({"sessionId":"fake-1","key":"secret-fake-key","approvalId":"card-1","approve":false})]);
}
