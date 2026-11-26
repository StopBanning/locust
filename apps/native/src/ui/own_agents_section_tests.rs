//! Settings › AI agents › Your own agents, driven in the real window (`crate::view`, `crate::poll`) over a data folder
//! of the test's own, as a person would: what it lists, Test, Add agent and Remove (`AcpAgentsSettings.tsx`). Electron
//! has no renderer test of this component; its engine cases (`an-agent-of-your-own-is-added-by-its-command.test.ts`)
//! are carried in core/src/acp_agents_tests.rs. The agent that answers is a small Python program speaking ACP.
use super::*;
use rune::{Env, Runtime};
use std::time::Duration;

/// An ACP agent that answers initialize and session/new, as core's acp_agents_tests.rs stub does.
const STUB_PY: &str = r#"
import json, sys
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    def reply(result):
        print(json.dumps({"jsonrpc": "2.0", "id": msg["id"], "result": result}), flush=True)
    method = msg.get("method")
    if method == "initialize":
        reply({"protocolVersion": 1, "agentInfo": {"name": "Stub Agent", "version": "2.1.0"}})
    elif method == "session/new":
        reply({
            "sessionId": "s1",
            "configOptions": [{
                "id": "model",
                "category": "model",
                "currentValue": "stub/fast",
                "options": [{"value": "stub/fast", "name": "Fast"}, {"value": "stub/deep", "name": "Deep"}],
            }],
        })
"#;

struct Dirs(std::path::PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The real window on Settings › AI agents, over a fresh data folder, with Electron's acp-agents.json if given.
fn window(electron_list: Option<serde_json::Value>) -> (Dirs, store::Store, Runtime<State>) {
    let root = std::env::temp_dir().join(format!("locust-own-agents-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    if let Some(list) = electron_list {
        std::fs::write(root.join("electron").join(locust_core::acp_agents::ACP_AGENTS_FILE), list.to_string()).unwrap();
    }
    std::fs::write(root.join("stub-agent.py"), STUB_PY).unwrap();
    let data = store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let mut s = State::default();
    s.data = Some(data.clone());
    let mut rt = Runtime::new(s, crate::view, (1100.0, 2400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.update(|s, _| {
        crate::settings_panel::open(s);
        s.settings_panel.page = crate::settings_pages::SettingsPageId::from_str("runtimes");
    });
    pump(&mut rt, |_| true);
    (Dirs(root), data, rt)
}

fn stub_command(dirs: &Dirs) -> String {
    format!("python3 \"{}\"", dirs.0.join("stub-agent.py").to_string_lossy())
}

/// The window's own poll and frames, until `done` (or a minute passes).
fn pump(rt: &mut Runtime<State>, mut done: impl FnMut(&State) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let mut now = 0.0;
    loop {
        rt.update(move |s, tx| {
            crate::poll(s, now, tx);
        });
        for _ in 0..10 {
            rt.frame(0.05);
        }
        now += 0.5;
        if done(&rt.state) {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "timed out; said {:?}", rt.state.own_agents.said);
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    pump(rt, |_| true);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    pump(rt, |_| true);
}

fn type_into(rt: &mut Runtime<State>, key: &str, words: &str) {
    click(rt, key);
    rt.paste(words);
    rt.frame(0.016);
}

fn text_in(rt: &Runtime<State>, key: &str) -> String {
    let ([x, y, w, h], _) = rt.presented(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.elements()
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn said(rt: &Runtime<State>) -> String {
    text_in(rt, "acp-agent-said")
}

fn settled(s: &State) -> bool {
    !s.own_agents.busy
}

fn goose() -> serde_json::Value {
    serde_json::json!({"agents": [{
        "id": "acp_goose",
        "name": "Goose",
        "commandLine": "goose acp",
        "answer": {"agentName": "goose", "version": "1.9.0", "modelOption": {"id": "model", "models": [{"value": "a", "name": "A"}, {"value": "b", "name": "B"}]}}
    }, {
        "id": "acp_kimi",
        "name": "Kimi",
        "commandLine": "kimi --acp"
    }]})
}

#[test]
fn lists_each_agent_by_name_with_its_command_who_answered_and_how_many_models() {
    let (_dirs, _data, rt) = window(Some(goose()));
    let section = text_in(&rt, "acp-agents");
    assert!(section.contains("Your own agents"), "{section}");
    assert!(section.contains("Any agent that speaks the Agent Client Protocol, added by the command that starts it. It joins every teammate\u{2019}s model list, in a group of its own."), "{section}");
    assert!(!section.contains("runs in Locust for now"), "{section}");
    let goose = text_in(&rt, "acp-agent-acp_goose");
    assert!(goose.contains("Goose") && goose.contains("goose acp \u{b7} goose 1.9.0 \u{b7} 2 models"), "{goose}");
    let kimi = text_in(&rt, "acp-agent-acp_kimi");
    assert!(kimi.contains("Kimi") && kimi.contains("kimi --acp \u{b7} 0 models"), "{kimi}");
}

#[test]
fn test_says_who_answered_and_how_many_models_it_offers_and_adds_nothing() {
    let (dirs, data, mut rt) = window(None);
    let command = stub_command(&dirs);
    type_into(&mut rt, "acp-agent-command", &command);
    click(&mut rt, "acp-agent-test");
    pump(&mut rt, settled);
    assert_eq!(said(&rt), "Stub Agent 2.1.0 answered. It offers 2 models to pick from.");
    assert!(!data.root().join(locust_core::acp_agents::ACP_AGENTS_FILE).exists(), "Test keeps nothing");
    assert_eq!(rt.state.own_agents.command_line.text, command, "the command stays for Add agent");
}

#[test]
fn test_says_so_when_the_program_is_not_on_this_computer() {
    let (_dirs, _data, mut rt) = window(None);
    type_into(&mut rt, "acp-agent-command", "nowhere-agent acp");
    click(&mut rt, "acp-agent-test");
    assert!(rt.state.own_agents.busy || rt.state.own_agents.said.as_ref().is_some_and(|(_, good)| !good));
    pump(&mut rt, settled);
    assert_eq!(said(&rt), "Locust could not find nowhere-agent. Install it, or check it opens in a new terminal, then test again.");
    assert_eq!(rt.state.own_agents.said.as_ref().map(|(_, good)| *good), Some(false), "said in amber");
}

#[test]
fn says_it_is_testing_while_an_agent_starts() {
    let (dirs, _data, mut rt) = window(None);
    type_into(&mut rt, "acp-agent-command", &stub_command(&dirs));
    let (x, y) = rt.find("acp-agent-test").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert_eq!(said(&rt), TESTING);
    pump(&mut rt, settled);
}

#[test]
fn add_agent_tests_first_keeps_one_that_answered_and_clears_the_form() {
    let (dirs, data, mut rt) = window(None);
    type_into(&mut rt, "acp-agent-name", "Stub");
    type_into(&mut rt, "acp-agent-command", &stub_command(&dirs));
    click(&mut rt, "acp-agent-add");
    pump(&mut rt, settled);
    assert_eq!(said(&rt), "Stub is in every teammate's model list, in a group of its own. It runs in Approve each.");
    let row = text_in(&rt, "acp-agent-acp_stub");
    assert!(row.contains("Stub") && row.contains("Stub Agent 2.1.0 \u{b7} 2 models"), "{row}");
    assert!(rt.state.own_agents.name.text.is_empty() && rt.state.own_agents.command_line.text.is_empty(), "the form is cleared");
    let kept = locust_core::acp_agents::read_saved_agents(data.root());
    assert_eq!(kept.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["acp_stub"]);
}

#[test]
fn add_agent_says_why_and_keeps_nothing_when_the_agent_cannot_be_found() {
    let (_dirs, data, mut rt) = window(None);
    type_into(&mut rt, "acp-agent-name", "Nowhere");
    type_into(&mut rt, "acp-agent-command", "nowhere-agent acp");
    click(&mut rt, "acp-agent-add");
    pump(&mut rt, settled);
    assert_eq!(said(&rt), "Locust could not find nowhere-agent. Install it, or check it opens in a new terminal, then test again.");
    assert_eq!(rt.state.own_agents.name.text, "Nowhere", "the form stays as it was");
    assert!(!data.root().join(locust_core::acp_agents::ACP_AGENTS_FILE).exists());
}

#[test]
fn add_agent_does_nothing_until_both_the_name_and_the_command_are_filled() {
    let (_dirs, _data, mut rt) = window(None);
    type_into(&mut rt, "acp-agent-name", "Goose");
    click(&mut rt, "acp-agent-add");
    assert!(!rt.state.own_agents.busy && rt.state.own_agents.said.is_none());
    assert!(rt.find("acp-agent-said").is_none());
}

#[test]
fn remove_asks_on_the_first_press_and_removes_on_the_second() {
    let (_dirs, data, mut rt) = window(Some(goose()));
    click(&mut rt, "acp-agent-remove-acp_goose");
    assert!(text_in(&rt, "acp-agent-acp_goose").contains("Remove Goose?"), "the first press asks");
    assert!(rt.state.own_agents.agents.as_ref().unwrap().iter().any(|a| a.id == "acp_goose"));
    // The second click of a double-click is not a second decision (ArmedButton SETTLE_MS).
    click(&mut rt, "acp-agent-remove-acp_goose");
    assert!(rt.find("acp-agent-acp_goose").is_some(), "a press inside the settle does not remove");
    std::thread::sleep(Duration::from_millis(450));
    click(&mut rt, "acp-agent-remove-acp_goose");
    assert!(rt.find("acp-agent-acp_goose").is_none(), "the second press removes");
    assert!(rt.find("acp-agent-acp_kimi").is_some());
    let kept = locust_core::acp_agents::read_saved_agents(data.root());
    assert_eq!(kept.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["acp_kimi"], "the rest of Electron's list is kept");
}

// AcpAgentsSettings.tsx: Add agent is disabled while a Test or an Add is under way (`busy`), so a second press is not a
// second start. The press itself is refused by the button; this is the function behind it, reached some other way.
#[test]
fn add_agent_starts_nothing_while_a_test_or_an_add_is_under_way() {
    let (dirs, data, mut rt) = window(None);
    type_into(&mut rt, "acp-agent-name", "Stub");
    type_into(&mut rt, "acp-agent-command", &stub_command(&dirs));
    rt.update(|s, _| s.own_agents.busy = true);
    rt.update(|s, _| add(s));
    assert_eq!(rt.state.own_agents.said, None, "nothing was started, so nothing is being tested");
    assert!(rt.state.own_agents.busy, "the one under way is still the only one");
    assert!(!data.root().join(locust_core::acp_agents::ACP_AGENTS_FILE).exists(), "nothing kept");

    // The same press once nothing is under way starts the Add.
    rt.update(|s, _| s.own_agents.busy = false);
    rt.update(|s, _| add(s));
    assert!(rt.state.own_agents.busy, "Add agent started");
    pump(&mut rt, settled);
    assert_eq!(rt.state.own_agents.said.as_ref().map(|(_, good)| *good), Some(true));
    assert!(data.root().join(locust_core::acp_agents::ACP_AGENTS_FILE).exists(), "the agent that answered is kept");
}
