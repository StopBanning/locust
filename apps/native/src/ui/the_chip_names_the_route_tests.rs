//! THE CHIP NAMES THE ROUTE AS ELECTRON DOES (chip-1 row 1, LIVE-HOUR-1 fault 29): the same Claude Code conversation
//! with no teammate, on Opus in Auto, read `claude-opus-5-5 · Fixed` here and `Opus 5.5 · Medium` in Electron. The
//! window's list of every agent carries no levels, and the chip printed the id the run reported as it stood. Drawn
//! headless through `thread_view`; the rules' own cases are in core/src/a_chip_reads_the_catalogue_tests.rs.
use super::*;
use locust_core::engine::{Model, Models, Teammate};
use rune::{Env, Runtime};
use serde_json::json;

/// The words the model chip reads in a conversation with no teammate on `runtime` and `model`, whose window list (the
/// door's `list_models` with no teammate) names `listed` with no levels; `resolved` is what a finished run on the route
/// reported, and `catalogue` what this process's catalogue read holds.
fn chip_reads(runtime: &str, model: &str, listed: &[(&str, &str)], resolved: Option<&str>, catalogue: &[serde_json::Value]) -> Vec<String> {
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate {
            id: LEDGER_TARGET.into(),
            name: "Claude Code".into(),
            role: "Conversation".into(),
            runtime: Some(runtime.into()),
            model: Some(model.into()),
            account: None,
            busy: false,
            runs_in: String::new(),
            folder: None,
        },
        conversation: Conversation { conversation_id: Some("mission_1".into()), turns: Vec::new() },
        items: Vec::new(),
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot::default(),
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    s.models.insert(
        LEDGER_TARGET.into(),
        vec![Models {
            teammate: None,
            runtime: runtime.into(),
            model: None,
            effort: None,
            models: listed.iter().map(|(id, name)| Model { id: (*id).into(), name: (*name).into(), efforts: vec![] }).collect(),
        }],
    );
    s.settings_panel.agents = Some(vec![locust_core::agents::Found {
        id: runtime.into(),
        display_name: runtime.into(),
        installed: true,
        version: None,
        auth: locust_core::agents::Auth::Authenticated,
        ready: true,
        status: locust_core::agents::Probe::Ready,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }]);
    if let Some(id) = resolved {
        s.resolved_model_names.insert(format!("{runtime}:{model}"), id.into());
    }
    locust_core::models::keep_for_the_chip(catalogue);
    fn view(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view, (1000.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let ([x, y, w, h], _) = rt.presented("model-chip").expect("the model chip is drawn");
    rt.elements()
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect()
}

/// model-catalog.ts `claudeModelsFrom`: the alias row, named for the version it means, with the levels the CLI names.
fn opus_row() -> serde_json::Value {
    json!({"id": "opus", "runtime": "claude", "displayName": "Opus 5.5", "description": "Always the newest Opus",
        "supportedEfforts": ["low", "medium", "high", "xhigh", "max"]})
}

// case: names the version on the controls that pick a route, and nothing else changes
// a-claude-route-names-its-version.test.tsx: `routeModelName('claude', 'opus', 'claude-opus-5-5-20260901')` is
// "Opus 5.5"; status.test.ts `defaultEffort` prefers medium. The fault's conversation: Electron's `Opus 5.5 · Medium`.
#[test]
fn a_conversation_with_no_teammate_on_opus_reads_the_version_and_its_level() {
    let words = chip_reads("claude", "opus", &[("opus", "Opus 5.5")], Some("claude-opus-5-5-20260901"), &[opus_row()]);
    assert_eq!(words, ["Opus 5.5 · Medium"]);
}

// case: is the version the alias means, until a run on it says otherwise
// A run reported `claude-opus-5-5`: the chip says the version, never the id as it stands.
#[test]
fn the_id_a_run_reported_is_said_as_its_version() {
    let words = chip_reads("claude", "opus", &[("opus", "Opus 5.5")], Some("claude-opus-5-5"), &[opus_row()]);
    assert_eq!(words, ["Opus 5.5 · Medium"]);
    // A route stored on the resolved id itself, which the catalogue folds under its alias: Electron's `modelFamily`
    // finds no row, so the version alone, with no level claimed (Composer.tsx `effortPhrase`).
    let words = chip_reads("claude", "claude-opus-5-5", &[("opus", "Opus 5.5")], None, &[opus_row()]);
    assert_eq!(words, ["Opus 5.5"]);
}

// case: spells the id when no catalog has been read
// a-model-has-one-name.test.ts: Codex's id spelt out (`GPT-6 Luna`), whatever its run reported; its levels from the
// catalogue's row (model-catalog.ts `parseModels`), medium first (status.test.ts).
#[test]
fn a_codex_route_reads_its_name_and_level_not_the_id_its_run_reported() {
    let row = json!({"id": "gpt-6.2-sol", "runtime": "codex", "displayName": "gpt-6.2-sol", "description": "",
        "supportedEfforts": ["low", "medium", "high", "xhigh"]});
    let words = chip_reads("codex", "gpt-6.2-sol", &[("gpt-6.2-sol", "gpt-6.2-sol")], Some("gpt-6.2-sol-2026-11-01"), &[row]);
    assert_eq!(words, ["GPT-6.2 Sol · Medium"]);
}

// case: never takes an identifier for a name (seen on the probe: "nemotron-3-ultra-free" in the chip)
// a-model-has-one-name.test.ts: "Nemotron 3 Ultra Free", and the-chip-keeps-free.test.ts draws "Free" as its own tag.
#[test]
fn an_opencode_free_route_is_spelt_with_free_as_its_own_tag() {
    let row = json!({"id": "opencode/nemotron-3-ultra-free", "runtime": "opencode", "displayName": "nemotron-3-ultra-free",
        "description": "Free · no sign-in", "supportedEfforts": ["low", "medium", "high"]});
    let listed = [("opencode/nemotron-3-ultra-free", "nemotron-3-ultra-free")];
    let words = chip_reads("opencode", "opencode/nemotron-3-ultra-free", &listed, None, &[row]);
    assert_eq!(words, ["Nemotron 3 Ultra · Medium", "Free"]);
}

// case: has nothing to show when the model reports no levels
// Composer.tsx `effortIsGenuinelyFixed`: only once the catalogue has answered for this model, and it lists none.
#[test]
fn fixed_is_said_only_when_the_catalogue_lists_no_levels() {
    let row = json!({"id": "claude-opus-4-5", "runtime": "claude", "displayName": "Opus 4.5", "description": "This version, always",
        "supportedEfforts": [], "older": true});
    let words = chip_reads("claude", "claude-opus-4-5", &[("claude-opus-4-5", "Opus 4.5")], None, &[opus_row(), row]);
    assert_eq!(words, ["Opus 4.5 · Fixed"]);
}

// Composer.tsx `effortIsGenuinelyFixed` ("Only true once the catalog has actually answered for THIS model"): before this
// process has read the catalogue, the window's list of every agent is all there is, and it names no levels, so the chip
// claims none -- not "Fixed", which is what the fault's conversation read.
#[test]
fn before_the_catalogue_answers_a_conversation_with_no_teammate_claims_no_level() {
    let words = chip_reads("claude", "opus", &[("opus", "Opus 5.5")], Some("claude-opus-5-5"), &[]);
    assert_eq!(words, ["Opus 5.5"]);
}

// Lane check of chip-1 row 1: the level shown is the runtime's own default where the catalogue names one. Composer.tsx:1663
// `defaultEffort(supportedEfforts, shownFamily?.defaultEffort)`, and status.ts:1422 takes `runtimeDefault` whenever the
// model lists it, before medium: a family whose catalogue default is High reads High, not the Medium a list with no
// default would give.
#[test]
fn the_level_shown_is_the_catalogues_own_default_where_it_names_one() {
    let row = json!({"id": "gpt-6.2-sol", "runtime": "codex", "displayName": "gpt-6.2-sol", "description": "",
        "supportedEfforts": ["low", "medium", "high", "xhigh"], "defaultEffort": "high"});
    let words = chip_reads("codex", "gpt-6.2-sol", &[("gpt-6.2-sol", "gpt-6.2-sol")], None, &[row]);
    assert_eq!(words, ["GPT-6.2 Sol · High"]);
}
