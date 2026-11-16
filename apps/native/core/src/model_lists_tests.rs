//! Three of Electron's main-process files about which models a run may use and what one costs, a case to a test with
//! Electron's values:
//! - `main/a-model-list-missing-one-is-asked-again.test.ts` (0.552): a list missing a ready runtime's models is not kept;
//! - `main/free-routes.test.ts`: which routes are free (`isFreeRoute`; `freeRoutesOnly` is not built, see the ledger);
//! - `main/a-model-of-your-own-is-unpriced-not-free.test.ts` (0.689): what a model of your own is, and that unpriced
//!   runs never move the limit.

use crate::models::{kept_catalog, lists_its_models_but_listed_none, Listed};
use crate::scout::host::is_free_route;
use crate::spend::{is_own_model, limit_reached, Spend};
use serde_json::json;
use std::cell::Cell;
use std::sync::Mutex;
use std::time::Instant;

// ---- a model list missing a ready runtime is asked for again ----

#[test]
fn a_ready_runtime_that_lists_its_models_and_listed_none_is_named_as_the_case() {
    assert!(lists_its_models_but_listed_none(&[("opencode", None)]));
    let listed = json!({"aliases": [], "efforts": [], "models": [{"id": "opencode/a-free", "displayName": "A"}]});
    assert!(!lists_its_models_but_listed_none(&[("opencode", Some(&listed))]));
    assert!(!lists_its_models_but_listed_none(&[("copilot", None)]));
}

#[test]
fn a_model_list_missing_a_ready_runtime_is_read_again_instead_of_kept() {
    let cache: Mutex<Option<(Instant, Vec<serde_json::Value>)>> = Mutex::new(None);
    let calls = Cell::new(0);
    let opencode = json!({"aliases": ["opencode/a-free"], "efforts": [], "models": [{"id": "opencode/a-free", "displayName": "A"}]});
    let read = || {
        calls.set(calls.get() + 1);
        let listed = Listed { opencode: Some(opencode.clone()), ..Default::default() };
        // First: Cursor is ready but listed nothing. Then it is not ready at all.
        let ready: Vec<(&str, Option<&serde_json::Value>)> =
            if calls.get() == 1 { vec![("opencode", Some(&opencode)), ("cursor", None)] } else { vec![("opencode", Some(&opencode))] };
        (listed.clone(), lists_its_models_but_listed_none(&ready))
    };
    let first = kept_catalog(&cache, read);
    let second = kept_catalog(&cache, read);
    let third = kept_catalog(&cache, read);
    assert!(!first.is_empty());
    assert_eq!(calls.get(), 3 - 1);
    // Complete from the second read on, so it is kept.
    assert_eq!(third, second);
}

// ---- free routes only ----

#[test]
fn the_free_routes_the_drives_use_are_known_as_free() {
    assert!(is_free_route("opencode", Some("opencode/muse-spark-1.3-contributor-free")));
    assert!(is_free_route("opencode", Some("ling-3.0-flash-fin-free")));
    assert!(is_free_route("kilo", Some("kilo/kilo-auto/free")));
    assert!(is_free_route("kilo", Some("kilo/some-vendor/model:free")));
    assert!(is_free_route("kilo", Some("vendor/model-free")));
}

#[test]
fn a_route_is_not_free_because_of_its_runtime_or_a_word_in_its_name() {
    assert!(!is_free_route("opencode", Some("anthropic/claude-sonnet-5")));
    assert!(!is_free_route("opencode", None));
    assert!(!is_free_route("cursor", Some("cursor-grok-4.6-free")));
    assert!(!is_free_route("opencode", Some("freestyle-pro")));
    // "free" inside an id is a word, not a price (code review B4, own runs 4).
    assert!(!is_free_route("opencode", Some("opencode/x-free-pro")));
    assert!(!is_free_route("opencode", Some("opencode/free-tier-preview-max")));
    assert!(!is_free_route("kilo", Some("kilo/google/gemini-3-pro-image")));
    assert!(!is_free_route("kilo", None));
    assert!(!is_free_route("kilo", Some("kilo/free-tier-preview")));
}

#[test]
fn every_free_model_opencode_1_18_27_lists_is_known_as_free() {
    // MEASURED 2026-09-26: `opencode models`, the six whose price is free.
    for model in [
        "ling-3.0-flash-fin-free",
        "mimo-v2.6-flash-free",
        "muse-spark-1.3-contributor-free",
        "nemotron-3-ultra-free",
        "nemotron-3.5-lightning-free",
        "space-bunny-free",
    ] {
        assert!(is_free_route("opencode", Some(&format!("opencode/{model}"))), "{model}");
    }
}

// ---- a model of your own is unpriced, not free ----

#[test]
fn a_model_of_your_own_is_one_the_person_added_which_opencode_runs_and_an_old_record_without_a_model_is_not() {
    assert!(is_own_model("opencode", "own-gateway/big-model"));
    assert!(!is_own_model("opencode", "opencode/free-model"));
    assert!(!is_own_model("claude", "own-gateway/big-model"));
    // Electron's `undefined`: a record without a model reads as an empty one here.
    assert!(!is_own_model("opencode", ""));
}

#[test]
fn unpriced_runs_never_move_the_limit() {
    assert!(!limit_reached(Some(&Spend { unpriced_runs: Some(40), ..Default::default() }), 5.0));
}
