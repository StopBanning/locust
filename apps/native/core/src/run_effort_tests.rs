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

// ---- status.test.ts "the route a mission starts on": start_route (status.ts startRoute), case by case ----

/// status.test.ts's catalogue for these cases: a Cursor family never listed without a level, and Claude's Sonnet.
fn route_models() -> Vec<Value> {
    vec![
        json!({"id": "cursor-grok-4.6-high-fast", "runtime": "cursor", "displayName": "cursor-grok-4.6", "description": "",
            "supportedEfforts": ["low", "high", "high-fast"],
            "variants": {"low": "cursor-grok-4.6-low", "high": "cursor-grok-4.6-high", "high-fast": "cursor-grok-4.6-high-fast"}}),
        json!({"id": "sonnet", "runtime": "claude", "displayName": "Sonnet", "description": "", "supportedEfforts": ["low", "medium", "high"]}),
    ]
}

fn sent(model: &str) -> (Option<String>, Option<String>) {
    (Some(model.to_owned()), None)
}

fn sent_with(model: &str, effort: &str) -> (Option<String>, Option<String>) {
    (Some(model.to_owned()), Some(effort.to_owned()))
}

#[test]
fn turns_an_effort_into_the_variant_it_names_and_sends_no_effort_beside_it() {
    assert_eq!(start_route(&route_models(), "cursor", Some("cursor-grok-4.6-high-fast"), Some("low")), sent("cursor-grok-4.6-low"));
}

#[test]
fn drops_an_effort_the_variants_do_not_name_rather_than_refuse_the_run() {
    assert_eq!(start_route(&route_models(), "cursor", Some("cursor-grok-4.6-high-fast"), Some("medium")), sent("cursor-grok-4.6-high-fast"));
}

#[test]
fn finds_the_family_when_the_id_in_hand_is_one_of_its_variants_not_its_own_id() {
    assert_eq!(start_route(&route_models(), "cursor", Some("cursor-grok-4.6-low"), Some("high")), sent("cursor-grok-4.6-high"));
    for from in ["cursor-grok-4.6-low", "cursor-grok-4.6-high", "cursor-grok-4.6-high-fast"] {
        for effort in ["low", "high", "high-fast", "medium", "nonsense"] {
            assert_eq!(start_route(&route_models(), "cursor", Some(from), Some(effort)).1, None, "{from} at {effort}");
        }
    }
}

#[test]
fn never_sends_an_effort_to_a_model_that_advertises_none() {
    let mut with_none = route_models();
    with_none.push(json!({"id": "opencode/muse-spark-1.3", "runtime": "opencode", "displayName": "Muse Spark", "description": "", "supportedEfforts": []}));
    assert_eq!(start_route(&with_none, "opencode", Some("opencode/muse-spark-1.3"), Some("medium")), sent("opencode/muse-spark-1.3"));
}

#[test]
fn sends_an_opencode_model_only_an_effort_it_lists() {
    let mut ling = route_models();
    ling.push(json!({"id": "opencode/ling", "runtime": "opencode", "displayName": "Ling", "description": "", "supportedEfforts": ["low", "medium", "high"]}));
    assert_eq!(start_route(&ling, "opencode", Some("opencode/ling"), Some("high")), sent_with("opencode/ling", "high"));
    assert_eq!(start_route(&ling, "opencode", Some("opencode/ling"), Some("xhigh")), sent("opencode/ling"));
}

#[test]
fn still_sends_an_effort_for_a_model_the_catalogue_has_never_heard_of() {
    assert_eq!(start_route(&route_models(), "claude", Some("account-default"), Some("high")), sent_with("account-default", "high"));
}

#[test]
fn sends_the_effort_as_itself_where_the_runtime_takes_it_as_a_flag() {
    assert_eq!(start_route(&route_models(), "claude", Some("sonnet"), Some("medium")), sent_with("sonnet", "medium"));
    assert_eq!(start_route(&route_models(), "claude", Some("sonnet"), None), sent("sonnet"));
}

// Rust's own: send_from hands over no model for the account's default (it names none on the command); the level is
// still judged on the account default's own row.
#[test]
fn judges_the_account_default_on_its_own_row_when_no_model_is_named() {
    let defaults = vec![json!({"id": "account-default", "runtime": "opencode", "supportedEfforts": []})];
    assert_eq!(start_route(&defaults, "opencode", None, Some("high")), (None, None));
    assert_eq!(start_route(&[], "claude", None, Some("high")), (None, Some("high".to_owned())));
}
