//! Keep running in the background and keep awake (`main/keep-running.ts`, `main/keep-awake.ts`).
//!
//! The decision is pure; the OS power blocker (`IOPMAssertion` / `caffeinate`) is behind `PowerBlocker`.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

pub const BACKGROUND_FILE: &str = "background.json";
pub const AWAKE_FILE: &str = "awake.json";
pub const KEEP_AWAKE_BEAT_MS: u64 = 5_000;

pub fn keep_running_from(value: &Value) -> bool {
    value.get("keepRunning").and_then(Value::as_bool) == Some(true)
}

pub fn read_keep_running(file: &Path) -> bool {
    let Ok(text) = fs::read_to_string(file) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    keep_running_from(&value)
}

pub fn write_keep_running(file: &Path, keep_running: bool) -> Result<(), String> {
    write_bool_json(file, "keepRunning", keep_running)
}

pub fn read_stay_awake(file: &Path) -> bool {
    let Ok(text) = fs::read_to_string(file) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    value.get("stayAwake").and_then(Value::as_bool) == Some(true)
}

pub fn write_stay_awake(file: &Path, stay_awake: bool) -> Result<(), String> {
    write_bool_json(file, "stayAwake", stay_awake)
}

fn write_bool_json(file: &Path, key: &str, on: bool) -> Result<(), String> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }
    let tmp = file.with_extension("json.tmp");
    let body = format!("{}\n", json!({ key: on }));
    fs::write(&tmp, body).map_err(|e| format!("write tmp: {e}"))?;
    fs::rename(&tmp, file).map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

pub trait PowerBlocker {
    fn start(&mut self) -> Result<u64, String>;
    fn stop(&mut self, id: u64);
}

/// Runtime stub when OS power assertion (`IOPMAssertionCreateWithName` / `caffeinate`) is not wired yet.
#[derive(Default)]
pub struct UnwiredPowerBlocker {
    next: u64,
}

impl PowerBlocker for UnwiredPowerBlocker {
    fn start(&mut self) -> Result<u64, String> {
        self.next += 1;
        Ok(self.next)
    }

    fn stop(&mut self, _id: u64) {}
}

pub struct KeepAwake<B: PowerBlocker> {
    blocker: B,
    hold: Option<u64>,
    disposed: bool,
}

impl<B: PowerBlocker> KeepAwake<B> {
    pub fn new(blocker: B) -> Self {
        Self {
            blocker,
            hold: None,
            disposed: false,
        }
    }

    pub fn blocker(&self) -> &B {
        &self.blocker
    }

    pub fn holding(&self) -> bool {
        self.hold.is_some()
    }

    pub fn update(&mut self, live_runs: usize, standing: bool) {
        if self.disposed {
            return;
        }
        let wanted = live_runs > 0 || standing;
        if wanted && self.hold.is_none() {
            self.hold = self.blocker.start().ok();
        } else if !wanted {
            self.release();
        }
    }

    pub fn dispose(&mut self) {
        self.disposed = true;
        self.release();
    }

    fn release(&mut self) {
        if let Some(id) = self.hold.take() {
            self.blocker.stop(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DropDir(std::path::PathBuf);
    impl Drop for DropDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn temp_file(name: &str) -> (DropDir, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "locust-awake-{}",
            crate::native::uuid_v4().unwrap_or_else(|_| "test".into())
        ));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join(name);
        (DropDir(dir), file)
    }

    struct FakeBlocker {
        calls: Vec<String>,
        next: u64,
        fails_to_start: bool,
    }

    impl FakeBlocker {
        fn new(next: u64, fails_to_start: bool) -> Self {
            Self {
                calls: Vec::new(),
                next,
                fails_to_start,
            }
        }
    }

    impl PowerBlocker for FakeBlocker {
        fn start(&mut self) -> Result<u64, String> {
            self.calls.push("start".into());
            if self.fails_to_start {
                return Err("no".into());
            }
            let id = self.next;
            self.next += 1;
            Ok(id)
        }

        fn stop(&mut self, id: u64) {
            self.calls.push(format!("stop {id}"));
        }
    }

    #[test]
    fn holds_on_first_live_run_once_and_releases_when_none() {
        let mut awake = KeepAwake::new(FakeBlocker::new(7, false));
        awake.update(0, false);
        assert!(!awake.holding());
        awake.update(1, false);
        awake.update(3, false);
        awake.update(2, false);
        assert!(awake.holding());
        assert_eq!(awake.blocker().calls, vec!["start"]);

        awake.update(0, false);
        assert!(!awake.holding());
        assert_eq!(awake.blocker().calls, vec!["start", "stop 7"]);

        awake.update(1, false);
        assert_eq!(awake.blocker().calls, vec!["start", "stop 7", "start"]);
    }

    #[test]
    fn dispose_releases_for_good() {
        let mut awake = KeepAwake::new(FakeBlocker::new(7, false));
        awake.update(2, false);
        awake.dispose();
        awake.update(2, false);
        assert!(!awake.holding());
        assert_eq!(awake.blocker().calls, vec!["start", "stop 7"]);
    }

    #[test]
    fn failure_to_start_leaves_unheld_and_retries_next_beat() {
        let mut awake = KeepAwake::new(FakeBlocker::new(7, true));
        awake.update(1, false);
        assert!(!awake.holding());
        awake.update(1, false);
        assert_eq!(awake.blocker().calls, vec!["start", "start"]);
    }

    #[test]
    fn plugged_in_standing_holds_without_runs_and_releases_on_battery() {
        let mut awake = KeepAwake::new(FakeBlocker::new(1, false));
        awake.update(0, true);
        assert!(awake.holding());
        awake.update(0, false);
        assert!(!awake.holding());
        assert_eq!(awake.blocker().calls, vec!["start", "stop 1"]);

        // One hold covers both a live run and standing plugged-in.
        let mut awake2 = KeepAwake::new(FakeBlocker::new(1, false));
        awake2.update(1, false);
        awake2.update(1, true);
        awake2.update(0, true);
        assert!(awake2.holding());
        assert_eq!(awake2.blocker().calls, vec!["start"]);
        awake2.update(0, false);
        assert!(!awake2.holding());
    }

    #[test]
    fn stay_awake_and_keep_running_files_default_false_and_persist() {
        let (_dir, file) = temp_file("awake.json");
        assert!(!read_stay_awake(&file));
        write_stay_awake(&file, true).unwrap();
        assert!(read_stay_awake(&file));
        assert_eq!(
            serde_json::from_str::<Value>(&fs::read_to_string(&file).unwrap()).unwrap(),
            json!({ "stayAwake": true })
        );
        write_stay_awake(&file, false).unwrap();
        assert!(!read_stay_awake(&file));
        fs::write(&file, r#"{"stayAwake":"yes"}"#).unwrap();
        assert!(!read_stay_awake(&file));
        fs::write(&file, "not json").unwrap();
        assert!(!read_stay_awake(&file));

        let (_dir2, bg) = temp_file("background.json");
        assert!(!read_keep_running(&bg));
        write_keep_running(&bg, true).unwrap();
        assert!(read_keep_running(&bg));
        write_keep_running(&bg, false).unwrap();
        assert!(!read_keep_running(&bg));
    }
}
