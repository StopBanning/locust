//! Schema 27: mission-store createMission / appendEvents. One owned writer per new mission.
use crate::store::Store;
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
/// `continuesFrom` of a follow-up turn, as `codex-mission.ts` writes it: checkpointEpoch 1 (a follow-up reconciles
/// nothing, so epoch 1 is the only one a reader could mean), reason `follow-up`, the resumed thread when there is one.
pub struct Continuation {
    pub mission_id: String,
    pub thread: Option<String>,
}
pub struct LedgerWriter {
    file: File,
    pub path: PathBuf,
    pub mission: String,
    pub run: String,
    pub route: String,
    runtime: String,
    /// Next `event.sequence` (1-based). Approvals do not consume this.
    sequence: u64,
    /// Next `ledgerSequence` (created is 1; next record is 2).
    ledger_seq: u64,
    poisoned: bool,
    bytes: u64,
}
impl LedgerWriter {
    pub fn create(
        store: &Store,
        id: &str,
        prompt: &str,
        model: &str,
        executable: &str,
    ) -> io::Result<Self> {
        Self::create_continuing(store, id, prompt, model, executable, None)
    }
    pub fn create_continuing(
        store: &Store,
        id: &str,
        prompt: &str,
        model: &str,
        executable: &str,
        continues: Option<&Continuation>,
    ) -> io::Result<Self> {
        Self::create_runtime(
            store,
            id,
            prompt,
            model,
            executable,
            "codex",
            &["app-server".into()],
            continues,
        )
    }
    /// New mission for any runtime (`codex`, `claude`, …). Args are the CLI argv after the executable.
    pub fn create_runtime(
        store: &Store,
        id: &str,
        prompt: &str,
        model: &str,
        executable: &str,
        runtime: &str,
        args: &[String],
        continues: Option<&Continuation>,
    ) -> io::Result<Self> {
        if prompt.trim().is_empty() || prompt.contains('\0') || prompt.encode_utf16().count() > 8000
        {
            return Err(io::Error::other("Prompt must be nonempty text without NUL"));
        }
        if model.is_empty() || model.len() > 250 || model.contains('\0') {
            return Err(io::Error::other("Invalid model ID"));
        }
        if !matches!(runtime, "codex" | "claude" | "acp" | "opencode" | "kilo") {
            return Err(io::Error::other("Unsupported runtime"));
        }
        // Validated before anything is created: a bad link leaves no empty ledger.
        if let Some(c) = continues {
            let safe = |v: &str| {
                !v.is_empty() && v.len() <= 128 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            };
            if !safe(&c.mission_id) || c.thread.as_deref().is_some_and(|t| t.is_empty() || t.len() > 2048) {
                return Err(io::Error::other("Invalid continuation"));
            }
        }
        let path = store.write_path(id)?;
        // The directory is created and walked without following any symlink (`Store::create_new_file_in`), so the
        // ancestor race closes: no file is ever created through a swapped component.
        let file = open_checked(store, &path)?;
        let route = format!("{runtime}:{model}");
        let mut writer = Self {
            file,
            path,
            mission: id.into(),
            run: format!("run_{id}"),
            route,
            runtime: runtime.into(),
            sequence: 0,
            ledger_seq: 2,
            poisoned: false,
            bytes: 0,
        };
        let at = now();
        let continuation = continues.map(|c| {
            let mut link = json!({"missionId":c.mission_id,"checkpointEpoch":1,"reason":"follow-up"});
            if let Some(thread) = &c.thread {
                link["runtimeThreadId"] = json!(thread);
            }
            link
        });
        let mut record = json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":at,"metadata":{
            "missionId":id,"runId":writer.run,"prompt":prompt,"runtime":runtime,"model":model,
            "requestedRouteId":writer.route,"resolvedRouteId":writer.route,"cliVersion":null,"workspaceId":"native",
            "sandbox":"read-only","mode":"ask","executionPolicyVersion":1,"createdAt":at,
            "command":{"executablePath":executable,"args":args}
        }});
        if let Some(link) = continuation {
            record["metadata"]["continuesFrom"] = link;
        }
        writer.line(record)?;
        if let Ok(dir) = File::open(writer.path.parent().unwrap()) {
            let _ = dir.sync_all();
        }
        Ok(writer)
    }
    fn line(&mut self, value: Value) -> io::Result<()> {
        if self.poisoned {
            return Err(io::Error::other(
                "Ledger write previously failed; refusing further appends",
            ));
        }
        let mut bytes = serde_json::to_vec(&value)?;
        bytes.push(b'\n');
        if bytes.len() > 512 * 1024 || self.bytes + bytes.len() as u64 > 64 * 1024 * 1024 {
            return Err(io::Error::other(
                "Ledger exceeds Electron's record/file safety limit",
            ));
        }
        if self.file.metadata()?.len() != self.bytes {
            self.poisoned = true;
            return Err(io::Error::other("Ledger changed outside the active writer"));
        }
        let result = self
            .file
            .write_all(&bytes)
            .and_then(|_| self.file.sync_all());
        if result.is_err() {
            self.poisoned = true;
        }
        if result.is_ok() {
            self.bytes += bytes.len() as u64;
        }
        result
    }
    pub fn append(
        &mut self,
        kind: &str,
        payload: Value,
        thread: Option<&str>,
    ) -> io::Result<Value> {
        let seq = self.sequence + 1;
        let ledger = self.ledger_seq;
        let at = now();
        let mut event = json!({"id":format!("{}:app:{seq}",self.run),"runId":self.run,"missionId":self.mission,
            "sequence":seq,"occurredAt":at,"sourceAdapter":self.runtime,"requestedRouteId":self.route,"resolvedRouteId":self.route});
        if let Some(thread) = thread {
            event["runtimeThreadId"] = json!(thread);
        }
        event["type"] = json!(kind);
        event["payload"] = payload;
        self.line(json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":ledger,"occurredAt":at,"event":event}))?;
        self.sequence = seq;
        self.ledger_seq = ledger + 1;
        Ok(event)
    }
    /// `mission-store` appendApproval: a card the person answered (schema ≥ 20).
    pub fn append_approval(&mut self, approval: &Value) -> io::Result<()> {
        let ledger = self.ledger_seq;
        let at = approval["occurredAt"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(now);
        self.line(json!({
            "schemaVersion": 27,
            "recordType": "mission.approval",
            "ledgerSequence": ledger,
            "occurredAt": at,
            "approval": approval,
        }))?;
        self.ledger_seq = ledger + 1;
        Ok(())
    }
}

// Keep opening and descriptor validation together: no ledger byte precedes this check. The file is created by
// `openat` relative to a directory descriptor walked without following symlinks, so nothing is created through a
// swapped path component; `check_open_file` then confirms the descriptor's own location (defence in depth).
fn open_checked(store: &Store, path: &std::path::Path) -> io::Result<File> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return Err(io::Error::other("Invalid ledger path"));
    };
    let file = store.create_new_file_in(dir, name)?;
    store.check_open_file(&file)?;
    Ok(file)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn temp(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("locust-open-{tag}-{}", std::process::id()))
    }
    #[test]
    fn swapped_ancestor_is_refused_and_nothing_is_created_in_electrons_folder() {
        let temp = temp("race");
        let _ = std::fs::remove_dir_all(&temp);
        let native = temp.join("native");
        let electron = temp.join("electron");
        std::fs::create_dir_all(native.join("mission-ledger")).unwrap();
        std::fs::create_dir_all(electron.join("mission-ledger")).unwrap();
        let store = Store::new(native.clone(), electron.clone()).unwrap();
        // The path was checked while it was good ...
        let checked = store.write_path("race").unwrap();
        // ... then an ancestor is swapped for a symlink into Electron's folder.
        std::fs::rename(&native, temp.join("old-native")).unwrap();
        std::os::unix::fs::symlink(&electron, &native).unwrap();
        let error = open_checked(&store, &checked).unwrap_err().to_string();
        assert!(error.contains("nothing was created") || error.contains("Refusing"), "{error}");
        let leaked = electron.join("mission-ledger/race.jsonl");
        assert!(!leaked.exists(), "nothing may be created inside Electron's folder");
        assert_eq!(std::fs::read_dir(electron.join("mission-ledger")).unwrap().count(), 0);
        std::fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn a_symlink_in_the_final_directory_or_the_file_name_is_refused_and_creates_nothing() {
        let temp = temp("final");
        let _ = std::fs::remove_dir_all(&temp);
        let native = temp.join("native");
        let electron = temp.join("electron");
        std::fs::create_dir_all(&native).unwrap();
        std::fs::create_dir_all(electron.join("mission-ledger")).unwrap();
        let store = Store::new(native.clone(), electron.clone()).unwrap();
        // mission-ledger itself is a symlink into Electron's folder.
        let checked = store.write_path("x").unwrap();
        std::os::unix::fs::symlink(electron.join("mission-ledger"), native.join("mission-ledger")).unwrap();
        assert!(open_checked(&store, &checked).is_err());
        assert_eq!(std::fs::read_dir(electron.join("mission-ledger")).unwrap().count(), 0);
        // A final-component symlink (even a dangling one) is never followed or created through.
        std::fs::remove_file(native.join("mission-ledger")).unwrap();
        std::fs::create_dir_all(native.join("mission-ledger")).unwrap();
        let target = electron.join("mission-ledger/victim.jsonl");
        std::os::unix::fs::symlink(&target, native.join("mission-ledger/link.jsonl")).unwrap();
        assert!(open_checked(&store, &native.join("mission-ledger/link.jsonl")).is_err());
        assert!(!target.exists());
        // A good path still works, makes missing directories with mode 0700, and refuses a second create.
        let good = store.write_path("good").unwrap();
        std::fs::remove_dir_all(native.join("mission-ledger")).unwrap();
        let file = open_checked(&store, &good).unwrap();
        drop(file);
        assert!(open_checked(&store, &good).is_err(), "exclusive");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(native.join("mission-ledger")).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(std::fs::metadata(&good).unwrap().permissions().mode() & 0o777, 0o600);
        std::fs::remove_dir_all(temp).unwrap();
    }
}
