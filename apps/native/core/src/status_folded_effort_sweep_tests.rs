//! The status picker and actual native job must agree on where the chosen level lives.
use crate::run_effort::start_route;
use serde_json::{Value, json};

fn catalog() -> [Value; 2] {
    [
        json!({"runtime":"cursor","id":"composer-2.5","supportedEfforts":["fast"],
            "variants":{"fast":"composer-2.5-fast"}}),
        json!({"runtime":"codex","id":"gpt-5.6","supportedEfforts":["low","high"]}),
    ]
}

// case: is recognised through the family id
#[test]
fn cursor_family_id_sends_its_variant_without_a_second_effort() {
    assert_eq!(
        start_route(&catalog(), "cursor", Some("composer-2.5"), Some("fast")),
        (Some("composer-2.5-fast".into()), None)
    );
}

// case: is recognised through the variant id a run actually resolves to
#[test]
fn cursor_variant_id_keeps_the_variant_and_removes_the_second_effort() {
    assert_eq!(
        start_route(&catalog(), "cursor", Some("composer-2.5-fast"), Some("fast")),
        (Some("composer-2.5-fast".into()), None)
    );
}

// case: is not claimed for a runtime that takes the effort beside the model
#[test]
fn codex_gets_the_effort_beside_the_model_it_names() {
    assert_eq!(
        start_route(&catalog(), "codex", Some("gpt-5.6"), Some("high")),
        (Some("gpt-5.6".into()), Some("high".into()))
    );
}

// case: is not claimed for a model nothing in the catalog folds
#[test]
fn an_unlisted_cursor_model_does_not_have_a_proven_fold() {
    assert_eq!(
        start_route(&catalog()[1..], "cursor", Some("composer-2.5-fast"), Some("fast")),
        (Some("composer-2.5-fast".into()), Some("fast".into()))
    );
}
