//! Electron's `main/permission-host.test.ts`: Locust answers Claude Code's permission questions, and only Locust's own
//! runs can ask. As in Electron these go over the real wire, here the run's private Unix socket
//! (`runtime/permission_bridge.rs`), because what goes wrong is the wire. Electron's host is a token-checked loopback
//! HTTP server that every run shares; the native bridge is one socket per run in a folder only this user can open, so
//! the token, the shared listener and Electron's `bridgeConfig` env have no counterpart (see the row).

use crate::approval::{always_covers, describe_claude_tool, tool_use_of, Answer, Channel, Decision, Kind, Request};
use crate::runtime::claude::BRIDGE_TOOL;
use crate::runtime::permission_bridge::Bridge;
use crate::store::Store;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A temporary store for the saved rules the channel reads before raising a card, so no test reads the person's own.
struct Host {
    channel: Arc<Channel>,
    root: PathBuf,
    _hook: crate::native::hooks::Guard,
}
impl Drop for Host {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn host() -> Host {
    let root = std::env::temp_dir().join(format!(
        "locust-permission-host-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let hook = crate::native::hooks::install(store, None);
    Host { channel: Channel::new(), root, _hook: hook }
}

fn register(host: &Host, mission: &str, cwd: &str) -> Bridge {
    Bridge::prepare(host.channel.clone(), mission, &format!("run_{mission}"), cwd, Arc::new(AtomicBool::new(false))).unwrap()
}

fn config_of(bridge: &Bridge) -> Value {
    serde_json::from_str(&std::fs::read_to_string(bridge.config()).unwrap()).unwrap()
}

fn socket_of(bridge: &Bridge) -> String {
    config_of(bridge)["mcpServers"]["locust"]["args"][1].as_str().unwrap().to_owned()
}

/// What the bridge's MCP side sends for one `approve` call: Claude Code's arguments.
fn ask(bridge: &Bridge, asked: Value) -> UnixStream {
    let mut stream = UnixStream::connect(socket_of(bridge)).unwrap();
    writeln!(stream, "{asked}").unwrap();
    stream
}

fn answer(stream: UnixStream) -> Value {
    // A request nobody answers is a failure here, not a wait as long as the bridge's own.
    let _ = stream.set_read_timeout(Some(crate::load_waits::UNDER_LOAD));
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap()
}

/// The next card raised that is not one of `seen`.
fn card(channel: &Channel, seen: &[String]) -> Request {
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    loop {
        if let Some(r) = channel.waiting().into_iter().find(|r| !seen.contains(&r.approval_id)) {
            return r;
        }
        assert!(Instant::now() < deadline, "no card was raised");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn decide(channel: &Channel, card: &Request, decision: Decision, reason: Option<&str>) -> Result<crate::approval::Recorded, String> {
    channel.decide(Answer::Decision { approval_id: card.approval_id.clone(), decision, reason: reason.map(str::to_owned) })
}

// ---- the permission host ----

#[test]
fn takes_a_card_down_when_the_bridge_stops_waiting_for_it_so_the_question_asked_again_is_the_one_card() {
    let host = host();
    let bridge = register(&host, "m_late", "C:/work");
    let stream = ask(&bridge, json!({"tool_name": "Write", "input": {"file_path": "C:/work/a.txt", "content": "OK"}}));
    let raised = card(&host.channel, &[]);
    // The bridge's own patience runs out: it lets go of the request without an answer.
    stream.shutdown(std::net::Shutdown::Both).unwrap();
    drop(stream);
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    while !host.channel.waiting().is_empty() {
        assert!(Instant::now() < deadline, "the card stayed up after the bridge let go");
        std::thread::sleep(Duration::from_millis(5));
    }
    // Pressing the old card now answers nothing.
    assert!(decide(&host.channel, &raised, Decision::ApproveOnce, None).is_err());
}

#[test]
fn leaves_an_answered_card_alone_when_its_connection_closes_after_the_answer() {
    let host = host();
    let bridge = register(&host, "m_ok", "C:/work");
    let stream = ask(&bridge, json!({"tool_name": "Bash", "input": {"command": "npm test"}}));
    let raised = card(&host.channel, &[]);
    assert!(decide(&host.channel, &raised, Decision::ApproveOnce, None).is_ok());
    assert_eq!(answer(stream)["behavior"], "allow");
    std::thread::sleep(Duration::from_millis(50));
    let recorded = host.channel.take_recorded_for("m_ok");
    assert_eq!(recorded.len(), 1);
    assert_eq!((recorded[0].approval_id.as_str(), recorded[0].by.as_str()), (raised.approval_id.as_str(), "card"));
}

#[test]
fn only_this_run_can_reach_its_bridge_and_a_request_that_is_not_a_tool_call_raises_no_card() {
    let host = host();
    let bridge = register(&host, "m1", "C:/work");
    // Electron checks a token before reading anything else; the native bridge's socket sits in a folder only this user
    // can open, and what reaches it without a tool name is refused without the window being asked.
    let folder = PathBuf::from(socket_of(&bridge)).parent().unwrap().to_path_buf();
    assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&folder).unwrap().permissions()) & 0o777, 0o700);
    let refused = answer(ask(&bridge, json!({"token": "not-ours", "toolName": "mcp__claude_ai_Robinhood__place_equity_order"})));
    assert_eq!(refused["behavior"], "deny");
    assert!(host.channel.waiting().is_empty());
}

#[test]
fn turns_a_question_into_the_card_the_app_already_has_and_answers_with_the_decision() {
    let host = host();
    let bridge = register(&host, "m1", "C:/work");
    assert_eq!(BRIDGE_TOOL, "mcp__locust__approve");
    let stream = ask(&bridge, json!({"tool_name": "mcp__claude_ai_Robinhood__get_watchlists", "input": {"limit": 5}}));
    let raised = card(&host.channel, &[]);
    assert_eq!(raised.mission_id, "m1");
    assert_eq!(raised.kind, Kind::Connector);
    assert_eq!(raised.summary, "Use get_watchlists on Robinhood");
    assert_eq!(raised.cwd.as_deref(), Some("C:/work"));
    assert!(raised.detail.contains("\"limit\": 5"), "{}", raised.detail);
    assert!(decide(&host.channel, &raised, Decision::ApproveOnce, None).is_ok());
    assert_eq!(answer(stream), json!({"behavior": "allow", "updatedInput": {"limit": 5}}));
}

#[test]
fn names_the_call_that_asks_so_a_helpers_card_can_name_the_helper() {
    let host = host();
    let bridge = register(&host, "m1", "C:/work");
    let asked = ask(&bridge, json!({"tool_name": "Bash", "input": {"command": "ls"}, "tool_use_id": "toolu_0134dcHNk1GH8kL57hFJ7THL"}));
    let first = card(&host.channel, &[]);
    let odd = ask(&bridge, json!({"tool_name": "Bash", "input": {"command": "pwd"}, "tool_use_id": "not an id; rm -rf"}));
    let second = card(&host.channel, std::slice::from_ref(&first.approval_id));
    let (ls, pwd) = if first.detail.contains("ls") { (first, second) } else { (second, first) };
    assert_eq!(tool_use_of(&ls.approval_id).as_deref(), Some("toolu_0134dcHNk1GH8kL57hFJ7THL"));
    // Anything that is not an id is not carried.
    assert_eq!(pwd.detail, "pwd");
    assert_eq!(tool_use_of(&pwd.approval_id), None);
    for c in [&ls, &pwd] {
        decide(&host.channel, c, Decision::Deny, None).unwrap();
    }
    answer(asked);
    answer(odd);
}

#[test]
fn carries_a_denial_to_the_model_with_a_sentence_not_a_bare_refusal() {
    let host = host();
    let bridge = register(&host, "m1", "");
    let stream = ask(&bridge, json!({"tool_name": "mcp__claude_ai_Gmail__send_message"}));
    let raised = card(&host.channel, &[]);
    assert_eq!(raised.cwd, None);
    decide(&host.channel, &raised, Decision::Deny, None).unwrap();
    assert_eq!(answer(stream), json!({"behavior": "deny", "message": "Denied in Locust."}));
}

#[test]
fn carries_the_persons_reason_for_a_denial_when_they_gave_one() {
    let host = host();
    let bridge = register(&host, "m1", "");
    let stream = ask(&bridge, json!({"tool_name": "mcp__claude_ai_Gmail__send_message"}));
    let raised = card(&host.channel, &[]);
    decide(&host.channel, &raised, Decision::Deny, Some("draft it instead of sending")).unwrap();
    assert_eq!(
        answer(stream),
        json!({"behavior": "deny", "message": "Denied in Locust. The person declined this, and said: draft it instead of sending"})
    );
}

#[test]
fn always_is_the_tool_not_the_connector_each_card_keyed_by_its_own_tool() {
    // QA-2026-09-29 round 2, R35: "list_issues" let "delete_file" through with no card.
    let host = host();
    let bridge = register(&host, "m1", "");
    let first = ask(&bridge, json!({"tool_name": "mcp__claude_ai_Robinhood__get_watchlists"}));
    let watchlists = card(&host.channel, &[]);
    assert_eq!(watchlists.always_covers.as_deref(), Some("get_watchlists on Robinhood again, and no other tool"));
    assert_eq!(watchlists.always_key.as_deref(), Some("claude:mcp__claude_ai_Robinhood__get_watchlists"));
    decide(&host.channel, &watchlists, Decision::ApproveAlways, None).unwrap();
    assert_eq!(answer(first)["behavior"], "allow");

    // The same tool again: in this app the host and the main process are one channel, which answers it under the
    // Always after the saved rules (Electron's who-decides), recorded as that and never as the person.
    assert_eq!(answer(ask(&bridge, json!({"tool_name": "mcp__claude_ai_Robinhood__get_watchlists"})))["behavior"], "allow");
    let recorded = host.channel.take_recorded_for("m1");
    assert_eq!(recorded.last().map(|r| r.by.as_str()), Some("earlier-always"));

    // Same connector, a different tool: a card and a key of its own.
    let other = ask(&bridge, json!({"tool_name": "mcp__claude_ai_Robinhood__get_accounts"}));
    let accounts = card(&host.channel, &[]);
    assert_eq!(accounts.always_key.as_deref(), Some("claude:mcp__claude_ai_Robinhood__get_accounts"));
    decide(&host.channel, &accounts, Decision::Deny, None).unwrap();
    assert_eq!(answer(other)["behavior"], "deny");

    // Bash: one key for every command, as its card says.
    let command = ask(&bridge, json!({"tool_name": "Bash", "input": {"command": "npm test"}}));
    let bash = card(&host.channel, &[]);
    assert_eq!((bash.always_key.as_deref(), bash.always_covers.as_deref()), (Some("claude:Bash"), Some("every command it runs")));
    assert_eq!(always_covers("Bash"), "every command it runs");
    decide(&host.channel, &bash, Decision::Deny, None).unwrap();
    assert_eq!(answer(command)["behavior"], "deny");
}

#[test]
fn does_not_let_one_run_answer_for_another() {
    let host = host();
    let a = register(&host, "mA", "");
    let b = register(&host, "mB", "");
    let asked_a = ask(&a, json!({"tool_name": "mcp__claude_ai_Robinhood__get_watchlists"}));
    let first = card(&host.channel, &[]);
    decide(&host.channel, &first, Decision::ApproveAlways, None).unwrap();
    answer(asked_a);
    // B asks the same connector: its own run never said always.
    let asked_b = ask(&b, json!({"tool_name": "mcp__claude_ai_Robinhood__get_watchlists"}));
    let second = card(&host.channel, std::slice::from_ref(&first.approval_id));
    assert_eq!(second.mission_id, "mB");
    decide(&host.channel, &second, Decision::Deny, None).unwrap();
    assert_eq!(answer(asked_b)["behavior"], "deny");
}

#[test]
fn refuses_whatever_is_still_waiting_when_the_run_ends_and_removes_its_config() {
    let host = host();
    let bridge = register(&host, "m1", "");
    let config = PathBuf::from(bridge.config());
    let socket = socket_of(&bridge);
    assert!(config.is_file());
    let stream = ask(&bridge, json!({"tool_name": "mcp__claude_ai_Robinhood__get_watchlists"}));
    let raised = card(&host.channel, &[]);
    drop(bridge);
    assert_eq!(answer(stream)["behavior"], "deny");
    assert!(!config.parent().unwrap().exists());
    // The way in died with the run.
    assert!(UnixStream::connect(&socket).is_err());
    assert!(decide(&host.channel, &raised, Decision::ApproveOnce, None).is_err());
}

#[test]
fn refuses_a_questions_answer_aimed_at_a_permission_and_says_so() {
    let host = host();
    let bridge = register(&host, "m1", "");
    let stream = ask(&bridge, json!({"tool_name": "mcp__x__y"}));
    let raised = card(&host.channel, &[]);
    let refused = host.channel.decide(Answer::Answers { approval_id: raised.approval_id.clone(), answers: vec![("q1".into(), vec!["yes".into()])] });
    assert!(refused.unwrap_err().contains("belongs to a question"));
    let said = answer(stream);
    assert_eq!(said["behavior"], "deny");
    assert!(said["message"].as_str().unwrap().contains("belongs to a question"), "{said}");
}

// ---- what the CLI is handed ----

#[test]
fn is_an_mcp_config_naming_the_bridge_under_the_name_the_tool_flag_expects() {
    let host = host();
    let bridge = register(&host, "m1", "");
    let config = config_of(&bridge);
    // `mcp__locust__approve` is `mcp__<server>__<tool>`: the server MUST be called `locust` or the flag names a tool that
    // does not exist.
    let servers: Vec<&String> = config["mcpServers"].as_object().unwrap().keys().collect();
    assert_eq!(servers, ["locust"]);
    assert_eq!(config["mcpServers"]["locust"]["args"][0], "--permission-bridge");
}

#[test]
fn names_a_connector_the_way_the_activity_rows_do() {
    assert_eq!(describe_claude_tool("mcp__claude_ai_Robinhood__get_watchlists", &json!({}), "").1, "Use get_watchlists on Robinhood");
    assert_eq!(describe_claude_tool("mcp__linear__create_issue", &json!({}), "").1, "Use create_issue on linear");
}
