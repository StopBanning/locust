//! Electron's `other-app-access-is-an-explicit-setting.test.tsx`, the cases Rust's `settings_panel::door_section`
//! builds, drawn headless with Electron's values and words: off state, enabled setup lines and tradeoffs,
//! listener error message, `Use each teammate's own mode` off/on.
//! Presses go to a door of the test's own (`TEST_DOOR`, this thread only), which records each change and answers as
//! the host would. The editor switch (`editor_seen` / `editor_answers`) is pressed the same way; `turnPromptLine` is
//! core's `conversation_turns::turn_line`, read with Electron's values.
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
        settings: door::Settings { enabled: true, own_mode: false, conversations: false, ..Default::default() },
        claude_command: Some("claude mcp add example".into()),
        codex_config: Some("[mcp_servers.locust]\ncommand = \"example\"".into()),
        openclaw_command: Some("openclaw mcp add locust --command example".into()),
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
    assert!(said.contains("can ask teammates, run your routines and read replies"), "{said}");
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

// The rest of Electron's cases: what a press asks the door for, and what the screen says once the host answers.

/// The door section drawn in a runtime of its own, with presses answered by `answer` and every change kept.
struct Pressing {
    rt: Runtime<State>,
    asked: std::rc::Rc<std::cell::RefCell<Vec<locust_core::engine_api::DoorChange>>>,
}

impl Pressing {
    fn new(first: door::State, mut answer: impl FnMut(locust_core::engine_api::DoorChange) -> door::State + 'static) -> Self {
        let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let kept = asked.clone();
        crate::TEST_DOOR.with(|d| {
            *d.borrow_mut() = Some(Box::new(move |change| {
                kept.borrow_mut().push(change);
                answer(change)
            }))
        });
        let mut state = State::default();
        state.settings_panel.door = Some(first);
        let mut rt = Runtime::new(state, view, (900.0, 1400.0), Env::new(true, 1.0));
        rt.frame(0.0);
        Pressing { rt, asked }
    }

    fn press(&mut self, label: &str) {
        let [left, top, width, height] = self
            .rt
            .elements()
            .into_iter()
            .find(|e| e.role == "switch" && e.label.as_deref() == Some(label))
            .map(|e| e.rect)
            .unwrap_or_else(|| panic!("no switch named {label}"));
        let (x, y) = (left + width / 2.0, top + height / 2.0);
        self.rt.pointer_down(x, y);
        self.rt.pointer_up(x, y);
        self.rt.frame(0.016);
        self.rt.frame(1.0);
    }

    fn said(&self) -> String {
        self.rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
    }

    fn on(&self, label: &str) -> bool {
        let look = crate::look::controls::Look::new(true);
        self.rt.elements().iter().any(|e| e.role == "switch" && e.label.as_deref() == Some(label) && e.fill == look.c("switch-on"))
    }
}

impl Drop for Pressing {
    fn drop(&mut self) {
        crate::TEST_DOOR.with(|d| *d.borrow_mut() = None);
    }
}

#[test]
fn loads_the_actual_setting_makes_no_enable_request_on_mount_and_shows_no_setup_while_off() {
    let p = Pressing::new(door::State::default(), |_| enabled_door());
    assert!(p.said().contains("Off. No local server is listening."));
    assert!(!p.on(DOOR_HEADING));
    assert!(!p.said().contains("mcp_servers"));
    assert!(p.asked.borrow().is_empty(), "drawing asked the door for {:?}", p.asked.borrow());
}

#[test]
fn enables_only_after_a_press_then_shows_the_exact_host_owned_command_config_and_tradeoff() {
    use locust_core::engine_api::DoorChange;
    let mut p = Pressing::new(door::State::default(), |_| enabled_door());
    p.press(DOOR_HEADING);
    assert_eq!(*p.asked.borrow(), vec![DoorChange::Enabled(true)]);
    assert!(p.on(DOOR_HEADING));
    let said = p.said();
    for words in [
        "claude mcp add example",
        "[mcp_servers.locust]",
        "openclaw mcp add locust --command example",
        "its tool policy decides which of its chats can use it",
        "Add Locust to Claude Code, Codex or OpenClaw by hand",
        "New turns are Ask mode (read only)",
        "can ask teammates, run your routines and read replies",
        "with no automatic teammate handoffs",
        "Locust must be running",
        "monthly limits still apply",
        "Locust changes another app\u{2019}s settings only when you press Add to the Claude app",
    ] {
        assert!(said.contains(words), "{words} in {said}");
    }
    let copies: Vec<String> = p.rt.elements().into_iter().filter(|e| e.role == "button").filter_map(|e| e.label).collect();
    assert!(copies.iter().any(|c| c == "Copy the OpenClaw command"), "{copies:?}");
}

#[test]
fn turns_the_server_off_and_removes_the_setup_text_when_the_host_confirms_it() {
    use locust_core::engine_api::DoorChange;
    let mut p = Pressing::new(enabled_door(), |_| door::State::default());
    assert!(p.said().contains("[mcp_servers.locust]"));
    p.press(DOOR_HEADING);
    assert_eq!(*p.asked.borrow(), vec![DoorChange::Enabled(false)]);
    assert!(!p.said().contains("[mcp_servers.locust]"));
    assert!(!p.on(DOOR_HEADING));
}

#[test]
fn turns_own_mode_on_only_after_a_press_and_says_who_answers_approvals() {
    use locust_core::engine_api::DoorChange;
    let mut p = Pressing::new(enabled_door(), |change| {
        let mut next = enabled_door();
        next.settings.own_mode = change == DoorChange::OwnMode(true);
        next
    });
    p.press("Use each teammate's own mode");
    assert_eq!(*p.asked.borrow(), vec![DoorChange::OwnMode(true)], "own mode only; the door itself untouched");
    let said = p.said();
    assert!(said.contains("New turns use each teammate's own mode"), "{said}");
    assert!(said.contains("only you can answer them"), "{said}");
    assert!(said.contains("Auto runs only if Auto is on in Settings"), "{said}");
}

// case: shows the editor switch only once an editor has connected, off until a press, and says who answers
#[test]
fn shows_the_editor_switch_only_once_an_editor_has_connected_off_until_a_press_and_says_who_answers() {
    use locust_core::engine_api::DoorChange;
    const EDITORS: &str = "Let editors answer cards for their own turns";
    let mut plain = enabled_door();
    plain.settings.own_mode = true;
    let (said, switches) = drawn(plain.clone());
    assert!(!said.contains(EDITORS) && !switches.iter().any(|(l, _)| l == EDITORS), "no editor yet: {said}");

    let mut first = plain;
    first.settings.editor_seen = true;
    let answered = first.clone();
    let mut p = Pressing::new(first, move |change| {
        let mut next = answered.clone();
        if let DoorChange::EditorAnswers(on) = change {
            next.settings.editor_answers = on;
        }
        next
    });
    assert!(p.rt.elements().iter().any(|e| e.role == "switch" && e.label.as_deref() == Some(EDITORS)), "{}", p.said());
    assert!(!p.on(EDITORS), "off until a press");
    let said = p.said();
    assert!(said.contains("Cards for a turn an editor starts wait in this window, and only you answer them."), "{said}");
    assert!(said.contains("only you can answer them"), "{said}");
    assert!(p.asked.borrow().is_empty(), "drawing asked the door for {:?}", p.asked.borrow());

    p.press(EDITORS);
    assert_eq!(*p.asked.borrow(), vec![DoorChange::EditorAnswers(true)], "the editor switch only; own mode untouched");
    assert!(p.on(EDITORS));
    let said = p.said();
    assert!(said.contains("An editor that starts a turn can approve or deny that turn\u{2019}s cards there."), "{said}");
    assert!(said.contains("an editor can answer the ones for turns it started"), "{said}");
}

// case: keeps an externally sent message visible without turning it into relay instructions
/// Electron's `turnPromptLine({ prompt: 'Review this.', startedBy: { kind: 'mcp' } })`: what another app sent is the
/// bubble, word for word. Rust's is `conversation_turns::turn_line`, which the thread reads at messages.rs:357.
#[test]
fn keeps_an_externally_sent_message_visible_without_turning_it_into_relay_instructions() {
    let sent = locust_core::ledger::Metadata {
        prompt: "Review this.".into(),
        started_by_kind: Some("mcp".into()),
        ..Default::default()
    };
    assert_eq!(locust_core::conversation_turns::turn_line(&sent).as_deref(), Some("Review this."));
}
