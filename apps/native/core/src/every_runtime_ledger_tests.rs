// Carries packages/mission-store/test/every-runtime-can-be-written-down.test.ts (Electron 4ce2f37a): anything a person
// can pick is something the ledger can write down and read back. Electron's list has Gemini, which Rust does not run
// (no Gemini runtime is offered), so Rust's list is the nine it does.
use crate::{ledger::try_list_recent, ledger_write::LedgerWriter, store::Store};
use serde_json::Value;
use std::path::PathBuf;

const EVERY_RUNTIME: [&str; 9] = ["codex", "claude", "cursor", "opencode", "copilot", "antigravity", "muse", "kilo", "acp"];

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn every_runtime_a_person_can_choose_is_recorded_read_back_and_listed() {
    let temp = Temp(std::env::temp_dir().join(format!("locust-runtimes-{}-{}", std::process::id(), crate::invariants::unique_nanos())));
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    // The control: a walk that found nothing would pass every assertion below.
    assert!(EVERY_RUNTIME.len() >= 8);
    for runtime in EVERY_RUNTIME {
        LedgerWriter::create_runtime(&store, &format!("mission_{runtime}"), "Read this workspace and say what it is.", "account-default", runtime, runtime, &[], None)
            .unwrap_or_else(|e| panic!("{runtime} cannot be written to the ledger, so no mission can run under it: {e}"));
    }
    let dir = store.ledger_dir().unwrap();
    let mut listed: Vec<String> = try_list_recent(&dir, 100).unwrap().into_iter().map(|m| m.runtime).collect();
    listed.sort();
    let mut every: Vec<&str> = EVERY_RUNTIME.to_vec();
    every.sort();
    assert_eq!(listed, every);
    for runtime in EVERY_RUNTIME {
        let first = std::fs::read_to_string(dir.join(format!("mission_{runtime}.jsonl"))).unwrap();
        let created: Value = serde_json::from_str(first.lines().next().unwrap()).unwrap();
        assert_eq!(created["schemaVersion"], 27, "{runtime}");
    }
}

// Rust's own: a runtime nobody can pick is refused before any file is made.
#[test]
fn a_runtime_nobody_can_pick_is_refused_before_a_file_is_made() {
    let temp = Temp(std::env::temp_dir().join(format!("locust-runtimes-{}-{}", std::process::id(), crate::invariants::unique_nanos())));
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    assert!(LedgerWriter::create_runtime(&store, "mission_x", "Hello.", "account-default", "x", "teleporter", &[], None).is_err());
    assert!(!store.ledger_dir().unwrap().join("mission_x.jsonl").exists());
}
