use super::*;
use crate::{ledger::read_transcript, ledger_write::LedgerWriter, store::Store};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("locust-origin-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn record(&self, origin: Value, mode: &str, sandbox: &str) -> PathBuf {
        let store = Store::new(self.0.join("native"), self.0.join("electron")).unwrap();
        let writer = LedgerWriter::create_runtime_with(
            &store, "mission_1", "Review this", "account-default", "codex", "codex", &[], None,
            &json!({"startedBy": origin, "mode": mode, "sandbox": sandbox}),
        ).unwrap();
        let path = writer.path.clone();
        drop(writer);
        path
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn header(path: &Path) -> Value {
    serde_json::from_str(fs::read_to_string(path).unwrap().lines().next().unwrap()).unwrap()
}

// case: records schema 23 or later, recovers the origin, and projects it into history without forgetting Ask mode
#[test]
fn sweep_other_app_origin_and_ask_survive_the_actual_ledger() {
    let scratch = Scratch::new();
    let path = scratch.record(json!({"kind":"mcp"}), "ask", "read-only");
    let mut head = header(&path);
    assert!(head["schemaVersion"].as_u64().unwrap() >= 23);
    let reopened = read_transcript(&path).metadata.unwrap();
    assert_eq!(reopened.started_by_kind.as_deref(), Some("mcp"));
    assert_eq!(reopened.mode.as_deref(), Some("ask"));
    head["schemaVersion"] = json!(22);
    fs::write(&path, format!("{head}\n")).unwrap();
    assert!(read_transcript(&path).metadata.is_none(), "v22 never claimed this origin");
}

// case: keeps a voice call’s hand-off as one, across a restart (0.739)
#[test]
fn sweep_a_voice_origin_still_reads_as_one_after_reopening_the_ledger() {
    let scratch = Scratch::new();
    let path = scratch.record(json!({"kind":"mcp","via":"voice"}), "accept-edits", "workspace-write");
    assert!(header(&path)["schemaVersion"].as_u64().unwrap() >= 23);
    let reopened = read_transcript(&path).metadata.unwrap();
    assert_eq!(reopened.started_by_kind.as_deref(), Some("mcp"));
    assert!(reopened.started_by_voice);
}

// case: classifies the token file and opt-in as excluded from every profile backup
#[test]
fn sweep_a_written_backup_excludes_the_local_door_token_and_opt_in() {
    let scratch = Scratch::new();
    let profile = scratch.0.join("profile");
    fs::create_dir(&profile).unwrap();
    for file in ["locust-mcp-connection.json", "locust-mcp.json", "teammates.json"] {
        fs::write(profile.join(file), "{}").unwrap();
    }
    let written = write_backup(&profile, &scratch.0, "0.1.0", Local::now()).unwrap();
    let manifest: Value = serde_json::from_str(&fs::read_to_string(written.folder.join(BACKUP_MANIFEST)).unwrap()).unwrap();
    for (file, words) in [("locust-mcp-connection.json", "token"), ("locust-mcp.json", "enable again")] {
        assert!(!written.folder.join("profile").join(file).exists());
        assert!(manifest["leftOut"].as_array().unwrap().contains(&json!(file)));
        assert!(LEFT_OUT.iter().any(|(name, reason)| *name == file && reason.contains(words)));
    }
    assert!(written.folder.join("profile/teammates.json").is_file());
}
