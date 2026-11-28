//! a first-launch promise checked against the actual model catalogue at Home's drawing path.
use super::*;
use locust_core::agents::{Auth, Found, Probe};

// Electron FirstLaunch.tsx: the second sentence goes only on positive disproof.
const PROMISE: &str = "Your AI agents, on your own accounts. OpenCode works without one.";
const WITHOUT_PROMISE: &str = "Your AI agents, on your own accounts.";

fn status(status: Probe) -> Found {
    let ready = status == Probe::Ready;
    Found { id: "opencode".into(), display_name: "OpenCode".into(),
        installed: status != Probe::Missing, version: None, auth: Auth::Unknown,
        ready, status, unsupported_note: None, through_cli: false, checking: false }
}

fn drawn_intro(found: Vec<Found>, models: &[(&str, &[&str])]) -> String {
    let mut s = State::default();
    s.settings_panel.agents = Some(found);
    s.models.insert("test-catalogues".into(), models.iter().map(|(runtime, ids)| locust_core::engine::Models {
        teammate: None, runtime: (*runtime).into(), model: None, effort: None,
        models: ids.iter().map(|id| locust_core::engine::Model {
            id: (*id).into(), name: (*id).into(), efforts: Vec::new(),
        }).collect(),
    }).collect());
    let mut rt = rune::Runtime::new(
        s, |s, _| home_view(s, &Palette::new(true)),
        (1215.0, 2100.0), rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some("home-intro"))
        .and_then(|e| e.text).expect("the first-launch sentence is drawn")
}

// case: stands when a free model is listed
#[test]
fn free_promise_stands_when_a_free_model_is_listed() {
    let models = &["opencode/muse-spark-1.3-contributor-free", "opencode/paid-thing"];
    assert_eq!(drawn_intro(vec![status(Probe::Ready)], &[("opencode", models)]), PROMISE);
}

// case: is disproved only when the list was read and nothing in it is free
#[test]
fn free_promise_is_disproved_only_by_a_read_paid_only_catalogue() {
    assert_eq!(drawn_intro(vec![status(Probe::Ready)], &[("opencode", &["opencode/paid-thing"])]), WITHOUT_PROMISE);
}

// case: says unknown when the runtime is not ready, and never weakens the promise
#[test]
fn free_promise_is_not_weakened_when_opencode_is_not_ready() {
    for not_ready in [Probe::Missing, Probe::AuthRequired, Probe::ProbeFailed, Probe::Offline] {
        assert_eq!(drawn_intro(vec![status(not_ready)], &[]), PROMISE);
        assert_eq!(drawn_intro(vec![status(not_ready)], &[("opencode", &["opencode/paid-thing"])]), PROMISE);
    }
    assert_eq!(drawn_intro(vec![], &[]), PROMISE);
}

// case: says unknown when it is ready but no list has been read
#[test]
fn free_promise_is_not_weakened_by_an_unread_catalogue() {
    assert_eq!(drawn_intro(vec![status(Probe::Ready)], &[]), PROMISE);
    assert_eq!(drawn_intro(vec![status(Probe::Ready)], &[("opencode", &[])]), PROMISE);
}

// case: ignores other runtimes entirely, free or not
#[test]
fn free_promise_ignores_other_runtimes_entirely() {
    for other in ["cursor-thing-free", "cursor-paid"] {
        let ready = || vec![status(Probe::Ready)];
        assert_eq!(drawn_intro(ready(), &[("cursor", &[other])]), PROMISE);
        assert_eq!(drawn_intro(ready(), &[("opencode", &["opencode/free-one-free"]), ("cursor", &[other])]), PROMISE);
        assert_eq!(drawn_intro(ready(), &[("opencode", &["opencode/paid-thing"]), ("cursor", &[other])]), WITHOUT_PROMISE);
    }
}
