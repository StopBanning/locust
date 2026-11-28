//! Settings' real inputs and buttons, through the production subprocess runner.
//! Only executable discovery is faked. The Python fixture never calls Codex or an MCP server.
use super::*;
use rune::Runtime;
use std::{fs, path::PathBuf, sync::Arc, time::{Duration, Instant}};

const ADDRESS: &str = "https://example.com/mcp";
const CLI: &str = r#"
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
args = sys.argv[2:]
with (root / "argv.jsonl").open("a", encoding="utf8") as log:
    log.write(json.dumps(args) + "\n")
registry = root / "codex-registry.json"
if args == ["mcp", "get", "docs"]:
    sys.exit(0 if registry.exists() else 1)
if args == ["mcp", "add", "docs", "--url", "https://example.com/mcp"] and not registry.exists():
    registry.write_text(json.dumps({"docs": args[-1]}), encoding="utf8")
    sys.exit(0)
if args == ["mcp", "remove", "docs"] and registry.exists():
    registry.unlink()
    sys.exit(0)
print("Unexpected connector arguments.", file=sys.stderr)
sys.exit(2)
"#;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

struct Installed(Option<PathBuf>);
impl Agents for Installed {
    fn launch(&self, agent: &str) -> Option<Launch> {
        let dir = self.0.as_ref().filter(|_| agent == "codex")?;
        Some(Launch {
            executable: PathBuf::from("python3"),
            prefix_args: vec!["-I".into(), "-c".into(), CLI.into(), dir.to_string_lossy().into_owned()],
        })
    }
    fn runner(&self) -> &dyn connector_add::Runner { &connector_add::RealRunner }
}

struct ResetAgents;
impl Drop for ResetAgents {
    fn drop(&mut self) { *AGENTS_IN_USE.lock().unwrap_or_else(|e| e.into_inner()) = None; }
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..20 { rt.frame(0.05); }
}

fn wait_until(rt: &mut Runtime<State>, done: impl Fn(&State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(6);
    while !done(&rt.state) {
        rt.update(|s, _| { crate::connectors_section::poll(s); });
        assert!(Instant::now() < deadline, "the real Settings connector operation never replied");
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(rt);
}

fn settings(agents: Installed) -> Runtime<State> {
    use_agents(Arc::new(agents));
    let mut s = State::default();
    crate::pages::close_all(&mut s);
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::Connectors);
    let mut rt = Runtime::new(s, crate::view, (1200.0, 1800.0), rune::Env::new(true, 1.0));
    rt.update(|s, _| crate::connectors_section::request(s, false));
    wait_until(&mut rt, |s| s.connectors.add.installed.is_some());
    rt
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key} on Settings > Connectors"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn paste(rt: &mut Runtime<State>, key: &str, words: &str) {
    click(rt, key);
    rt.paste(words);
    settle(rt);
}

fn label(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key)).and_then(|e| e.label)
}

fn enter_url(rt: &mut Runtime<State>, address: &str) {
    click(rt, "add-connector-start");
    click(rt, "add-connector-url");
    paste(rt, "add-connector-name", "docs");
    paste(rt, "add-connector-value", address);
    assert_eq!(rt.state.connectors.add.name.text, "docs");
    assert_eq!(rt.state.connectors.add.value.text, address);
    assert_eq!(label(rt, "add-connector-add").as_deref(), Some("Add to 1 agent"));
    click(rt, "add-connector-add");
    assert!(rt.state.connectors.add.busy.is_some(), "the actual Add button must dispatch");
    wait_until(rt, |s| s.connectors.add.busy.is_none());
}

// case: starts as one button, and says so when no agent here can take one
#[test]
fn connector_settings_opens_from_one_button_and_adds_through_a_real_process() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let _reset = ResetAgents;
    let temp = Temp(std::env::temp_dir().join(format!("locust-b39-connector-{}", std::process::id())));
    fs::create_dir(&temp.0).unwrap();
    let log = temp.0.join("argv.jsonl");
    let registry = temp.0.join("codex-registry.json");

    let mut none = settings(Installed(None));
    let shown = none.elements();
    assert!(shown.iter().any(|el| el.key.as_deref() == Some("add-connector-none")
        && el.text.as_deref() == Some("None of the agents that can take one is installed here.")));
    assert!(none.find("add-connector-form").is_none());
    click(&mut none, "add-connector-start");
    assert!(none.find("add-connector-form").is_none());
    assert!(!log.exists(), "nothing to offer never starts a process");
    drop(none);

    let mut rt = settings(Installed(Some(temp.0.clone())));
    assert_eq!(label(&rt, "add-connector-start").as_deref(), Some("Add a connector…"));
    assert!(rt.find("add-connector-form").is_none());
    assert!(rt.find("add-connector-none").is_none());
    assert!(!log.exists(), "discovery did not edit the connector registry");
    enter_url(&mut rt, ADDRESS);
    assert_eq!(label(&rt, "add-connector-result-codex").as_deref(), Some("Added"));
    assert_eq!(label(&rt, "add-connector-start").as_deref(), Some("Add another…"));
    assert!(rt.find("add-connector-form").is_none());
    assert_eq!(serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&registry).unwrap()).unwrap(), serde_json::json!({"docs": ADDRESS}));
    let mut cold = settings(Installed(Some(temp.0.clone())));
    enter_url(&mut cold, "https://example.com/other");
    assert_eq!(label(&cold, "add-connector-result-codex").as_deref(), Some("Had one"));
    assert!(cold.find("add-connector-undo").is_none(), "this window did not add anything");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&registry).unwrap()).unwrap(), serde_json::json!({"docs": ADDRESS}));
    drop(cold);

    click(&mut rt, "add-connector-undo");
    wait_until(&mut rt, |s| s.connectors.add.busy.is_none());
    assert_eq!(label(&rt, "add-connector-result-codex").as_deref(), Some("Taken back"));
    assert!(!registry.exists(), "the real subprocess received Undo and removed its own registry entry");

    let calls: Vec<Vec<String>> = fs::read_to_string(log).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(calls, [
        vec!["mcp", "get", "docs"], vec!["mcp", "add", "docs", "--url", ADDRESS],
        vec!["mcp", "get", "docs"], vec!["mcp", "remove", "docs"],
    ]);
}
