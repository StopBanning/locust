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

/// Whether a discovery answer starts the set-up (index.ts:4771): OpenCode found ready at version 1, agents not hidden.
pub fn wants_set_up(found: &[crate::agents::Found], hidden: bool) -> bool {
    !hidden
        && found.iter().any(|entry| {
            entry.id == "opencode" && entry.ready && entry.version.as_deref().is_some_and(|v| v.starts_with("1."))
        })
}

static SHARED: std::sync::OnceLock<Arc<OpenCodeFirstStart>> = std::sync::OnceLock::new();

/// This process's set-up, on this Mac: the OpenCode on PATH, the command guard a run names, its real config folder.
pub fn shared() -> Arc<OpenCodeFirstStart> {
    SHARED.get_or_init(|| Arc::new(this_mac())).clone()
}

fn this_mac() -> OpenCodeFirstStart {
    OpenCodeFirstStart::new(
        || {
            crate::runtime::discovery::find_named("opencode").ok().map(|path| SetupLaunch {
                executable_path: path.to_string_lossy().into_owned(),
                prefix_args: Vec::new(),
                env: BTreeMap::new(),
            })
        },
        || {
            let store = crate::store::Store::local().ok()?;
            let guard = crate::runtime::opencode_config::write_guard(&store, false).ok()?;
            Some(crate::runtime::opencode_config::file_url(&guard))
        },
        run_bounded,
        || {
            let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
            opencode_config_folder(std::env::var("XDG_CONFIG_HOME").ok().as_deref(), &home)
        },
        |path| path.exists(),
    )
}

/// Runs the set-up command with no shell, its env merged over this process's, ended once `timeout` passes.
fn run_bounded(command: SetupCommand) -> Option<SetupResult> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new(&command.executable_path)
        .args(&command.args)
        .envs(&command.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let (mut out, mut err) = (child.stdout.take()?, child.stderr.take()?);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut o, mut e) = (String::new(), String::new());
        let _ = out.read_to_string(&mut o);
        let _ = err.read_to_string(&mut e);
        let _ = tx.send((o, e));
    });
    match rx.recv_timeout(command.timeout) {
        Ok((stdout, stderr)) => {
            let exit_code = child.wait().ok().and_then(|s| s.code());
            Some(SetupResult { exit_code, timed_out: false, stdout, stderr })
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            Some(SetupResult { exit_code: None, timed_out: true, stdout: String::new(), stderr: String::new() })
        }
    }
}

/// What the window does with each discovery answer (index.ts:4771): when OpenCode 1 is found ready, its first-start
/// set-up runs in the background, once a launch, while the person is still reading the screen. True when it was asked.
pub fn after_discovery(found: &[crate::agents::Found], first_start: &Arc<OpenCodeFirstStart>) -> bool {
    if !wants_set_up(found, std::env::var("LOCUST_HIDE_RUNTIMES").as_deref() == Ok("1")) {
        return false;
    }
    let first_start = first_start.clone();
    std::thread::spawn(move || first_start.ensure());
    true
}

#[cfg(test)]
#[path = "opencode_first_start_tests.rs"]
mod tests;
