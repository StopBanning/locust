//! other-apps-reach-locust-through-stdio.test.ts: the bridge's actual JSON-RPC loop and local HTTP listener.
use super::tests::{echo_door, temp};
use super::*;

#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn rpc(file: &Path, method: &str, params: Value) -> Value {
    let output = Output::default();
    let input = format!(
        "{}\n",
        json!({"jsonrpc":"2.0", "id":1, "method":method, "params":params})
    );
    bridge_io(file, std::io::Cursor::new(input), output.clone());
    let bytes = output.0.lock().unwrap();
    let reply: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(reply["id"], 1);
    assert_eq!(reply["jsonrpc"], "2.0");
    reply
}
fn call(file: &Path, name: &str) -> Value {
    rpc(file, "tools/call", json!({"name":name,"arguments":{}}))["result"].clone()
}

// case: advertises exactly the twelve tools, with Ask mode and spend made explicit
#[test]
fn advertises_tools_with_ask_and_spend_explicit() {
    let dir = temp("stdio-list");
    let file = dir.join(CONNECTION_FILE);
    let tools = rpc(&file, "tools/list", json!({}))["result"]["tools"].clone();
    let tools = tools.as_array().unwrap();
    assert_eq!(
        tools
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "list_teammates",
            "start_conversation",
            "send_message",
            "read_reply",
            "answer_question",
            "stop_turn",
            "list_conversations",
            "carry_on",
            "read_usage",
            "set_teammate_model",
            "list_models",
            "list_background_runs",
            "list_routines",
            "run_routine",
            "routine_status"
        ]
    );
    let description = |name| {
        tools.iter().find(|t| t["name"] == name).unwrap()["description"]
            .as_str()
            .unwrap()
    };
    assert!(
        description("start_conversation")
            .contains("Ask mode (read only), unless the person has turned on")
    );
    assert!(
        description("start_conversation")
            .contains("answered by the person in Locust's window, never from here")
    );
    assert!(description("start_conversation").contains("monthly limits apply"));
    for name in ["start_conversation", "send_message"] {
        assert!(
            description(name)
                .to_lowercase()
                .contains("no automatic teammate handoffs")
        );
    }
    assert_eq!(
        tools
            .iter()
            .filter(|t| t["annotations"]["readOnlyHint"] == true)
            .map(|t| t["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "list_teammates",
            "read_reply",
            "list_conversations",
            "read_usage",
            "list_models",
            "list_background_runs",
            "list_routines",
            "routine_status"
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

// case: answers every tool call with the required plain sentence when Locust is not running
#[test]
fn every_tool_answers_when_not_running() {
    let dir = temp("stdio-off");
    for name in [
        "list_teammates",
        "start_conversation",
        "send_message",
        "read_reply",
        "list_background_runs",
        "list_routines",
        "run_routine",
        "routine_status",
    ] {
        assert_eq!(
            call(&dir.join(CONNECTION_FILE), name),
            bridge_result("Locust is not running. Open it and try again.", true)
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

// case: gives the required not-running sentence for a stale file whose listener has closed
#[test]
fn stale_listener_is_not_running() {
    let dir = temp("stdio-stale");
    let door = echo_door(&dir);
    door.set_enabled(true);
    let file = door.connection_path();
    let saved = std::fs::read(&file).unwrap();
    door.set_enabled(false);
    std::fs::write(&file, saved).unwrap();
    assert_eq!(
        call(&file, "list_teammates"),
        bridge_result("Locust is not running. Open it and try again.", true)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

// case: does not retry or claim nothing ran when a connection drops after the app received the request
#[test]
fn dropped_request_is_never_retried() {
    let dir = temp("stdio-drop");
    let file = dir.join(CONNECTION_FILE);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    std::fs::write(&file,json!({"url":format!("http://{}/tools/call",listener.local_addr().unwrap()),"token":"a".repeat(64)}).to_string()).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let req = read_head(&mut stream).unwrap();
        let size: usize = req.header("content-length").unwrap().parse().unwrap();
        let mut body = req.rest;
        while body.len() < size {
            let mut b = [0; 1024];
            let n = stream.read(&mut b).unwrap();
            assert!(n > 0);
            body.extend_from_slice(&b[..n]);
        }
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap()["name"],
            "start_conversation"
        );
        drop(stream);
        listener.set_nonblocking(true).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert!(listener.accept().is_err(), "no second request");
    });
    assert_eq!(
        call(&file, "start_conversation"),
        bridge_result(
            "The connection to Locust ended without an answer. Nothing was retried; check its history before starting again.",
            true
        )
    );
    server.join().unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

// case: forwards an authenticated tool call, and sees a rotated connection without restarting the bridge
#[test]
fn authenticated_calls_reload_rotated_connection() {
    // One long-lived bridge with an interactive pipe, so re-reading is tested across calls in the same loop.
    use std::os::unix::net::UnixStream;
    let dir = temp("stdio-rotate");
    let door = echo_door(&dir);
    let file = door.connection_path();
    let (mut client, input) = UnixStream::pair().unwrap();
    let (output, read) = UnixStream::pair().unwrap();
    read.set_read_timeout(Some(crate::load_waits::UNDER_LOAD))
        .unwrap();
    let mut read = std::io::BufReader::new(read);
    let thread =
        std::thread::spawn(move || bridge_io(&file, std::io::BufReader::new(input), output));
    let mut request = |name: &str| {
        writeln!(
            client,
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name}})
        )
        .unwrap();
        let mut line = String::new();
        read.read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()["result"].clone()
    };
    door.set_enabled(true);
    let old = std::fs::read(door.connection_path()).unwrap();
    assert_eq!(
        request("list_teammates"),
        bridge_result("list_teammates {}", false)
    );
    door.set_enabled(false);
    assert_eq!(request("read_reply"), bridge_result("Locust is not running. Open it and try again.", true));
    door.set_enabled(true);
    assert_ne!(old, std::fs::read(door.connection_path()).unwrap());
    assert_eq!(
        request("list_background_runs"),
        bridge_result("list_background_runs {}", false)
    );
    drop(request);
    drop(client);
    thread.join().unwrap();
    door.dispose();
    std::fs::remove_dir_all(dir).unwrap();
}

// case: never sends a token to a non-loopback address from a modified profile file
#[test]
fn modified_profile_never_redirects_token() {
    let dir = temp("stdio-redirect");
    let file = dir.join(CONNECTION_FILE);
    std::fs::write(
        &file,
        json!({"url":"https://example.test/tools/call","token":"a".repeat(64)}).to_string(),
    )
    .unwrap();
    assert!(read_connection(&file).is_none());
    assert_eq!(
        call(&file, "list_teammates"),
        bridge_result("Locust is not running. Open it and try again.", true)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

// case: refuses unknown tools and methods without forwarding them
#[test]
fn unknown_tools_and_methods_stay_here() {
    let dir = temp("stdio-unknown");
    let file = dir.join(CONNECTION_FILE);
    assert_eq!(
        rpc(&file, "tools/call", json!({"name":"write_files"}))["error"]["code"],
        -32602
    );
    assert_eq!(rpc(&file, "exec", json!({}))["error"]["code"], -32601);
    assert_eq!(rpc(&file, "ping", json!({}))["result"], json!({}));
    std::fs::remove_dir_all(dir).unwrap();
}

// case: does not return a bearer token or connection metadata in the window settings
#[test]
fn window_settings_hide_connection_secrets() {
    let dir = temp("stdio-settings");
    let door = echo_door(&dir);
    let state = door.set_enabled(true);
    let connection: Value =
        serde_json::from_slice(&std::fs::read(door.connection_path()).unwrap()).unwrap();
    let displayed = format!("{state:?}");
    assert!(!displayed.contains(connection["token"].as_str().unwrap()));
    assert!(!displayed.contains(connection["url"].as_str().unwrap()));
    assert!(
        state
            .claude_command
            .unwrap()
            .contains(door.connection_path().to_str().unwrap())
    );
    door.dispose();
    std::fs::remove_dir_all(dir).unwrap();
}
