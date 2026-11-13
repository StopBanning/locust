//! Version-split OpenCode runtime: V2 ACP; V1 authenticated loopback HTTP/SSE.
use super::{
    Event, Runtime, Turn,
    acp::{Acp, Profile},
    acp_events::AcpNormalizer,
    process::copy_command,
};
use crate::{approval::Mode, store::Store};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io,
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
/// discovery.ts:openCodeUsesAcp — only major 2 uses ACP; V1 keeps serve; any other major is refused.
pub fn version_major(version: &str) -> Option<u32> {
    let s = version.trim();
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let major: String = s[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    major.parse().ok()
}
#[cfg(any(test, feature = "test-hooks"))]
#[cfg(any(test, feature = "test-hooks"))]
pub fn version_uses_acp(version: &str) -> bool {
    version_major(version) == Some(2)
}
pub fn command(folder: &std::path::Path, mode: Mode, store: &Store) -> io::Result<Command> {
    let path = super::discovery::find_named("opencode")?;
    let output = super::version::probe(&path, "OpenCode")?;
    let version = output.trim();
    let major =
        version_major(version).ok_or_else(|| err("OpenCode's version could not be identified"))?;
    let v2 = match major {
        1 => false,
        2 => true,
        _ => {
            return Err(err(format!(
                "OpenCode {version} is not supported. Locust runs OpenCode 1 through its local server and OpenCode 2 through ACP."
            )));
        }
    };
    let mut c = Command::new(path);
    if v2 {
        c.arg("acp");
    } else {
        c.args(["serve", "--port", "0", "--hostname", "127.0.0.1"]);
    }
    c.current_dir(folder);
    super::opencode_config::configure(&mut c, mode, store, v2)?;
    c.env(
        "LOCUST_NATIVE_OPENCODE_PROTOCOL",
        if v2 { "acp" } else { "serve" },
    );
    Ok(c)
}
enum Driver {
    Acp(Box<Acp>),
    Serve(Box<super::opencode_serve::Serve>),
}
pub struct OpenCode {
    command: Command,
    driver: Option<Driver>,
    queued: VecDeque<Event>,
    cancel: Arc<AtomicBool>,
    v2: bool,
    started: bool,
    fork: bool,
    /// One of OpenCode's own commands the person typed: sent by name to its server (0.427).
    slash: Option<String>,
}
/// Whether this OpenCode command runs OpenCode 2 over ACP (its `acp` argument, or the tests' protocol switch).
pub fn uses_acp(c: &Command) -> bool {
    c.get_envs()
        .find(|(k, _)| *k == "LOCUST_NATIVE_OPENCODE_PROTOCOL")
        .and_then(|(_, v)| v)
        .map(|s| s == "acp")
        .unwrap_or_else(|| c.get_args().any(|a| a == "acp"))
}

impl OpenCode {
    pub fn spawn(c: &mut Command) -> io::Result<Self> {
        let v2 = uses_acp(c);
        c.env_remove("LOCUST_NATIVE_OPENCODE_PROTOCOL");
        Ok(Self {
            command: copy_command(c),
            driver: None,
            queued: VecDeque::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            v2,
            started: false,
            fork: false,
            slash: None,
        })
    }
    pub fn with_fork(mut self, fork: bool) -> Self {
        self.fork = fork;
        self
    }
    pub fn with_command(mut self, name: Option<String>) -> Self {
        self.slash = name;
        self
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
        self.queued.push_back(normal.event("adapter.diagnostic",json!({"level":"error","code":format!("opencode.{code}"),"message":super::events::text(message,16384),"terminal":false,"stage":stage,"attempts":attempts})));
        self.queued
            .extend(normal.finish("", Value::Null, self.cancelled(), Some(message), false));
    }
}
impl Runtime for OpenCode {
    fn start(&mut self, t: &Turn) -> io::Result<()> {
        if self.started {
            return Err(err("OpenCode turn already started"));
        }
        self.started = true;
        // OpenCode 2 over ACP has no command call: the words go as typed, `/name` first.
        let typed;
        let t = match (&self.slash, self.v2) {
            (Some(name), true) => {
                typed = Turn {
                    prompt: format!("/{name} {}", t.prompt).trim().to_owned(),
                    folder: t.folder.clone(),
                    model: t.model.clone(),
                    effort: t.effort.clone(),
                    resume: t.resume.clone(),
                    mode: t.mode,
                    approvals: t.approvals.clone(),
                    mission_id: t.mission_id.clone(),
                    run_id: t.run_id.clone(),
                };
                &typed
            }
            _ => t,
        };
        if self.cancelled() {
            self.failure(
                "OpenCode was stopped before startup.",
                "cancelled",
                0,
                "startup",
            );
            return Ok(());
        }
        if !self.v2 {
            let mut serve =
                super::opencode_serve::Serve::new(copy_command(&self.command), self.cancel.clone())
                    .with_fork(self.fork)
                    .with_command(self.slash.clone());
            serve.start(t)?;
            self.driver = Some(Driver::Serve(Box::new(serve)));
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut attempts = 0;
        loop {
            if self.cancelled() {
                self.failure(
                    "OpenCode was stopped during startup.",
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
                    self.failure("OpenCode's ACP process could not start. Check its installed command; no prompt was sent.","process-start-failed",attempts,"startup");
                    break;
                }
            };
            let mut required = vec![("mode".into(), "locust".into())];
            if let Some(model) = t.model.as_ref().filter(|m| m.as_str() != "account-default") {
                required.push(("model".into(), model.clone()));
            }
            if let Some(e) = &t.effort {
                required.push(("effort".into(), e.clone()));
            }
            rt.set_profile(Profile {
                runtime: "opencode".into(),
                required,
                strict: true,
                deadline: Some(deadline),
                named: true,
                host_permissions: true,
            });
            match rt.startup(t) {
                Ok(()) => {
                    self.driver = Some(Driver::Acp(Box::new(rt)));
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
                        self.failure("OpenCode's ACP process did not confirm that it exited after being stopped. No replacement process was started.","termination-unconfirmed",attempts,if admitted{"prompt"}else{"startup"});
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
        match self.driver.as_mut() {
            Some(Driver::Acp(rt)) => {
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
                    e.payload["message"] = json!(
                        "OpenCode's ACP process did not confirm its exit. No prompt was resent."
                    );
                    e.payload["runtimeTerminal"] = json!("missing");
                    e.payload["process"]["terminationUnconfirmed"] = json!(true);
                }
                Ok(e)
            }
            Some(Driver::Serve(rt)) => rt.next_event(),
            None => Ok(None),
        }
    }
    fn stop(&mut self) -> io::Result<()> {
        self.cancel.store(true, Ordering::Relaxed);
        match self.driver.as_mut() {
            Some(Driver::Acp(rt)) => rt.stop(),
            Some(Driver::Serve(rt)) => rt.stop(),
            None => Ok(()),
        }
    }
}
#[cfg(test)]
#[path = "opencode_tests.rs"]
mod tests;
