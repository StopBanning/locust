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
            claude: Some(g["hints"]["claude"].clone()).filter(|v| !v.is_null()),
            cursor: Some(g["hints"]["cursor"].clone()).filter(|v| !v.is_null()),
            opencode: Some(g["hints"]["opencode"].clone()).filter(|v| !v.is_null()),
            antigravity: Some(g["hints"]["antigravity"].clone()).filter(|v| !v.is_null()),
            copilot: true,
            muse: machine["muse"] == true,
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
