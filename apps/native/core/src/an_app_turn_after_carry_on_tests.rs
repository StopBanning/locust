//! Electron's `main/an-app-turn-after-a-carry-on-stays-in-its-folder.test.ts` (0.882):
//! a carried turn written to a real ledger file keeps its `startedBy.folder` when read back, and an app's next
//! `send_message` runs in that folder rather than the folder Locust has open.
use super::tests::temp;
use super::*;
use crate::door_tools::{conversation_folder, Turn};
use crate::ledger::Tail;
use crate::ledger_write::LedgerWriter;
use std::collections::HashMap;

fn carried_turn(tag: &str, started_by: Value, completed: bool) -> Turn {
    let root = temp(tag);
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let mut writer = LedgerWriter::create_runtime_with(
        &store,
        "mission_carried",
        "Carry on the brief.",
        "opus",
        "claude",
        "claude",
        &[],
        None,
        &json!({"mode": "auto", "sandbox": "workspace-write", "startedBy": started_by}),
    )
    .unwrap();
    if completed {
        writer.append("run.completed", json!({}), None).unwrap();
    }
    let path = writer.path.clone();
    drop(writer);
    let records = Tail::new(&path, 0).poll();
    let turn = Turn::from_records(&records).expect("carried turn read back from ledger");
    let _ = std::fs::remove_dir_all(&root);
    turn
}

struct Fake {
    turns: Mutex<HashMap<String, Turn>>,
    started: Mutex<Vec<StartRequest>>,
}

impl Locust for Fake {
    fn teammates(&self) -> Result<Vec<(Teammate, Option<String>)>, String> {
        Ok(vec![])
    }
    fn busy(&self, _: &str) -> bool {
        false
    }
    fn start(&self, request: StartRequest) -> Result<Begun, String> {
        self.started.lock().unwrap().push(request);
        Ok(Begun {
            mission_id: "mission_next".into(),
            mode: "auto".into(),
            runtime: Some("claude".into()),
            model: Some("opus".into()),
        })
    }
    fn read(&self, mission: &str) -> Option<Turn> {
        self.turns.lock().unwrap().get(mission).cloned()
    }
    fn newest(&self, mission: &str) -> Option<String> {
        Some(mission.to_owned())
    }
    fn owner(&self, _: &str) -> Option<String> {
        None
    }
    fn live(&self, _: &str) -> bool {
        false
    }
    fn waiting(&self, _: &str) -> bool {
        false
    }
    fn cancel(&self, _: &str) -> bool {
        false
    }
    fn conversations(&self) -> Vec<Turn> {
        vec![]
    }
    fn check_folder(&self, path: &str) -> Result<PathBuf, String> {
        Ok(PathBuf::from(path))
    }
    fn folder(&self) -> Option<String> {
        Some("/Users/someone/Documents/Locust".into())
    }
    fn own_mode(&self) -> bool {
        true
    }
    fn see_all(&self) -> bool {
        true
    }
    fn models(&self) -> Vec<Value> {
        vec![]
    }
    fn carry_on(&self, _: &str, _: &str, _: Option<&str>, _: &crate::carry_on::CarryOnAlso) -> Result<(String, Option<String>), String> {
        Err("not here".into())
    }
}

// case: reads the folder a carried turn recorded, as the window reads the turn
#[test]
fn reads_the_folder_a_carried_turn_recorded_as_the_window_reads_the_turn() {
    let with_folder = carried_turn(
        "carry-folder-with",
        json!({"kind": "carry-on", "from": "codex", "folder": "/work/lane"}),
        false,
    );
    assert_eq!(with_folder.started_by, json!({"kind": "carry-on", "from": "codex", "folder": "/work/lane"}));
    assert_eq!(conversation_folder(&with_folder.started_by).as_deref(), Some("/work/lane"));

    // A carry-on with no folder named stays without one.
    let without_folder = carried_turn("carry-folder-none", json!({"kind": "carry-on", "from": "codex"}), false);
    assert_eq!(without_folder.started_by, json!({"kind": "carry-on", "from": "codex"}));
    assert_eq!(conversation_folder(&without_folder.started_by), None);
}

// case: sends the next message to the same folder, not the one Locust has open
#[test]
fn sends_the_next_message_to_the_same_folder_not_the_one_locust_has_open() {
    let turn = carried_turn(
        "carry-folder-next",
        json!({"kind": "carry-on", "from": "codex", "folder": "/work/lane"}),
        true,
    );
    assert_eq!(turn.phase(), "completed");
    let fake = Fake {
        turns: Mutex::new(HashMap::from([("mission_carried".to_owned(), turn)])),
        started: Mutex::new(Vec::new()),
    };
    let tools = Tools::new(fake);
    let reply = tools.call("send_message", &json!({"conversation_id": "mission_carried", "message": "Keep going."}));
    assert!(!reply.is_error, "{}", reply.text);
    let started = tools.locust.started.lock().unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].folder, Some(PathBuf::from("/work/lane")));
    let (_, origin, _) = job_of(started[0].clone(), None);
    assert_eq!(origin.folder, Some(PathBuf::from("/work/lane")));
    assert_eq!(origin.started_by, Some(json!({"kind": "mcp", "folder": "/work/lane"})));
}
