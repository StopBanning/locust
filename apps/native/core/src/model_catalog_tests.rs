//! Electron's `main/model-catalog.test.ts`, a case to a test with its values: Codex's `model/list` read, the catalog
//! kept and read again, Cursor's variants made into models, Claude Code's aliases and older versions, and the account
//! default row each runtime gets. What Rust has no counterpart for is in `parity/tests.tsv`.

use crate::models::{
    account_default_model, catalog, claude_models_from, cursor_models_from, kept_catalog, parse_codex_models,
    shared_cursor_name, split_cursor_model_id, with_account_defaults, Listed,
};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn real_result() -> Value {
    json!({"data": [
        {"id": "gpt-5.6-sol", "displayName": "GPT-5.6-Sol", "description": "Latest frontier agentic coding model.", "hidden": false,
         "supportedReasoningEfforts": [{"reasoningEffort": "low"}, {"reasoningEffort": "medium"}, {"reasoningEffort": "high"},
            {"reasoningEffort": "xhigh"}, {"reasoningEffort": "max"}, {"reasoningEffort": "ultra"}]},
        {"id": "gpt-5.4-mini", "displayName": "GPT-5.4-mini", "supportedReasoningEfforts": [{"reasoningEffort": "low"}, {"reasoningEffort": "high"}]}
    ]})
}

fn ids(models: &[Value]) -> Vec<&str> {
    models.iter().map(|m| m["id"].as_str().unwrap_or("")).collect()
}

fn names(models: &[Value]) -> Vec<&str> {
    models.iter().map(|m| m["displayName"].as_str().unwrap_or("")).collect()
}

fn current(models: &[Value]) -> Vec<Value> {
    models.iter().filter(|m| m["older"] != true).cloned().collect()
}

// ---- parsing the protocol list ----

#[test]
fn codex_models_are_read_with_their_own_supported_efforts() {
    let models = parse_codex_models(&real_result());
    assert_eq!(ids(&models), ["gpt-5.6-sol", "gpt-5.4-mini"]);
    assert_eq!(models[0]["supportedEfforts"], json!(["low", "medium", "high", "xhigh", "max", "ultra"]));
    assert_eq!(models[1]["supportedEfforts"], json!(["low", "high"]));
}

#[test]
fn a_codex_model_with_no_id_and_a_hidden_one_are_dropped() {
    let models = parse_codex_models(&json!({"data": [{"displayName": "nameless"}, {"id": "secret", "hidden": true}, {"id": "kept"}]}));
    assert_eq!(ids(&models), ["kept"]);
}

#[test]
fn unreadable_codex_efforts_are_none_rather_than_all() {
    let models = parse_codex_models(&json!({"data": [{"id": "x", "supportedReasoningEfforts": "nonsense"}]}));
    assert_eq!(models[0]["supportedEfforts"], json!([]));
}

#[test]
fn a_codex_list_of_an_unknown_shape_reads_as_no_models() {
    assert!(parse_codex_models(&Value::Null).is_empty());
    assert!(parse_codex_models(&json!({"models": [{"id": "x"}]})).is_empty());
}

// ---- the catalog probe ----

#[test]
fn codexs_two_models_come_after_the_account_default_row() {
    let models = catalog(&Listed { codex: Some(real_result()), ..Default::default() });
    assert_eq!(models.len(), 3);
    assert_eq!(models[0]["id"], "account-default");
}

fn codex_only() -> (Listed, bool) {
    (Listed { codex: Some(real_result()), ..Default::default() }, false)
}

#[test]
fn a_whole_catalog_is_kept_instead_of_read_per_call() {
    let cache = Mutex::new(None);
    let reads = AtomicUsize::new(0);
    let read = || {
        reads.fetch_add(1, Ordering::SeqCst);
        codex_only()
    };
    kept_catalog(&cache, read);
    kept_catalog(&cache, read);
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn a_catalog_that_listed_nothing_is_not_kept() {
    let cache = Mutex::new(None);
    let reads = AtomicUsize::new(0);
    let read = || {
        reads.fetch_add(1, Ordering::SeqCst);
        (Listed { codex: Some(json!({"data": []})), ..Default::default() }, false)
    };
    assert!(kept_catalog(&cache, read).is_empty());
    assert!(kept_catalog(&cache, read).is_empty());
    assert_eq!(reads.load(Ordering::SeqCst), 2);
}

#[test]
fn a_second_caller_is_answered_from_the_first_read_rather_than_starting_two() {
    let cache = Arc::new(Mutex::new(None));
    let reads = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let first = {
        let (cache, reads) = (cache.clone(), reads.clone());
        std::thread::spawn(move || {
            kept_catalog(&cache, || {
                reads.fetch_add(1, Ordering::SeqCst);
                started_tx.send(()).unwrap();
                // A read takes a while: the second caller arrives while it runs.
                std::thread::sleep(Duration::from_millis(300));
                codex_only()
            })
        })
    };
    started_rx.recv_timeout(Duration::from_secs(30)).unwrap();
    let second = kept_catalog(&cache, || {
        reads.fetch_add(1, Ordering::SeqCst);
        codex_only()
    });
    let first = first.join().unwrap();
    assert_eq!(first.len(), 3);
    assert_eq!(second, first);
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn a_kept_catalog_is_read_again_after_ten_minutes() {
    let Some(eleven_minutes_ago) = Instant::now().checked_sub(Duration::from_secs(11 * 60)) else {
        return; // a clock that has not run eleven minutes cannot hold an expired catalog
    };
    let cache = Mutex::new(Some((eleven_minutes_ago, vec![json!({"id": "old"})])));
    let reads = AtomicUsize::new(0);
    let models = kept_catalog(&cache, || {
        reads.fetch_add(1, Ordering::SeqCst);
        codex_only()
    });
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert_eq!(models.len(), 3);
}

// ---- Cursor models from what its CLI listed ----

fn cursor_listed(models: &[(&str, &str)]) -> Value {
    json!({"aliases": [], "efforts": [], "models": models.iter().map(|(id, name)| json!({"id": id, "displayName": name})).collect::<Vec<_>>()})
}

#[test]
fn cursors_per_effort_listings_collapse_into_one_model_with_efforts() {
    let models = cursor_models_from(Some(&cursor_listed(&[
        ("composer-2.5", "Composer 2.5"),
        ("composer-2.5-fast", "Composer 2.5 Fast"),
        ("cursor-grok-4.6-low", "Cursor Grok 4.6 Low"),
        ("cursor-grok-4.6-high", "Cursor Grok 4.6"),
        ("cursor-grok-4.6-high-fast", "Cursor Grok 4.6 Fast"),
    ])));
    assert_eq!(ids(&models), ["composer-2.5", "cursor-grok-4.6-high"]);
    let composer = &models[0];
    assert_eq!(composer["displayName"], "Composer 2.5");
    assert_eq!(composer["supportedEfforts"], json!(["fast"]));
    assert_eq!(composer["variants"], json!({"fast": "composer-2.5-fast"}));
    assert!(composer.get("defaultEffort").is_none());
    let grok = &models[1];
    assert_eq!(grok["displayName"], "Cursor Grok 4.6");
    assert_eq!(grok["defaultEffort"], "high");
    assert_eq!(grok["supportedEfforts"], json!(["low", "high", "high-fast"]));
    assert_eq!(
        grok["variants"],
        json!({"low": "cursor-grok-4.6-low", "high": "cursor-grok-4.6-high", "high-fast": "cursor-grok-4.6-high-fast"})
    );
}

#[test]
fn a_cursor_id_splits_into_model_and_effort_longest_suffix_first() {
    let split = |id: &str| split_cursor_model_id(id);
    let at = |family: &str, effort: &str| (family.to_owned(), Some(effort.to_owned()));
    let whole = |family: &str| (family.to_owned(), None);
    assert_eq!(split("cursor-grok-4.6-high-fast"), at("cursor-grok-4.6", "high-fast"));
    assert_eq!(split("gpt-5.3-codex-xhigh"), at("gpt-5.3-codex", "xhigh"));
    assert_eq!(split("auto"), whole("auto"));
    assert_eq!(split("fast"), whole("fast"));
    assert_eq!(split("claude-opus-5-5-max-fast"), at("claude-opus-5-5", "max-fast"));
    assert_eq!(split("gpt-5.6-sol-none"), at("gpt-5.6-sol", "none"));
    assert_eq!(split("muse-spark-1.3-minimal"), at("muse-spark-1.3", "minimal"));
    assert_eq!(split("gpt-5.5-extra-high-fast"), at("gpt-5.5", "xhigh-fast"));
    assert_eq!(split("claude-opus-5-thinking-high-fast"), at("claude-opus-5-thinking", "high-fast"));
    assert_eq!(split("claude-4.6-opus-max-thinking"), at("claude-4.6-opus-thinking", "max"));
    assert_eq!(split("claude-4.5-sonnet-thinking"), whole("claude-4.5-sonnet-thinking"));
}

#[test]
fn every_cursor_model_is_named_as_cursor_names_it_on_a_real_accounts_list() {
    let models = cursor_models_from(Some(&cursor_listed(&[
        ("claude-opus-5-5-low", "Claude Opus 5.5 1M Low"),
        ("claude-opus-5-5-low-fast", "Claude Opus 5.5 1M Low Fast"),
        ("claude-opus-5-5-medium", "Claude Opus 5.5 1M"),
        ("claude-opus-5-5-medium-fast", "Claude Opus 5.5 1M Fast"),
        ("claude-opus-5-5-xhigh", "Claude Opus 5.5 1M Extra High"),
        ("claude-opus-5-5-max", "Claude Opus 5.5 1M Max"),
        ("claude-opus-5-5-max-fast", "Claude Opus 5.5 1M Max Fast"),
        ("claude-opus-5-thinking-high", "Claude Opus 5 1M Thinking"),
        ("claude-opus-5-thinking-high-fast", "Claude Opus 5 1M Thinking Fast"),
        ("claude-opus-5-thinking-low", "Claude Opus 5 1M Low Thinking"),
        ("claude-opus-5-thinking-low-fast", "Claude Opus 5 1M Low Thinking Fast"),
        ("claude-opus-5-thinking-max", "Claude Opus 5 1M Max Thinking"),
        ("claude-fable-5-1-low", "Claude Fable 5.1 1M Low (NO ZDR)"),
        ("claude-fable-5-1-high", "Claude Fable 5.1 1M (NO ZDR)"),
        ("claude-fable-5-1-max", "Claude Fable 5.1 1M Max (NO ZDR)"),
        ("grok-4.7-low", "Grok 4.7  Low"),
        ("grok-4.7-low-fast", "Grok 4.7  Low Fast\u{200B}\u{200B}"),
        ("grok-4.7-high", "Grok 4.7  High"),
        ("kimi-k3-low", "Kimi K3 Low"),
        ("kimi-k3-high", "Kimi K3 High"),
        ("kimi-k3-max", "Kimi K3"),
    ])));
    let row = |name: &str| models.iter().find(|m| m["displayName"] == name).cloned().unwrap_or(Value::Null);
    assert_eq!(names(&models), ["Claude Opus 5.5 1M", "Claude Opus 5 1M Thinking", "Claude Fable 5.1 1M (NO ZDR)", "Grok 4.7", "Kimi K3"]);
    assert_eq!(row("Claude Opus 5.5 1M")["id"], "claude-opus-5-5-medium");
    assert_eq!(row("Claude Opus 5.5 1M")["defaultEffort"], "medium");
    assert_eq!(row("Claude Opus 5.5 1M")["variants"]["max-fast"], "claude-opus-5-5-max-fast");
    assert_eq!(row("Claude Opus 5 1M Thinking")["defaultEffort"], "high");
    assert_eq!(row("Claude Fable 5.1 1M (NO ZDR)")["defaultEffort"], "high");
    assert_eq!(row("Kimi K3")["id"], "kimi-k3-max");
    assert_eq!(row("Kimi K3")["supportedEfforts"], json!(["low", "high", "max"]));
    assert!(row("Grok 4.7").get("defaultEffort").is_none());
    assert_eq!(row("Grok 4.7")["id"], "grok-4.7-low");
}

#[test]
fn a_cursor_familys_name_is_the_words_its_variants_share_and_never_an_invented_one() {
    let shared = |names: &[&str]| shared_cursor_name(&names.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>());
    assert_eq!(
        shared(&["Claude Opus 5 1M Low Thinking", "Claude Opus 5 1M Thinking", "Claude Opus 5 1M Extra High Thinking"]).as_deref(),
        Some("Claude Opus 5 1M Thinking")
    );
    assert_eq!(shared(&["GPT-5.5 1M None", "GPT-5.5 1M", "GPT-5.5 1M High"]).as_deref(), Some("GPT-5.5 1M"));
    assert_eq!(shared(&["Grok 4.7  Low", "Grok 4.7  High"]).as_deref(), Some("Grok 4.7"));
    assert_eq!(shared(&["Claude Sonnet 4.6 1M"]).as_deref(), Some("Claude Sonnet 4.6 1M"));
    assert_eq!(shared(&["Alpha", "Beta"]), None);
    assert_eq!(shared(&[]), None);
}

#[test]
fn a_cursor_that_listed_nothing_offers_nothing() {
    // Electron's signed-out Cursor offering nothing has no counterpart: Rust's list carries no readiness.
    assert!(cursor_models_from(Some(&json!({"aliases": [], "efforts": []}))).is_empty());
    assert!(cursor_models_from(None).is_empty());
}

// ---- Claude models from what its CLI advertised ----

fn claude_hints(aliases: &[&str], efforts: &[&str]) -> Value {
    json!({"aliases": aliases, "efforts": efforts})
}

#[test]
fn each_advertised_claude_alias_is_offered_with_the_advertised_efforts() {
    let models = claude_models_from(Some(&claude_hints(&["fable", "opus", "sonnet"], &["low", "medium", "high", "xhigh", "max"])));
    assert_eq!(ids(&current(&models)), ["fable", "opus", "sonnet", "haiku"]);
    assert!(models.iter().all(|m| m["runtime"] == "claude"));
    assert_eq!(models[0]["supportedEfforts"], json!(["low", "medium", "high", "xhigh", "max"]));
}

#[test]
fn haiku_the_help_leaves_out_is_offered_once_and_only_beside_advertised_aliases() {
    let named = claude_models_from(Some(&claude_hints(&["sonnet", "haiku"], &[])));
    assert_eq!(ids(&current(&named)), ["sonnet", "haiku"]);
    assert!(claude_models_from(Some(&claude_hints(&[], &["high"]))).is_empty());
    let haiku = current(&claude_models_from(Some(&claude_hints(&["opus"], &[])))).pop().unwrap();
    assert_eq!(haiku["displayName"], "Haiku 5.5");
}

#[test]
fn each_claude_alias_is_named_for_the_version_it_runs() {
    let models = claude_models_from(Some(&claude_hints(&["fable", "opus", "sonnet"], &[])));
    assert_eq!(names(&current(&models)), ["Fable 5.1", "Opus 5.5", "Sonnet 5.5", "Haiku 5.5"]);
    assert_eq!(models[1]["description"], "Always the newest Opus");
    let unknown = claude_models_from(Some(&claude_hints(&["mythic"], &[])));
    assert_eq!(unknown[0]["displayName"], "Mythic");
}

#[test]
fn claudes_older_versions_follow_the_current_ones_marked_for_the_fold() {
    let models = claude_models_from(Some(&claude_hints(&["fable", "opus", "sonnet"], &["low", "high"])));
    let older: Vec<&Value> = models.iter().filter(|m| m["older"] == true).collect();
    assert_eq!(models.iter().position(|m| m["older"] == true), Some(4));
    assert_eq!(
        older.iter().map(|m| m["displayName"].as_str().unwrap()).collect::<Vec<_>>(),
        ["Opus 5", "Opus 4.8", "Opus 4.7", "Opus 4.6", "Opus 4.5", "Fable 5", "Sonnet 5", "Sonnet 4.6", "Sonnet 4.5", "Haiku 4.5"]
    );
    assert_eq!(older.iter().find(|m| m["id"] == "claude-sonnet-4-5").unwrap()["supportedEfforts"], json!([]));
    assert!(claude_models_from(Some(&claude_hints(&[], &["high"]))).is_empty());
}

#[test]
fn the_claude_account_defaults_levels_come_from_the_current_models_only() {
    let models = with_account_defaults(claude_models_from(Some(&claude_hints(&["opus"], &["low", "high"]))));
    let fallback = models.iter().find(|m| m["runtime"] == "claude" && m["id"] == "account-default").unwrap();
    assert_eq!(fallback["supportedEfforts"], json!(["low", "high"]));
}

#[test]
fn a_claude_that_advertised_nothing_offers_nothing() {
    // Electron's not-ready Claude offering nothing has no counterpart: Rust's list carries no readiness.
    assert!(claude_models_from(Some(&json!({}))).is_empty());
    assert!(claude_models_from(None).is_empty());
}

#[test]
fn claude_models_are_listed_even_when_codex_cannot_be_read() {
    let models = catalog(&Listed { codex: None, claude: Some(claude_hints(&["opus"], &["high"])), ..Default::default() });
    let rows: Vec<String> = current(&models).iter().map(|m| format!("{}:{}", m["runtime"].as_str().unwrap(), m["id"].as_str().unwrap())).collect();
    assert_eq!(rows, ["claude:account-default", "claude:opus", "claude:haiku"]);
}

// ---- the account default is a row you can select ----

fn model(runtime: &str, id: &str, efforts: &[&str]) -> Value {
    json!({"id": id, "runtime": runtime, "displayName": id, "description": "", "supportedEfforts": efforts})
}

#[test]
fn the_account_default_offers_only_the_levels_every_listed_model_agrees_on() {
    let row = account_default_model(
        &[
            model("codex", "gpt-6-astra", &["low", "medium", "high", "ultra"]),
            model("codex", "gpt-5.6-sol", &["low", "medium", "high"]),
            model("codex", "gpt-5.4-mini", &["low", "high"]),
        ],
        "codex",
    );
    assert_eq!(row.unwrap()["supportedEfforts"], json!(["low", "high"]));
}

#[test]
fn the_account_default_offers_no_level_when_the_models_share_none() {
    let row = account_default_model(&[model("codex", "a", &["low"]), model("codex", "b", &["high"])], "codex");
    assert_eq!(row.unwrap()["supportedEfforts"], json!([]));
}

#[test]
fn there_is_no_account_default_when_nothing_was_listed() {
    assert_eq!(account_default_model(&[], "codex"), None);
}

#[test]
fn an_earlier_account_default_row_is_not_evidence_for_another() {
    assert_eq!(account_default_model(&[model("codex", "account-default", &["low", "high"])], "codex"), None);
}

#[test]
fn the_account_default_is_named_for_what_it_is_and_sits_under_its_runtime() {
    let row = account_default_model(&[model("codex", "gpt-5.6-sol", &["low"])], "codex").unwrap();
    assert_eq!((&row["id"], &row["runtime"], &row["displayName"]), (&json!("account-default"), &json!("codex"), &json!("Account default")));
}

// ---- every runtime that lists models gets an Account default row ----

#[test]
fn each_runtime_gets_its_own_account_default_row() {
    let rows = with_account_defaults(vec![
        model("codex", "gpt-5.6-sol", &["low", "high"]),
        model("opencode", "big-pickle", &[]),
        model("claude", "sonnet", &["low", "high"]),
    ]);
    let mut runtimes: Vec<&str> = rows.iter().filter(|m| m["id"] == "account-default").map(|m| m["runtime"].as_str().unwrap()).collect();
    runtimes.sort();
    assert_eq!(runtimes, ["claude", "codex", "opencode"]);
}

#[test]
fn each_runtimes_account_default_carries_its_own_shared_levels() {
    let rows = with_account_defaults(vec![
        model("codex", "a", &["low", "medium", "high"]),
        model("codex", "b", &["low", "high"]),
        model("claude", "c", &["xhigh"]),
    ]);
    let levels = |runtime: &str| rows.iter().find(|m| m["id"] == "account-default" && m["runtime"] == runtime).unwrap()["supportedEfforts"].clone();
    assert_eq!(levels("codex"), json!(["low", "high"]));
    assert_eq!(levels("claude"), json!(["xhigh"]));
}

#[test]
fn a_runtime_that_listed_nothing_gets_no_account_default() {
    assert!(with_account_defaults(vec![]).is_empty());
}

#[test]
fn a_list_that_has_its_account_default_is_not_given_a_second() {
    let once = with_account_defaults(vec![model("codex", "a", &["low"])]);
    assert_eq!(with_account_defaults(once).iter().filter(|m| m["id"] == "account-default").count(), 1);
}
