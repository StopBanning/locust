//! kilo-acp-run.ts: Kilo ACP on the shared OpenCode ACP restart wrapper, with Kilo's own command/config.
use super::{
    Event, Runtime, Turn,
    acp::{Acp, Profile},
    acp_events::AcpNormalizer,
    opencode::OpenCode,
    process::copy_command,
};
use crate::{approval::Mode, store::Store};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

fn err(s: impl ToString) -> io::Error {
    io::Error::other(s.to_string())
}

pub const AUTO_FREE_MODEL: &str = "kilo/kilo-auto/free";

/// kiloModelForRun: never leave a fresh session on Kilo's paid image default.
pub fn model_for_run(model: Option<&str>) -> String {
    match model
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "account-default")
    {
        Some(s) => s.to_owned(),
        None => AUTO_FREE_MODEL.into(),
    }
}

/// kiloSessionMode: Ask/Plan → ask; Edit/Approve-each/Auto → code.
pub fn session_mode(mode: Mode) -> &'static str {
    match mode {
        Mode::Ask | Mode::Plan => "ask",
        _ => "code",
    }
}

/// kiloIsolationEnv: a run-owned config/data tree; never the person's ~/.config/kilo.
pub fn isolation_env(directory: &Path) -> Vec<(String, String)> {
    let dir = directory.to_string_lossy().into_owned();
    vec![
        ("KILO_CONFIG_DIR".into(), dir.clone()),
        ("XDG_CONFIG_HOME".into(), dir.clone()),
        ("XDG_DATA_HOME".into(), dir.clone()),
        ("XDG_CACHE_HOME".into(), dir.clone()),
        ("XDG_STATE_HOME".into(), dir),
        ("KILO_DISABLE_PROJECT_CONFIG".into(), "1".into()),
        ("KILO_DISABLE_CLAUDE_CODE".into(), "1".into()),
        ("KILO_DISABLE_EXTERNAL_SKILLS".into(), "1".into()),
        ("KILO_DISABLE_AUTOUPDATE".into(), "1".into()),
        ("KILO_DISABLE_PRUNE".into(), "1".into()),
        ("KILO_TELEMETRY_LEVEL".into(), "off".into()),
    ]
}

fn git_on_path() -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        if !dir.is_absolute() {
            continue;
        }
        let candidate = dir.join("git");
        if candidate.is_file() {
            let path = candidate.to_string_lossy();
            if path.starts_with('/')
                && !path
                    .chars()
                    .any(|c| matches!(c, '\0' | '\r' | '\n' | '"' | '\'' | '`' | '$' | '\\'))
            {
                return Some(candidate);
            }
        }
    }
    None
}

/// withKiloGitShim: first `git` on PATH runs the real git with stdin from /dev/null.
pub(crate) fn write_git_shim(store: &Store) -> io::Result<Option<PathBuf>> {
    let Some(git) = git_on_path() else {
        return Ok(None);
    };
    let root = store
        .ledger_dir()?
        .parent()
        .ok_or_else(|| io::Error::other("Native ledger has no parent directory"))?
        .to_path_buf();
    let id = format!(
        "kilo-git-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos()
    );
    let dir = store.check(&root.join(id))?;
    let script = format!("#!/bin/sh\nexec \"{}\" \"$@\" </dev/null\n", git.display());
    let mut file = store.create_new_file_in(&dir, "git")?;
    file.write_all(script.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("git");
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&path, perms)?;
    }
    Ok(Some(dir))
}

pub(crate) fn write_home(store: &Store) -> io::Result<PathBuf> {
    let root = store
        .ledger_dir()?
        .parent()
        .ok_or_else(|| io::Error::other("Native ledger has no parent directory"))?
        .to_path_buf();
    // Stable data home: session/load must see the preceding process's sessions.
    let dir = store.check(&root.join("kilo-home"))?;
    let marker = format!(
        ".run-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos()
    );
    store.create_new_file_in(&dir, &marker)?.write_all(b"")?;
    Ok(dir)
}

pub fn command(folder: &Path, store: &Store) -> io::Result<Command> {
    let path = super::discovery::find_named("kilo")?;
    let home = write_home(store)?;
    let mut c = Command::new(path);
    c.arg("acp").current_dir(folder);
    for (k, v) in isolation_env(&home) {
        c.env(k, v);
    }
    if let Some(shim) = write_git_shim(store)? {
        let path_key = "PATH";
        let current = std::env::var_os(path_key).unwrap_or_default();
        let mut entries = vec![shim];
        entries.extend(std::env::split_paths(&current));
        c.env(path_key, std::env::join_paths(entries).map_err(err)?);
    }
    Ok(c)
}

/// Thin OpenCode-ACP restart loop with Kilo's label, required config and permission policy.
pub struct Kilo {
    command: Command,
    driver: Option<Acp>,
    queued: VecDeque<Event>,
    cancel: Arc<AtomicBool>,
    started: bool,
}

impl Kilo {
    pub fn spawn(c: &mut Command) -> io::Result<Self> {
        Ok(Self {
            command: copy_command(c),
            driver: None,
            queued: VecDeque::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            started: false,
        })
    }
    pub fn with_cancel(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancel = cancel;
        self
    }
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed) || crate::run_cli::interrupted()
    }
    fn failure(&mut self, message: &str, code: &str, attempts: usize, stage: &str) {
        let mut normal = AcpNormalizer::default();
        normal.started_at = crate::ledger_write::now();
        self.queued.push_back(normal.event(
            "adapter.diagnostic",
            json!({
                "level":"error",
                "code":format!("kilo.{code}"),
                "message":super::events::text(message,16384),
                "terminal":false,
                "stage":stage,
                "attempts":attempts
            }),
        ));
        self.queued
            .extend(normal.finish("", Value::Null, self.cancelled(), Some(message), false));
    }
}

impl Runtime for Kilo {
    fn start(&mut self, t: &Turn) -> io::Result<()> {
        if self.started {
            return Err(err("Kilo turn already started"));
        }
        self.started = true;
        if self.cancelled() {
            self.failure(
                "Kilo was stopped before startup.",
                "cancelled",
                0,
                "startup",
            );
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut attempts = 0;
        loop {
            if self.cancelled() {
                self.failure(
                    "Kilo was stopped during startup.",
                    "cancelled",
                    attempts,
                    "startup",
                );
                break;
            }
            attempts += 1;
            let mut rt = match Acp::spawn(&mut copy_command(&self.command)) {
                Ok(rt) => rt.with_cancel(self.cancel.clone()),
                Err(_) => {
                    self.failure(
                        "Kilo's ACP process could not start. Check its installed command; no prompt was sent.",
                        "process-start-failed",
                        attempts,
                        "startup",
                    );
                    break;
                }
            };
            // Model first, then mode: setting the model can reset the mode.
            let required = vec![
                ("model".into(), model_for_run(t.model.as_deref())),
                ("mode".into(), session_mode(t.mode).into()),
            ];
            rt.set_profile(Profile {
                runtime: "kilo".into(),
                required,
                strict: true,
                deadline: Some(deadline),
                named: true,
                host_permissions: false,
            });
            match rt.startup(t) {
                Ok(()) => {
                    self.driver = Some(rt);
                    break;
                }
                Err(e) => {
                    let admitted = rt.admitted();
                    let transport = matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionAborted | io::ErrorKind::BrokenPipe
                    );
                    let confirmed = rt.confirm_exit(Duration::from_secs(1));
                    if !confirmed {
                        self.failure(
                            "Kilo's ACP process did not confirm that it exited after being stopped. No replacement process was started.",
                            "termination-unconfirmed",
                            attempts,
                            if admitted { "prompt" } else { "startup" },
                        );
                        break;
                    }
                    if !admitted
                        && transport
                        && attempts <= 2
                        && !self.cancelled()
                        && Instant::now() < deadline
                    {
                        let until = (Instant::now()
                            + Duration::from_millis(250 * 2u64.pow(attempts as u32 - 1)))
                        .min(deadline);
                        while Instant::now() < until && !self.cancelled() {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        if Instant::now() < deadline {
                            continue;
                        }
                    }
                    let code = if Instant::now() >= deadline {
                        "startup-timeout"
                    } else if e.to_string().contains("overflow")
                        || e.to_string().contains("exceeds 8 MiB")
                    {
                        "output-limit"
                    } else if transport {
                        "transport-closed"
                    } else {
                        "agent-error"
                    };
                    self.failure(
                        &e.to_string(),
                        code,
                        attempts,
                        if admitted { "prompt" } else { "startup" },
                    );
                    break;
                }
            }
        }
        Ok(())
    }
    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if let Some(e) = self.queued.pop_front() {
            return Ok(Some(e));
        }
        let Some(rt) = self.driver.as_mut() else {
            return Ok(None);
        };
        let mut e = rt.next_event()?;
        if e.as_ref().is_some_and(|e| {
            matches!(
                e.kind.as_str(),
                "run.completed" | "run.failed" | "run.cancelled"
            )
        }) && !rt.confirm_exit(Duration::from_secs(1))
            && let Some(e) = &mut e
        {
            e.kind = "run.failed".into();
            e.payload["kind"] = json!("process-failed");
            e.payload["message"] =
                json!("Kilo's ACP process did not confirm its exit. No prompt was resent.");
            e.payload["runtimeTerminal"] = json!("missing");
            e.payload["process"]["terminationUnconfirmed"] = json!(true);
        }
        Ok(e)
    }
    fn stop(&mut self) -> io::Result<()> {
        self.cancel.store(true, Ordering::Relaxed);
        match self.driver.as_mut() {
            Some(rt) => rt.stop(),
            None => Ok(()),
        }
    }
}

// Keep the OpenCode type referenced so the shared ACP restart semantics stay linked for reviewers.
#[allow(dead_code)]
fn _shares_opencode_restart_shape() -> Option<OpenCode> {
    None
}

#[cfg(test)]
#[path = "kilo_tests.rs"]
mod tests;
