//! Your own ACP agents (`apps/desktop/src/shared/acp-agents.ts` and `apps/desktop/src/main/acp-agents.ts`, 0.777).
//!
//! An agent is added by its command line (never invoked through a shell), tested by opening a no-prompt ACP
//! session in an empty scratch folder and closing it again, saved in `acp-agents.json`, listed in the model
//! picker, and resolved again for each run.

use crate::opencode_acp_models::{AcpAgentModel, ModelOption, MAX_AGENT_MODELS, model_option_of};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub const ACP_AGENTS_FILE: &str = "acp-agents.json";
pub const MAX_ACP_AGENTS: usize = 12;
const MAX_NAME: usize = 40;
pub const TEST_TIMEOUT: Duration = Duration::from_secs(45);

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcpAgentAnswer {
    pub agent_name: Option<String>,
    pub version: Option<String>,
    pub model_option: Option<ModelOption>,
    pub sign_in: Vec<String>,
}

impl AcpAgentAnswer {
    pub fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        if let Some(name) = &self.agent_name {
            obj.insert("agentName".into(), json!(name));
        }
        if let Some(version) = &self.version {
            obj.insert("version".into(), json!(version));
        }
        if let Some(opt) = &self.model_option {
            let mut mo = serde_json::Map::new();
            mo.insert("id".into(), json!(opt.id));
            if let Some(current) = &opt.current {
                mo.insert("current".into(), json!(current));
            }
            let models: Vec<Value> = opt
                .models
                .iter()
                .map(|m| json!({"value": m.value, "name": m.name}))
                .collect();
            mo.insert("models".into(), Value::Array(models));
            obj.insert("modelOption".into(), Value::Object(mo));
        }
        if !self.sign_in.is_empty() {
            obj.insert("signIn".into(), json!(self.sign_in));
        }
        Value::Object(obj)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcpAgent {
    pub id: String,
    pub name: String,
    pub command_line: String,
    pub answer: Option<AcpAgentAnswer>,
    pub tested_at: Option<String>,
}

impl AcpAgent {
    pub fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("id".into(), json!(self.id));
        obj.insert("name".into(), json!(self.name));
        obj.insert("commandLine".into(), json!(self.command_line));
        if let Some(answer) = &self.answer {
            obj.insert("answer".into(), answer.to_json());
        }
        if let Some(at) = &self.tested_at {
            obj.insert("testedAt".into(), json!(at));
        }
        Value::Object(obj)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedAcpRoute {
    pub agent_id: String,
    pub model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcpExecutableLaunch {
    pub command_name: String,
    pub executable_path: String,
    pub prefix_args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcpLaunch {
    pub launch: AcpExecutableLaunch,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedAcpAgent {
    pub agent: AcpAgent,
    pub found: AcpLaunch,
}

/// `shared/acp-agents.ts` `commandWords`: program and arguments, never a shell line.
pub fn command_words(line: &str) -> Option<Vec<String>> {
    crate::runtime::acp::command_words(line).ok()
}

/// `shared/acp-agents.ts` `cleanAgentName`: collapsed whitespace, non-empty, up to 40 UTF-16 units.
pub fn clean_agent_name(name: &str) -> Option<String> {
    let clean = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let len = clean.encode_utf16().count();
    (len > 0 && len <= MAX_NAME).then_some(clean)
}

/// `shared/acp-agents.ts` `acpAgentId`: `acp_<slug>`, numbered `-2`, `-3`, ... when already taken.
pub fn acp_agent_id(name: &str, taken: &[String]) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for ch in name.to_lowercase().chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            if dash && !slug.is_empty() {
                slug.push('-');
            }
            dash = false;
            slug.push(ch);
        } else {
            dash = true;
        }
    }
    let trimmed: String = slug.trim_matches('-').chars().take(24).collect();
    let base = format!("acp_{}", if trimmed.is_empty() { "agent" } else { &trimmed });
    if !taken.iter().any(|t| t == &base) {
        return base;
    }
    for n in 2u32.. {
        let candidate = format!("{base}-{n}");
        if !taken.iter().any(|t| t == &candidate) {
            return candidate;
        }
    }
    unreachable!()
}

/// `shared/acp-agents.ts` `acpRouteModel`: route model id for an agent and optional picked model.
pub fn acp_route_model(agent_id: &str, model: Option<&str>) -> String {
    match model.filter(|m| !m.is_empty()) {
        Some(picked) => format!("{agent_id}::{picked}"),
        None => agent_id.to_owned(),
    }
}

/// `shared/acp-agents.ts` `parsedAcpRouteModel`: extracts `agent_id` and optional `model` from an `acp_*` route.
pub fn parsed_acp_route_model(model: &str) -> Option<ParsedAcpRoute> {
    if !model.starts_with("acp_") {
        return None;
    }
    match model.split_once("::") {
        None => Some(ParsedAcpRoute {
            agent_id: model.to_owned(),
            model: None,
        }),
        Some((agent_id, picked)) => Some(ParsedAcpRoute {
            agent_id: agent_id.to_owned(),
            model: (!picked.is_empty()).then(|| picked.to_owned()),
        }),
    }
}

fn text_bounded(value: Option<&Value>, max_chars: usize) -> Option<String> {
    let s = value?.as_str()?;
    if s.is_empty() {
        return None;
    }
    Some(s.chars().take(max_chars).collect())
}

/// `shared/acp-agents.ts` `parsedAnswer`: bounds and validates a stored `AcpAgentAnswer`.
pub fn parsed_answer(value: &Value) -> AcpAgentAnswer {
    let option = value.get("modelOption").and_then(Value::as_object);
    let model_option = option
        .and_then(|o| text_bounded(o.get("id"), 100))
        .map(|id| {
            let opt = option.unwrap();
            let current = text_bounded(opt.get("current"), 200);
            let models: Vec<AcpAgentModel> = opt
                .get("models")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|raw| {
                    let entry = raw.as_object()?;
                    let val = text_bounded(entry.get("value"), 200)?;
                    let name = text_bounded(entry.get("name"), 200).unwrap_or_else(|| val.clone());
                    Some(AcpAgentModel { value: val, name })
                })
                .take(MAX_AGENT_MODELS)
                .collect();
            ModelOption { id, current, models }
        });
    let sign_in: Vec<String> = value
        .get("signIn")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| text_bounded(Some(v), 300))
        .take(5)
        .collect();
    AcpAgentAnswer {
        agent_name: text_bounded(value.get("agentName"), 80),
        version: text_bounded(value.get("version"), 40),
        model_option,
        sign_in,
    }
}

/// `shared/acp-agents.ts` `parsedAcpAgents`: keeps only well-formed unique agents up to `MAX_ACP_AGENTS`.
pub fn parsed_acp_agents(value: &Value) -> Vec<AcpAgent> {
    let Some(list) = value.get("agents").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut agents: Vec<AcpAgent> = Vec::new();
    for raw in list {
        let Some(entry) = raw.as_object() else { continue };
        let Some(id) = entry.get("id").and_then(Value::as_str) else { continue };
        let valid_id = id.strip_prefix("acp_").is_some_and(|rest| {
            !rest.is_empty()
                && rest.len() <= 30
                && rest
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        });
        let Some(name) = entry.get("name").and_then(Value::as_str).and_then(clean_agent_name) else {
            continue;
        };
        let Some(command_line) = entry.get("commandLine").and_then(Value::as_str) else {
            continue;
        };
        if !valid_id || command_words(command_line).is_none() {
            continue;
        }
        if agents.iter().any(|a| a.id == id) {
            continue;
        }
        let answer = entry
            .get("answer")
            .filter(|v| v.is_object())
            .map(parsed_answer);
        let tested_at = entry.get("testedAt").and_then(Value::as_str).map(str::to_owned);
        agents.push(AcpAgent {
            id: id.to_owned(),
            name,
            command_line: command_line.to_owned(),
            answer,
            tested_at,
        });
        if agents.len() >= MAX_ACP_AGENTS {
            break;
        }
    }
    agents
}

/// `shared/acp-agents.ts` `acpCatalog`: one picker row per model, or one default row when an agent offers none.
pub fn acp_catalog(agents: &[AcpAgent]) -> Vec<Value> {
    let mut rows = Vec::new();
    for agent in agents {
        let mut about_parts = Vec::new();
        if let Some(name) = agent.answer.as_ref().and_then(|a| a.agent_name.as_deref()) {
            about_parts.push(name);
        }
        if let Some(ver) = agent.answer.as_ref().and_then(|a| a.version.as_deref()) {
            about_parts.push(ver);
        }
        let about = about_parts.join(" ");
        let description = if about.is_empty() {
            "Approve each: every action it asks to take is a card".to_owned()
        } else {
            format!("{about} · Approve each: every action it asks to take is a card")
        };
        let models = agent
            .answer
            .as_ref()
            .and_then(|a| a.model_option.as_ref())
            .map(|o| o.models.as_slice())
            .unwrap_or(&[]);
        if models.is_empty() {
            rows.push(json!({
                "id": acp_route_model(&agent.id, None),
                "runtime": "acp",
                "displayName": "Its own default model",
                "description": description,
                "supportedEfforts": [],
                "agentName": agent.name
            }));
        } else {
            for model in models {
                rows.push(json!({
                    "id": acp_route_model(&agent.id, Some(&model.value)),
                    "runtime": "acp",
                    "displayName": model.name,
                    "description": description,
                    "supportedEfforts": [],
                    "agentName": agent.name
                }));
            }
        }
    }
    rows
}

/// `main/acp-agents.ts` `launchOf`: resolves a command line into its executable launch and arguments, or why not.
pub fn launch_of(
    command_line: &str,
    locate: &dyn Fn(&str) -> Option<AcpExecutableLaunch>,
) -> Result<AcpLaunch, String> {
    let Some(words) = command_words(command_line) else {
        return Err(
            "Write the command that starts the agent, like goose acp. It runs on its own, not through a shell."
                .into(),
        );
    };
    let program = &words[0];
    let args = words[1..].to_vec();
    if Path::new(program).is_absolute() {
        if !Path::new(program).exists() {
            return Err(format!("{program} is not on this computer."));
        }
        if cfg!(windows) && !program.to_lowercase().ends_with(".exe") {
            return Err(
                "On Windows, a full path must end in .exe. A command on your PATH, like goose, can be anything."
                    .into(),
            );
        }
        return Ok(AcpLaunch {
            launch: AcpExecutableLaunch {
                command_name: program.clone(),
                executable_path: program.clone(),
                prefix_args: Vec::new(),
                env: BTreeMap::new(),
            },
            args,
        });
    }
    let Some(launch) = locate(program) else {
        return Err(format!(
            "Locust could not find {program}. Install it, or check it opens in a new terminal, then test again."
        ));
    };
    Ok(AcpLaunch { launch, args })
}

fn default_scratch() -> io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "locust-acp-test-{}-{}-{}",
        std::process::id(),
        SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed),
        crate::invariants::unique_nanos(),
    ));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

enum LineEvent {
    Line(String),
    Closed,
}

struct TestChildGuard {
    child: Child,
    cwd: PathBuf,
}

impl Drop for TestChildGuard {
    fn drop(&mut self) {
        crate::runtime::process::kill(self.child.id());
        crate::runtime::process::kill_reap(&mut self.child);
        let _ = std::fs::remove_dir_all(&self.cwd);
    }
}

/// `main/acp-agents.ts` `testAcpAgent`: opens an ACP session in an empty folder with no prompt, keeps what the agent
/// answered (name, version, models, sign-in instructions), then closes it and cleans up.
pub fn test_acp_agent(
    launch: &AcpLaunch,
    scratch: Option<&dyn Fn() -> io::Result<PathBuf>>,
    timeout: Duration,
) -> Result<AcpAgentAnswer, String> {
    let cwd = match scratch {
        Some(f) => f(),
        None => default_scratch(),
    }
    .map_err(|_| "The agent could not be started.".to_owned())?;

    let mut cmd = Command::new(&launch.launch.executable_path);
    cmd.args(&launch.launch.prefix_args)
        .args(&launch.args)
        .envs(&launch.launch.env)
        .current_dir(&cwd);

    let mut child = match crate::runtime::process::spawn(&mut cmd, Stdio::piped(), Stdio::piped()) {
        Ok(c) => c,
        Err(_) => {
            let _ = std::fs::remove_dir_all(&cwd);
            return Err("The agent could not be started.".into());
        }
    };

    let mut stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (stderr_tail, stderr_join) = stderr
        .map(crate::runtime::process::stderr_end)
        .unzip();

    let (tx, rx) = mpsc::channel::<LineEvent>();
    if let Some(mut out) = stdout {
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut acc = Vec::new();
            loop {
                match out.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        if !acc.is_empty() {
                            let _ = tx.send(LineEvent::Line(String::from_utf8_lossy(&acc).into_owned()));
                        }
                        let _ = tx.send(LineEvent::Closed);
                        break;
                    }
                    Ok(n) => {
                        acc.extend_from_slice(&buf[..n]);
                        while let Some(pos) = acc.iter().position(|&b| b == b'\n') {
                            let line: Vec<u8> = acc.drain(..=pos).collect();
                            if tx
                                .send(LineEvent::Line(String::from_utf8_lossy(&line).into_owned()))
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                }
            }
        });
    }

    let _guard = TestChildGuard {
        child,
        cwd: cwd.clone(),
    };

    let mut next_id: u64 = 0;

    let mut call = |method: &str, params: Value| -> Result<Value, &'static str> {
        next_id += 1;
        let id = next_id;
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        if let Some(s) = stdin.as_mut() {
            let _ = writeln!(s, "{req}").and_then(|_| s.flush());
        }
        let deadline = Instant::now() + timeout;
        loop {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err("timed-out");
            };
            match rx.recv_timeout(remaining) {
                Ok(LineEvent::Line(raw)) => {
                    let trimmed = raw.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let Ok(msg) = serde_json::from_str::<Value>(trimmed) else {
                        continue;
                    };
                    if !msg.is_object() {
                        continue;
                    }
                    if msg.get("id").and_then(Value::as_u64) == Some(id)
                        && (msg.get("result").is_some() || msg.get("error").is_some())
                    {
                        return Ok(msg);
                    }
                    if msg.get("id").is_some()
                        && !msg["id"].is_null()
                        && msg.get("method").and_then(Value::as_str).is_some()
                    {
                        let refusal = json!({
                            "jsonrpc": "2.0",
                            "id": msg["id"],
                            "error": { "code": -32601, "message": "Locust offers nothing during a test." }
                        });
                        if let Some(s) = stdin.as_mut() {
                            let _ = writeln!(s, "{refusal}").and_then(|_| s.flush());
                        }
                    }
                }
                Ok(LineEvent::Closed) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("exited");
                }
                Err(mpsc::RecvTimeoutError::Timeout) => return Err("timed-out"),
            }
        }
    };

    let init = match call(
        "initialize",
        json!({
            "protocolVersion": 1,
            "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false }, "terminal": false }
        }),
    ) {
        Ok(v) => v,
        Err("exited") => {
            drop(stdin);
            if let Some(join) = stderr_join {
                let _ = join.join();
            }
            let said: String = stderr_tail
                .as_ref()
                .map(|t| {
                    let text = crate::invariants::lock(t);
                    text.trim()
                        .lines()
                        .filter(|l| !l.trim().is_empty())
                        .next_back()
                        .unwrap_or("")
                        .chars()
                        .take(240)
                        .collect()
                })
                .unwrap_or_default();
            let detail = if said.is_empty() {
                String::new()
            } else {
                format!(" It said: {said}")
            };
            return Err(format!(
                "It stopped without answering.{detail} Check that the command starts its ACP mode (often acp or --acp)."
            ));
        }
        Err(_) => {
            return Err(
                "It did not answer within 45 seconds. Check that the command starts its ACP mode (often acp or --acp)."
                    .into(),
            );
        }
    };

    let initialized = init.get("result").and_then(Value::as_object);
    if init.get("error").is_some_and(|e| !e.is_null())
        || initialized
            .and_then(|r| r.get("protocolVersion"))
            .is_none_or(|v| !v.is_number())
    {
        return Err("It answered, but not in the Agent Client Protocol.".into());
    }
    let initialized = initialized.unwrap();
    let info = initialized.get("agentInfo").and_then(Value::as_object);
    let agent_name = info
        .and_then(|i| text_bounded(i.get("name"), 80).or_else(|| text_bounded(i.get("title"), 80)));
    let version = info.and_then(|i| text_bounded(i.get("version"), 40));
    let sign_in: Vec<String> = initialized
        .get("authMethods")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let obj = m.as_object()?;
            obj.get("description")
                .and_then(Value::as_str)
                .or_else(|| obj.get("name").and_then(Value::as_str))
                .map(str::to_owned)
        })
        .collect();

    let session = call(
        "session/new",
        json!({ "cwd": cwd.to_string_lossy(), "mcpServers": [] }),
    );
    let Ok(session) = session else {
        let how = if sign_in.is_empty() {
            String::new()
        } else {
            format!(" It says to sign in: {}.", sign_in.join("; "))
        };
        return Err(format!("It answered, but would not open a session.{how}"));
    };
    if session.get("error").is_some_and(|e| !e.is_null()) {
        let how = if sign_in.is_empty() {
            String::new()
        } else {
            format!(" It says to sign in: {}.", sign_in.join("; "))
        };
        return Err(format!("It answered, but would not open a session.{how}"));
    }

    let model_option = session.get("result").and_then(model_option_of);
    Ok(AcpAgentAnswer {
        agent_name,
        version,
        model_option,
        sign_in,
    })
}

/// Reads saved custom ACP agents from `<dir>/acp-agents.json` (missing or malformed file reads as empty).
pub fn read_saved_agents(directory: &Path) -> Vec<AcpAgent> {
    let path = directory.join(ACP_AGENTS_FILE);
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let mut bytes = Vec::new();
    if file.take(1_000_001).read_to_end(&mut bytes).is_err() || bytes.len() > 1_000_000 {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    parsed_acp_agents(&value)
}

/// Reads custom ACP agents for a `Store`: native's own `acp-agents.json` if present, else Electron's read-only.
pub fn agents_for_store(store: &crate::store::Store) -> Vec<AcpAgent> {
    let native_path = store.root().join(ACP_AGENTS_FILE);
    if native_path.is_file() {
        read_saved_agents(store.root())
    } else {
        read_saved_agents(store.electron())
    }
}

pub struct AcpAgentsStore {
    directory: PathBuf,
    locate: Box<dyn Fn(&str) -> Option<AcpExecutableLaunch> + Send + Sync>,
    scratch: Option<Box<dyn Fn() -> io::Result<PathBuf> + Send + Sync>>,
    timeout: Duration,
    serial: Mutex<()>,
}

impl AcpAgentsStore {
    pub fn new(
        directory: PathBuf,
        locate: impl Fn(&str) -> Option<AcpExecutableLaunch> + Send + Sync + 'static,
    ) -> Self {
        Self {
            directory,
            locate: Box::new(locate),
            scratch: None,
            timeout: TEST_TIMEOUT,
            serial: Mutex::new(()),
        }
    }

    pub fn for_store(store: &crate::store::Store) -> Self {
        Self::new(store.root().to_path_buf(), |program| {
            let path = crate::runtime::discovery::find_named(program).ok()?;
            let s = path.to_string_lossy().into_owned();
            Some(AcpExecutableLaunch {
                command_name: program.to_owned(),
                executable_path: s,
                prefix_args: Vec::new(),
                env: BTreeMap::new(),
            })
        })
    }

    pub fn with_scratch(
        mut self,
        scratch: impl Fn() -> io::Result<PathBuf> + Send + Sync + 'static,
    ) -> Self {
        self.scratch = Some(Box::new(scratch));
        self
    }

    fn write_agents(&self, agents: &[AcpAgent]) -> Result<(), String> {
        std::fs::create_dir_all(&self.directory)
            .map_err(|e| format!("Could not save custom agents: {e}"))?;
        let file = self.directory.join(ACP_AGENTS_FILE);
        let tmp = self.directory.join(format!("{ACP_AGENTS_FILE}.tmp"));
        let body = serde_json::to_string_pretty(&json!({
            "agents": agents.iter().map(AcpAgent::to_json).collect::<Vec<_>>()
        }))
        .map_err(|e| format!("Could not encode custom agents: {e}"))?;
        std::fs::write(&tmp, body).map_err(|e| format!("Could not write custom agents: {e}"))?;
        std::fs::rename(&tmp, &file).map_err(|e| format!("Could not replace custom agents: {e}"))
    }

    pub fn list(&self) -> Result<Vec<AcpAgent>, String> {
        Ok(read_saved_agents(&self.directory))
    }

    pub fn test(&self, command_line: &str) -> Result<AcpAgentAnswer, String> {
        let found = launch_of(command_line, &*self.locate)?;
        let scratch_fn = || self.scratch.as_ref().map(|f| f());
        if self.scratch.is_some() {
            let wrap = || scratch_fn().unwrap();
            test_acp_agent(&found, Some(&wrap), self.timeout)
        } else {
            test_acp_agent(&found, None, self.timeout)
        }
    }

    pub fn save(&self, name: &str, command_line: &str) -> Result<Vec<AcpAgent>, String> {
        let _lock = crate::invariants::lock(&self.serial);
        let Some(clean) = clean_agent_name(name) else {
            return Err("Give it a name of up to 40 letters.".into());
        };
        let mut agents = read_saved_agents(&self.directory);
        if agents.len() >= MAX_ACP_AGENTS {
            return Err(format!(
                "Locust keeps up to {MAX_ACP_AGENTS} of your own agents. Remove one first."
            ));
        }
        if agents
            .iter()
            .any(|agent| agent.name.to_lowercase() == clean.to_lowercase())
        {
            return Err(format!("You already have an agent called {clean}."));
        }
        let answer = self.test(command_line)?;
        let taken: Vec<String> = agents.iter().map(|a| a.id.clone()).collect();
        let agent = AcpAgent {
            id: acp_agent_id(&clean, &taken),
            name: clean,
            command_line: command_line.trim().to_owned(),
            answer: Some(answer),
            tested_at: Some(crate::ledger_write::now()),
        };
        agents.push(agent);
        self.write_agents(&agents)?;
        Ok(agents)
    }

    pub fn remove(&self, id: &str) -> Result<Vec<AcpAgent>, String> {
        let _lock = crate::invariants::lock(&self.serial);
        let agents = read_saved_agents(&self.directory);
        let kept: Vec<AcpAgent> = agents.into_iter().filter(|a| a.id != id).collect();
        self.write_agents(&kept)?;
        Ok(kept)
    }

    pub fn resolve(&self, agent_id: &str) -> Result<ResolvedAcpAgent, String> {
        let agents = read_saved_agents(&self.directory);
        let Some(agent) = agents.into_iter().find(|a| a.id == agent_id) else {
            return Err(
                "That agent is not in Settings > AI agents > Your own agents any more.".into(),
            );
        };
        let found = launch_of(&agent.command_line, &*self.locate)?;
        Ok(ResolvedAcpAgent { agent, found })
    }
}

#[cfg(test)]
#[path = "acp_agents_tests.rs"]
mod tests;
