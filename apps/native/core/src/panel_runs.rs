//! The runs whose answer belongs to a panel (Electron `main/panel-runs.ts`, 0.771): a commit message written from the
//! changes, a walk through a change. Each is in the record like any run, and listed in `panel-runs.json` so the
//! window never shows one among the conversations. Native keeps its own panel runs in a store of their own
//! (`native::run_once`), so this list is read to hide the ones Electron made, whose ledgers native lists beside its own.
use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Thousands fit in a few hundred kilobytes; past that the oldest go first.
pub const KEEP: usize = 5_000;
const MAX_ID_CHARS: usize = 200;

pub struct PanelRuns {
    file: PathBuf,
    keep: usize,
    /// One write at a time.
    writing: Mutex<()>,
}

impl PanelRuns {
    pub fn new(file: impl Into<PathBuf>, keep: usize) -> Self {
        Self { file: file.into(), keep: keep.max(1), writing: Mutex::new(()) }
    }

    /// Electron's own list, in its profile folder.
    pub fn of_electron(electron: &Path) -> Self {
        Self::new(electron.join("panel-runs.json"), KEEP)
    }

    /// The listed runs, oldest first; a file it cannot use reads as none.
    pub fn list(&self) -> Vec<String> {
        let Ok(text) = fs::read_to_string(&self.file) else { return Vec::new() };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else { return Vec::new() };
        let Some(ids) = parsed.get("missionIds").and_then(|v| v.as_array()) else { return Vec::new() };
        let ids: Vec<String> = ids
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|id| id.encode_utf16().count() <= MAX_ID_CHARS)
            .map(str::to_owned)
            .collect();
        let skip = ids.len().saturating_sub(self.keep);
        ids.into_iter().skip(skip).collect()
    }

    pub fn ids(&self) -> HashSet<String> {
        self.list().into_iter().collect()
    }

    /// Lists a run once, as the newest, dropping the oldest past the limit; written whole through a scratch file.
    pub fn add(&self, mission_id: &str) -> io::Result<()> {
        let _one = crate::invariants::lock(&self.writing);
        let mut ids: Vec<String> = self.list().into_iter().filter(|id| id != mission_id).collect();
        ids.push(mission_id.to_owned());
        let skip = ids.len().saturating_sub(self.keep);
        let ids: Vec<String> = ids.into_iter().skip(skip).collect();
        if let Some(parent) = self.file.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut scratch = self.file.clone().into_os_string();
        scratch.push(format!(".{}.tmp", std::process::id()));
        let scratch = PathBuf::from(scratch);
        fs::write(&scratch, serde_json::json!({ "missionIds": ids }).to_string())?;
        fs::rename(&scratch, &self.file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn folder() -> Temp {
        let dir = std::env::temp_dir().join(format!("locust-commit-drafts-{}", crate::invariants::unique_nanos()));
        fs::create_dir_all(&dir).unwrap();
        Temp(dir)
    }

    // case: are kept across a restart, each once, the oldest going first past the limit
    #[test]
    fn are_kept_across_a_restart_each_once_the_oldest_going_first_past_the_limit() {
        let dir = folder();
        let file = dir.0.join("panel-runs.json");
        let runs = PanelRuns::new(&file, 3);
        std::thread::scope(|s| {
            for id in ["mission_a", "mission_b"] {
                let runs = &runs;
                s.spawn(move || runs.add(id).unwrap());
            }
        });
        runs.add("mission_a").unwrap();
        let mut first = runs.list();
        assert_eq!(first.pop().as_deref(), Some("mission_a"));
        assert_eq!(first, ["mission_b"]);
        runs.add("mission_c").unwrap();
        runs.add("mission_d").unwrap();
        assert_eq!(PanelRuns::new(&file, 3).list(), ["mission_a", "mission_c", "mission_d"]);
        let written: serde_json::Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(written, serde_json::json!({ "missionIds": ["mission_a", "mission_c", "mission_d"] }));
        assert!(fs::read_dir(&dir.0).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
    }

    // case: reads a file it cannot use as none
    #[test]
    fn reads_a_file_it_cannot_use_as_none() {
        let dir = folder();
        let file = dir.0.join("panel-runs.json");
        fs::write(&file, "not json").unwrap();
        assert!(PanelRuns::new(&file, KEEP).list().is_empty());
        // An id too long to be one, and anything not text, are not listed.
        fs::write(&file, serde_json::json!({ "missionIds": ["m1", 7, "x".repeat(201)] }).to_string()).unwrap();
        assert_eq!(PanelRuns::new(&file, KEEP).list(), ["m1"]);
    }
}
