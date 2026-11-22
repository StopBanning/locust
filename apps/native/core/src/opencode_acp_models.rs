//! OpenCode 2 (and Kilo) model discovery over ACP (`apps/desktop/src/main/opencode-acp-models.ts`).
//!
//! Opens an empty disposable folder, runs `initialize` + `session/new` with no client capabilities and no prompt,
//! reads the session's `model` config option, closes the process, and removes the scratch folder. A failed probe
//! returns no models; V1 hints are never used as a fallback for V2.

use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub const CATALOG_DEADLINE: Duration = Duration::from_secs(20);
pub const MAX_OUTPUT: usize = 4 * 1024 * 1024;
/// Kilo's `session/new` carries every model in one line. 336 of them passed 4 MiB in measurement.
pub const KILO_MAX_OUTPUT: usize = 8 * 1024 * 1024;
/// Upper bound on models read from one ACP session (`shared/acp-agents.ts` `MAX_AGENT_MODELS`).
pub const MAX_AGENT_MODELS: usize = 1_000;

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcpAgentModel {
    pub value: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelOption {
    pub id: String,
    pub current: Option<String>,
    pub models: Vec<AcpAgentModel>,
}

fn text_bounded(value: &Value, max_chars: usize) -> Option<String> {
    let s = value.as_str()?;
    if s.is_empty() {
        return None;
    }
    Some(s.chars().take(max_chars).collect())
}

/// `shared/acp-agents.ts` `modelOptionOf`: reads the session's `model` config option (grouped or ungrouped).
pub fn model_option_of(session: &Value) -> Option<ModelOption> {
    let options = session.get("configOptions")?.as_array()?;
    for raw in options {
        let Some(option) = raw.as_object() else { continue };
        let is_model = option.get("category").and_then(Value::as_str) == Some("model")
            || option.get("id").and_then(Value::as_str) == Some("model");
        if !is_model {
            continue;
        }
        let id = text_bounded(option.get("id")?, 100)?;
        let mut flat: Vec<&Value> = Vec::new();
        for entry in option.get("options").and_then(Value::as_array).into_iter().flatten() {
            if let Some(group_options) = entry.get("options").and_then(Value::as_array) {
                flat.extend(group_options.iter());
            } else {
                flat.push(entry);
            }
        }
        let mut models = Vec::new();
        for entry in flat {
            let Some(model) = entry.as_object() else { continue };
            let value = model
                .get("value")
                .and_then(|v| text_bounded(v, 200))
                .or_else(|| model.get("id").and_then(|v| text_bounded(v, 200)));
            let Some(value) = value else { continue };
            let name = model
                .get("name")
                .and_then(|v| text_bounded(v, 200))
                .unwrap_or_else(|| value.clone());
            models.push(AcpAgentModel { value, name });
            if models.len() >= MAX_AGENT_MODELS {
                break;
            }
        }
        let current = option.get("currentValue").and_then(|v| text_bounded(v, 200));
        return Some(ModelOption { id, current, models });
    }
    None
}

#[derive(Clone, Debug)]
pub struct ProbeLaunch {
    pub executable_path: String,
    pub prefix_args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct ProbeRuntime {
    pub id: String,
    pub ready: bool,
    pub major: Option<u32>,
    pub executable: Option<ProbeLaunch>,
    pub hints: Option<Value>,
}

impl ProbeRuntime {
    pub fn for_path(id: &str, executable_path: String, major: Option<u32>) -> Self {
        Self {
            id: id.to_owned(),
            ready: true,
            major,
            executable: Some(ProbeLaunch {
                executable_path,
                prefix_args: Vec::new(),
                env: BTreeMap::new(),
            }),
            hints: None,
        }
    }
}

pub enum ProbeEvent {
    Data(Vec<u8>),
    StdoutClosed,
    Exited,
}

pub trait AcpProbeProcess: Send {
    fn write_line(&mut self, line: &str) -> io::Result<()>;
    fn kill(&mut self) -> io::Result<()>;
}

pub type ProbeSpawn<'a> = &'a (dyn Fn(
    &str,
    &[String],
    &BTreeMap<String, String>,
    &Path,
) -> io::Result<(Box<dyn AcpProbeProcess>, mpsc::Receiver<ProbeEvent>)>
         + Sync);

pub type ScratchFactory = Box<dyn FnOnce() -> io::Result<PathBuf> + Send>;

pub struct OpenCodeAcpModelsOptions<'a> {
    pub spawn: ProbeSpawn<'a>,
    pub scratch: Option<ScratchFactory>,
    pub runtime: &'a str,
    pub deadline: Duration,
}

struct RealProbeProcess {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl AcpProbeProcess for RealProbeProcess {
    fn write_line(&mut self, line: &str) -> io::Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdin closed"))?;
        stdin.write_all(line.as_bytes())?;
        stdin.flush()
    }

    fn kill(&mut self) -> io::Result<()> {
        self.stdin.take();
        crate::runtime::process::kill(self.child.id());
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }
}

pub fn spawn_real_acp_process(
    executable_path: &str,
    args: &[String],
    env: &BTreeMap<String, String>,
    cwd: &Path,
) -> io::Result<(Box<dyn AcpProbeProcess>, mpsc::Receiver<ProbeEvent>)> {
    let mut cmd = Command::new(executable_path);
    cmd.args(args).current_dir(cwd).envs(env);
    let mut child = crate::runtime::process::spawn(&mut cmd, Stdio::piped(), Stdio::null())?;
    let stdin = child.stdin.take();
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("ACP probe stdout missing"))?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match stdout.read(&mut buf) {
                Ok(0) => {
                    let _ = tx.send(ProbeEvent::StdoutClosed);
                    break;
                }
                Ok(n) => {
                    if tx.send(ProbeEvent::Data(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = tx.send(ProbeEvent::StdoutClosed);
                    break;
                }
            }
        }
    });
    Ok((Box::new(RealProbeProcess { child, stdin }), rx))
}

fn default_scratch(prefix: &str) -> io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}-{}",
        std::process::id(),
        SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed),
        crate::invariants::unique_nanos(),
    ));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

struct ProbeGuard {
    child: Option<Box<dyn AcpProbeProcess>>,
    exited: bool,
    cwd: Option<PathBuf>,
    config_home: Option<PathBuf>,
}

impl Drop for ProbeGuard {
    fn drop(&mut self) {
        if !self.exited
            && let Some(child) = self.child.as_mut()
        {
            // The result never depends on shutdown succeeding.
            let _ = child.kill();
        }
        if let Some(dir) = self.cwd.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
        if let Some(dir) = self.config_home.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Reads advertised models from OpenCode 2's (or Kilo's) ACP `session/new` without sending any prompt.
pub fn read_opencode_acp_models(
    runtime: &ProbeRuntime,
    options: OpenCodeAcpModelsOptions<'_>,
) -> Vec<Value> {
    if !runtime.ready {
        return Vec::new();
    }
    let Some(launch) = runtime.executable.as_ref() else {
        return Vec::new();
    };
    let which = if options.runtime.is_empty() {
        "opencode"
    } else {
        options.runtime
    };
    let output_cap = if which == "kilo" { KILO_MAX_OUTPUT } else { MAX_OUTPUT };
    let deadline = Instant::now() + options.deadline;

    let prefix = if which == "kilo" {
        "locust-kilo-models"
    } else {
        "locust-opencode-models"
    };
    let scratch_fn: ScratchFactory = options
        .scratch
        .unwrap_or_else(|| Box::new(move || default_scratch(prefix)));

    // Scratch creation can finish after the overall deadline. It cannot then start a process,
    // and it still owns cleaning up that empty folder.
    let active = Arc::new(AtomicBool::new(true));
    let active_for_scratch = active.clone();
    let (scratch_tx, scratch_rx) = mpsc::channel();
    std::thread::spawn(move || {
        if let Ok(dir) = scratch_fn()
            && (!active_for_scratch.load(Ordering::SeqCst) || scratch_tx.send(dir.clone()).is_err())
        {
            let _ = std::fs::remove_dir_all(&dir);
        }
    });

    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
        active.store(false, Ordering::SeqCst);
        return Vec::new();
    };
    let Ok(cwd) = scratch_rx.recv_timeout(remaining) else {
        active.store(false, Ordering::SeqCst);
        return Vec::new();
    };

    let mut guard = ProbeGuard {
        child: None,
        exited: false,
        cwd: Some(cwd.clone()),
        config_home: None,
    };

    if Instant::now() >= deadline {
        return Vec::new();
    }

    let mut env = launch.env.clone();
    if which == "kilo" {
        let Ok(home) = default_scratch("locust-kilo-config") else {
            return Vec::new();
        };
        guard.config_home = Some(home.clone());
        if Instant::now() >= deadline {
            return Vec::new();
        }
        for (k, v) in crate::runtime::kilo::isolation_env(&home) {
            env.insert(k.to_owned(), v);
        }
    }

    let mut args = launch.prefix_args.clone();
    args.push("acp".to_owned());

    let Ok((child, rx)) = (options.spawn)(&launch.executable_path, &args, &env, &cwd) else {
        return Vec::new();
    };
    guard.child = Some(child);

    let mut buffer: Vec<u8> = Vec::new();
    let mut received: usize = 0;
    let mut next_id: u64 = 0;
    let mut session_answered = false;

    let mut call = |method: &str, params: Value, is_session: bool| -> Option<Value> {
        next_id += 1;
        let id = next_id;
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = format!("{req}\n");
        if guard.child.as_mut()?.write_line(&line).is_err() {
            return None;
        }
        loop {
            while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                let raw: Vec<u8> = buffer.drain(..=pos).collect();
                let trimmed = raw
                    .iter()
                    .copied()
                    .skip_while(u8::is_ascii_whitespace)
                    .collect::<Vec<_>>();
                let end = trimmed
                    .iter()
                    .rposition(|b| !b.is_ascii_whitespace())
                    .map_or(0, |i| i + 1);
                if end == 0 {
                    continue;
                }
                let Ok(message) = serde_json::from_slice::<Value>(&trimmed[..end]) else {
                    continue;
                };
                if !message.is_object() {
                    continue;
                }
                if message.get("id").and_then(Value::as_u64) == Some(id)
                    && (message.get("result").is_some() || message.get("error").is_some())
                {
                    if message.get("error").is_some_and(|e| !e.is_null()) {
                        return None;
                    }
                    if is_session {
                        session_answered = true;
                    }
                    return Some(message.get("result").cloned().unwrap_or(Value::Null));
                }
                if message.get("method").and_then(Value::as_str).is_some()
                    && message
                        .get("id")
                        .is_some_and(|v| v.is_string() || v.is_number())
                {
                    let refusal = json!({
                        "jsonrpc": "2.0",
                        "id": message["id"],
                        "error": {
                            "code": -32601,
                            "message": "No client operations are available during model discovery."
                        }
                    });
                    if guard.child.as_mut()?.write_line(&format!("{refusal}\n")).is_err() {
                        return None;
                    }
                }
            }
            let remaining = deadline.checked_duration_since(Instant::now())?;
            match rx.recv_timeout(remaining) {
                Ok(ProbeEvent::Data(chunk)) => {
                    received += chunk.len();
                    if received > output_cap {
                        return None;
                    }
                    buffer.extend_from_slice(&chunk);
                }
                Ok(ProbeEvent::Exited) => {
                    guard.exited = true;
                    if !session_answered {
                        return None;
                    }
                }
                Ok(ProbeEvent::StdoutClosed) => {
                    if !session_answered {
                        return None;
                    }
                }
                Err(_) => return None,
            }
        }
    };

    let Some(initialized) = call(
        "initialize",
        json!({
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": { "readTextFile": false, "writeTextFile": false },
                "terminal": false
            }
        }),
        false,
    ) else {
        return Vec::new();
    };

    if initialized.get("protocolVersion").and_then(Value::as_i64) != Some(1) {
        return Vec::new();
    }

    let Some(session) = call(
        "session/new",
        json!({
            "cwd": cwd.to_string_lossy(),
            "mcpServers": []
        }),
        true,
    ) else {
        return Vec::new();
    };

    let Some(advertised) = model_option_of(&session) else {
        return Vec::new();
    };

    let mut seen: HashSet<String> = HashSet::new();
    let mut models: Vec<Value> = Vec::new();
    for model in advertised.models {
        if !seen.insert(model.value.clone()) {
            continue;
        }
        let display_name = if which == "kilo" {
            crate::kilo_models::kilo_model_name(&model.value, &model.name)
        } else {
            model.name
        };
        let description = if which == "kilo" {
            crate::kilo_models::kilo_model_description(&model.value)
        } else if model.value.ends_with("-free") {
            "Free · no sign-in"
        } else {
            "Through OpenCode"
        };
        models.push(json!({
            "id": model.value,
            "runtime": which,
            "displayName": display_name,
            "description": description,
            "supportedEfforts": []
        }));
    }

    if which == "kilo" {
        let mut ordered: Vec<Value> = models
            .iter()
            .filter(|m| m["id"] == crate::kilo_models::KILO_AUTO_FREE_MODEL)
            .cloned()
            .collect();
        ordered.extend(
            models
                .into_iter()
                .filter(|m| m["id"] != crate::kilo_models::KILO_AUTO_FREE_MODEL),
        );
        return ordered;
    }

    models
}

/// Production entry point: spawns the real binary with a default scratch folder and deadline.
pub fn read_opencode_acp_models_default(runtime: &ProbeRuntime, deadline: Duration) -> Vec<Value> {
    read_opencode_acp_models(
        runtime,
        OpenCodeAcpModelsOptions {
            spawn: &spawn_real_acp_process,
            scratch: None,
            runtime: if runtime.id == "kilo" { "kilo" } else { "opencode" },
            deadline,
        },
    )
}

#[cfg(test)]
#[path = "opencode_acp_models_tests.rs"]
mod tests;
