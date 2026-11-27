use super::*;
use serde_json::Value;
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

const CHILD: &str = "LOCUST_B21_CLI_TEST_CHILD";

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Only the fixture bin is on PATH. Every fake puts down a marker even for --version,
// so a refused call proves not just "no turn" but "not even discovery started a child".
const STAND_IN: &str = r#"
from pathlib import Path
import json, sys

with Path(sys.argv[0]).with_name("invoked").open("a") as log:
    print(Path(sys.argv[0]).name, *sys.argv[1:], file=log)
if sys.argv[1:] == ["--version"]:
    print("codex-cli 0.172.0")
    sys.exit(0)
assert Path(sys.argv[0]).name == "codex" and sys.argv[1] == "app-server"

def send(**msg):
    print(json.dumps({"jsonrpc": "2.0", **msg}), flush=True)

for line in sys.stdin:
    call = json.loads(line)
    method = call["method"]
    if method == "initialized":
        continue
    id = call["id"]
    params = call.get("params", {})
    if method == "initialize":
        send(id=id, result={})
    elif method == "config/read":
        send(id=id, result={"config": {}})
    elif method == "thread/start":
        assert params["sandbox"] == "read-only" and params["approvalPolicy"] == "never", params
        send(id=id, result={"thread": {"id": "thread-live"}})
    elif method == "turn/start":
        assert params.get("model") == "gpt-6", params
        assert params["input"][0]["text"] == "Inspect the workspace.", params
        send(id=id, result={"turn": {"id": "turn-live"}})
        send(method="turn/started", params={"threadId": "thread-live", "turn": {"id": "turn-live"}})
        send(method="turn/completed", params={"threadId": "thread-live", "turn": {"id": "turn-live", "status": "completed"}})
    else:
        raise AssertionError(method)
"#;

// case: keeps window, routine, relay, compare, voice handoff, CLI and MCP starts on the guarded mission service
#[test]
fn headless_run_checks_organization_policy_before_discovery_and_recording() {
    // Run the real entry point once per child. This keeps the policy cache, PATH, and Ctrl-C
    // registration out of the test process and keeps unrelated tests' environment untouched.
    if let Ok(args) = std::env::var(CHILD) {
        let args: Vec<String> = serde_json::from_str(&args).unwrap();
        match run(&args) {
            Ok(code) => println!("CLI_RESULT: {code}"),
            Err(error) => println!("CLI_REFUSAL: {error}"),
        }
        return;
    }
    let python = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let python = String::from_utf8(python.stdout).unwrap();
    let python = python.trim();
    let root =
        Scratch(std::env::temp_dir().join(format!("b21-{}", crate::invariants::unique_nanos())));
    fs::create_dir(&root.0).unwrap();

    let run_case = |name: &str,
                    runtime: &str,
                    model: Option<&str>,
                    policy: &str,
                    history: Option<&str>,
                    expected: Option<(&str, &str)>| {
        let at = root.0.join(name);
        for folder in ["bin", "home", "work", "codex", "config", "native"] {
            fs::create_dir_all(at.join(folder)).unwrap();
        }
        let policy_file = at.join("policy.json");
        if !policy.is_empty() {
            fs::write(&policy_file, policy).unwrap();
        }
        let ledger = at.join("native/mission-ledger");
        if let Some(history) = history {
            fs::create_dir(&ledger).unwrap();
            fs::write(ledger.join("mission_prior.jsonl"), history).unwrap();
        }
        for program in ["codex", "agy", "kilo", "opencode"] {
            let path = at.join("bin").join(program);
            fs::write(&path, format!("#!{python}\n{STAND_IN}")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let mut args = vec![runtime, "Inspect the workspace."];
        if let Some(model) = model {
            args.extend(["--model", model]);
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "run_cli::policy_tests::headless_run_checks_organization_policy_before_discovery_and_recording", "--nocapture"])
            .env_clear()
            .env(CHILD, json!(args).to_string())
            .env("PATH", at.join("bin"))
            .env("HOME", at.join("home"))
            .env("CODEX_HOME", at.join("codex"))
            .env("XDG_CONFIG_HOME", at.join("config"))
            .env("LOCUST_NATIVE_DATA", at.join("native"))
            .env("LOCUST_TEST_ROOT", &at)
            .env("LOCUST_POLICY_FILE", &policy_file)
            .env("TMPDIR", &at)
            .current_dir(at.join("work"))
            .output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{name}: {stdout}\n{stderr}");
        let ledger_names = || {
            fs::read_dir(&ledger)
                .map(|d| {
                    let mut names: Vec<_> = d.map(|entry| entry.unwrap().file_name()).collect();
                    names.sort();
                    names
                })
                .unwrap_or_default()
        };
        if let Some((why, setting)) = expected {
            let refusal = stdout
                .lines()
                .find_map(|l| l.split_once("CLI_REFUSAL: ").map(|(_, s)| s))
                .unwrap_or_else(|| {
                    panic!("{name}: expected an organization refusal, got {stdout}\n{stderr}")
                });
            assert!(refusal.starts_with(why), "{name}: {refusal}");
            if !setting.is_empty() {
                let suffix = format!(
                    "Set by your organization ({setting} in {}). Nothing was started.",
                    policy_file.display()
                );
                assert!(refusal.ends_with(&suffix), "{name}: {refusal}");
            }
            assert!(
                !at.join("bin/invoked").exists(),
                "{name}: discovered or started a runtime"
            );
            if let Some(history) = history {
                assert_eq!(
                    ledger_names(),
                    ["mission_prior.jsonl"],
                    "{name}: added a ledger"
                );
                assert_eq!(
                    fs::read_to_string(ledger.join("mission_prior.jsonl")).unwrap(),
                    history
                );
            } else {
                assert!(!ledger.exists(), "{name}: created a ledger directory");
            }
            Some(refusal.to_string())
        } else {
            assert!(
                stdout.contains("CLI_RESULT: 0"),
                "{name}: {stdout}\n{stderr}"
            );
            let invoked = fs::read_to_string(at.join("bin/invoked")).unwrap();
            assert_eq!(
                invoked
                    .lines()
                    .filter(|l| l.starts_with("codex app-server"))
                    .count(),
                1,
                "{invoked}"
            );
            assert_eq!(ledger_names().len(), usize::from(history.is_some()) + 1);
            None
        }
    };
    let agent = "Codex CLI is not an allowed AI agent.";
    let model = "The codex model gpt-9 is not allowed.";
    let default = "The codex model account default is not allowed.";
    let disabled = "Access from other AI apps, the locust command and editors is off.";
    let bad = "Locust cannot use the organization policy at ";
    let cap =
        "Locust has reached this month's effective limit of 10.00 USD. It starts again next month.";
    let unreadable = "The monthly spending limit could not be checked. Ask your administrator to restore access to the spending record.";
    let at = chrono::Local::now().to_rfc3339();
    let receipt = format!(
        "{}\n{}\n",
        json!({"schemaVersion":27, "recordType":"mission.created", "metadata":{
            "missionId":"mission_prior", "runtime":"opencode", "model":"fixture", "createdAt":at
        }}),
        json!({"schemaVersion":27, "recordType":"mission.event", "event":{
            "type":"run.completed", "occurredAt":at, "sequence":1, "payload":{"usage":{"usd":10}}
        }})
    );

    let agent_policy = r#"{"allowedAgents":["claude"]}"#;
    let model_policy = r#"{"allowedModels":{"codex":["gpt-6"]}}"#;
    let cap_policy = r#"{"monthlyLimitUsd":10}"#;
    let deny = run_case(
        "agent",
        "codex",
        Some("gpt-6"),
        agent_policy,
        None,
        Some((agent, "allowedAgents")),
    )
    .unwrap();
    let lower = crate::org_policy::PolicyState {
        path: root
            .0
            .join("agent/policy.json")
            .to_string_lossy()
            .into_owned(),
        present: true,
        policy: serde_json::from_str::<Value>(agent_policy)
            .unwrap()
            .as_object()
            .unwrap()
            .clone(),
        error: None,
    };
    assert_eq!(
        Some(deny),
        crate::org_policy::turn_refusal(Some(&lower), "codex", Some("gpt-6"), "ask", false, false)
    );
    run_case(
        "cli-off",
        "codex",
        Some("gpt-6"),
        r#"{"otherApps":false}"#,
        None,
        Some((disabled, "otherApps")),
    );
    run_case(
        "bad-file",
        "codex",
        Some("gpt-6"),
        "{",
        None,
        Some((bad, "")),
    );
    run_case(
        "model",
        "codex",
        Some("gpt-9"),
        model_policy,
        None,
        Some((model, "allowedModels")),
    );
    run_case(
        "default-model",
        "codex",
        Some("account-default"),
        model_policy,
        None,
        Some((default, "allowedModels")),
    );
    run_case(
        "agy-alias",
        "antigravity",
        Some("pro"),
        r#"{"allowedModels":{"antigravity":["pro"]}}"#,
        None,
        Some((
            "The antigravity model gemini-3.1-pro is not allowed.",
            "allowedModels",
        )),
    );
    run_case(
        "opencode-unqualified",
        "opencode",
        Some("fixture"),
        r#"{"allowedModels":{"opencode":["fixture"]}}"#,
        None,
        Some((
            "The opencode model account default is not allowed.",
            "allowedModels",
        )),
    );
    run_case(
        "kilo-default",
        "kilo",
        None,
        r#"{"allowedModels":{"kilo":["other/model"]}}"#,
        None,
        Some((
            "The kilo model kilo/kilo-auto/free is not allowed.",
            "allowedModels",
        )),
    );
    run_case(
        "cap",
        "codex",
        Some("gpt-6"),
        cap_policy,
        Some(&receipt),
        Some((cap, "monthlyLimitUsd")),
    );
    run_case(
        "unreadable",
        "codex",
        Some("gpt-6"),
        cap_policy,
        Some("broken\n"),
        Some((unreadable, "monthlyLimitUsd")),
    );
    run_case(
        "allowed",
        "codex",
        Some("gpt-6"),
        r#"{"allowedAgents":["codex"],"allowedModels":{"codex":["gpt-6"]},"highestMode":"ask","cloud":false,"accounts":false,"monthlyLimitUsd":11}"#,
        Some(&receipt),
        None,
    );
    run_case("no-policy", "codex", Some("gpt-6"), "", None, None);
}
