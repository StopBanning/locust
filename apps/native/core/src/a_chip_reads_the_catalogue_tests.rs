//! What the composer's model chip reads off the catalogue (chip-1 row 1): `chip_family` (status.ts `modelFamily`),
//! `chip_levels` (Composer.tsx `supportedEfforts` and `effortIsGenuinelyFixed`) and `chip_model_name` (Composer.tsx
//! `namedModel` and `modelLabel`). The chip drawn is in src/ui/the_chip_names_the_route_tests.rs.
use super::*;

fn levels(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_owned()).collect()
}

/// model-catalog.ts `cursorModelsFrom`: a family Cursor lists only through its variants, under its first variant's id.
fn cursor_family() -> Value {
    json!({"id": "cursor-grok-4.6-low", "runtime": "cursor", "displayName": "Grok 4.6", "supportedEfforts": ["low", "high", "high-fast"],
        "variants": {"low": "cursor-grok-4.6-low", "high": "cursor-grok-4.6-high", "high-fast": "cursor-grok-4.6-high-fast"}})
}

// case: finds the family when the id in hand is one of its variants, not its own id
// status.test.ts: the id in hand is a sibling variant of the family's own; the family is still found, by its name.
#[test]
fn a_variant_id_finds_its_family() {
    let catalog = vec![cursor_family()];
    let found = chip_family(&catalog, "cursor", "cursor-grok-4.6-high-fast").expect("found through its variants");
    assert_eq!(found.levels, levels(&["low", "high", "high-fast"]));
    assert_eq!(found.family_name.as_deref(), Some("Grok 4.6"));
    // Only on its own runtime, and a model it does not list is not found.
    assert_eq!(chip_family(&catalog, "claude", "cursor-grok-4.6-high-fast"), None);
    assert_eq!(chip_family(&catalog, "cursor", "cursor-grok-4.7-low"), None);
}

// A row with no variants is named by the route, not by the row (Electron: `family?.variants !== undefined`), and keeps
// the runtime's own default (`shownFamily?.defaultEffort`).
#[test]
fn a_row_of_its_own_keeps_its_levels_and_default_and_no_family_name() {
    let catalog = vec![json!({"id": "muse-spark", "runtime": "muse", "displayName": "Muse Spark", "supportedEfforts": ["low", "high"], "defaultEffort": "low"})];
    let found = chip_family(&catalog, "muse", "muse-spark").expect("listed");
    assert_eq!(found, ChipFamily { levels: levels(&["low", "high"]), default_effort: Some("low".into()), family_name: None });
}

// The window's list of every agent carries no levels: the catalogue's family speaks for it; the window's own levels
// (a teammate's agent, narrowed by policy) speak first; an empty row with nothing else known says "lists none"; and
// before anything has answered, nothing is claimed (Composer.tsx: "Only true once the catalog has actually answered").
#[test]
fn the_levels_come_from_the_catalogue_when_the_window_list_has_none() {
    let family = ChipFamily { levels: levels(&["low", "medium", "high"]), default_effort: Some("high".into()), family_name: None };
    let from_catalogue = chip_levels(Some(&[]), Some(family.clone())).expect("answered");
    assert_eq!(from_catalogue.levels, levels(&["low", "medium", "high"]));
    assert_eq!(from_catalogue.default_effort.as_deref(), Some("high"));
    let narrowed = levels(&["low"]);
    let from_window = chip_levels(Some(&narrowed), Some(family)).expect("answered");
    assert_eq!(from_window.levels, narrowed);
    assert_eq!(from_window.default_effort.as_deref(), Some("high"));
    assert_eq!(chip_levels(Some(&[]), None), Some(ChipFamily::default()));
    assert_eq!(chip_levels(None, None), None);
}

// case: names the version on the controls that pick a route, and nothing else changes
// a-claude-route-names-its-version.test.tsx: `routeModelName('claude', 'opus', 'claude-opus-5-5-20260901')`.
#[test]
fn a_claude_route_is_named_by_the_version_its_run_reported() {
    assert_eq!(chip_model_name("claude", "opus", None, Some("Opus 5.5"), Some("claude-opus-5-5-20260901"), Some("medium")), "Opus 5.5");
    assert_eq!(chip_model_name("claude", "opus", None, Some("Opus 5.5"), Some("claude-opus-4-7"), None), "Opus 4.7");
}

// case: is the version the alias means, until a run on it says otherwise
// "A pinned full id reads as itself": `claudeRouteModelName('claude-opus-4-8')` is "Opus 4.8".
#[test]
fn a_stored_claude_id_reads_as_its_version() {
    assert_eq!(chip_model_name("claude", "claude-opus-5-5", None, Some("claude-opus-5-5"), None, None), "Opus 5.5");
    assert_eq!(chip_model_name("claude", "claude-opus-4-8", None, None, None, None), "Opus 4.8");
}

// case: spells the id when no catalog has been read
// a-model-has-one-name.test.ts: `modelDisplayName('codex', 'gpt-6-luna')`; outside Claude a run's report is not a name.
#[test]
fn a_codex_id_is_spelt_and_its_report_is_not_a_name() {
    let row = ChipFamily { levels: levels(&["low", "medium"]), ..ChipFamily::default() };
    assert_eq!(chip_model_name("codex", "gpt-6-luna", Some(&row), Some("gpt-6-luna"), None, None), "GPT-6 Luna");
    assert_eq!(chip_model_name("codex", "gpt-6.2-sol", Some(&row), Some("gpt-6.2-sol"), Some("gpt-6.2-sol-2026-11-01"), Some("medium")), "GPT-6.2 Sol");
}

// case: never takes an identifier for a name (seen on the probe: "nemotron-3-ultra-free" in the chip)
#[test]
fn an_identifier_is_never_a_name() {
    let name = chip_model_name("opencode", "opencode/nemotron-3-ultra-free", None, Some("nemotron-3-ultra-free"), None, None);
    assert_eq!(name, "Nemotron 3 Ultra Free");
    // Before the catalogue is read here, the window's own name for a model stands in for the catalogue's.
    assert_eq!(chip_model_name("cursor", "claude-opus-5-5-medium", None, Some("Claude Opus 5.5 1M"), None, Some("medium")), "Claude Opus 5.5 1M");
}

// A family is named by the family (Composer.tsx `namedModel`: `shownFamily.displayName` where it has variants).
#[test]
fn a_family_is_named_by_the_family() {
    let found = chip_family(&[cursor_family()], "cursor", "cursor-grok-4.6-high").expect("found");
    assert_eq!(chip_model_name("cursor", "cursor-grok-4.6-high", Some(&found), Some("cursor-grok-4.6-high"), None, Some("high")), "Grok 4.6");
}

// Composer.tsx `modelLabel`: "the row read `Grok 4.6 Medium` beside a slider saying `medium`" -- the level a name ends
// in is not said twice; a name that is only the level stays whole.
#[test]
fn the_level_a_name_ends_in_is_not_said_twice() {
    assert_eq!(chip_model_name("cursor", "cursor-grok-4.6-medium", None, None, None, Some("medium")), "Grok 4.6");
    assert_eq!(chip_model_name("cursor", "cursor-grok-4.6-medium", None, None, None, Some("high")), "Grok 4.6 Medium");
    assert_eq!(chip_model_name("cursor", "cursor-grok-4.6-high", None, None, None, Some("high-fast")), "Grok 4.6");
    assert_eq!(chip_model_name("codex", "high", None, None, None, Some("high")), "High");
}
