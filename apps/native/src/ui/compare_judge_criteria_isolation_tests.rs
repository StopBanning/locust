use super::*;
use locust_core::{approval::{Decision, Request}, approval_rules::RuleEffect, engine_api::EngineApi};
use std::path::PathBuf;

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct FakeEngine(Mutex<Vec<(native::Send, Origin)>>);
impl EngineApi for FakeEngine {
    fn send(&self, job: native::Send, origin: Origin) -> Result<locust_core::engine::Started, String> {
        let mut calls = self.0.lock().unwrap();
        calls.push((job, origin));
        Ok(locust_core::engine::Started {
            conversation_id: format!("fixture_iso_judge_{}", calls.len()),
            mode: "ask".into(),
        })
    }
    fn stop(&self, _: &str) -> Result<String, String> { Err("no live run".into()) }
    fn owns(&self, _: &str) -> bool { false }
    fn running(&self) -> Vec<String> { vec![] }
    fn waiting(&self) -> Vec<Request> { vec![] }
    fn decide(&self, _: &str, _: Decision, _: Option<String>) -> Result<(), String> { Err("no approval".into()) }
    fn answer(&self, _: &str, _: Vec<(String, Vec<String>)>) -> Result<(), String> { Err("no question".into()) }
    fn save_rule_and_decide(&self, _: &Request, _: RuleEffect) -> Result<(), String> { Err("no rule".into()) }
    fn carry(&self, _: &str, _: &str, _: Option<&str>) -> Result<(String, Option<String>, String), String> { Err("no carry".into()) }
    fn shut_down(&self) {}
    fn wait_for_shutdown(&self) {}
    fn room_notices(&self) -> Vec<locust_core::room_tasks::RoomNotice> { vec![] }
}

fn seed_compare(store: &locust_core::compare::CompareStore, s: &mut State, prompt: &str) -> String {
    let cmp = store
        .create(CreateCompareInput {
            teammate_id: None,
            prompt: prompt.into(),
            routes: vec![
                CompareRoute::new("opencode", "opencode/m1"),
                CompareRoute::new("opencode", "opencode/m2"),
            ],
            changes: false,
            changes_in: None,
            blind: true,
            base: None,
            rerun_of: None,
        })
        .unwrap();
    for (slot, suffix) in [(CompareSlotId::A, "a"), (CompareSlotId::B, "b")] {
        let mid = format!("{}_{suffix}", cmp.compare_id);
        store.add_turn(&cmp.compare_id, slot, &mid).unwrap();
        s.compare.snapshots.insert(
            mid.clone(),
            ledger::Snapshot {
                tip: mid,
                items: vec![ledger::Item::Assistant {
                    item_id: "answer".into(),
                    text: "Done.".into(),
                    r#final: true,
                }],
                ..Default::default()
            },
        );
    }
    s.compare.compares.push(store.get(&cmp.compare_id).unwrap().unwrap());
    cmp.compare_id
}

#[test]
fn compare_ask_judge_does_not_inherit_another_comparisons_criteria() {
    const CHILD: &str = "LOCUST_COMPARE_ASK_ISO_FIXTURE";
    if std::env::var_os(CHILD).is_none() {
        let test = concat!(module_path!(), "::compare_ask_judge_does_not_inherit_another_comparisons_criteria");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test.split_once("::").unwrap().1, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let engine = Arc::new(FakeEngine::default());
    locust_core::engine_api::install(engine.clone()).unwrap();

    let dir = Dirs(std::env::temp_dir().join(format!(
        "locust-compare-ask-iso-{}",
        locust_core::invariants::unique_nanos()
    )));
    std::fs::create_dir_all(dir.0.join("electron/mission-ledger")).unwrap();
    let store = crate::store::Store::new(dir.0.join("native"), dir.0.join("electron")).unwrap();
    let compare_store = locust_core::compare::CompareStore::from_store(&store).unwrap();

    let mut s = State::default();
    s.data = Some(store);
    s.models.insert(
        "opencode".into(),
        vec![locust_core::engine::Models {
            teammate: None,
            runtime: "opencode".into(),
            model: None,
            effort: None,
            models: vec![locust_core::engine::Model {
                id: "opencode/judge-free".into(),
                name: "Judge Free".into(),
                efforts: vec![],
            }],
        }],
    );

    let first = seed_compare(&compare_store, &mut s, "First comparison");
    let second = seed_compare(&compare_store, &mut s, "Second comparison");
    s.compare
        .criteria
        .entry(first)
        .or_default()
        .replace_text("SECRET_FIRST_COMPARISON_POINT");
    s.compare.active_id = Some(second.clone());

    ask_judge(&mut s);

    let captured = engine.0.lock().unwrap();
    assert_eq!(captured.len(), 1, "one judge request was sent for the second comparison");
    assert!(
        !captured[0].0.prompt.contains("SECRET_FIRST_COMPARISON_POINT"),
        "asking a judge on a comparison with no criteria must not send another comparison's points"
    );
    let saved = compare_store.get(&second).unwrap().unwrap();
    assert_eq!(
        saved.judge.expect("judge recorded").criteria,
        None,
        "the second comparison's judge record must have no criteria"
    );
}
