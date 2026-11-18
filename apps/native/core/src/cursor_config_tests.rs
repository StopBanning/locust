//! Electron's `main/a-cursor-run-leaves-cursor-as-it-was.test.ts`, one Rust test per case, with its values. Every test
//! uses a temp folder, never the real `~/.cursor`.
use super::*;
use serde_json::json;
use std::path::Path;

fn grok() -> Value {
    json!({
        "model": {"modelId": "grok-4.7", "displayName": "Grok 4.7 256K Medium", "maxMode": false},
        "selectedModel": {"modelId": "grok-4.7", "parameters": [{"id": "context", "value": "256k"}, {"id": "reasoning_effort", "value": "medium"}]},
        "modelSelectionHistory": ["grok-4.7", "claude-opus-5"]
    })
}
fn composer() -> Value {
    json!({
        "model": {"modelId": "composer-2.5", "displayName": "Composer 2.5", "maxMode": false},
        "selectedModel": {"modelId": "composer-2.5", "parameters": [{"id": "fast", "value": "false"}]},
        "modelSelectionHistory": ["composer-2.5", "grok-4.7", "claude-opus-5"]
    })
}
fn sign_in() -> Value {
    json!({"authInfo": {"accessToken": "not-a-real-token-7f3a", "email": "person@example.com"}, "permissions": {"allow": ["Shell(ls)"]}})
}
fn merged(parts: &[Value], version: Option<i64>) -> Value {
    let mut m = Map::new();
    for p in parts {
        for (k, v) in p.as_object().unwrap() {
            m.insert(k.clone(), v.clone());
        }
    }
    if let Some(v) = version {
        m.insert("version".into(), json!(v));
    }
    Value::Object(m)
}

struct Fixture {
    dir: PathBuf,
    file: PathBuf,
    kept: PathBuf,
}
impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("locust-cursor-default-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self { file: dir.join("cli-config.json"), kept: dir.join("cursor-default-model.json"), dir }
    }
    fn write(&self, v: &Value) {
        fs::write(&self.file, serde_json::to_string_pretty(v).unwrap()).unwrap();
    }
    fn config(&self) -> Value {
        serde_json::from_str(&fs::read_to_string(&self.file).unwrap()).unwrap()
    }
    fn guard(&self) -> CursorDefaultModel {
        CursorDefaultModel::new(self.file.clone(), self.kept.clone())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(Path::new(&self.dir));
    }
}

#[test]
fn is_put_back_when_the_run_ends_and_everything_else_in_the_file_comes_back_exactly_the_sign_in_included() {
    let f = Fixture::new("putback");
    f.write(&merged(&[sign_in(), grok()], Some(1)));
    let g = f.guard();
    g.before("composer-2.5");
    f.write(&merged(&[sign_in(), composer()], Some(1)));
    g.after();
    assert_eq!(f.config(), merged(&[sign_in(), grok()], Some(1)));
    assert!(fs::read_to_string(&f.file).unwrap().starts_with("{\n  \"authInfo\""));
    assert!(fs::read_to_string(&f.kept).is_err());
}

#[test]
fn never_keeps_the_sign_in_anywhere_of_its_own_even_mid_run() {
    let f = Fixture::new("signin");
    f.write(&merged(&[sign_in(), grok()], None));
    let g = f.guard();
    g.before("composer-2.5");
    let kept = fs::read_to_string(&f.kept).unwrap();
    assert!(!kept.contains("not-a-real-token"));
    assert!(!kept.contains("person@example.com"));
    assert!(kept.contains("grok-4.7"));
}

#[test]
fn leaves_it_alone_when_the_person_chose_another_model_themselves_during_the_run() {
    let f = Fixture::new("theirs");
    f.write(&merged(&[sign_in(), grok()], None));
    let g = f.guard();
    g.before("composer-2.5");
    let theirs = json!({"model": {"modelId": "gpt-6-sol"}, "selectedModel": {"modelId": "gpt-6-sol"}, "modelSelectionHistory": ["gpt-6-sol"]});
    f.write(&merged(&[sign_in(), theirs.clone()], None));
    g.after();
    assert_eq!(f.config(), merged(&[sign_in(), theirs], None));
}

#[test]
fn with_runs_overlapping_puts_back_the_state_before_the_first_when_the_last_ends() {
    let f = Fixture::new("overlap");
    f.write(&merged(&[sign_in(), grok()], None));
    let g = f.guard();
    g.before("composer-2.5");
    f.write(&merged(&[sign_in(), composer()], None));
    g.before("composer-2.5[fast=true]");
    g.after();
    assert_eq!(f.config()["model"], composer()["model"]);
    g.after();
    assert_eq!(f.config(), merged(&[sign_in(), grok()], None));
}

#[test]
fn is_put_back_on_the_next_start_when_locust_quit_mid_run() {
    let f = Fixture::new("recover");
    f.write(&merged(&[sign_in(), grok()], None));
    f.guard().before("composer-2.5");
    f.write(&merged(&[sign_in(), composer()], None));
    f.guard().recover();
    assert_eq!(f.config(), merged(&[sign_in(), grok()], None));
    assert!(fs::read_to_string(&f.kept).is_err());
}

#[test]
fn does_nothing_when_there_is_no_cursor_config_or_the_run_changed_nothing() {
    let f = Fixture::new("nothing");
    let g = f.guard();
    g.before("composer-2.5");
    g.after();
    assert!(fs::read_to_string(&f.file).is_err());
    f.write(&merged(&[sign_in(), grok()], None));
    let text = fs::read_to_string(&f.file).unwrap();
    g.before("grok-4.7");
    g.after();
    assert_eq!(fs::read_to_string(&f.file).unwrap(), text);
}

#[test]
fn is_put_back_when_the_run_named_a_list_id_cursor_saves_as_its_base_id_composer_2_5_fast_is_composer_2_5() {
    let f = Fixture::new("base");
    f.write(&merged(&[sign_in(), grok()], None));
    let g = f.guard();
    g.before("composer-2.5-fast");
    let mut c = merged(&[sign_in(), composer()], None);
    c["selectedModel"] = json!({"modelId": "composer-2.5", "parameters": [{"id": "fast", "value": "true"}]});
    f.write(&c);
    g.after();
    assert_eq!(f.config(), merged(&[sign_in(), grok()], None));
}

#[test]
fn matches_a_model_to_the_id_it_is_saved_under_and_nothing_else() {
    assert!(named_model("composer-2.5-fast", Some("composer-2.5")));
    assert!(named_model("grok-4.7-256k-medium", Some("grok-4.7")));
    assert!(named_model("composer-2.5[fast=true]", Some("composer-2.5")));
    assert!(named_model("composer-2.5", Some("composer-2.5")));
    assert!(!named_model("composer-2.5", Some("gpt-6-sol")));
    assert!(!named_model("grok-4.7", Some("grok-4.6")));
    assert!(named_model("gpt-6", Some("gpt-6-sol")));
    assert!(!named_model("composer-2.5", None));
}

#[test]
fn reads_a_model_with_bracketed_options_as_its_plain_id() {
    assert_eq!(base_model_id("composer-2.5[fast=true]"), "composer-2.5");
    assert_eq!(base_model_id("grok-4.7"), "grok-4.7");
}

/// codex-mission.ts adds Cursor's connector rules only for `runtime === 'cursor' && effectiveSandbox === 'workspace-write'`.
#[test]
fn adds_connector_rules_only_for_a_cursor_edit_run() {
    use crate::approval::Mode;
    assert!(adds_connector_rules("cursor", Mode::AcceptEdits));
    assert!(adds_connector_rules("cursor", Mode::ApproveEach));
    assert!(!adds_connector_rules("cursor", Mode::Auto));
    assert!(!adds_connector_rules("cursor", Mode::Ask));
    assert!(!adds_connector_rules("cursor", Mode::Plan));
    assert!(!adds_connector_rules("codex", Mode::AcceptEdits));
}
