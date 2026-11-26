//! Three of Electron's main-process files about which models a run may use and what one costs, a case to a test with
//! Electron's values:
//! - `main/a-model-list-missing-one-is-asked-again.test.ts` (0.552): a list missing a ready runtime's models is not kept;
//! - `main/free-routes.test.ts`: which routes are free (`isFreeRoute`), whether a launch is held to them (`freeRoutesOnly`),
//!   and the start gate that holds it (`native::send_from`, as codex-mission.ts and antigravity-mission.ts);
//! - `main/a-model-of-your-own-is-unpriced-not-free.test.ts` (0.689): what a model of your own is, and that unpriced
//!   runs never move the limit.

use crate::model_lists::{FREE_ONLY_REFUSAL, free_routes_only, hold_free_routes_for_test};
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

// ---- whether a launch is held to free routes (freeRoutesOnly) ----

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_owned()).collect()
}

fn environment(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |key| pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_owned())
}

#[test]
fn holds_a_launch_with_a_debugging_port_to_free_routes() {
    // Every drive and smoke takes the window over this way, harness or not.
    assert!(free_routes_only(&argv(&["Locust.exe", "--remote-debugging-port=9294"]), &environment(&[])));
    assert!(free_routes_only(&argv(&["electron.exe", ".", "--remote-debugging-port=9294", "--user-data-dir=x"]), &environment(&[])));
}

#[test]
fn leaves_an_ordinary_launch_alone() {
    // A person's own Locust must never be held to free routes by this.
    assert!(!free_routes_only(&argv(&["Locust.exe"]), &environment(&[])));
    assert!(!free_routes_only(&argv(&["Locust.exe", "--some-other-switch"]), &environment(&[])));
}

#[test]
fn lifts_only_for_the_person_who_said_to_spend() {
    let port = argv(&["Locust.exe", "--remote-debugging-port=9294"]);
    assert!(!free_routes_only(&port, &environment(&[("LOCUST_SPEND", "1")])));
    assert!(free_routes_only(&port, &environment(&[("LOCUST_SPEND", "yes")])));
    assert!(free_routes_only(&argv(&["Locust.exe"]), &environment(&[("LOCUST_FREE_ONLY", "1")])));
}

// ---- the start gate: a launch held to free routes starts nothing else ----

/// A temporary data folder, removed after the engine has let go of it.
struct Root(std::path::PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The engine on a temporary store, running `command` for its agent. The guard is dropped first: every turn is stopped
/// and has ended before the folder goes.
struct Host {
    _engine: crate::native::hooks::Guard,
    store: crate::store::Store,
    root: Root,
}

fn host(command: Option<(std::path::PathBuf, Vec<String>)>) -> Host {
    let root = Root(std::env::temp_dir().join(format!("locust-free-gate-{}", crate::invariants::unique_nanos())));
    std::fs::create_dir_all(root.0.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.0.join("work")).unwrap();
    let store = crate::store::Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    Host { _engine: crate::native::hooks::install(store.clone(), command), store, root }
}

fn job(host: &Host, runtime: &str, model: Option<&str>) -> crate::native::Send {
    crate::native::Send {
        teammate: crate::engine::Teammate {
            id: "tm_drive".into(),
            name: "Drive".into(),
            role: String::new(),
            runtime: Some(runtime.into()),
            model: model.map(str::to_owned),
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: Some(host.root.0.join("work").to_string_lossy().into_owned()),
        },
        conversation: None,
        prompt: "Say hello.".into(),
        model: None,
        effort: None,
    }
}

fn ledgers(host: &Host) -> usize {
    std::fs::read_dir(host.store.ledger_dir().unwrap()).map_or(0, |dir| dir.flatten().count())
}

// case: refuses a paid route in a window held to free routes, before anything is asked or recorded
#[test]
fn a_launch_held_to_free_routes_refuses_a_route_that_is_not_free_before_anything_is_recorded() {
    // No agent to run: a start that got past the gate would fail some other way, not with these words.
    let host = host(None);
    let _held = hold_free_routes_for_test(true);
    // Electron's routes that are not free, and Antigravity, which has none.
    for (runtime, model) in [
        ("opencode", Some("anthropic/claude-sonnet-5")),
        ("opencode", Some("opencode/x-free-pro")),
        ("opencode", None),
        ("kilo", Some("kilo/google/gemini-3-pro-image")),
        ("cursor", Some("cursor-grok-4.6-medium")),
        ("codex", None),
        ("codex", Some("gpt-5.1-codex")),
        ("claude", Some("sonnet")),
        ("claude", None),
        ("antigravity", None),
    ] {
        let refused = crate::native::send_from(job(&host, runtime, model), Default::default()).err();
        assert_eq!(refused.as_deref(), Some(FREE_ONLY_REFUSAL), "{runtime} {model:?}");
    }
    assert_eq!(ledgers(&host), 0, "a refused start leaves no record");
}

// case: lets a free route through a window held to free routes
#[test]
fn a_launch_held_to_free_routes_starts_a_free_route() {
    let fake_opencode = (
        std::path::PathBuf::from("python3"),
        vec![concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m4-opencode-serve.py").into(), "serve".into(), "success".into()],
    );
    let host = host(Some(fake_opencode));
    let _held = hold_free_routes_for_test(true);
    // A free route the drives use (Electron's free-routes.test.ts).
    let started = crate::native::send_from(job(&host, "opencode", Some("opencode/muse-spark-1.3-contributor-free")), Default::default())
        .expect("a free route starts in a held launch");
    assert!(host.store.write_path(&started.conversation_id).unwrap().is_file(), "its turn is recorded");
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

/// The test file's RUNS, one turn per run: [mission, model, usage, owner], all on OpenCode, at its AT.
fn unpriced_turn(conversation: &str, model: &str, usd: Option<f64>, input: u64, output: u64) -> crate::usage::UsageTurn {
    crate::usage::UsageTurn {
        at: "2026-09-15T12:00:00.000Z".into(),
        runtime: "opencode".into(),
        model: model.into(),
        conversation: conversation.into(),
        input_tokens: Some(input),
        output_tokens: Some(output),
        cache_read_tokens: None,
        cache_write_tokens: None,
        usd,
        premium_requests: None,
        plan: false,
    }
}

#[test]
fn a_run_with_tokens_and_no_price_counts_as_unpriced_never_as_dollars_and_a_free_catalog_model_stays_nothing() {
    use crate::spend::spent_by;
    let turns = [
        // Wren runs on a gateway of the person's own: two unpriced runs, one priced.
        unpriced_turn("mission_own_1", "own-gateway/big-model", None, 900, 40),
        unpriced_turn("mission_own_2", "own-gateway/big-model", None, 300, 20),
        unpriced_turn("mission_own_3", "own-gateway/big-model", Some(0.4), 300, 20),
        // A run that used no tokens cost nothing anywhere.
        unpriced_turn("mission_own_4", "own-gateway/big-model", None, 0, 0),
        // Juno runs on OpenCode's own free model: free, and said as nothing, as before.
        unpriced_turn("mission_free_1", "opencode/free-model", None, 900, 40),
    ];
    let owner = |mission: &str| if mission.starts_with("mission_own_") { "tm_wren" } else { "tm_juno" };
    assert_eq!(spent_by(&turns, "2026-09", |m| owner(m) == "tm_wren"), Spend { usd: Some(0.4), premium_requests: None, unpriced_runs: Some(2) });
    assert_eq!(spent_by(&turns, "2026-09", |m| owner(m) == "tm_juno"), Spend::default());
    assert_eq!(spent_by(&turns, "2026-10", |_| true), Spend::default());
}

#[test]
fn unpriced_runs_never_move_the_limit() {
    assert!(!limit_reached(Some(&Spend { unpriced_runs: Some(40), ..Default::default() }), 5.0));
}
