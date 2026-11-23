use crate::{
    approval::{Channel, Mode},
    browser_tools::{self, Registration},
    runtime::{Runtime, Turn, copilot::Copilot},
};
use serde_json::{Value, json};
use std::{
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
};

const FAKE_ACP: &str = r#"
import json, os, sys
agent = "https://agentclientprotocol.com/protocol/session-modes#agent"
for line in sys.stdin:
    request = json.loads(line)
    method = request["method"]
    if method == "initialize":
        result = {"protocolVersion": 1, "agentCapabilities": {"loadSession": True}}
    elif method in ("session/new", "session/load"):
        assert method == os.environ["LOCUST_EXPECTED_SESSION_METHOD"], request
        assert request["params"]["mcpServers"] == json.loads(os.environ["LOCUST_EXPECTED_BROWSER"]), request
        result = {"sessionId": "copilot-session", "modes": {
            "currentModeId": agent, "availableModes": [{"id": agent}]
        }, "configOptions": [{"id": "allow_all", "currentValue": "off"}]}
    elif method == "session/prompt":
        result = {"stopReason": "end_turn"}
    else:
        raise AssertionError(request)
    print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)
"#;

#[test]
fn browser_tools_passes_copilots_browser_over_acp_and_keeps_the_print_flag() {
    // Browser installation is process-wide. Confine it to this test's child.
    const CHILD: &str = "LOCUST_COPILOT_BROWSER_GRID_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let test = concat!(
            module_path!(),
            "::browser_tools_passes_copilots_browser_over_acp_and_keeps_the_print_flag"
        );
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test.split_once("::").unwrap().1, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    browser_tools::install(Arc::new(Mutex::new(Default::default())));
    let folder = std::env::temp_dir();
    let channel = Channel::new();
    for (method, resume) in [
        ("session/new", None),
        ("session/load", Some("copilot-session")),
    ] {
        let mut command = Command::new("python3");
        command.args(["-c", FAKE_ACP, "--acp"]);
        let _registration = Registration::prepare(
            &mut command,
            "copilot",
            method,
            "copilot-test",
            &folder,
            channel.clone(),
            Default::default(),
        )
        .unwrap()
        .unwrap();
        assert!(
            !command
                .get_args()
                .any(|arg| arg == "--additional-mcp-config")
        );

        let browser = browser_tools::config(method).unwrap();
        let servers = json!([{
            "name": "locust_browser", "command": browser["command"], "args": browser["args"], "env": []
        }]);
        command
            .env("LOCUST_EXPECTED_BROWSER", servers.to_string())
            .env("LOCUST_EXPECTED_SESSION_METHOD", method);
        let mut runtime = Copilot::spawn(&mut command).unwrap();
        assert!(matches!(runtime, Copilot::Acp(_)));
        runtime
            .start(&Turn {
                prompt: "Check the browser configuration.".into(),
                folder: folder.clone(),
                model: None,
                effort: None,
                resume: resume.map(str::to_owned),
                mode: Mode::ApproveEach,
                approvals: Some(channel.clone()),
                mission_id: method.into(),
                run_id: method.into(),
            })
            .unwrap();
        // The fake asserts on the session request and dies when it is wrong; only a completed run proves it saw the
        // browser (the merge check's break that dropped mcpServers got past a test that stopped at start()).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let last = loop {
            assert!(std::time::Instant::now() < deadline, "{method}: the fake never finished");
            if let Some(e) = runtime.next_event().unwrap() {
                if matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled") {
                    break e;
                }
            }
        };
        assert_eq!(last.kind, "run.completed", "{method}: {:?}", last.payload);
    }

    let mut print = Command::new("copilot");
    print.args(["-p", "Say hi."]);
    let _registration = Registration::prepare(
        &mut print,
        "copilot",
        "print",
        "copilot-test",
        &folder,
        channel,
        Default::default(),
    )
    .unwrap()
    .unwrap();
    let args: Vec<_> = print.get_args().collect();
    assert_eq!(args[..3], ["-p", "Say hi.", "--additional-mcp-config"]);
    assert_eq!(args.len(), 4);
    let config_path = args[3].to_str().unwrap().strip_prefix('@').unwrap();
    let cfg: Value =
        serde_json::from_slice(&std::fs::read(Path::new(config_path)).unwrap()).unwrap();
    let browser = browser_tools::config("print").unwrap();
    assert_eq!(
        cfg["mcpServers"]["locust_browser"],
        json!({
            "type": "local", "tools": ["*"], "command": browser["command"], "args": browser["args"]
        })
    );
}
