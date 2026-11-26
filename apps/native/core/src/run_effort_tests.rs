//! status.test.ts "the level on the chip is the level the run is given": one test per case. Each asks the rule both
//! ways the app does: as the chip asks it (the window's levels for the model, no catalogue default) and as `send_from`
//! asks it (the catalogue as read). Expected values are Electron's, from its test and App.tsx runEffortFor.
use super::*;
use serde_json::json;

fn levels(of: &[&str]) -> Vec<String> {
    of.iter().map(|l| (*l).to_owned()).collect()
}

/// The chip's call (thread_view.rs) and send_from's (run_effort_at_start) for one route, with `catalog` as read.
fn both(catalog: Vec<Value>, runtime: &str, model: &str, chosen: Option<&str>) -> (Option<String>, Option<String>) {
    let listed: Vec<String> = catalog
        .iter()
        .find(|m| m["id"] == model && m["runtime"] == runtime)
        .and_then(|m| m["supportedEfforts"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|l| l.as_str().map(str::to_owned))
        .collect();
    let chip = run_effort(runtime, model, &listed, None, chosen);
    use_catalog_on_this_thread(Some(catalog));
    let run = run_effort_at_start(runtime, Some(model), chosen);
    use_catalog_on_this_thread(None);
    (chip, run)
}

/// status.test.ts's models: Sol lists low, medium and high; Copilot's Auto lists none.
fn models() -> Vec<Value> {
    vec![
        json!({"id": "gpt-5.6-sol", "runtime": "codex", "displayName": "Sol", "description": "", "supportedEfforts": ["low", "medium", "high"]}),
        json!({"id": "auto", "runtime": "copilot", "displayName": "Auto", "description": "", "supportedEfforts": []}),
    ]
}

/// status.test.ts's two Cursor families: Opus 5.5 stands on its `-medium` variant, Grok 4.7 on its `-low` one.
fn cursor() -> Vec<Value> {
    vec![
        json!({"id": "claude-opus-5-5-medium", "runtime": "cursor", "displayName": "Claude Opus 5.5 1M", "description": "",
            "supportedEfforts": ["low", "medium", "high", "max"],
            "variants": {"low": "claude-opus-5-5-low", "medium": "claude-opus-5-5-medium", "high": "claude-opus-5-5-high", "max": "claude-opus-5-5-max"},
            "defaultEffort": "medium"}),
        json!({"id": "grok-4.7-low", "runtime": "cursor", "displayName": "Grok 4.7", "description": "",
            "supportedEfforts": ["low", "medium", "high"],
            "variants": {"low": "grok-4.7-low", "medium": "grok-4.7-medium", "high": "grok-4.7-high"}}),
    ]
}

#[test]
fn agree_when_nobody_has_chosen_a_level() {
    let (chip, run) = both(models(), "codex", "gpt-5.6-sol", None);
    assert_eq!(chip.as_deref(), Some("medium"));
    assert_eq!(run.as_deref(), Some("medium"));
}

#[test]
fn agree_when_someone_has() {
    let (chip, run) = both(models(), "codex", "gpt-5.6-sol", Some("high"));
    assert_eq!(chip.as_deref(), Some("high"));
    assert_eq!(run.as_deref(), Some("high"));
}

#[test]
fn agree_on_a_cursor_family_whose_id_already_names_a_level() {
    for (model, chosen, expected) in [("claude-opus-5-5-medium", None, "medium"), ("grok-4.7-low", None, "low"), ("grok-4.7-low", Some("high"), "high")] {
        let (chip, run) = both(cursor(), "cursor", model, chosen);
        assert_eq!(chip.as_deref(), Some(expected), "chip, {model} {chosen:?}");
        assert_eq!(run.as_deref(), Some(expected), "run, {model} {chosen:?}");
    }
}

#[test]
fn agree_on_a_model_with_no_levels_both_say_nothing_and_nothing_is_sent() {
    let (chip, run) = both(models(), "copilot", "auto", None);
    assert_eq!(chip, None);
    assert_eq!(run, None);
}

#[test]
fn never_invents_a_level_for_a_model_the_catalogue_does_not_know() {
    let (chip, run) = both(models(), "codex", "not-in-catalogue", None);
    assert_eq!(chip, None);
    assert_eq!(run, None);
}

// Rust's own: before the window has read the catalogue, a run nobody chose a level for is sent none, never one guessed.
#[test]
fn sends_no_level_before_the_catalogue_has_been_read() {
    use_catalog_on_this_thread(None);
    assert_eq!(run_effort_at_start("codex", Some("gpt-5.6-sol"), None), None);
    assert_eq!(run_effort_at_start("codex", Some("gpt-5.6-sol"), Some("low")).as_deref(), Some("low"));
}

// Rust's own: the window's catalogue carries no `defaultEffort`, so the chip falls back to the one runtime with a
// default of its own beyond Cursor's, and agrees with the run, which reads it from the catalogue (model-catalog.ts).
#[test]
fn a_muse_model_shows_and_is_sent_its_own_default_where_the_window_has_no_catalogue_default() {
    let muse = vec![json!({"id": "muse-spark", "runtime": "muse", "supportedEfforts": ["low", "medium", "high"], "defaultEffort": "high"})];
    let (chip, run) = both(muse, "muse", "muse-spark", None);
    assert_eq!(chip.as_deref(), Some("high"));
    assert_eq!(run.as_deref(), Some("high"));
    assert_eq!(run_effort("codex", "x", &levels(&["low", "medium", "high"]), None, None).as_deref(), Some("medium"));
}
