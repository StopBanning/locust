//! Electron's `routeName.test.ts`, `a-model-has-one-name.test.ts`, `a-kilo-model-has-one-name.test.ts` and
//! `your-own-model-is-named-as-yours.test.tsx`, the cases Rust builds, with Electron's values and words.
//!
//! Rust's `teammate_tooltip::route_model_name` is Electron's `routeModelName` (a Claude alias or id read through
//! `claudeModelName`); for every other runtime it is `modelDisplayName` without a catalog read. The hover's
//! `teammate_tooltip` is Rust's "Runtime / Model" (`routeChrome(shortRuntimeName, modelDisplayName)`), and it spells a
//! Claude id as `modelDisplayName` does, so the Claude cases on `modelDisplayName` are read there.
use crate::engine::Teammate;
use crate::teammate_tooltip::{model_display_name, route_model_name as name, teammate_tooltip};

/// The route as the hover says it after "Message Wren · Custom · ".
fn route(runtime: &str, model: &str) -> String {
    let mate = Teammate { id: "tm_test".into(), name: "Wren".into(), role: "Custom".into(), runtime: Some(runtime.into()),
        model: Some(model.into()), account: None, busy: false, runs_in: String::new(), folder: None };
    teammate_tooltip(&mate).strip_prefix("Message Wren · Custom · ").expect("the hover names the route").to_owned()
}

// routeName.test.ts: how a route reads on a chip
#[test]
fn says_the_three_colin_named_exactly_as_he_named_them() {
    assert_eq!(route("cursor", "cursor-grok-4.6"), "Cursor / Grok 4.6");
    assert_eq!(route("claude", "fable-5.1"), "Claude / Fable 5.1");
    assert_eq!(route("codex", "gpt-6-astra"), "Codex / GPT-6 Astra");
}

#[test]
fn keeps_the_hyphen_only_where_the_makers_keep_one() {
    assert_eq!(name("codex", "gpt-5.6-luna"), "GPT-5.6 Luna");
    assert_eq!(name("cursor", "cursor-grok-4.6-medium"), "Grok 4.6 Medium");
    assert_eq!(name("cursor", "composer-2.5"), "Composer 2.5");
}

#[test]
fn spells_the_free_models_a_new_person_meets_first_as_their_makers_do() {
    assert_eq!(name("opencode", "opencode/deepseek-v4.1-flash-free"), "DeepSeek V4.1 Flash Free");
    assert_eq!(name("opencode", "opencode/glm-5.3-flash-free"), "GLM 5.3 Flash Free");
    assert_eq!(name("opencode", "opencode/mimo-v2.6-flash-free"), "MiMo V2.6 Flash Free");
    assert_eq!(name("opencode", "opencode/nemotron-3.5-super-vl-free"), "Nemotron 3.5 Super VL Free");
    assert_eq!(name("opencode", "opencode/minimax-m2.5-free"), "MiniMax M2.5 Free");
}

#[test]
fn does_not_touch_a_name_that_already_reads_as_one() {
    assert_eq!(name("codex", "GPT-6 Astra"), "GPT-6 Astra");
    assert_eq!(name("cursor", "Grok 4.6"), "Grok 4.6");
    assert_eq!(name("claude", "Sonnet"), "Sonnet");
}

#[test]
fn drops_a_provider_that_repeats_the_runtime_and_keeps_one_that_does_not() {
    assert_eq!(name("opencode", "opencode/ling-3.0-flash-fin-free"), "Ling 3.0 Flash Fin Free");
    assert_eq!(name("opencode", "anthropic/claude-sonnet-4"), "Anthropic/Claude Sonnet 4");
}

#[test]
fn invents_nothing_every_word_comes_from_the_id() {
    assert_eq!(name("cursor", "some-unheard-of-model-9"), "Some Unheard Of Model 9");
    assert_eq!(model_display_name("claude", "sonnet"), "Sonnet");
}

#[test]
fn leaves_an_id_with_nothing_but_the_runtime_in_it_alone() {
    assert_eq!(name("cursor", "cursor"), "Cursor");
}

#[test]
fn reads_a_version_written_with_a_hyphen_as_a_version() {
    assert_eq!(name("cursor", "claude-opus-5-5-medium"), "Claude Opus 5.5 Medium");
    assert_eq!(name("cursor", "claude-opus-4-8-thinking-low"), "Claude Opus 4.8 Thinking Low");
    assert_eq!(name("cursor", "claude-3-5-sonnet"), "Claude 3.5 Sonnet");
    assert_eq!(route("claude", "claude-haiku-4-5-20251001"), "Claude / Haiku 4.5 20251001");
    assert_eq!(name("codex", "gpt-4o-2024-08-06"), "GPT-4o 2024 08 06");
    assert_eq!(name("cursor", "cursor-grok-4.6-high"), "Grok 4.6 High");
    assert_eq!(name("cursor", "some-model-1-2-3"), "Some Model 1 2 3");
}

#[test]
fn keeps_the_second_word_where_it_is_the_program_being_named() {
    assert_eq!(route("opencode", "x"), "OpenCode / X");
    assert_eq!(route("antigravity", "x"), "Antigravity / X");
}

// routeName.test.ts: a model the runtime picks for itself
#[test]
fn says_it_is_a_model_never_the_bare_word_a_mode_is_named() {
    assert_eq!(name("copilot", "auto"), "Auto model");
    assert_eq!(name("cursor", "auto"), "Auto model");
    assert!(!name("copilot", "gpt-5.6-luna").contains("model"));
}

// a-model-has-one-name.test.ts
#[test]
fn spells_the_id_when_no_catalog_has_been_read() {
    assert_eq!(name("codex", "gpt-6-luna"), "GPT-6 Luna");
}

// a-kilo-model-has-one-name.test.ts
#[test]
fn spells_the_id_while_kilo_has_not_listed_its_models_yet() {
    assert_eq!(name("kilo", "kilo/nvidia/nemotron-3-nano-30b-a3b"), "Nvidia/Nemotron 3 Nano 30b A3b");
}

// your-own-model-is-named-as-yours.test.tsx
#[test]
fn a_model_of_your_own_is_known_by_its_route() {
    use crate::own_models::is_own_route_model;
    assert!(is_own_route_model("own-a1b2c3d0/acme-70b"));
    assert!(!is_own_route_model("opencode/muse-spark-1.3-contributor-free"));
    assert!(!is_own_route_model("gpt-6-astra"));
}

// routeChrome, with Electron's values from a-kilo-model-has-one-name, your-own-model-is-named-as-yours,
// the-screens-say-less and routine-dialog-names-the-model.
#[test]
fn route_chrome_says_a_route_as_the_composer_chip_does() {
    use crate::teammate_tooltip::route_chrome;
    assert_eq!(route_chrome("kilo", "kilo/kilo-auto/free", "Kilo Auto Free", " / "), "Kilo Auto Free");
    assert_eq!(
        route_chrome("kilo", "kilo/nvidia/nemotron-3.5-lightning:free", "NVIDIA: Nemotron 3.5 Lightning (free)", " / "),
        "Kilo / NVIDIA: Nemotron 3.5 Lightning (free)"
    );
    assert_eq!(route_chrome("opencode", "own-a1b2c3d0/acme-70b", "Acme Chat", " / "), "Acme Chat");
    assert_eq!(route_chrome("opencode", "own-a1b2c3d0/acme-70b", "Acme Chat", " · "), "Acme Chat");
    assert_eq!(route_chrome("codex", "gpt-6-astra", "GPT-6 Astra", " / "), "Codex / GPT-6 Astra");
    assert_eq!(
        route_chrome("opencode", "opencode/ling-3.0-flash-fin-free", "Ling 3.0 Flash Fin Free", " · "),
        "OpenCode · Ling 3.0 Flash Fin Free"
    );
    assert_eq!(route_chrome("codex", "account-default", "Account Default", " / "), "Codex");
    assert_eq!(route_chrome("claude", "account-default", "default", " / "), "Claude");
    assert_eq!(route_chrome("codex", "account-default", "Account Default · Medium", " · "), "Codex · Medium");
    assert_eq!(route_chrome("codex", "gpt-6-luna", "GPT-6 Luna", " / "), "Codex / GPT-6 Luna");
    assert_eq!(
        route_chrome("opencode", "opencode/nemotron-3-ultra-free", &model_display_name("opencode", "opencode/nemotron-3-ultra-free"), " / "),
        "OpenCode / Nemotron 3 Ultra Free"
    );
    // Not own: the hex is not eight, so the runtime is said.
    assert_eq!(route_chrome("opencode", "own-a1b2/acme", "Acme", " / "), "OpenCode / Acme");
}
