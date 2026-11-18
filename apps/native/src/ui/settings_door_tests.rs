//! Electron's `other-app-access-is-an-explicit-setting.test.tsx`, the cases Rust's `settings_panel::door_section`
//! builds, drawn headless with Electron's values and words: off state, enabled setup lines and tradeoffs,
//! listener error message, `Use each teammate's own mode` off/on.
//! Not checked: React mock IPC toggle-off, OpenClaw line, `editorSeen` / `editorAnswers`, and `turnPromptLine`.
use super::*;
use locust_core::door;
use rune::{Env, Runtime};

fn view(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().child(door_section(s, &Palette::new(true)))
}
fn drawn(door_state: door::State) -> (String, Vec<(String, bool)>) {
    let mut state = State::default();
    state.settings_panel.door = Some(door_state);
    let mut rt = Runtime::new(state, view, (900.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let look = crate::look::controls::Look::new(true);
    let words = all.iter().filter_map(|n| n.text.clone()).collect::<Vec<_>>().join("\n");
    let switches = all
        .iter()
        .filter(|n| n.role == "switch")
        .filter_map(|n| n.label.clone().map(|l| (l, n.fill == look.c("switch-on"))))
        .collect();
    (words, switches)
}
fn enabled_door() -> door::State {
    door::State {
        settings: door::Settings { enabled: true, own_mode: false, conversations: false },
        claude_command: Some("claude mcp add example".into()),
        codex_config: Some("[mcp_servers.locust]\ncommand = \"example\"".into()),
        message: None,
    }
}

#[test]
fn shows_no_setup_while_off() {
    let (said, switches) = drawn(door::State::default());
    assert!(said.contains("Off. No local server is listening."), "{said}");
    assert!(switches.iter().any(|(l, on)| l == DOOR_HEADING && !on));
    assert!(!said.contains("mcp_servers"));
}

#[test]
fn shows_the_exact_commands_and_tradeoffs_when_on() {
    let (said, _) = drawn(enabled_door());
    assert!(said.contains("claude mcp add example"), "{said}");
    assert!(said.contains("[mcp_servers.locust]"), "{said}");
    assert!(said.contains("New turns are Ask mode (read only)"), "{said}");
    assert!(said.contains("can ask teammates and read replies"), "{said}");
    assert!(said.contains("with no automatic teammate handoffs"), "{said}");
    assert!(said.contains("Locust must be running"), "{said}");
    assert!(said.contains("monthly limits still apply"), "{said}");
    assert!(said.contains("Locust changes another app’s settings only when you press Add to the Claude app"), "{said}");
}

#[test]
fn keeps_the_off_state_and_says_why_if_the_listener_could_not_be_enabled() {
    let mut d = door::State::default();
    d.message = Some("The server is still off.".into());
    let (said, switches) = drawn(d);
    assert!(switches.iter().any(|(l, on)| l == DOOR_HEADING && !on));
    assert!(said.contains("The server is still off."), "{said}");
}

#[test]
fn offers_each_teammates_own_mode_only_while_on_off_by_default() {
    let (off_said, _) = drawn(door::State::default());
    assert!(!off_said.contains("own mode"));

    let (on_said, switches) = drawn(enabled_door());
    assert!(switches.iter().any(|(l, on)| l == "Use each teammate's own mode" && !on));
    assert!(on_said.contains("Every turn another app starts is Ask mode (read only)"), "{on_said}");
}

#[test]
fn says_who_answers_approvals_when_own_mode_is_on() {
    let mut d = enabled_door();
    d.settings.own_mode = true;
    let (said, _) = drawn(d);
    assert!(said.contains("New turns use each teammate's own mode"), "{said}");
    assert!(said.contains("only you can answer them"), "{said}");
    assert!(said.contains("Auto runs only if Auto is on in Settings"), "{said}");
}
