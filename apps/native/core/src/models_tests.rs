//! The model list against Electron's own catalog: `parity/models.json` is written by `scripts/model-goldens.mjs` from
//! the real list output of each agent on the Mac mini (`parity/models/`).
use super::*;

fn file(name: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../parity").join(name)).unwrap()
}

fn goldens() -> Value {
    serde_json::from_str(&file("models.json")).unwrap()
}

fn none_as_null(v: Option<Value>) -> Value {
    v.unwrap_or(Value::Null)
}

#[test]
fn each_agents_list_is_read_as_electron_reads_it() {
    let g = goldens();
    assert_eq!(none_as_null(parse_claude_model_hints(&file("models/claude.txt"))), g["hints"]["claude"], "Claude Code");
    assert_eq!(none_as_null(parse_cursor_model_list(&file("models/cursor.txt"))), g["hints"]["cursor"], "Cursor");
    assert_eq!(none_as_null(parse_opencode_model_list(&file("models/opencode.txt"))), g["hints"]["opencode"], "OpenCode");
    assert_eq!(none_as_null(parse_agy_model_list(&file("models/agy.txt"))), g["hints"]["antigravity"], "Antigravity");
}

#[test]
fn the_catalog_matches_electrons_model_for_model() {
    let g = goldens();
    let codex: Value = serde_json::from_str(&file("models/codex-model-list.json")).unwrap();
    for machine in g["machines"].as_array().unwrap() {
        let listed = Listed {
            codex: (machine["codex"] == true).then(|| codex.clone()),
            codex_usage: None,
            claude: Some(g["hints"]["claude"].clone()).filter(|v| !v.is_null()),
            cursor: Some(g["hints"]["cursor"].clone()).filter(|v| !v.is_null()),
            opencode: Some(g["hints"]["opencode"].clone()).filter(|v| !v.is_null()),
            opencode_acp: Vec::new(),
            antigravity: Some(g["hints"]["antigravity"].clone()).filter(|v| !v.is_null()),
            copilot: true,
            muse: machine["muse"] == true,
            apple: false,
            antigravity_usage: None,
        };
        let ours = catalog(&listed);
        let want = machine["models"].as_array().unwrap();
        assert_eq!(ours.len(), want.len(), "{}: how many", machine["name"]);
        for (a, b) in ours.iter().zip(want) {
            assert_eq!(a, b, "{}: {}", machine["name"], b["id"]);
        }
    }
}

#[test]
fn cursor_ids_split_into_a_model_and_its_effort() {
    assert_eq!(split_cursor_model_id("cursor-grok-4.6-high-fast"), ("cursor-grok-4.6".into(), Some("high-fast".into())));
    assert_eq!(split_cursor_model_id("claude-4.6-opus-high-thinking"), ("claude-4.6-opus-thinking".into(), Some("high".into())));
    assert_eq!(split_cursor_model_id("gpt-5.5-extra-high"), ("gpt-5.5".into(), Some("xhigh".into())));
    assert_eq!(split_cursor_model_id("fast"), ("fast".into(), None));
    assert_eq!(claude_model_name("claude-haiku-4-5-20251001").as_deref(), Some("Haiku 4.5"));
}


#[test]
fn a_cursor_level_becomes_the_variant_id_it_names() {
    let models = vec![json!({"id": "gpt-5.5-medium", "runtime": "cursor", "variants": {"low": "gpt-5.5-low", "medium": "gpt-5.5-medium", "high": "gpt-5.5-high"}})];
    assert_eq!(cursor_choice_for(&models, Some("gpt-5.5-medium"), Some("high")), (Some("gpt-5.5-high".into()), None));
    assert_eq!(cursor_choice_for(&models, Some("gpt-5.5-low"), Some("max")), (Some("gpt-5.5-low".into()), Some("max".into())));
    assert_eq!(cursor_choice_for(&models, Some("gpt-5.5-low"), None), (Some("gpt-5.5-low".into()), None));
}

// --- older-claude-versions-are-folded.test.tsx: what the fold holds (exec/native-picker)

#[test]
fn what_the_fold_holds_is_claude_codes_own_registry_newest_first_with_no_retired_generation() {
    let ids: Vec<&str> = claude_older_models().into_iter().map(|(id, _)| id).collect();
    assert_eq!(
        ids,
        [
            "claude-opus-5",
            "claude-opus-4-8",
            "claude-opus-4-7",
            "claude-opus-4-6",
            "claude-opus-4-5",
            "claude-fable-5",
            "claude-sonnet-5",
            "claude-sonnet-4-6",
            "claude-sonnet-4-5",
            "claude-haiku-4-5-20251001",
        ]
    );
    assert!(!ids.iter().any(|id| id.contains("claude-3") || id.contains("mythos")));
}

#[test]
fn what_the_fold_holds_offers_each_version_only_the_effort_levels_its_registry_entry_lists() {
    let levels = |want: &str| claude_older_models().into_iter().find(|(id, _)| *id == want).map(|(_, e)| e.join(",")).unwrap();
    assert_eq!(levels("claude-opus-4-8"), "low,medium,high,xhigh,max");
    assert_eq!(levels("claude-opus-4-6"), "low,medium,high,max");
    assert_eq!(levels("claude-sonnet-4-5"), "");
}

/// claudeModelsFrom's rule for the fold: a version an alias already means now is not offered again under it.
#[test]
fn the_fold_skips_the_version_an_alias_means_now() {
    let ids: Vec<String> = claude_older_offered(&["Opus 5".to_owned()]).into_iter().map(|(id, _, _)| id).collect();
    assert!(!ids.contains(&"claude-opus-5".to_owned()), "{ids:?}");
    assert_eq!(ids.len(), 9);
}

/// a-model-is-one-line's second case names its row from Claude Code's alias table: `opus` reads Opus 5.5.
#[test]
fn the_opus_alias_reads_opus_5_5() {
    let models = claude_models_from(Some(&json!({"aliases": ["opus"], "efforts": []})));
    assert_eq!(models.iter().find(|m| m["id"] == "opus").map(|m| m["displayName"].clone()), Some(json!("Opus 5.5")));
}
