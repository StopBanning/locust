//! Electron `the-free-promise-checks-itself.test.ts` (5 cases): Home's first-launch OpenCode
//! promise stands when a free model is listed, is withdrawn only when OpenCode's list was read
//! and contains no free model, and stays `Unknown` (leaving the promise intact) otherwise.

use super::*;
use locust_core::agents::{Auth, Found, Probe};
use locust_core::home_order::FreeStart;

fn opencode(status: Probe) -> Found {
    Found {
        id: "opencode".into(),
        display_name: "OpenCode".into(),
        installed: status != Probe::Missing,
        version: None,
        auth: if status == Probe::Ready { Auth::Authenticated } else { Auth::Unknown },
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

fn agent_models(runtime: &str, ids: &[&str]) -> engine::Models {
    engine::Models {
        teammate: None,
        runtime: runtime.into(),
        model: None,
        effort: None,
        models: ids
            .iter()
            .map(|id| engine::Model { id: (*id).into(), name: (*id).into(), efforts: Vec::new() })
            .collect(),
    }
}

fn state_with(runtimes: Vec<Found>, catalog: Vec<engine::Models>) -> State {
    let mut s = State::default();
    s.settings_panel.agents = Some(runtimes);
    if !catalog.is_empty() {
        s.models.insert(CARRY_TARGET.into(), catalog);
    }
    s
}

fn drawn_intro(s: State) -> Option<String> {
    let mut rt = rune::Runtime::new(
        s,
        |s, _| home_view(s, &Palette::new(true)),
        (1200.0, 1200.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("home-intro"))
        .and_then(|e| e.text)
}

// case: stands when a free model is listed
#[test]
fn stands_when_a_free_model_is_listed() {
    let s = state_with(
        vec![opencode(Probe::Ready)],
        vec![agent_models("opencode", &["opencode/muse-spark-1.3-contributor-free", "opencode/paid-thing"])],
    );
    assert_eq!(free_start_for(&s), FreeStart::Yes);
    assert_eq!(
        drawn_intro(s).as_deref(),
        Some("Your AI agents, on your own accounts. OpenCode works without one.")
    );
}

// case: is disproved only when the list was read and nothing in it is free
#[test]
fn is_disproved_only_when_the_list_was_read_and_nothing_in_it_is_free() {
    let s = state_with(
        vec![opencode(Probe::Ready)],
        vec![agent_models("opencode", &["opencode/paid-thing"])],
    );
    assert_eq!(free_start_for(&s), FreeStart::No);
    assert_eq!(
        drawn_intro(s).as_deref(),
        Some("Your AI agents, on your own accounts.")
    );
}

// case: says unknown when the runtime is not ready, and never weakens the promise
#[test]
fn says_unknown_when_the_runtime_is_not_ready_and_never_weakens_the_promise() {
    for status in [Probe::Missing, Probe::AuthRequired, Probe::ProbeFailed, Probe::Offline] {
        let s = state_with(vec![opencode(status)], Vec::new());
        assert_eq!(free_start_for(&s), FreeStart::Unknown, "{status:?}");
        assert_eq!(
            drawn_intro(s).as_deref(),
            Some("Your AI agents, on your own accounts. OpenCode works without one."),
            "{status:?}"
        );
    }
    let empty = state_with(Vec::new(), Vec::new());
    assert_eq!(free_start_for(&empty), FreeStart::Unknown);
    assert_eq!(
        drawn_intro(empty).as_deref(),
        Some("Your AI agents, on your own accounts. OpenCode works without one.")
    );
}

// case: says unknown when it is ready but no list has been read
#[test]
fn says_unknown_when_it_is_ready_but_no_list_has_been_read() {
    let s = state_with(vec![opencode(Probe::Ready)], Vec::new());
    assert_eq!(free_start_for(&s), FreeStart::Unknown);
    assert_eq!(
        drawn_intro(s).as_deref(),
        Some("Your AI agents, on your own accounts. OpenCode works without one.")
    );
}

// case: ignores other runtimes entirely, free or not
#[test]
fn ignores_other_runtimes_entirely_free_or_not() {
    let other_free = state_with(
        vec![opencode(Probe::Ready)],
        vec![agent_models("cursor", &["cursor-thing-free"])],
    );
    assert_eq!(free_start_for(&other_free), FreeStart::Unknown);

    let other_paid = state_with(
        vec![opencode(Probe::Ready)],
        vec![agent_models("cursor", &["cursor-paid"])],
    );
    assert_eq!(free_start_for(&other_paid), FreeStart::Unknown);

    let both = state_with(
        vec![opencode(Probe::Ready)],
        vec![
            agent_models("opencode", &["opencode/free-one-free"]),
            agent_models("cursor", &["cursor-paid"]),
        ],
    );
    assert_eq!(free_start_for(&both), FreeStart::Yes);
    assert_eq!(
        drawn_intro(both).as_deref(),
        Some("Your AI agents, on your own accounts. OpenCode works without one.")
    );
}
