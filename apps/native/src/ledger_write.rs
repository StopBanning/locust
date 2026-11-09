//! Schema 27: mission-store createMission / appendEvents. One owned writer per new mission.
use crate::store::Store;
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
};
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
pub struct LedgerWriter {
    file: File,
    pub path: PathBuf,
    pub mission: String,
    pub run: String,
    pub route: String,
    sequence: u64,
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
        if prompt.trim().is_empty() || prompt.contains('\0') || prompt.encode_utf16().count() > 8000
        {
            return Err(io::Error::other("Prompt must be nonempty text without NUL"));
        }
        if model.is_empty() || model.len() > 250 || model.contains('\0') {
            return Err(io::Error::other("Invalid model ID"));
        }
        let path = store.write_path(id)?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        let path = store.write_path(id)?; // Recheck after directory creation.
        let file = open_checked(store, &path)?;
        let route = format!("codex:{model}");
        let mut writer = Self {
            file,
            path,
            mission: id.into(),
            run: format!("run_{id}"),
            route,
            sequence: 0,
            poisoned: false,
            bytes: 0,
        };
        let at = now();
        writer.line(json!({"schemaVersion":27,"recordType":"mission.created","ledgerSequence":1,"occurredAt":at,"metadata":{
            "missionId":id,"runId":writer.run,"prompt":prompt,"runtime":"codex","model":model,
            "requestedRouteId":writer.route,"resolvedRouteId":writer.route,"cliVersion":null,"workspaceId":"native",
            "sandbox":"read-only","mode":"ask","executionPolicyVersion":1,"createdAt":at,
            "command":{"executablePath":executable,"args":["app-server"]}
        }}))?;
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
        let at = now();
        let mut event = json!({"id":format!("{}:app:{seq}",self.run),"runId":self.run,"missionId":self.mission,
            "sequence":seq,"occurredAt":at,"sourceAdapter":"codex","requestedRouteId":self.route,"resolvedRouteId":self.route});
        if let Some(thread) = thread {
            event["runtimeThreadId"] = json!(thread);
        }
        event["type"] = json!(kind);
        event["payload"] = payload;
        self.line(json!({"schemaVersion":27,"recordType":"mission.event","ledgerSequence":seq+1,"occurredAt":at,"event":event}))?;
        self.sequence = seq;
        Ok(event)
    }
}

// Keep opening and descriptor validation together: no ledger byte precedes this check.
fn open_checked(store: &Store, path: &std::path::Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).append(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    store.check_open_file(&file)?;
    Ok(file)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn swapped_ancestor_leaves_only_reported_empty_file() {
        let temp = std::env::temp_dir().join(format!("locust-open-{}", std::process::id()));
        let native = temp.join("native");
        let electron = temp.join("electron");
        std::fs::create_dir_all(native.join("mission-ledger")).unwrap();
        std::fs::create_dir_all(electron.join("mission-ledger")).unwrap();
        let store = Store::new(native.clone(), electron.clone()).unwrap();
        let checked = store.write_path("race").unwrap();
        std::fs::rename(&native, temp.join("old-native")).unwrap();
        std::os::unix::fs::symlink(&electron, &native).unwrap();
        let error = open_checked(&store, &checked).unwrap_err().to_string();
        let leaked = electron.join("mission-ledger/race.jsonl");
        assert!(
            error.contains("empty file left") && error.contains(leaked.to_str().unwrap()),
            "{error}"
        );
        assert_eq!(std::fs::metadata(&leaked).unwrap().len(), 0);
        // Exclusive create + NOFOLLOW must also refuse a final-component symlink.
        let link = temp.join("link.jsonl");
        std::os::unix::fs::symlink(&leaked, &link).unwrap();
        assert!(open_checked(&store, &link).is_err());
        assert_eq!(std::fs::metadata(&leaked).unwrap().len(), 0);
        std::fs::remove_dir_all(temp).unwrap();
    }
}
