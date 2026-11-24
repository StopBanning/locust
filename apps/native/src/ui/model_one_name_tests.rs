//! Electron's `a-model-has-one-name.test.ts`, `a-kilo-model-has-one-name.test.ts` and the picker's half of
//! `a-claude-route-names-its-version.test.tsx`, through the window: a catalogue read into `State.models` (the app's own
//! `ModelCatalogues::insert`, Electron's `rememberOwnModels`) and the names a team row and the picker then draw.
use crate::*;
use super::route_label;
use locust_core::groups::GroupRoute;
use locust_core::teammate_tooltip::{model_display_name, route_chrome, route_model_name};
use rune::{Env, Runtime};

fn catalogue(s: &mut State, rows: &[(&str, &str, &str)]) {
    let mut agents: Vec<engine::Models> = Vec::new();
    for (runtime, id, name) in rows {
        if !agents.iter().any(|a| a.runtime == *runtime) {
            agents.push(engine::Models { teammate: None, runtime: (*runtime).into(), model: None, effort: None, models: vec![] });
        }
        let agent = agents.iter_mut().find(|a| a.runtime == *runtime).expect("added above");
        agent.models.push(engine::Model { id: (*id).into(), name: (*name).into(), efforts: vec![] });
    }
    s.models.insert("tm_wren".into(), agents);
}

fn row(s: &State, runtime: &str, model: &str) -> String {
    let route = GroupRoute { runtime: runtime.into(), model: model.into(), mode: "auto".into(), account: None, effort: None };
    route_label(s, &route)
}

// a-model-has-one-name.test.ts
#[test]
fn takes_the_runtimes_spelling_of_the_same_name() {
    let mut s = State::default();
    catalogue(&mut s, &[("codex", "gpt-6-luna", "GPT-6-Luna"), ("codex", "gpt-6.1-sol", "GPT-6.1-Sol")]);
    assert_eq!(model_display_name("codex", "gpt-6-luna"), "GPT-6-Luna");
    assert_eq!(route_model_name("codex", "gpt-6.1-sol"), "GPT-6.1-Sol");
    assert_eq!(row(&s, "codex", "gpt-6-luna"), "Codex / GPT-6-Luna");
}

#[test]
fn never_takes_an_identifier_for_a_name_seen_on_the_probe_nemotron_3_ultra_free_in_the_chip() {
    let mut s = State::default();
    catalogue(&mut s, &[("opencode", "opencode/nemotron-3-ultra-free", "nemotron-3-ultra-free"), ("codex", "gpt-6-luna", "gpt-6-luna")]);
    assert_eq!(model_display_name("opencode", "opencode/nemotron-3-ultra-free"), "Nemotron 3 Ultra Free");
    assert_eq!(model_display_name("codex", "gpt-6-luna"), "GPT-6 Luna");
    assert_eq!(row(&s, "opencode", "opencode/nemotron-3-ultra-free"), "OpenCode / Nemotron 3 Ultra Free");
}

#[test]
fn leaves_locusts_own_account_default_as_it_reads_everywhere_seen_on_the_screen_tour() {
    let mut s = State::default();
    catalogue(&mut s, &[("codex", "account-default", "Account default")]);
    assert_eq!(model_display_name("codex", "account-default"), "Account Default");
}

#[test]
fn never_takes_a_different_name_and_keeps_runtimes_apart() {
    let mut s = State::default();
    catalogue(&mut s, &[("codex", "gpt-6-luna", "Fast and affordable"), ("codex", "gpt-6-sol", "GPT-6-Sol")]);
    assert_eq!(model_display_name("codex", "gpt-6-luna"), "GPT-6 Luna");
    assert_eq!(model_display_name("copilot", "gpt-6-sol"), "GPT-6 Sol");
    assert_eq!(row(&s, "codex", "gpt-6-luna"), "Codex / GPT-6 Luna");
}

// a-kilo-model-has-one-name.test.ts
#[test]
fn says_the_name_kilo_lists_it_by_as_the_picker_does() {
    let mut s = State::default();
    catalogue(&mut s, &[
        ("kilo", "kilo/kilo-auto/free", "Kilo Auto Free"),
        ("kilo", "kilo/nvidia/nemotron-3.5-lightning:free", "NVIDIA: Nemotron 3.5 Lightning (free)"),
        ("kilo", "kilo/~anthropic/claude-sonnet-latest", "Anthropic: Claude Sonnet Latest"),
    ]);
    assert_eq!(model_display_name("kilo", "kilo/kilo-auto/free"), "Kilo Auto Free");
    // A row of the team or a room says the runtime once.
    assert_eq!(route_chrome("kilo", "kilo/kilo-auto/free", &model_display_name("kilo", "kilo/kilo-auto/free"), " / "), "Kilo Auto Free");
    assert_eq!(row(&s, "kilo", "kilo/kilo-auto/free"), "Kilo Auto Free");
    assert_eq!(route_chrome("kilo", "kilo/nvidia/nemotron-3.5-lightning:free", "NVIDIA: Nemotron 3.5 Lightning (free)", " / "), "Kilo / NVIDIA: Nemotron 3.5 Lightning (free)");
    assert_eq!(row(&s, "kilo", "kilo/nvidia/nemotron-3.5-lightning:free"), "Kilo / NVIDIA: Nemotron 3.5 Lightning (free)");
    assert_eq!(model_display_name("kilo", "kilo/nvidia/nemotron-3.5-lightning:free"), "NVIDIA: Nemotron 3.5 Lightning (free)");
    assert_eq!(model_display_name("kilo", "kilo/~anthropic/claude-sonnet-latest"), "Anthropic: Claude Sonnet Latest");
}

#[test]
fn leaves_every_other_runtime_to_spell_its_ids() {
    let mut s = State::default();
    catalogue(&mut s, &[("codex", "gpt-6-luna", "Fast and affordable")]);
    assert_eq!(model_display_name("codex", "gpt-6-luna"), "GPT-6 Luna");
}

// a-claude-route-names-its-version.test.tsx: the picker
fn picker_words(resolved: &[(&str, &str)]) -> Vec<String> {
    let mut s = State::default();
    s.teammates.push(locust_core::engine::Teammate {
        id: "tm_wren".into(), name: "Wren".into(), role: "Code".into(), runtime: Some("claude".into()),
        model: Some("opus".into()), account: None, busy: false, runs_in: "ask".into(), folder: None,
    });
    s.selected = 0;
    s.conversations.insert("tm_wren".into(), Conversation::default());
    catalogue(&mut s, &[("claude", "opus", "Opus 5.5")]);
    for (key, id) in resolved {
        s.resolved_model_names.insert((*key).into(), (*id).into());
    }
    s.model_menu = Some("tm_wren".into());
    fn view(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view, (1000.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let row = rt.find("model-claude-opus").expect("the picker draws the opus row");
    let _ = row;
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

#[test]
fn labels_the_row_with_the_version() {
    assert!(picker_words(&[]).iter().any(|w| w == "Opus 5.5"));
}

#[test]
fn believes_a_finished_run_over_the_table() {
    let words = picker_words(&[("claude:opus", "claude-opus-4-7")]);
    assert!(words.iter().any(|w| w == "Opus 4.7"), "got: {words:?}");
    assert!(!words.iter().any(|w| w == "Opus 5.5"), "got: {words:?}");
}
