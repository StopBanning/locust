//! The team as an Agent Client Protocol agent for editors (Electron `packages/cli/src/acp.ts`): an editor that speaks
//! ACP v1 over stdio (Zed and others) talks to a Locust teammate. JSON-RPC 2.0, one message per line; stdout belongs to
//! the protocol alone. This app reaches its teammates through its own door's tools, so a turn here is a turn the
//! window shows, under the door's rules: its mode, its folder checks, and approval cards answered only in Locust.

use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One door tool call, answered as the door answers (`ToolResult::to_json`).
pub type Call = Arc<dyn Fn(&str, &Value) -> Value + Send + Sync>;
/// One protocol message out.
pub type Out = Arc<dyn Fn(Value) + Send + Sync>;

const NOT_OPEN: &str = "This editor session is not open. Create a new session in the editor.";
const MAX_PROMPT: usize = 8_000;

#[derive(Clone, Debug)]
struct Mate {
    id: String,
    name: String,
    runtime: Option<String>,
    model: Option<String>,
}

struct Session {
    cwd: String,
    teammate: String,
    team: Vec<Mate>,
    conversation: Option<String>,
    used: bool,
    active: Option<Arc<AtomicBool>>,
}

#[derive(Default)]
struct State {
    initialized: bool,
    closing: bool,
    sessions: HashMap<String, Session>,
    next: u64,
}

pub struct AcpAgent {
    call: Call,
    out: Out,
    /// The teammate an editor session starts on, by name or id; the first on the team otherwise.
    teammate: Option<String>,
    state: Arc<Mutex<State>>,
    poll: Duration,
}

struct RpcError(i64, String);

fn err(code: i64, message: &str) -> RpcError {
    RpcError(code, message.to_owned())
}

/// What a door answer said: its structured part, or its text read as JSON, and its words.
fn answer_of(result: &Value) -> (Value, String, bool) {
    let text = result["content"][0]["text"].as_str().unwrap_or("").to_owned();
    let structured = result.get("structuredContent").cloned().or_else(|| serde_json::from_str::<Value>(&text).ok()).unwrap_or(Value::Null);
    (structured, text, result["isError"] == true)
}

impl AcpAgent {
    pub fn new(call: Call, out: Out, teammate: Option<String>) -> Self {
        Self { call, out, teammate, state: Arc::default(), poll: Duration::from_secs(5) }
    }

    #[cfg(test)]
    fn quick(mut self) -> Self {
        self.poll = Duration::from_millis(0);
        self
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn send(&self, message: Value) {
        if self.lock().closing {
            return;
        }
        let mut message = message;
        message["jsonrpc"] = json!("2.0");
        (self.out)(message);
    }

    fn update(out: &Out, session_id: &str, update: Value) {
        out(json!({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": session_id, "update": update}}));
    }

    fn configs(session: &Session) -> Value {
        let options: Vec<Value> = session
            .team
            .iter()
            .map(|t| json!({"value": t.id, "name": format!("{} ({} / {})", t.name, t.runtime.as_deref().unwrap_or("agent not set"), t.model.as_deref().unwrap_or("model not set"))}))
            .collect();
        json!([{"id": "teammate", "name": "Teammate", "type": "select", "currentValue": session.teammate, "options": options}])
    }

    fn team(&self) -> Result<Vec<Mate>, RpcError> {
        let (list, words, refused) = answer_of(&(self.call)("list_teammates", &json!({})));
        let rows = list.as_array().or_else(|| list["teammates"].as_array()).cloned();
        match rows {
            Some(rows) if !refused => Ok(rows
                .iter()
                .filter_map(|t| Some(Mate { id: t["id"].as_str()?.into(), name: t["name"].as_str()?.into(), runtime: t["runtime"].as_str().map(str::to_owned), model: t["model"].as_str().map(str::to_owned) }))
                .collect()),
            _ => Err(RpcError(-32000, if words.is_empty() { "Locust could not list its teammates. Check that Locust is open.".into() } else { words })),
        }
    }

    /// One line from the editor.
    pub fn accept(self: &Arc<Self>, raw: &str) {
        if self.lock().closing {
            return;
        }
        let Ok(message) = serde_json::from_str::<Value>(raw) else {
            return self.send(json!({"id": null, "error": {"code": -32700, "message": "This ACP message is not valid JSON."}}));
        };
        if message["jsonrpc"] != "2.0" {
            return self.send(json!({"id": null, "error": {"code": -32600, "message": "Use a JSON-RPC 2.0 object."}}));
        }
        let id = message.get("id").filter(|v| v.is_string() || v.as_f64().is_some_and(f64::is_finite)).cloned();
        let Some(method) = message["method"].as_str() else { return };
        // Only cancel is a supported notification: never start a hidden turn without a request id to answer.
        if id.is_none() && method != "session/cancel" {
            return;
        }
        let params = message["params"].clone();
        let method = method.to_owned();
        let me = self.clone();
        let respond = move |answer: Result<Value, RpcError>| {
            if let Some(id) = id {
                match answer {
                    Ok(result) => me.send(json!({"id": id, "result": result})),
                    Err(RpcError(code, words)) => me.send(json!({"id": id, "error": {"code": code, "message": words}})),
                }
            }
        };
        if method == "session/prompt" {
            // A turn takes as long as it takes: it answers from its own thread.
            let me = self.clone();
            std::thread::spawn(move || respond(me.prompt(&params)));
        } else {
            respond(self.handle(&method, &params));
        }
    }

    fn handle(&self, method: &str, params: &Value) -> Result<Value, RpcError> {
        if method == "initialize" {
            let mut state = self.lock();
            if state.initialized {
                return Err(err(-32600, "This editor connection is already initialized."));
            }
            if params["protocolVersion"] != 1 {
                return Err(err(-32602, "Locust speaks Agent Client Protocol version 1. Update the editor\u{2019}s ACP client."));
            }
            state.initialized = true;
            return Ok(json!({
                "protocolVersion": 1,
                "agentCapabilities": {"loadSession": false, "promptCapabilities": {"image": false, "audio": false, "embeddedContext": false}, "mcpCapabilities": {"http": false, "sse": false}},
                "agentInfo": {"name": "locust", "title": "Locust", "version": env!("CARGO_PKG_VERSION")},
                "authMethods": []
            }));
        }
        if !self.lock().initialized {
            return Err(err(-32600, "Initialize this ACP connection before creating a session."));
        }
        match method {
            "session/new" => {
                let cwd = params["cwd"].as_str().filter(|c| Path::new(c).is_absolute());
                let servers_empty = params["mcpServers"].as_array().is_some_and(Vec::is_empty);
                let (Some(cwd), true) = (cwd, servers_empty) else {
                    return Err(err(-32602, "Use an absolute cwd and an empty mcpServers list. Locust uses the teammate\u{2019}s connectors configured in Locust, not programs supplied by the editor."));
                };
                let team = self.team()?;
                let wanted = self.teammate.as_deref().map(str::to_lowercase);
                let pick = match &wanted {
                    Some(w) => team.iter().find(|t| t.id.to_lowercase() == *w || t.name.to_lowercase() == *w),
                    None => team.first(),
                };
                let Some(pick) = pick.cloned() else {
                    return Err(err(-32000, "That teammate is not on the team in Locust. Choose one of its teammates."));
                };
                let mut state = self.lock();
                state.next += 1;
                let session_id = format!("locust-editor-{}", state.next);
                let session = Session { cwd: cwd.into(), teammate: pick.id, team, conversation: None, used: false, active: None };
                let options = Self::configs(&session);
                state.sessions.insert(session_id.clone(), session);
                Ok(json!({"sessionId": session_id, "configOptions": options}))
            }
            "session/set_config_option" => {
                let mut state = self.lock();
                let session = params["sessionId"].as_str().and_then(|id| state.sessions.get_mut(id)).ok_or_else(|| err(-32602, NOT_OPEN))?;
                if session.used || session.active.is_some() {
                    return Err(err(-32602, "Create a new editor session to switch teammates after a conversation has started."));
                }
                let value = params["value"].as_str().unwrap_or("");
                if params["configId"] != "teammate" || !session.team.iter().any(|t| t.id == value) {
                    return Err(err(-32602, "Choose a teammate from this session\u{2019}s Teammate option."));
                }
                session.teammate = value.into();
                Ok(json!({"configOptions": Self::configs(session)}))
            }
            "session/cancel" => {
                let state = self.lock();
                let session = params["sessionId"].as_str().and_then(|id| state.sessions.get(id)).ok_or_else(|| err(-32602, NOT_OPEN))?;
                if let Some(active) = &session.active {
                    active.store(true, Ordering::SeqCst);
                }
                Ok(json!({}))
            }
            _ => Err(err(-32601, "Locust does not offer that ACP method. It offers no editor filesystem or terminal operations.")),
        }
    }

    fn prompt(&self, params: &Value) -> Result<Value, RpcError> {
        let session_id = params["sessionId"].as_str().unwrap_or("").to_owned();
        let blocks = params["prompt"].as_array().cloned().unwrap_or_default();
        let (cancel, teammate, cwd, conversation) = {
            let mut state = self.lock();
            if !state.initialized {
                return Err(err(-32600, "Initialize this ACP connection before creating a session."));
            }
            let session = state.sessions.get_mut(&session_id).ok_or_else(|| err(-32602, NOT_OPEN))?;
            if session.active.is_some() {
                return Err(err(-32600, "This session is still working. Cancel the turn or wait for its reply."));
            }
            if blocks.is_empty() || blocks.iter().any(|b| b["type"] != "text" || !b["text"].is_string()) {
                return Err(err(-32602, "Locust accepts text prompts only. Send text, not a file, image, audio, or embedded resource."));
            }
            let cancel = Arc::new(AtomicBool::new(false));
            session.active = Some(cancel.clone());
            session.used = true;
            (cancel, session.teammate.clone(), session.cwd.clone(), session.conversation.clone())
        };
        let text = blocks.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n\n");
        let finish = |me: &Self| {
            if let Some(s) = me.lock().sessions.get_mut(&session_id) {
                s.active = None;
            }
        };
        if text.trim().is_empty() || text.encode_utf16().count() > MAX_PROMPT {
            finish(self);
            return Err(err(-32602, "Write a text prompt of up to 8,000 characters. Nothing was sent."));
        }
        let started = match &conversation {
            None => (self.call)("start_conversation", &json!({"teammate": teammate, "message": text, "folder": cwd})),
            Some(id) => (self.call)("send_message", &json!({"conversation_id": id, "message": text})),
        };
        let (begun, words, refused) = answer_of(&started);
        let Some(conversation_id) = begun["conversation_id"].as_str().map(str::to_owned).filter(|_| !refused) else {
            finish(self);
            return Err(RpcError(-32000, if words.is_empty() { "Locust could not start the turn. Check the conversation in Locust.".into() } else { words }));
        };
        if let Some(s) = self.lock().sessions.get_mut(&session_id) {
            s.conversation = Some(conversation_id.clone());
        }
        let mut told_waiting = false;
        let stop_reason = loop {
            if cancel.load(Ordering::SeqCst) {
                let _ = (self.call)("stop_turn", &json!({"conversation_id": conversation_id}));
                break "cancelled";
            }
            let wait = self.poll.as_secs();
            let (status, words, refused) = answer_of(&(self.call)("read_reply", &json!({"conversation_id": conversation_id, "wait_seconds": wait})));
            match status["status"].as_str() {
                _ if refused => {
                    Self::update(&self.out, &session_id, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": words}}));
                    break "end_turn";
                }
                Some("working") => {
                    told_waiting = false;
                    if self.poll.is_zero() {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
                Some("waiting_for_person") => {
                    // An editor cannot answer a card: it waits for the person in Locust.
                    if !told_waiting {
                        Self::update(&self.out, &session_id, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": format!("\nWaiting for you in Locust: {words}\n")}}));
                        told_waiting = true;
                    }
                    std::thread::sleep(if self.poll.is_zero() { Duration::from_millis(5) } else { Duration::from_secs(2) });
                }
                Some("stopped") => {
                    Self::update(&self.out, &session_id, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": words}}));
                    break "cancelled";
                }
                _ => {
                    Self::update(&self.out, &session_id, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": words}}));
                    break "end_turn";
                }
            }
        };
        finish(self);
        Ok(json!({"stopReason": stop_reason}))
    }

    /// The editor went away: every turn it started stops.
    pub fn close(&self) {
        let actives: Vec<Arc<AtomicBool>> = {
            let mut state = self.lock();
            state.closing = true;
            state.sessions.values().filter_map(|s| s.active.clone()).collect()
        };
        for active in actives {
            active.store(true, Ordering::SeqCst);
        }
    }
}

/// `locust-native --acp [teammate]`: the agent on stdin and stdout, through this machine's open door.
pub fn acp_main(connection: &Path, teammate: Option<String>) {
    use std::io::{BufRead, Write};
    let connection = connection.to_path_buf();
    let call: Call = Arc::new(move |name: &str, args: &Value| crate::door::bridge_call(&connection, name, args));
    let stdout = Arc::new(Mutex::new(std::io::stdout()));
    let out: Out = Arc::new(move |message: Value| {
        let mut out = stdout.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(out, "{message}");
        let _ = out.flush();
    });
    let agent = Arc::new(AcpAgent::new(call, out, teammate));
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if !line.trim().is_empty() {
            agent.accept(&line);
        }
    }
    agent.close();
}

#[cfg(test)]
#[path = "acp_agent_tests.rs"]
mod tests;
