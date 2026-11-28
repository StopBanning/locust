use super::*;
use serde_json::json;
use std::path::PathBuf;

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn compare_drafted_points_do_not_overwrite_typed_criteria() {
    let dir = Dirs(std::env::temp_dir().join(format!(
        "locust-compare-draft-keep-{}",
        locust_core::invariants::unique_nanos()
    )));
    std::fs::create_dir_all(dir.0.join("electron/mission-ledger")).unwrap();
    let store = crate::store::Store::new(dir.0.join("native"), dir.0.join("electron")).unwrap();
    let ledger_dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger_dir).unwrap();

    let cmp_id = "cmp_keep_typed".to_owned();
    let draft_id = "draft_mission_keep".to_owned();
    let rows = [
        json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":draft_id,"runtime":"opencode","model":"opencode/judge-free","prompt":"Draft","createdAt":"2026-10-21T12:00:00Z"}}),
        json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"occurredAt":"2026-10-21T12:00:01Z","type":"message.delta",
            "payload":{"itemId":"draft","text":"POINT 1: AI drafted point.","operation":"replace","final":true}}}),
        json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":2,"occurredAt":"2026-10-21T12:00:02Z","type":"run.completed","payload":{}}}),
    ];
    std::fs::write(
        ledger_dir.join(format!("{draft_id}.jsonl")),
        rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();

    let mut s = State::default();
    s.data = Some(store);
    s.compare.loaded = true;
    s.compare.drafting = true;
    s.compare.draft_mission_id = Some(draft_id);
    s.compare.draft_compare_id = Some(cmp_id.clone());
    s.compare
        .criteria
        .entry(cmp_id.clone())
        .or_default()
        .replace_text("My own hand-typed criteria");

    assert!(poll(&mut s));
    assert!(!s.compare.drafting);
    assert_eq!(
        s.compare.criteria.get(&cmp_id).map(|e| e.text.as_str()),
        Some("My own hand-typed criteria"),
        "arriving drafted points must not overwrite criteria the person already typed"
    );
}
