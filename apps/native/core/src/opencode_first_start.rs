//! OpenCode sets itself up before the first message (`apps/desktop/src/main/opencode-first-start.ts`, 0.780).
//!
//! On a clean machine, OpenCode's first start with any plugin installs `@opencode-ai/plugin` into its config folder
//! (~61 MB of node_modules), stalling the first message for ~90 s. Running `opencode debug config` once with the
//! command guard plugin named performs that install up front without starting a session or asking a model.

use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

/// Long enough for 61 MB on a slow line; the command ends by itself once the install is done.
pub const OPENCODE_SETUP_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// OpenCode's config folder: `XDG_CONFIG_HOME` when non-empty, else `<home>/.config`, then `opencode`.
pub fn opencode_config_folder(xdg_config_home: Option<&str>, home: &Path) -> PathBuf {
    let base = match xdg_config_home.filter(|s| !s.is_empty()) {
        Some(xdg) => PathBuf::from(xdg),
        None => home.join(".config"),
    };
    base.join("opencode")
}

/// Whether OpenCode has already installed its plugin package there, which is what its first start waits for.
pub fn opencode_is_set_up(folder: &Path, exists: impl Fn(&Path) -> bool) -> bool {
    exists(&folder.join("node_modules").join("@opencode-ai").join("plugin"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenCodeSetUpOutcome {
    SetUp,
    Already,
    Failed,
    NotFound,
    Tried,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupLaunch {
    pub executable_path: String,
    pub prefix_args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupCommand {
    pub purpose: &'static str,
    pub executable_path: String,
    pub args: Vec<String>,
    pub timeout: Duration,
    pub env: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupResult {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
}

type OnceCell = Arc<(Mutex<Option<OpenCodeSetUpOutcome>>, Condvar)>;

#[derive(Default)]
struct State {
    in_flight: Option<OnceCell>,
    tried: bool,
}

pub struct OpenCodeFirstStart {
    locate: Box<dyn Fn() -> Option<SetupLaunch> + Send + Sync>,
    plugin: Box<dyn Fn() -> Option<String> + Send + Sync>,
    run: Box<dyn Fn(SetupCommand) -> Option<SetupResult> + Send + Sync>,
    folder: Box<dyn Fn() -> PathBuf + Send + Sync>,
    exists: Box<dyn Fn(&Path) -> bool + Send + Sync>,
    state: Mutex<State>,
}

impl OpenCodeFirstStart {
    pub fn new(
        locate: impl Fn() -> Option<SetupLaunch> + Send + Sync + 'static,
        plugin: impl Fn() -> Option<String> + Send + Sync + 'static,
        run: impl Fn(SetupCommand) -> Option<SetupResult> + Send + Sync + 'static,
        folder: impl Fn() -> PathBuf + Send + Sync + 'static,
        exists: impl Fn(&Path) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            locate: Box::new(locate),
            plugin: Box::new(plugin),
            run: Box::new(run),
            folder: Box::new(folder),
            exists: Box::new(exists),
            state: Mutex::new(State::default()),
        }
    }

    /// True while a start of OpenCode would have to set it up first: what the start line says.
    pub fn will_set_up(&self) -> bool {
        !opencode_is_set_up(&(self.folder)(), &*self.exists)
    }

    /// Set OpenCode up if it is not, at most once per launch; a second call while it runs shares the first.
    pub fn ensure(&self) -> OpenCodeSetUpOutcome {
        if opencode_is_set_up(&(self.folder)(), &*self.exists) {
            return OpenCodeSetUpOutcome::Already;
        }
        let cell = {
            let mut state = crate::invariants::lock(&self.state);
            if let Some(existing) = state.in_flight.clone() {
                drop(state);
                let (lock, cv) = &*existing;
                let mut guard = crate::invariants::lock(lock);
                while guard.is_none() {
                    guard = cv.wait(guard).unwrap_or_else(|e| e.into_inner());
                }
                return guard.expect("in-flight setup published outcome");
            }
            if state.tried {
                return OpenCodeSetUpOutcome::Tried;
            }
            state.tried = true;
            let cell: OnceCell = Arc::new((Mutex::new(None), Condvar::new()));
            state.in_flight = Some(cell.clone());
            cell
        };

        let outcome = self.run_setup();

        {
            let (lock, cv) = &*cell;
            *crate::invariants::lock(lock) = Some(outcome);
            cv.notify_all();
        }
        let mut state = crate::invariants::lock(&self.state);
        if state.in_flight.as_ref().is_some_and(|c| Arc::ptr_eq(c, &cell)) {
            state.in_flight = None;
        }
        outcome
    }

    fn run_setup(&self) -> OpenCodeSetUpOutcome {
        let Some(launch) = (self.locate)() else {
            return OpenCodeSetUpOutcome::NotFound;
        };
        let Some(plugin) = (self.plugin)() else {
            return OpenCodeSetUpOutcome::Failed;
        };
        let mut args = launch.prefix_args;
        args.push("debug".to_owned());
        args.push("config".to_owned());
        let mut env = launch.env;
        env.insert(
            "OPENCODE_CONFIG_CONTENT".to_owned(),
            json!({"plugin": [plugin]}).to_string(),
        );
        let _ = (self.run)(SetupCommand {
            purpose: "readiness",
            executable_path: launch.executable_path,
            args,
            timeout: OPENCODE_SETUP_TIMEOUT,
            env,
        });
        if opencode_is_set_up(&(self.folder)(), &*self.exists) {
            OpenCodeSetUpOutcome::SetUp
        } else {
            OpenCodeSetUpOutcome::Failed
        }
    }
}

#[cfg(test)]
#[path = "opencode_first_start_tests.rs"]
mod tests;
