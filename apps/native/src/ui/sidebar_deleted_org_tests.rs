use super::*;
use crate::{org, store::Store};
use serde_json::{Value, json};

struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
// case: lets a deleted conversation go: its pin, its put-away mark, owned or not
fn lets_a_deleted_conversation_go_its_pin_its_put_away_mark_owned_or_not() {
    let path = Fixture(std::env::temp_dir().join(format!(
        "locust-sidebar-delete-org-{}-{}",
        std::process::id(),
        locust_core::invariants::unique_nanos()
    )));
    std::fs::create_dir_all(path.0.join("electron")).unwrap();
    let store = Store::new(path.0.join("native"), path.0.join("electron")).unwrap();
    let ids = ["m_owned_delete_org", "m_unowned_delete_org"];
    let pins: Vec<_> = ids
        .into_iter()
        .map(str::to_owned)
        .chain((0..48).map(|at| format!("m_other_{at}")))
        .collect();
    store.save_json("teammates.json", &json!({
        "schemaVersion":1, "teammates":[], "missionOwners": {"m_owned_delete_org":"tm_ash"},
        "missionPins": pins, "missionSettled": {
            "m_owned_delete_org":{"at":"2026-10-11T13:00:00Z"},
            "m_unowned_delete_org":{"at":"2026-10-11T13:00:00Z","until":"2026-10-12T13:00:00Z"},
            "m_other_0":{"at":"2026-10-11T13:00:00Z"}
        }
    })).unwrap();
    let dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    for id in ids {
        std::fs::write(dir.join(format!("{id}.jsonl")), format!(
            "{}\n", json!({"recordType":"mission.created","metadata":{"missionId":id,"prompt":"keep in Trash"}})
        )).unwrap();
    }
    let mut state = State::default();
    state.data = Some(store.clone());
    state.org = org::load_org(&store);
    assert!(org::pin_mission(&store, &mut state.org, "m_new_at_cap", true).is_err());
    delete_conversation(&mut state, &store, &ids.map(str::to_owned)).unwrap();

    let from_disk = org::load_org(&store);
    for id in ids {
        assert!(!dir.join(format!("{id}.jsonl")).exists());
        assert!(dir.join(".trash").join(format!("{id}.jsonl")).is_file());
        assert!(!state.org.pins.contains(&id.to_owned()));
        assert!(!state.org.settled.contains_key(id));
        assert!(!from_disk.pins.contains(&id.to_owned()));
        assert!(!from_disk.settled.contains_key(id));
    }
    assert_eq!(from_disk.pins.len(), 48);
    assert!(from_disk.settled.contains_key("m_other_0"));
    assert!(org::pin_mission(&store, &mut state.org, "m_new_at_cap", true).is_ok());
    let disk: Value =
        serde_json::from_slice(&std::fs::read(store.teammates_json_path().unwrap()).unwrap())
            .unwrap();
    assert_eq!(
        disk["missionOwners"]["m_owned_delete_org"], "tm_ash",
        "restoring the record keeps its owner"
    );
}
