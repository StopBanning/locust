//! Electron's `picker-empty-states.test.tsx`, drawn headless through `thread_view::model_picker`.
//! When `s.models` has no models for the conversation yet, `model_picker` draws "Reading the models…" (Electron:
//! "Still reading the model list") and never "Nothing matches that."; when models exist and the search matches none,
//! it draws "Nothing matches that.".
use super::*;
use locust_core::engine::{Model, Models, Teammate};
use rune::{Env, Runtime};

fn view(s: &State, _: &mut Cx<State>) -> El {
    let mate = Teammate {
        id: "tm_1".into(),
        name: "Wren".into(),
        role: "Coder".into(),
        runtime: Some("opencode".into()),
        model: Some("account-default".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    };
    col().w_full().child(model_picker(s, &crate::look::controls::Look::new(true), "tm_1", &mate, None, "account-default", None, false))
}
fn drawn(state: State) -> String {
    let mut rt = Runtime::new(state, view, (600.0, 500.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|n| n.text).collect::<Vec<_>>().join("\n")
}

#[test]
fn does_not_blame_the_search_when_the_catalog_has_not_arrived() {
    let mut s = State::default();
    s.model_query.text = "free".into();
    let said = drawn(s);
    assert!(said.contains("Reading the models…"), "{said}");
    assert!(!said.contains("Nothing matches that."));
}

#[test]
fn still_blames_the_search_when_there_genuinely_is_nothing_like_it() {
    let mut s = State::default();
    s.models.insert(
        "tm_1".into(),
        vec![Models {
            runtime: "opencode".into(),
            model: Some("account-default".into()),
            effort: None,
            teammate: None,
            models: vec![Model { id: "account-default".into(), name: "Account default".into(), efforts: vec![] }],
        }],
    );
    s.model_query.text = "zzzz-nonexistent".into();
    let said = drawn(s);
    assert!(said.contains("Nothing matches that."), "{said}");
    assert!(!said.contains("Reading the models…"));
}

/// The wiring: no agents, no models and no search, so there are no rows at all, and the empty branch the picker draws
/// is the loading sentence, never the search one. Rust keeps no separate helper: the picker itself decides.
#[test]
fn is_what_the_picker_actually_renders_not_just_what_the_helper_returns() {
    let said = drawn(State::default());
    assert!(said.contains("Reading the models…"), "{said}");
    assert!(!said.contains("Nothing matches that."), "{said}");
}
