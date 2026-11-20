//! The free model "Use a free model" starts on, from Electron's `a-new-person-starts-on-the-steadiest-free-model`,
//! `a-free-model-that-is-down-is-passed-over`, `a-free-model-that-answered-comes-first` and
//! `a-retired-free-model-is-not-offered` (status.ts `freeStartModel`, `freeModelsBestFirst`), with Electron's values.
//! Rust has `connectors_off::free_start_model` and `free_models_best_first`; it has no `nextFreeModel`,
//! `downFreeModels`, `retiredFreeModels` or Compare's `defaultComparePicks`, so where a case
//! feeds one of those into `freeStartModel`, the set or list it gives is written in (each test says so).
use crate::connectors_off::{FREE_MODELS_BEST_FIRST, RunSeen, free_models_best_first, free_models_that_answered, free_start_model};
use crate::engine::Model;
use std::collections::HashSet;

fn model(id: &str) -> Model {
    Model { id: id.into(), name: id.into(), efforts: Vec::new() }
}
fn models(ids: &[&str]) -> Vec<Model> {
    ids.iter().map(|id| model(id)).collect()
}
fn start(listed: &[Model], answered: &[&str], down: &[&str]) -> Option<String> {
    let answered: Vec<String> = answered.iter().map(|s| s.to_string()).collect();
    let down: HashSet<String> = down.iter().map(|s| s.to_string()).collect();
    free_start_model("opencode", listed, &answered, &down)
}
fn ids(listed: Vec<&Model>) -> Vec<&str> {
    listed.into_iter().map(|m| m.id.as_str()).collect()
}

// a-new-person-starts-on-the-steadiest-free-model.test.ts. What OpenCode listed as free on 2026-10-22; none is one
// Electron knows retired (exo-free, fledge-alpha-free), so all are shown.
const FRESH: [&str; 9] = [
    "opencode/beam-free", "opencode/deepseek-v4.1-flash-free", "opencode/flint-40b-free",
    "opencode/glm-5.3-flash-free", "opencode/mimo-v2.6-flash-free", "opencode/minimax-m3.1-flash-free",
    "opencode/nemotron-3-ultra-free", "opencode/nemotron-3.5-lightning-free", "opencode/nemotron-3.5-super-vl-free",
];

#[test]
fn a_new_person_starts_not_on_the_first_the_alphabet_lists() {
    let shown = models(&FRESH);
    assert_eq!(start(&shown, &[], &[]).as_deref(), Some("opencode/deepseek-v4.1-flash-free"));
    assert_ne!(start(&shown, &[], &[]).as_deref(), Some("opencode/beam-free"));
}

/// The case's order half; its `nextFreeModel` half is not built.
#[test]
fn the_steadiest_come_first_with_the_unnamed_after_all_of_them() {
    let shown = models(&FRESH);
    let order = ids(free_models_best_first(&shown));
    assert_eq!(order[..FREE_MODELS_BEST_FIRST.len()], FREE_MODELS_BEST_FIRST[..]);
    assert_eq!(order[FREE_MODELS_BEST_FIRST.len()..], ["opencode/flint-40b-free", "opencode/minimax-m3.1-flash-free", "opencode/nemotron-3-ultra-free"]);
}

#[test]
fn the_order_only_orders_a_name_opencode_no_longer_lists_adds_nothing() {
    assert_eq!(ids(free_models_best_first(&models(&["opencode/zz-new-free"]))), ["opencode/zz-new-free"]);
    assert_eq!(start(&models(&["opencode/paid-thing"]), &[], &[]), None);
}

// a-free-model-that-is-down-is-passed-over.test.ts
const LING: &str = "opencode/ling-3.0-flash-fin-free";
const BEAM: &str = "opencode/beam-free";
const MIMO: &str = "opencode/mimo-v2.6-flash-free";

/// `down` is what `downFreeModels` gives for Ling's failed run an hour ago (Ling), written in.
#[test]
fn use_a_free_model_starts_on_one_that_is_up_even_one_that_answered_before() {
    let catalog = models(&[LING, MIMO, BEAM]);
    assert_eq!(start(&catalog, &[LING], &[]).as_deref(), Some(LING));
    assert_eq!(start(&catalog, &[LING], &[LING]).as_deref(), Some(MIMO));
    assert_eq!(start(&catalog, &[], &[LING, MIMO, BEAM]).as_deref(), Some(MIMO));
}

// a-free-model-that-answered-comes-first.test.ts
const LISTED: [&str; 4] = ["opencode/paid-thing", "opencode/ling-3.0-flash-fin-free", "opencode/muse-spark-1.3-contributor-free", "opencode/mimo-v2.6-flash-free"];

fn run<'a>(model: &'a str, phase: &'a str, at: &'a str) -> RunSeen<'a> {
    RunSeen { runtime: "opencode", model, phase, at }
}

#[test]
fn takes_the_newest_free_model_that_finished_a_run_when_the_catalogue_still_lists_it() {
    let answered = free_models_that_answered(&[
        run("opencode/muse-spark-1.3-contributor-free", "completed", "2026-09-29T10:00:00Z"),
        run("opencode/mimo-v2.6-flash-free", "completed", "2026-09-30T10:00:00Z"),
        run("opencode/ling-3.0-flash-fin-free", "failed", "2026-09-30T12:00:00Z"),
    ]);
    assert_eq!(answered, vec!["opencode/mimo-v2.6-flash-free".to_owned(), "opencode/muse-spark-1.3-contributor-free".to_owned()]);
    let answered: Vec<&str> = answered.iter().map(String::as_str).collect();
    assert_eq!(start(&models(&LISTED), &answered, &[]).as_deref(), Some("opencode/mimo-v2.6-flash-free"));
}

#[test]
fn a_run_that_failed_was_stopped_or_was_on_a_paid_or_other_model_does_not_count() {
    assert!(free_models_that_answered(&[
        run("opencode/ling-3.0-flash-fin-free", "failed", "2026-09-30T12:00:00Z"),
        run("opencode/mimo-v2.6-flash-free", "cancelled", "2026-09-30T12:00:00Z"),
        run("opencode/paid-thing", "completed", "2026-09-30T12:00:00Z"),
        RunSeen { runtime: "cursor", model: "cursor/grok-free", phase: "completed", at: "2026-09-30T12:00:00Z" },
        // Another agent on an OpenCode-named free model is still another agent's run (Electron's `runtime === 'opencode'`).
        RunSeen { runtime: "kilo", model: "opencode/mimo-v2.6-flash-free", phase: "completed", at: "2026-09-30T12:00:00Z" },
    ])
    .is_empty());
}

#[test]
fn one_no_longer_listed_is_passed_over_for_the_next_that_is() {
    let answered = ["opencode/gone-free", "opencode/muse-spark-1.3-contributor-free"];
    assert_eq!(start(&models(&LISTED), &answered, &[]).as_deref(), Some("opencode/muse-spark-1.3-contributor-free"));
}

#[test]
fn with_nothing_answered_yet_the_steadiest_listed_free_model() {
    assert_eq!(start(&models(&LISTED), &[], &[]).as_deref(), Some("opencode/mimo-v2.6-flash-free"));
}

/// Electron marks the person's own model `own: true`; a Rust model of your own is one whose route is
/// `own-<id>/<model>` (`own_models::own_route_model`), so the case's `mine-free` is written that way.
#[test]
fn never_a_model_of_the_persons_own_even_one_named_free() {
    let mine = "own-a1b2c3d0/mine-free";
    let mut own = models(&[mine]);
    own.extend(models(&LISTED));
    assert_eq!(start(&own, &[mine], &[]).as_deref(), Some("opencode/mimo-v2.6-flash-free"));
}

// a-retired-free-model-is-not-offered.test.ts
const RETIRED_CATALOG: [&str; 4] = ["opencode/exo-free", "opencode/ling-3.0-flash-fin-free", "opencode/glm-5.3-flash-free", "opencode/zz-later-free"];
fn shown_without(retired: &[&str]) -> Vec<Model> {
    models(&RETIRED_CATALOG.into_iter().filter(|id| !retired.contains(id)).collect::<Vec<_>>())
}

/// `retiredFreeModels([])` is Electron's known list (exo-free, fledge-alpha-free), written in.
#[test]
fn with_exo_free_left_out_a_new_person_starts_on_a_free_model_that_can_answer() {
    let shown = shown_without(&["opencode/exo-free", "opencode/fledge-alpha-free"]);
    assert!(!shown.iter().any(|m| m.id == "opencode/exo-free"));
    assert_eq!(start(&shown, &[], &[]).as_deref(), Some("opencode/glm-5.3-flash-free"));
}

/// The retired set is what `retiredFreeModels` gives once Ling's run ended "has been deprecated", written in.
#[test]
fn with_a_deprecated_model_left_out_it_starts_on_the_steadiest_left() {
    let shown = shown_without(&["opencode/exo-free", "opencode/fledge-alpha-free", "opencode/ling-3.0-flash-fin-free"]);
    assert_eq!(start(&shown, &[], &[]).as_deref(), Some("opencode/glm-5.3-flash-free"));
}
