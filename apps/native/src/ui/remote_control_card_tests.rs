//! a-remote-control-switch-shows-claudes-own-words.test.tsx, drawn headless through `remote_control_card::section`:
//! the off switch says its lifetime and what account it needs; off is one switch with its state opening its line and
//! nothing under it; running shows Claude Code's output and errors exactly as printed; a trust handoff stays off and
//! asks for setup; stopping is disabled and says when output was left out; and Settings search finds it.
use super::*;
use rune::{Cx, Env, Runtime};

fn view(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().children(section(s, &Palette::new(true)))
}

struct Drawn {
    texts: Vec<String>,
    status: Option<String>,
    checked: Option<bool>,
    pressable: Option<bool>,
    keys: Vec<String>,
}

fn drawn(state: RemoteControlState) -> Drawn {
    let mut s = State::default();
    s.remote_control = Some(state);
    let mut rt = Runtime::new(s, view, (900.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let look = crate::look::controls::Look::new(true);
    let switch = all.iter().find(|e| e.key.as_deref() == Some("remote-control-switch"));
    Drawn {
        texts: all.iter().filter_map(|e| e.text.clone()).collect(),
        status: all.iter().find(|e| e.role == "status").and_then(|e| e.text.clone()),
        checked: switch.map(|e| e.fill == look.c("switch-on")),
        pressable: switch.map(|e| e.focusable),
        keys: all.iter().filter_map(|e| e.key.clone()).collect(),
    }
}

fn off() -> RemoteControlState {
    RemoteControlState::default()
}

#[test]
fn the_off_switch_explains_its_lifetime_and_account_requirements() {
    let d = drawn(off());
    let all = d.texts.join("\n");
    assert!(all.contains("Let me start sessions on this computer from claude.ai"), "{all}");
    assert_eq!(d.checked, Some(false));
    for words in ["quit Locust", "API keys do not work", "Team and Enterprise owners"] {
        assert!(all.contains(words), "{words} in {all}");
    }
}

#[test]
fn off_it_reads_as_one_switch_its_state_opens_its_own_line_and_nothing_sits_under_it() {
    let d = drawn(off());
    let status = d.status.expect("its state, said as a status");
    assert!(status.starts_with("Off. When on, you can start Claude Code sessions in this folder from claude.ai"), "{status}");
    for under in ["remote-control-output", "remote-control-errors", "remote-control-problem"] {
        assert!(!d.keys.iter().any(|k| k == under), "{under} drawn while off");
    }
    assert!(!d.texts.iter().any(|t| t == "Off."), "no Off line of its own");
}

#[test]
fn a_running_switch_shows_claude_output_and_errors_without_summarizing_or_trimming_them() {
    let d = drawn(RemoteControlState {
        enabled: true,
        phase: Phase::Running,
        stdout: "  first\r\nhttps://claude.ai/code/session_fake\n".into(),
        stderr: " exact error\r\n".into(),
        error: Some("spawn ENOENT: exact".into()),
        ..off()
    });
    assert_eq!(d.checked, Some(true));
    for exact in ["  first\r\nhttps://claude.ai/code/session_fake\n", " exact error\r\n", "spawn ENOENT: exact"] {
        assert!(d.texts.iter().any(|t| t == exact), "{exact:?} verbatim in {:?}", d.texts);
    }
}

#[test]
fn a_trust_handoff_stays_off_and_asks_the_person_to_finish_setup_and_deliberately_enable_again() {
    let d = drawn(RemoteControlState { phase: Phase::NeedsPerson, ..off() });
    assert_eq!(d.checked, Some(false));
    assert!(d.texts.join("\n").contains("Complete setup in the Claude Code terminal, then enable this switch again."));
}

#[test]
fn the_switch_is_disabled_while_ending_and_states_when_later_output_was_omitted() {
    let d = drawn(RemoteControlState { phase: Phase::Stopping, truncated: true, ..off() });
    assert_eq!(d.pressable, Some(false));
    let all = d.texts.join("\n");
    assert!(all.contains("Waiting for Claude Code to end"), "{all}");
    assert!(all.contains("the rest was left out"), "{all}");
    assert_eq!(drawn(off()).pressable, Some(true), "the control: an off switch takes a press");
}

#[test]
fn settings_search_finds_remote_control_and_claude_ai_under_ai_agents_and_phone_at_your_phone_0_775() {
    use crate::settings_pages::{SettingsPageId, page_by_id, page_matches};
    for word in ["remote control", "claude.ai"] {
        assert!(page_matches(page_by_id(SettingsPageId::Runtimes), word), "{word}");
    }
    assert!(page_matches(page_by_id(SettingsPageId::App), "phone"));
}
