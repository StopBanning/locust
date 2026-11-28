//! The team as an Agent Client Protocol agent for editors (Electron `packages/cli/src/acp.ts`): an editor that speaks
//! ACP v1 over stdio (Zed and others) talks to a Locust teammate. JSON-RPC 2.0, one message per line; stdout belongs to
//! the protocol alone. This app reaches its teammates through its own door's tools, so a turn here is a turn the
//! window shows, under the door's rules: its mode, its folder checks, and approval cards answered in Locust unless the person enables own-turn answers.

use crate::cli_client::{Abort, CliApproval, CliSession, CliTeammate, Ended, LocalError, LocustClient, TextStream};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// One protocol message out.
pub type Out = Arc<dyn Fn(Value) + Send + Sync>;

const NOT_OPEN: &str = "This editor session is not open. Create a new session in the editor.";
const MAX_PROMPT: usize = 8_000;

struct Session {
    connection: CliSession,
    team: Vec<CliTeammate>,
    used: bool,
    active: Option<Abort>,
}

struct Prompt {
    session_id: String,
    text: String,
    cancel: Abort,
    connection: CliSession,
}

struct Permission {
    editor_session: String,
    approval: String,
    expires: Instant,
    answered: Option<bool>,
}

#[derive(Default)]
struct State {
    initialized: bool,
    closing: bool,
    sessions: HashMap<String, Session>,
    next: u64,
    permissions: HashMap<String, Permission>,
    tasks: Vec<std::thread::JoinHandle<()>>,
}

pub struct AcpAgent {
    client: LocustClient,
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

impl From<LocalError> for RpcError {
    fn from(error: LocalError) -> Self {
        Self(-32000, error.0)
    }
}

impl AcpAgent {
    pub fn new(client: LocustClient, out: Out, teammate: Option<String>) -> Self {
        Self { client, out, teammate, state: Arc::default(), poll: Duration::from_millis(250) }
    }

    #[cfg(test)]
    fn quick(mut self) -> Self {
        self.poll = Duration::from_millis(5);
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

    fn update(&self, session_id: &str, update: Value) {
        self.send(json!({ "method": "session/update", "params": {"sessionId": session_id, "update": update}}));
    }

    fn configs(session: &Session) -> Value {
        let options: Vec<Value> = session
            .team
            .iter()
            .map(|t| json!({"value": t.id, "name": format!("{} ({} / {})", t.name, t.runtime.as_deref().unwrap_or("agent not set"), t.model.as_deref().unwrap_or("model not set"))}))
            .collect();
        json!([{"id": "teammate", "name": "Teammate", "type": "select", "currentValue": session.connection.teammate.id, "options": options}])
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
        let Some(method) = message["method"].as_str() else {
            if let Some(id) = id.as_ref().and_then(Value::as_str)
                && let Some(pending) = self.lock().permissions.get_mut(id)
                && pending.answered.is_none()
                && Instant::now() < pending.expires
            {
                let outcome = &message["result"]["outcome"];
                pending.answered = Some(message.get("error").is_none()
                    && outcome["outcome"] == "selected" && outcome["optionId"] == "allow");
            }
            return;
        };
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
            // Reserve the session while reading this line. The next line may cancel or set its teammate.
            let prompt = match self.prepare_prompt(&params) {
                Ok(prompt) => prompt,
                Err(error) => return respond(Err(error)),
            };
            let me = self.clone();
            let mut state = self.lock();
            state.tasks.retain(|task| !task.is_finished());
            state.tasks.push(std::thread::spawn(move || respond(me.prompt(prompt))));
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
                let team = self.client.teammates()?;
                let connection = self.client.open(self.teammate.as_deref(), Some(cwd))?;
                let mut state = self.lock();
                state.next += 1;
                let session_id = format!("locust-editor-{}", state.next);
                let session = Session { connection, team, used: false, active: None };
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
                    return Err(err(-32602, "Choose a teammate from this session’s Teammate option."));
                }
                // The reader serializes setup and selection. Holding the state also stops a pending prompt from
                // starting on the old connection while the replacement is being opened.
                let next = self.client.open(Some(value), Some(&session.connection.workspace))?;
                if let Err(error) = self.client.close(&session.connection) {
                    let _ = self.client.close(&next);
                    return Err(error.into());
                }
                session.connection = next;
                Ok(json!({"configOptions": Self::configs(session)}))
            }
            "session/cancel" => {
                let id = params["sessionId"].as_str().unwrap_or("");
                let mut state = self.lock();
                let session = state.sessions.get(id).ok_or_else(|| err(-32602, NOT_OPEN))?;
                if let Some(active) = &session.active {
                    active.abort();
                }
                state.permissions.retain(|_, p| p.editor_session != id);
                Ok(json!({}))
            }
            _ => Err(err(-32601, "Locust does not offer that ACP method. It offers no editor filesystem or terminal operations.")),
        }
    }

    fn prepare_prompt(&self, params: &Value) -> Result<Prompt, RpcError> {
        let session_id = params["sessionId"].as_str().unwrap_or("").to_owned();
        let mut state = self.lock();
        if state.closing || !state.initialized {
            return Err(err(-32600, "Initialize this ACP connection before creating a session."));
        }
        let session = state.sessions.get_mut(&session_id).ok_or_else(|| err(-32602, NOT_OPEN))?;
        if session.active.is_some() {
            return Err(err(-32600, "This session is still working. Cancel the turn or wait for its reply."));
        }
        let blocks = params["prompt"].as_array().cloned().unwrap_or_default();
        if blocks.is_empty() || blocks.iter().any(|b| b["type"] != "text" || !b["text"].is_string()) {
            return Err(err(-32602, "Locust accepts text prompts only. Send text, not a file, image, audio, or embedded resource."));
        }
        let text = blocks.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n\n");
        if text.trim().is_empty() || text.encode_utf16().count() > MAX_PROMPT {
            return Err(err(-32602, "Write a text prompt of up to 8,000 characters. Nothing was sent."));
        }
        let cancel = Abort::new();
        session.active = Some(cancel.clone());
        session.used = true;
        Ok(Prompt { session_id, text, cancel, connection: session.connection.clone() })
    }

    fn prompt(&self, prompt: Prompt) -> Result<Value, RpcError> {
        let Prompt { session_id, text, cancel, connection } = prompt;
        let mut stream = TextStream::new(|text| { self.say(&session_id, text); Ok(()) });
        let mut seen = HashSet::new();
        let mut decision_error = None;
        let result = self.client.prompt_owned(&connection, &text, Some(&cancel), &mut |event| stream.accept(event),
            Some(&mut |cards, turn| {
                if turn.aborted() || cancel.aborted() { return; }
                let can_answer = |c: &CliApproval| !c.answer_in_locust && c.kind != "question";
                let offered: HashSet<_> = cards.iter().filter(|c| can_answer(c)).map(|c| c.approval_id.as_str()).collect();
                let actions = {
                    let mut state = self.lock();
                    let mut actions = Vec::new();
                    state.permissions.retain(|_, pending| {
                        if pending.editor_session != session_id { return true; }
                        if !offered.contains(pending.approval.as_str()) { return false; }
                        if let Some(approve) = pending.answered.or_else(|| (Instant::now() >= pending.expires).then_some(false)) {
                            actions.push((pending.approval.clone(), approve));
                            return false;
                        }
                        true
                    });
                    actions
                };
                for (approval, approve) in actions {
                    // The door rechecks the opt-in and same mission/run/card at the point of decision.
                    if cancel.aborted() || turn.aborted() { break; }
                    if let Err(error) = self.client.answer_editor_card(&connection, &approval, approve) {
                        decision_error = Some(error);
                        cancel.abort();
                        return;
                    }
                }
                for card in cards {
                    if cancel.aborted() { break; }
                    if seen.insert(card.approval_id.clone()) {
                        if can_answer(card) {
                            self.offer_permission(&session_id, card, &cancel);
                        } else {
                            self.say(&session_id, &format!("\nWaiting for you in Locust: {}\n", card.summary));
                        }
                    }
                }
            }), self.poll);
        {
            let mut state = self.lock();
            state.permissions.retain(|_, p| p.editor_session != session_id);
            if let Some(s) = state.sessions.get_mut(&session_id) { s.active = None; }
        }
        if let Some(error) = decision_error { return Err(error.into()); }
        Ok(json!({"stopReason": match result? { Ended::EndTurn => "end_turn", Ended::Cancelled => "cancelled" }}))
    }

    fn say(&self, session_id: &str, text: &str) {
        self.update(session_id, json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": text}}));
    }

    fn offer_permission(&self, session: &str, card: &CliApproval, cancel: &Abort) {
        let id = {
            let mut state = self.lock();
            if state.closing || cancel.aborted() { return; }
            state.next += 1;
            let id = format!("locust-permission-{}", state.next);
            state.permissions.insert(id.clone(), Permission {
                editor_session: session.into(), approval: card.approval_id.clone(),
                expires: Instant::now() + Duration::from_secs(120), answered: None,
            });
            id
        };
        self.send(json!({"id": id, "method": "session/request_permission", "params": {"sessionId": session,
            "toolCall": {"toolCallId": card.approval_id, "title": card.summary,
                "kind": match card.kind.as_str() { "command" => "execute", "file-change" => "edit", _ => "other" },
                "content": [{"type": "content", "content": {"type": "text", "text": card.detail}}],
                "rawInput": {"detail": card.detail, "cwd": card.cwd}},
            "options": [{"optionId": "allow", "kind": "allow_once", "name": "Allow once"},
                {"optionId": "deny", "kind": "reject_once", "name": "Deny"}]}}));
    }

    fn keepalive(&self) {
        let connections: Vec<_> = self.lock().sessions.values().map(|s| (s.connection.clone(), s.active.clone())).collect();
        for (connection, active) in connections {
            if self.client.keepalive(&connection).is_err() && let Some(active) = active {
                active.abort();
            }
        }
    }

    /// On EOF, cancel the pending prompts before closing their capability sessions, then wait for cleanup.
    pub fn close(&self) {
        let (actives, tasks) = {
            let mut state = self.lock();
            state.closing = true;
            state.permissions.clear();
            (state.sessions.values().filter_map(|s| s.active.clone()).collect::<Vec<_>>(), std::mem::take(&mut state.tasks))
        };
        for active in actives { active.abort(); }
        for task in tasks { let _ = task.join(); }
        let sessions = std::mem::take(&mut self.lock().sessions);
        for session in sessions.into_values() { let _ = self.client.close(&session.connection); }
    }
}

/// `locust-native --acp [teammate]`: the agent on stdin and stdout, through this machine's open door.
pub fn acp_main(connection: &Path, teammate: Option<String>) {
    let Some(profile) = connection.parent() else { return; };
    acp_stdio(LocustClient::new(profile), teammate, std::io::stdin().lock(), std::io::stdout());
}

fn acp_stdio(
    client: LocustClient,
    teammate: Option<String>,
    input: impl std::io::BufRead,
    output: impl std::io::Write + Send + 'static,
) {
    let stdout = Arc::new(Mutex::new(output));
    let out: Out = Arc::new(move |message: Value| {
        let mut out = stdout.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(out, "{message}");
        let _ = out.flush();
    });
    let agent = Arc::new(AcpAgent::new(client, out, teammate));
    let stop = Abort::new();
    let heartbeat = {
        let (agent, stop) = (agent.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.aborted() {
                stop.sleep(Duration::from_secs(10));
                if !stop.aborted() { agent.keepalive(); }
            }
        })
    };
    for line in input.lines() {
        let Ok(line) = line else { break };
        if !line.trim().is_empty() {
            agent.accept(&line);
        }
    }
    stop.abort();
    agent.close();
    let _ = heartbeat.join();
}

#[cfg(test)]
#[path = "acp_agent_sessions_tests.rs"]
mod tests;
