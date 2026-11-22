//! qa-0365-composer.test.tsx: the actual box, inspected through Rune's accessibility tree.
use super::*;
use rune::{Env, Runtime};
fn box_words(agents: Vec<Found>, opencode: bool) -> String {
    let mut s = found(agents);
    s.nothing_installed = !s
        .settings_panel
        .agents
        .as_ref()
        .unwrap()
        .iter()
        .any(|a| a.installed);
    if opencode {
        s.picks.insert(
            "codec".into(),
            TurnPick {
                model: "account-default".into(),
                runtime: Some("opencode".into()),
                effort: None,
                account: None,
            },
        );
    }
    let mut rt = Runtime::new(s, crate::view, (1200.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.016);
    assert!(rt.find("composer").is_some());
    fn find(v: &serde_json::Value) -> Option<String> {
        if v["key"] == "composer" {
            return v["placeholder"].as_str().map(str::to_owned);
        }
        match v {
            serde_json::Value::Object(o) => o.values().find_map(find),
            serde_json::Value::Array(a) => a.iter().find_map(find),
            _ => None,
        }
    }
    find(&rt.accessibility_json()).expect("the drawn box's placeholder")
}
fn just_installed() -> Vec<Found> {
    vec![
        agent("codex", "Codex CLI", Is::Absent),
        agent("opencode", "OpenCode", Is::Ready),
    ]
}

// case: does not tell a person who just installed OpenCode to install an AI agent
#[test]
fn does_not_tell_a_person_who_just_installed_opencode_to_install_an_ai_agent() {
    assert!(!box_words(just_installed(), false).contains("Install an AI agent"));
}
// case: invites a message, since one runtime can run it
#[test]
fn invites_a_message_since_one_runtime_can_run_it() {
    let s = box_words(just_installed(), false);
    assert_eq!(s, "OpenCode is ready — pick it in the model menu…");
}
// case: still says to install one when nothing is installed
#[test]
fn still_says_to_install_one_when_nothing_is_installed() {
    assert_eq!(
        box_words(vec![agent("codex", "Codex CLI", Is::Absent)], false),
        "Install an AI agent to start…"
    );
}
// case: invites a message when the route itself is the ready runtime
#[test]
fn invites_a_message_when_the_route_itself_is_the_ready_runtime() {
    let s = box_words(just_installed(), true);
    assert!(
        s.starts_with("Message ") || s.starts_with("Write a message"),
        "{s}"
    );
}
// case: does not tell the person to sign in, because there is nothing to sign in to
#[test]
fn does_not_tell_the_person_to_sign_in_because_there_is_nothing_to_sign_in_to() {
    let s = box_words(
        vec![
            agent("codex", "Codex CLI", Is::Absent),
            agent("opencode", "OpenCode", Is::Absent),
        ],
        false,
    );
    assert_eq!(s, "Install an AI agent to start…");
    assert!(!s.contains("sign in"));
}
// case: still says sign in when a runtime is installed but not usable
#[test]
fn still_says_sign_in_when_a_runtime_is_installed_but_not_usable() {
    assert!(
        box_words(vec![agent("codex", "Codex CLI", Is::SignedOut)], false)
            .contains("sign in to start…")
    );
}
