//! Electron's packages/cli/test/an-editor-can-talk-to-a-locust-teammate.test.ts, against a door that answers as the
//! real one does. Permission cards are always answered in Locust here: the door gives other apps no tool for them, so
//! the cases about an editor answering one are Electron's opt-in alone.
use super::*;

struct Door {
    calls: Mutex<Vec<(String, Value)>>,
    /// read_reply's answers, in turn; the last one repeats.
    replies: Mutex<Vec<Value>>,
}

fn ok(text: &str, structured: Value) -> Value {
    json!({"content": [{"type": "text", "text": text}], "structuredContent": structured})
}

fn door(replies: Vec<Value>) -> Arc<Door> {
    Arc::new(Door { calls: Mutex::new(vec![]), replies: Mutex::new(replies) })
}

fn agent(d: &Arc<Door>, teammate: Option<&str>) -> (Arc<AcpAgent>, Arc<Mutex<Vec<Value>>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let (d2, s2) = (d.clone(), sent.clone());
    let call: Call = Arc::new(move |name: &str, args: &Value| {
        d2.calls.lock().unwrap().push((name.to_owned(), args.clone()));
        match name {
            "list_teammates" => ok("[...]", json!([{"id": "tm_codec", "name": "Codec", "runtime": "codex", "model": "gpt-6"}, {"id": "tm_pip", "name": "Pip", "runtime": "claude", "model": null}])),
            "start_conversation" | "send_message" => ok("{}", json!({"conversation_id": "mission_1", "mode": "ask"})),
            "read_reply" => {
                let mut replies = d2.replies.lock().unwrap();
                if replies.len() > 1 { replies.remove(0) } else { replies[0].clone() }
            }
            "stop_turn" => ok("Stopped.", json!({})),
            _ => json!({"content": [{"type": "text", "text": "No such tool."}], "isError": true}),
        }
    });
    let out: Out = Arc::new(move |m: Value| s2.lock().unwrap().push(m));
    (Arc::new(AcpAgent::new(call, out, teammate.map(str::to_owned)).quick()), sent)
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
    assert_eq!(names(&d), vec!["list_teammates"], "setting up started nothing");
    a.accept(&rpc(3, "fs/read_text_file", json!({})));
    assert_eq!(answer(&sent, 3)["error"]["message"], "Locust does not offer that ACP method. It offers no editor filesystem or terminal operations.");
    a.accept(&rpc(4, "initialize", json!({"protocolVersion": 1})));
    assert_eq!(answer(&sent, 4)["error"]["message"], "This editor connection is already initialized.");
}

#[test]
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
    let started = d.calls.lock().unwrap().iter().find(|(n, _)| n == "start_conversation").cloned().unwrap();
    assert_eq!((started.1["teammate"].as_str(), started.1["folder"].as_str()), (Some("tm_codec"), Some("/work/app")));
    a.accept(&rpc(6, "session/set_config_option", json!({"sessionId": session, "configId": "teammate", "value": "tm_pip"})));
    assert_eq!(answer(&sent, 6)["error"]["message"], "Create a new editor session to switch teammates after a conversation has started.");
}

#[test]
fn answers_with_the_reply_and_keeps_the_conversation_for_a_second_prompt() {
    let d = door(vec![ok("Still working: reading.", json!({"status": "working"})), ok("The tests pass.", json!({"status": "done"}))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    a.accept(&rpc(3, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "Run the tests"}]})));
    assert_eq!(answer(&sent, 3)["result"]["stopReason"], "end_turn");
    assert!(chunks(&sent).contains("The tests pass."));
    a.accept(&rpc(4, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "And lint?"}]})));
    answer(&sent, 4);
    let send = d.calls.lock().unwrap().iter().find(|(n, _)| n == "send_message").cloned().unwrap();
    assert_eq!(send.1["conversation_id"], "mission_1");
    assert_eq!(names(&d).iter().filter(|n| *n == "start_conversation").count(), 1);
}

#[test]
fn leaves_an_approval_in_locust_and_says_so_once() {
    let waiting = ok("Waiting for the person to approve or deny an action in Locust's window.", json!({"status": "waiting_for_person"}));
    let d = door(vec![waiting.clone(), waiting, ok("Done after approval.", json!({"status": "done"}))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    a.accept(&rpc(3, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "Delete the build folder"}]})));
    answer(&sent, 3);
    let said = chunks(&sent);
    assert_eq!(said.matches("Waiting for you in Locust:").count(), 1, "{said}");
    assert!(!sent.lock().unwrap().iter().any(|m| m["method"] == "session/request_permission"), "an editor never answers a card here");
}

#[test]
fn cancels_a_turn_and_stops_it_in_locust() {
    let d = door(vec![ok("Still working.", json!({"status": "working"}))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    a.accept(&rpc(3, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "Long job"}]})));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !names(&d).contains(&"read_reply".to_owned()) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    a.accept(&json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": session}}).to_string());
    assert_eq!(answer(&sent, 3)["result"]["stopReason"], "cancelled");
    assert!(names(&d).contains(&"stop_turn".to_owned()));
}

#[test]
fn rejects_concurrent_prompts_instead_of_starting_two_turns() {
    let d = door(vec![ok("Still working.", json!({"status": "working"}))]);
    let (a, sent) = agent(&d, None);
    let session = open(&a, &sent);
    a.accept(&rpc(3, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "One"}]})));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !names(&d).contains(&"start_conversation".to_owned()) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    a.accept(&rpc(4, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "Two"}]})));
    assert_eq!(answer(&sent, 4)["error"]["message"], "This session is still working. Cancel the turn or wait for its reply.");
    assert_eq!(names(&d).iter().filter(|n| *n == "start_conversation").count(), 1);
    a.accept(&json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": session}}).to_string());
    answer(&sent, 3);
}

#[test]
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
    assert!(!names(&d).contains(&"start_conversation".to_owned()));
}

#[test]
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
    let d = door(vec![ok("Done.", json!({"status": "done"}))]);
    let sent = Arc::new(Mutex::new(Vec::new()));
    let s2 = sent.clone();
    let call: Call = Arc::new(|name: &str, _: &Value| match name {
        "list_teammates" => ok("[]", json!([{"id": "tm_codec", "name": "Codec"}])),
        _ => json!({"content": [{"type": "text", "text": "That folder is not one Locust knows. Nothing was started."}], "isError": true}),
    });
    let a = Arc::new(AcpAgent::new(call, Arc::new(move |m: Value| s2.lock().unwrap().push(m)), None).quick());
    let _ = d;
    let session = open(&a, &sent);
    a.accept(&rpc(3, "session/prompt", json!({"sessionId": session, "prompt": [{"type": "text", "text": "Hi"}]})));
    assert_eq!(answer(&sent, 3)["error"]["message"], "That folder is not one Locust knows. Nothing was started.");
}
