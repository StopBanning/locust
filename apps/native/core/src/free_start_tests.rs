//! Electron's `the-first-send-cannot-spend.test.ts` (status.ts `freeStartModel`): a first send on the free route takes a
//! free model, never a paid one.

use crate::connectors_off::free_start_model;
use crate::engine::{parse_models, Model};
use std::collections::HashSet;

fn model(id: &str) -> Model {
    Model { id: id.into(), name: id.into(), efforts: Vec::new() }
}
fn pick(runtime: &str, listed: &[Model]) -> Option<String> {
    free_start_model(runtime, listed, &[], &HashSet::new())
}

#[test]
fn takes_a_free_model_the_steadiest_listed_first_and_among_the_unnamed_the_first_listed() {
    let listed = [model("opencode/paid-thing"), model("opencode/ling-3.0-flash-fin-free"), model("opencode/glm-5.3-flash-free")];
    assert_eq!(pick("opencode", &listed).as_deref(), Some("opencode/glm-5.3-flash-free"));
    let unnamed = [model("opencode/paid-thing"), model("opencode/zz-one-free"), model("opencode/aa-two-free")];
    assert_eq!(pick("opencode", &unnamed).as_deref(), Some("opencode/zz-one-free"));
}

#[test]
fn leaves_the_route_alone_when_nothing_listed_is_free() {
    assert_eq!(pick("opencode", &[model("opencode/paid-thing")]), None);
}

#[test]
fn leaves_the_route_alone_before_the_catalogue_has_been_read() {
    assert_eq!(pick("opencode", &[]), None);
}

#[test]
fn never_answers_for_a_runtime_that_needs_an_account() {
    assert_eq!(pick("codex", &[model("codex/some-free")]), None);
}

#[test]
fn does_not_treat_a_free_model_as_a_free_model_of_another_runtime() {
    // Native's catalogue arrives grouped by runtime, and the free route reads only OpenCode's group (thread_view.rs,
    // scout_cards.rs): another runtime's "-free" model never reaches it.
    let groups = parse_models(r#"[{"runtime":"cursor","models":[{"id":"cursor/grok-free"}]}]"#).unwrap();
    let opencode: Vec<Model> = groups.into_iter().filter(|g| g.runtime == "opencode").flat_map(|g| g.models).collect();
    assert_eq!(pick("opencode", &opencode), None);
}

#[test]
fn still_reads_account_default_as_the_unchosen_state() {
    assert_eq!(crate::models::ACCOUNT_DEFAULT, "account-default");
}
