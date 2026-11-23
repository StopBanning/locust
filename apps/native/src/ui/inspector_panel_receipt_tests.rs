use super::*;
use crate::tests::TempDir;
use rune::{Env, Runtime};
use serde_json::json;

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}

#[test]
fn receipt_names_the_machine_and_the_selected_turns_recorded_folder() {
    let ledger_dir = TempDir::new();
    let at = "2026-11-23T10:00:00.000Z";
    for (id, parent, cwd) in [
        ("m_origin", None, None),
        ("m_review", Some("m_origin"), Some("/workspace/reviewed/")),
    ] {
        let mut created = json!({
            "schemaVersion": 24,
            "recordType": "mission.created",
            "occurredAt": at,
            "metadata": {
                "missionId": id, "runId": format!("run_{id}"), "prompt": "Review",
                "runtime": "codex", "model": "synthetic-model", "createdAt": at
            }
        });
        if let Some(parent) = parent {
            created["metadata"]["continuesFrom"] =
                json!({"missionId": parent, "reason": "follow-up"});
        }
        if let Some(cwd) = cwd {
            created["metadata"]["command"] = json!({"cwd": cwd});
        }
        ledger_dir.write(
            id,
            &[
                created,
                json!({
                    "schemaVersion": 24,
                    "recordType": "mission.event",
                    "occurredAt": at,
                    "event": {
                        "id": format!("end_{id}"), "sequence": 1, "type": "run.completed",
                        "sourceAdapter": "codex", "occurredAt": at, "payload": {}
                    }
                }),
            ],
        );
    }
    let mut worker = ledger::Session::new(ledger_dir.0.clone(), "m_origin".into());
    let snapshot = worker.poll().unwrap();
    let mut state = State::default();
    team_scene(&mut state);
    let mut header = state.teammates[0].clone();
    header.folder = Some("/workspace/todays-folder".into());
    state.inspector_open = true;
    state.ledger = Some(LedgerPane {
        header,
        conversation: Conversation {
            conversation_id: Some("m_origin".into()),
            turns: vec![],
        },
        items: snapshot.items.clone(),
        dir: ledger_dir.0.clone(),
        worker: Some(worker),
        snapshot,
        cold_tip: None,
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    let mut rt = Runtime::new(
        state,
        |s, _| panel(s, &Palette::new(true)).expect("open inspector"),
        (500.0, 600.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    click(&mut rt, "inspector-tab-receipt");
    click(&mut rt, "inspector-turn-1");

    let platform = match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    };
    let elements = rt.elements();
    let expected = format!("{platform} · reviewed");
    assert!(elements.iter().any(|e| e.text.as_deref() == Some("Ran on")));
    assert!(
        elements
            .iter()
            .any(|e| e.text.as_deref() == Some(expected.as_str()))
    );

    click(&mut rt, "inspector-turn-0");
    let elements = rt.elements();
    assert!(elements.iter().any(|e| e.text.as_deref() == Some(platform)));
    assert!(!elements.iter().any(|e| {
        e.text
            .as_deref()
            .is_some_and(|said| said.contains("todays-folder") || said == expected)
    }));
}
