//! Electron `main/cold-start-is-not-an-alert.test.ts`, the host's half: which turn started cold, as the window reads it
//! from the turn's first record (App.tsx `coldStart`, from `followsUp.runtimeThreadId`). The note's place is drawn in
//! `src/ui/cold_start_note_tests.rs`.
use super::*;
use crate::native::{Send, hooks, send_from, started_cold, turn_started_cold};

fn follow_up(link: Value) -> Value {
    json!({"missionId": "mission_2", "runtime": "codex", "continuesFrom": link})
}

#[test]
fn a_reply_with_no_session_to_resume_started_cold_and_one_that_resumed_did_not() {
    assert!(started_cold(&follow_up(json!({"missionId": "mission_1", "checkpointEpoch": 1, "reason": "follow-up"}))));
    assert!(started_cold(&follow_up(json!({"missionId": "mission_1", "reason": "follow-up", "runtimeThreadId": ""}))));
    assert!(!started_cold(&follow_up(json!({"missionId": "mission_1", "reason": "follow-up", "runtimeThreadId": "thread_1"}))));
    // A first turn follows nothing.
    assert!(!started_cold(&json!({"missionId": "mission_1", "runtime": "codex"})));
    assert!(!started_cold(&follow_up(json!({"missionId": "", "reason": "follow-up"}))));
}

#[test]
fn an_edit_a_switch_and_a_turn_locust_or_a_teammate_started_are_not_the_persons_cold_reply() {
    // An edit says "Started again from an edited message"; a switch draws its divider.
    assert!(!started_cold(&follow_up(json!({"missionId": "mission_1", "reason": "follow-up", "edited": true}))));
    assert!(!started_cold(&follow_up(json!({"missionId": "mission_1", "reason": "route-switch"}))));
    assert!(!started_cold(&follow_up(json!({"missionId": "mission_1", "reason": "carry-on"}))));
    for by in ["carry-on", "check-fix", "side", "compare", "relay"] {
        let mut meta = follow_up(json!({"missionId": "mission_1", "reason": "follow-up"}));
        meta["startedBy"] = json!({"kind": by});
        assert!(!started_cold(&meta), "{by}");
    }
    let mut hop = follow_up(json!({"missionId": "mission_1", "reason": "follow-up"}));
    hop["relay"] = json!({"from": "tm_wren", "hop": 1});
    assert!(!started_cold(&hop));
}

struct Temp(std::path::PathBuf);
impl Temp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-cold-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(p.join("electron/mission-ledger")).unwrap();
        std::fs::create_dir_all(p.join("work")).unwrap();
        Self(p)
    }
    fn store(&self) -> crate::store::Store {
        crate::store::Store::new(self.0.join("native"), self.0.join("electron")).unwrap()
    }
    fn missions(&self) -> Vec<String> {
        let mut ids: Vec<String> = std::fs::read_dir(self.0.join("native/mission-ledger"))
            .map(|d| d.filter_map(|e| e.ok()?.path().file_stem()?.to_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        ids.sort();
        ids
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn send(t: &Temp, conversation: Option<String>, mode: crate::approval::Mode) -> Result<crate::engine::Started, String> {
    send_from(
        Send {
            teammate: crate::engine::Teammate {
                id: "tm_codec".into(),
                name: "Codec".into(),
                role: "Code & Migrations".into(),
                runtime: Some("codex".into()),
                model: Some("synthetic-model".into()),
                account: None,
                busy: false,
                runs_in: "ask".into(),
                folder: Some(t.0.join("work").to_string_lossy().into()),
            },
            conversation,
            prompt: "Say hello.".into(),
            model: Some("synthetic-model".into()),
            effort: None,
        },
        crate::native::Origin { mode: Some(mode), ..Default::default() },
    )
}

#[test]
fn a_real_reply_whose_mode_changed_starts_cold_and_its_first_record_says_so() {
    let t = Temp::new("reply");
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py");
    let _g = hooks::install(t.store(), Some(("python3".into(), vec![fixture.into(), "success".into()])));
    let first = send(&t, None, crate::approval::Mode::Ask).expect("the first turn starts").conversation_id;
    // A resumed session keeps the tools it was built with, so a reply in another mode starts cold (codex-mission.ts).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let reply = loop {
        match send(&t, Some(first.clone()), crate::approval::Mode::Plan) {
            Ok(started) => break started,
            Err(e) if (e.contains("still running") || e.contains("already running")) && std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(100))
            }
            Err(e) => panic!("{e}"),
        }
    };
    assert_eq!(reply.conversation_id, first, "a reply answers in the same conversation");
    let turns = t.missions();
    assert_eq!(turns.len(), 2, "{turns:?}");
    let newest = turns.iter().find(|id| **id != first).unwrap();
    assert!(turn_started_cold(newest), "the reply started cold");
    assert!(!turn_started_cold(&first), "the first turn follows nothing");
    assert!(!turn_started_cold("mission_nobody"));
}
