//! Exercise the real Cargo-built command with its own loopback door. No window, global install or live account.

use locust_core::cli_client::CONNECTION_FILE;
use locust_core::door::{Caller, Door, Settings};
use locust_core::door_tools::ToolResult;
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        // A count as well as the time: this file's tests run at once, and two of them can read the same microsecond
        // (the gate of 28 Nov: create_dir said "File exists").
        static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("locust-standalone-{}-{n}-{stamp}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_locust"));
        cmd.arg("--profile").arg(&self.0);
        cmd
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

type Requests = Arc<Mutex<Vec<(String, Value)>>>;

fn fake(profile: &Temp, reply: impl Fn(&str, &Value) -> ToolResult + Send + Sync + 'static) -> (Door, Requests) {
    let calls: Requests = Arc::default();
    let observed = calls.clone();
    let call: Caller = Arc::new(move |name, args| {
        observed.lock().unwrap().push((name.into(), args.clone()));
        reply(name, args).to_json()
    });
    let door = Door::with(profile.0.clone(), "synthetic Locust", Arc::new(Mutex::new(Settings::default())), call)
        .with_refusal(Arc::new(|| None));
    assert!(door.set_enabled(true).settings.enabled);
    (door, calls)
}

fn ok(value: Value) -> ToolResult { ToolResult::ok(value.to_string()) }

fn team() -> Value {
    json!([{"id":"mate-one","name":"One","runtime":"codex","model":"test-model","runs_in":"ask"}])
}

// case: runs the built teammates command without a live app or a global install
#[test]
fn runs_the_built_teammates_command_without_a_live_app_or_a_global_install() {
    let profile = Temp::new();
    let (_door, calls) = fake(&profile, |name, _| {
        assert_eq!(name, "list_teammates");
        ok(team())
    });
    let out = profile.command().arg("teammates").output().unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "One\tmate-one\tcodex\ttest-model\task\n");
    assert!(out.stderr.is_empty());
    assert_eq!(*calls.lock().unwrap(), [("list_teammates".to_owned(), json!({}))]);
}

#[test]
fn a_built_command_with_the_wrong_token_cannot_reach_a_tool() {
    let profile = Temp::new();
    let (_door, calls) = fake(&profile, |_, _| ok(team()));
    let file = profile.0.join(CONNECTION_FILE);
    let mut connection: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    let wrong = if connection["token"] == "a".repeat(64) { "b" } else { "a" };
    connection["token"] = json!(wrong.repeat(64));
    let wrong_profile = Temp::new();
    std::fs::write(wrong_profile.0.join(CONNECTION_FILE), connection.to_string()).unwrap();
    let out = wrong_profile.command().arg("teammates").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8(out.stderr).unwrap().contains("Locust refused this connection"));
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn the_built_send_cancels_only_its_request_if_stdout_stops_reading() {
    let profile = Temp::new();
    let (_door, calls) = fake(&profile, |name, args| match name {
        "cli/open" => ok(json!({"sessionId":"id","key":"synthetic capability","teammate":team()[0],"workspace":"fake"})),
        "cli/prompt" | "cli/cancel" | "cli/close" => {
            assert_eq!(args["sessionId"], "id");
            assert_eq!(args["key"], "synthetic capability");
            ok(json!({"ok":true}))
        }
        "cli/poll" => {
            let i = args["after"].as_u64().unwrap() + 1;
            assert!(i <= 10, "a stalled pipe ought to overflow before this");
            ok(json!({"turn":{"phase":"running"},"events":[{"sequence":i,"itemId":format!("chunk-{i}"),
                "operation":"append","text":"x".repeat(1_200_000)}],"approvals":[],"cursor":i}))
        }
        _ => ToolResult::refused("Unexpected request."),
    });
    let mut child = profile.command().args(["send","mate-one","Synthetic request"])
        .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    // Keep the pipe open. Never read it until the process exits.
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if child.try_wait().unwrap().is_some() { break; }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("a blocked terminal held the actual process open");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8(out.stderr).unwrap().contains("The terminal did not keep up with the reply."));
    let calls = calls.lock().unwrap();
    let started = calls.iter().find(|(n, _)| n == "cli/prompt").unwrap();
    assert_eq!(started.1["message"], "Synthetic request");
    let cancels: Vec<_> = calls.iter().filter(|(n, _)| n == "cli/cancel").collect();
    assert_eq!(cancels.len(), 1);
    assert!(started.1["requestId"].is_string());
    assert_eq!(cancels[0].1["requestId"], started.1["requestId"]);
    assert_eq!(calls.last().unwrap().0, "cli/close");
    assert!(!calls.iter().any(|(n, _)| n == "cli/decide"));
}

#[test]
fn the_built_acp_command_routes_to_the_existing_agent() {
    let profile = Temp::new();
    let mut child = profile.command().args(["acp", "--teammate", "One"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    writeln!(child.stdin.take().unwrap(), "{}", json!({
        "jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}
    })).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert!(out.stderr.is_empty());
    let reply: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(reply["result"]["protocolVersion"], 1);
}
