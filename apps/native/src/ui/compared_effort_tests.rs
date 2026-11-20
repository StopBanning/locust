//! each-compared-model-has-its-own-effort.test.tsx (4 cases): each model in a comparison has its own effort at the level
//! chosen for it, its default when none was (fixed 20 Nov: Rust showed the first level, "Low", where Electron shows the
//! default, "Medium"), the level a Cursor id names, and none for a model with no levels; and each column is started with
//! its effort as a flag, or as the Cursor id it names (`models::cursor_choice_for`, which `native::send` applies).
use super::*;
use locust_core::engine::{Model, Models};

fn models() -> Vec<Models> {
    let agent = |runtime: &str, effort: Option<&str>, list: Vec<Model>| Models { teammate: None, runtime: runtime.into(), model: None, effort: effort.map(str::to_owned), models: list };
    let model = |id: &str, efforts: &[&str]| Model { id: id.into(), name: id.into(), efforts: efforts.iter().map(|e| (*e).to_owned()).collect() };
    vec![
        agent("claude", Some("high"), vec![model("opus", &["low", "medium", "high", "xhigh", "max"])]),
        agent("codex", Some("medium"), vec![model("gpt-6", &["low", "medium", "high", "xhigh"])]),
        agent("opencode", None, vec![model("big-pickle-free", &[])]),
        agent("cursor", None, vec![model("cursor-grok-4.7-high", &["low", "high"])]),
    ]
}

fn pick(runtime: &str, model: &str, effort: Option<&str>) -> ComparePick {
    ComparePick { runtime: runtime.into(), model: model.into(), account: None, effort: effort.map(str::to_owned), label: None }
}

fn labels(picks: &[ComparePick]) -> Vec<String> {
    let m = models();
    picks.iter().enumerate().filter_map(|(i, p)| effort_aria_label(i, p, &m)).collect()
}

// case: gives each model its own effort, at the level chosen for it
#[test]
fn each_model_has_its_own_chosen_effort() {
    assert_eq!(labels(&[pick("claude", "opus", Some("max")), pick("codex", "gpt-6", Some("low"))]), ["Effort for model A: Max", "Effort for model B: Low"]);
}

// case: shows a model's default when none was chosen, and the level a Cursor id already names
#[test]
fn a_models_default_and_the_level_a_cursor_id_names() {
    assert_eq!(labels(&[pick("codex", "gpt-6", None), pick("cursor", "cursor-grok-4.7-low", None)]), ["Effort for model A: Medium", "Effort for model B: Low"]);
    assert_eq!(labels(&[pick("claude", "opus", None)]), ["Effort for model A: High"], "the runtime's own default");
}

// case: offers no effort for a model that lists no levels
#[test]
fn no_effort_for_a_model_with_no_levels() {
    assert_eq!(labels(&[pick("claude", "opus", None), pick("opencode", "big-pickle-free", None)]), ["Effort for model A: High"]);
}

// case: sends the effort as a flag, or as the Cursor id it names
#[test]
fn the_effort_goes_as_a_flag_or_as_the_cursor_id() {
    let catalogue = vec![serde_json::json!({"id": "cursor-grok-4.7-high", "runtime": "cursor", "variants": {"low": "cursor-grok-4.7-low", "high": "cursor-grok-4.7-high"}})];
    assert_eq!(locust_core::models::cursor_choice_for(&catalogue, Some("cursor-grok-4.7-high"), Some("low")), (Some("cursor-grok-4.7-low".into()), None));
    assert_eq!(locust_core::models::cursor_choice_for(&[], Some("opus"), Some("max")), (Some("opus".into()), Some("max".into())));
}
