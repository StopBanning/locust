//! THE DOOR'S TOOLS: what another app on this computer may ask Locust to do (locust-mcp-tools.ts).
//!
//! Another app (Claude Code, Codex, a chat app) is a guest. It may list the teammates, start a conversation with one,
//! continue it, read its reply, stop a turn it started, and, while the person allows it, see their conversations. A turn
//! it starts runs in Ask unless the person turned on "Use each teammate's own mode"; then in the teammate's saved mode,
//! or a mode and folder the app names, checked here. Approval cards go only to Locust's window: there is no tool to
//! answer one. Every refusal says that nothing was started, sent or stopped, so an app never retries blind.
//!
//! The rules and words follow Electron's; what the native engine cannot do yet is said the way Electron's tools say it
//! for a server that does not offer it: a choice of model for a teammate's turn (no catalog yet) and routines.

use crate::approval::Mode;
use crate::engine::Teammate;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

/// The longest `read_reply` waits for an answer when asked to (MCP_WAIT_LIMIT_S).
pub const WAIT_LIMIT_S: u64 = 20;

/// The words Locust's own mode control uses (MODE_NAME), and the modes an app may name.
const MODES: [&str; 5] = ["ask", "plan", "accept-edits", "approve-each", "auto"];

/// One tool's answer, as MCP carries it: text, whether it is a refusal, and a status an app can read without parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    pub text: String,
    pub is_error: bool,
    pub structured: Option<Value>,
}

impl ToolResult {
    pub fn ok(text: impl Into<String>) -> Self {
        Self { text: text.into(), is_error: false, structured: None }
    }
    pub fn refused(text: impl Into<String>) -> Self {
        Self { text: text.into(), is_error: true, structured: None }
    }
    /// The JSON the door answers with (McpToolResult).
    pub fn to_json(&self) -> Value {
        let mut out = json!({"content": [{"type": "text", "text": self.text}]});
        if self.is_error {
            out["isError"] = json!(true);
        }
        if let Some(s) = &self.structured {
            out["structuredContent"] = s.clone();
        }
        out
    }
}

/// A turn the door asks the engine to start.
#[derive(Debug, Clone)]
pub struct StartRequest {
    pub teammate: Teammate,
    pub prompt: String,
    /// The conversation's newest turn, for a follow-up.
    pub follow_up_of: Option<String>,
    pub mode: Mode,
    /// A folder the app named (checked), or the folder the conversation was started in.
    pub folder: Option<PathBuf>,
    /// A follow-up keeps its conversation's runtime and model.
    pub runtime: Option<String>,
    pub model: Option<String>,
}

/// What the engine reports once a turn is running: its id and the mode it really runs in.
#[derive(Debug, Clone, PartialEq)]
pub struct Begun {
    pub mission_id: String,
    pub mode: String,
    pub runtime: Option<String>,
    pub model: Option<String>,
}

/// One turn read from its ledger: what the tools need of `PublicRecoveredMission`.
#[derive(Debug, Clone, Default)]
pub struct Turn {
    pub mission_id: String,
    pub prompt: String,
    pub runtime: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub sandbox: Option<String>,
    pub started_by: Value,
    pub cwd: Option<String>,
    pub events: Vec<Value>,
    pub updated: String,
}

impl Turn {
    /// Built from a ledger's records: the `mission.created` metadata and every event, in order.
    pub fn from_records(records: &[Value]) -> Option<Self> {
        let meta = records.iter().find(|r| r["recordType"] == "mission.created").map(|r| &r["metadata"])?;
        let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
        let mut events: Vec<(u64, Value)> = records
            .iter()
            .filter_map(|r| r.get("event"))
            .map(|e| (e["sequence"].as_u64().unwrap_or(0), e.clone()))
            .collect();
        events.sort_by_key(|(s, _)| *s);
        events.dedup_by_key(|(s, _)| *s);
        let updated = records.iter().rev().find_map(|r| r["occurredAt"].as_str().map(str::to_owned)).unwrap_or_default();
        Some(Self {
            mission_id: text(&meta["missionId"])?,
            prompt: meta["prompt"].as_str().unwrap_or_default().to_owned(),
            runtime: text(&meta["runtime"]),
            model: text(&meta["model"]).filter(|m| m != "account-default"),
            mode: text(&meta["mode"]),
            sandbox: text(&meta["sandbox"]),
            started_by: meta["startedBy"].clone(),
            cwd: text(&meta["command"]["cwd"]),
            events: events.into_iter().map(|(_, e)| e).collect(),
            updated,
        })
    }

    /// The terminal event's word: completed, failed or cancelled; otherwise the turn is unfinished.
    pub fn phase(&self) -> &'static str {
        match self.events.iter().rev().find_map(|e| e["type"].as_str().filter(|t| t.starts_with("run.") && *t != "run.started")) {
            Some("run.completed") => "completed",
            Some("run.failed") => "failed",
            Some("run.cancelled") => "cancelled",
            _ => "interrupted",
        }
    }

    /// Started by another app through the door.
    pub fn from_app(&self) -> bool {
        self.started_by["kind"] == "mcp"
    }
}

/// conversation-folder.ts: the folder a conversation an app (or a carry-on) started works in, when it named one.
pub fn conversation_folder(started_by: &Value) -> Option<String> {
    if started_by["kind"] != "mcp" && started_by["kind"] != "carry-on" {
        return None;
    }
    started_by["folder"].as_str().filter(|f| !f.is_empty()).map(str::to_owned)
}

/// What the door needs of Locust: the roster, the start path, the ledger readers and the person's switches. The live
/// one is `door::Engine`; the tests use their own.
pub trait Locust: Send + Sync {
    /// The roster, each with its saved mode in `runs_in` and the effort it runs at.
    fn teammates(&self) -> Result<Vec<(Teammate, Option<String>)>, String>;
    fn busy(&self, teammate: &str) -> bool;
    fn start(&self, request: StartRequest) -> Result<Begun, String>;
    /// One turn, read from its ledger.
    fn read(&self, mission: &str) -> Option<Turn>;
    /// The newest turn of the conversation a turn belongs to.
    fn newest(&self, mission: &str) -> Option<String>;
    fn owner(&self, mission: &str) -> Option<String>;
    /// A turn of this process is running there.
    fn live(&self, mission: &str) -> bool;
    /// A card of this turn is waiting in Locust's window.
    fn waiting(&self, mission: &str) -> bool;
    /// The window's own Stop.
    fn cancel(&self, mission: &str) -> bool;
    /// The newest turn of each recent conversation, newest first.
    fn conversations(&self) -> Vec<Turn>;
    /// A folder the app names: the full path it resolves to, or why it is refused (app-folder.ts).
    fn check_folder(&self, path: &str) -> Result<PathBuf, String>;
    /// The folder Locust has open.
    fn folder(&self) -> Option<String>;
    /// "Use each teammate's own mode": the app may then name a mode and folder.
    fn own_mode(&self) -> bool;
    /// "Let apps see your conversations".
    fn see_all(&self) -> bool;
    /// The agents a turn can be carried on to here.
    fn runtimes(&self) -> Vec<String>;
    /// Locust's own carry-on (main/carry-on.ts): the new turn's id and the folder it works in.
    fn carry_on(&self, mission: &str, runtime: &str, model: Option<&str>) -> Result<(String, Option<String>), String>;
    /// How a waiting read_reply sleeps between looks.
    fn pause(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
}

/// `text(value, max)`: a short string with something in it and no NUL.
fn text_arg(value: &Value, max: usize) -> Option<&str> {
    let s = value.as_str()?;
    (!s.trim().is_empty() && s.encode_utf16().count() <= max && !s.contains('\0')).then_some(s)
}

/// joinMessageFragments, for the answer: each message item's text, its appends joined, a replace starting it over. The
/// index of each item's last fragment is kept, so what came after the answer can be read.
fn messages(events: &[Value]) -> Vec<(String, String, bool, usize)> {
    let mut out: Vec<(String, String, bool, usize)> = Vec::new();
    for (index, e) in events.iter().enumerate() {
        if e["type"] != "message.delta" {
            continue;
        }
        let p = &e["payload"];
        let item = p["itemId"].as_str().unwrap_or_default().to_owned();
        let piece = p["text"].as_str().unwrap_or_default();
        let fin = p["final"] == true;
        match out.iter_mut().find(|m| m.0 == item) {
            Some(m) => {
                if p["operation"] == "replace" {
                    m.1 = piece.to_owned();
                } else {
                    m.1.push_str(piece);
                }
                m.2 = m.2 || fin;
                m.3 = index;
            }
            None => out.push((item, piece.to_owned(), fin, index)),
        }
    }
    out
}

/// protocolTags.ts unwrapProtocolTags: Locust's own tags come off a reply (their words stay), outside code.
pub fn unwrap_protocol_tags(text: &str) -> String {
    let code = crate::invariants::regex(r"```[\s\S]*?```|~~~[\s\S]*?~~~|`[^`\n]*`");
    let key = crate::invariants::regex(r#"(?i)<locust-key\s+name\s*=\s*"([^"]{1,80})"\s*>[\s\S]*?</locust-key>"#);
    // PROTOCOL_TAGS, each as a whole word.
    let loose = crate::invariants::regex(
        r"(?i)</?(?:locust-share|locust-memory|locust-ask|locust-task|locust-file|locust-tidy|locust-instructions|locust-key|locust-next)\b[^<>]{0,400}>",
    );
    let plain = |part: &str| -> String {
        let keyed = key.replace_all(part, |c: &regex::Captures| format!("Asks for the key {} (below).", c[1].trim()));
        loose.replace_all(&keyed, "").into_owned()
    };
    let mut out = String::new();
    let mut at = 0;
    for span in code.find_iter(text) {
        out.push_str(&plain(&text[at..span.start()]));
        out.push_str(span.as_str());
        at = span.end();
    }
    out.push_str(&plain(&text[at..]));
    let torn = crate::invariants::regex(r"(?i)\n?\s*</?(?:locu)[a-z-]*$");
    let out = torn.replace(&out, "").into_owned();
    let out = crate::invariants::regex(r"\n{3,}").replace_all(&out, "\n\n").into_owned();
    out.trim_start_matches('\n').trim_end().to_owned()
}

/// answerOf: a finished turn's answer, with warnings Locust gave after it, or Locust's words for why there is none.
pub fn answer_of(turn: &Turn) -> ToolResult {
    match turn.phase() {
        "failed" => {
            let said = turn
                .events
                .iter()
                .rev()
                .find(|e| e["type"] == "run.failed")
                .and_then(|e| e["payload"]["message"].as_str())
                .unwrap_or("The AI agent ended without an answer.");
            return ToolResult::refused(said);
        }
        "completed" => {}
        _ => return ToolResult::refused("The run was interrupted or stopped before it finished."),
    }
    let joined = messages(&turn.events);
    let Some((_, said, _, last)) = joined.iter().rev().find(|m| m.2) else {
        return ToolResult::ok("The AI agent finished without a text answer.");
    };
    let mut warned: Vec<&str> = Vec::new();
    for e in &turn.events[last + 1..] {
        let level = &e["payload"]["level"];
        if e["type"] == "adapter.diagnostic"
            && (level == "warning" || level == "error")
            && let Some(m) = e["payload"]["message"].as_str()
            && !warned.contains(&m)
        {
            warned.push(m);
        }
    }
    let answer = unwrap_protocol_tags(said);
    ToolResult::ok(if warned.is_empty() { answer } else { format!("{answer}\n\nLocust: {}", warned.join(" ")) })
}

/// Why a turn that ended can be carried on (main/carry-on.ts stoppedFor).
fn stopped_reason(turn: &Turn) -> Option<crate::carry_on::Reason> {
    match turn.phase() {
        "completed" | "cancelled" => None,
        _ => crate::carry_on::carry_on_reason(&turn.events),
    }
}

/// The door's tools, over a `Locust`.
pub struct Tools<L: Locust> {
    pub locust: L,
    /// Conversations a send_message is starting the next turn of, claimed before the first read.
    starting: Mutex<HashSet<String>>,
}

impl<L: Locust> Tools<L> {
    pub fn new(locust: L) -> Self {
        Self { locust, starting: Mutex::new(HashSet::new()) }
    }

    /// A conversation this app may read: one an app started, or any while the person lets apps see theirs. Then its
    /// newest turn.
    fn lookup(&self, id: &str) -> Option<Turn> {
        let root = self.locust.read(id)?;
        if !root.from_app() && !self.locust.see_all() {
            return None;
        }
        let newest = self.locust.newest(id).unwrap_or_else(|| id.to_owned());
        if newest == root.mission_id { Some(root) } else { self.locust.read(&newest) }
    }

    /// modeOf: Ask unless the person turned on each teammate's own mode; then a follow-up keeps its conversation's mode,
    /// and a new conversation takes the teammate's saved one.
    fn mode_of(&self, teammate: &Teammate, previous: Option<&Turn>) -> Mode {
        if !self.locust.own_mode() {
            return Mode::Ask;
        }
        if let Some(m) = previous.and_then(|p| p.mode.as_deref()).and_then(Mode::parse) {
            return m;
        }
        Mode::parse(&teammate.runs_in).unwrap_or(Mode::Ask)
    }

    fn turn_folder(&self, turn: &Turn) -> Option<String> {
        turn.cwd.clone().or_else(|| conversation_folder(&turn.started_by))
    }

    fn started(&self, teammate: Teammate, prompt: &str, conversation: Option<&str>, args: &Value, previous: Option<&Turn>) -> ToolResult {
        // A model and effort for one turn need the window's model list, which the native engine does not have yet.
        if !args["model"].is_null() || !args["effort"].is_null() {
            return ToolResult::refused(
                "This server does not take a model or effort. Leave them out to use the teammate’s own. Nothing was started.",
            );
        }
        let named = !args["mode"].is_null() || !args["folder"].is_null();
        if named && !self.locust.own_mode() {
            return ToolResult::refused(
                "Choosing the mode or folder is off: the person can turn on \"Use each teammate's own mode\" in Locust's Settings. Leave mode and folder out to run in Ask. Nothing was started.",
            );
        }
        if !args["mode"].is_null() && !args["mode"].as_str().is_some_and(|m| MODES.contains(&m)) {
            return ToolResult::refused(format!("mode is one of: {}. Nothing was started.", MODES.join(", ")));
        }
        let mut folder = None;
        if !args["folder"].is_null() {
            let Some(asked) = text_arg(&args["folder"], 1000) else {
                return ToolResult::refused("folder is a full path to a folder on this computer. Nothing was started.");
            };
            match self.locust.check_folder(asked) {
                Ok(path) => folder = Some(path),
                Err(why) => return ToolResult::refused(format!("{why} Nothing was started.")),
            }
        } else if let Some(was) = previous.and_then(|p| conversation_folder(&p.started_by)) {
            // A follow-up stays in the folder its conversation was started in, checked again: it may be gone.
            match self.locust.check_folder(&was) {
                Ok(path) => folder = Some(path),
                Err(why) => {
                    return ToolResult::refused(format!(
                        "This conversation worked in {was}: {why} Name another folder. Nothing was started."
                    ));
                }
            }
        }
        // An app may always step a turn down to Ask.
        let mode = if args["read_only"] == true {
            Mode::Ask
        } else if let Some(m) = args["mode"].as_str().and_then(Mode::parse) {
            m
        } else {
            self.mode_of(&teammate, previous)
        };
        let request = StartRequest {
            teammate,
            prompt: prompt.to_owned(),
            follow_up_of: previous.map(|p| p.mission_id.clone()),
            mode,
            folder: folder.clone(),
            runtime: previous.and_then(|p| p.runtime.clone()),
            model: previous.and_then(|p| p.model.clone()),
        };
        let begun = match self.locust.start(request) {
            Ok(b) => b,
            Err(why) => return ToolResult::refused(why),
        };
        let shown = self
            .locust
            .read(&begun.mission_id)
            .and_then(|t| self.turn_folder(&t))
            .or_else(|| folder.map(|f| f.to_string_lossy().into_owned()))
            .or_else(|| self.locust.folder());
        // What ran, not what was asked: the host decides the mode a turn really runs in.
        let mut reply = json!({"conversation_id": conversation.unwrap_or(&begun.mission_id)});
        if let Some(r) = &begun.runtime {
            reply["runtime"] = json!(r);
        }
        reply["mode"] = json!(begun.mode);
        if begun.mode != mode.as_str() {
            reply["mode_asked"] = json!(mode.as_str());
        }
        if let Some(m) = &begun.model {
            reply["model"] = json!(m);
        }
        if let Some(f) = shown {
            reply["folder"] = json!(f);
        }
        ToolResult::ok(reply.to_string())
    }

    fn with_status(&self, mut result: ToolResult, turn: &Turn) -> ToolResult {
        let live = self.locust.live(&turn.mission_id);
        let status = if live {
            if self.locust.waiting(&turn.mission_id) { "waiting_for_person" } else { "working" }
        } else {
            match turn.phase() {
                "completed" => "done",
                "failed" => "failed",
                _ => "stopped",
            }
        };
        result.structured = Some(json!({
            "status": status,
            "runtime": turn.runtime,
            "model": turn.model,
            "folder": self.turn_folder(turn).or_else(|| self.locust.folder()),
            "can_carry_on": !live && stopped_reason(turn).is_some()
        }));
        result
    }

    fn find_teammate(&self, given: &str) -> Result<Teammate, ToolResult> {
        let list = self.locust.teammates().map_err(ToolResult::refused)?;
        let lower = given.to_lowercase();
        let matches: Vec<_> = list.into_iter().filter(|(t, _)| t.id == given || t.name.to_lowercase() == lower).collect();
        if matches.len() != 1 {
            return Err(ToolResult::refused(
                "That teammate could not be identified. Use its id from list_teammates. Nothing was started.",
            ));
        }
        Ok(matches.into_iter().next().map(|(t, _)| t).expect("one match"))
    }

    /// One tool call. Never panics on what an app sends; an unknown tool is said plainly.
    pub fn call(&self, name: &str, arguments: &Value) -> ToolResult {
        let empty = json!({});
        let args = if arguments.is_object() { arguments } else { &empty };
        match name {
            "list_teammates" => {
                let list = match self.locust.teammates() {
                    Ok(l) => l,
                    Err(why) => return ToolResult::refused(why),
                };
                let own = self.locust.own_mode();
                let open = self.locust.folder();
                let rows: Vec<Value> = list
                    .iter()
                    .map(|(t, effort)| {
                        let mut row = json!({"id": t.id, "name": t.name, "role": t.role});
                        if let Some(f) = t.folder.clone().or_else(|| open.clone()) {
                            row["folder"] = json!(f);
                        }
                        row["runtime"] = json!(t.runtime);
                        row["model"] = json!(t.model);
                        row["effort"] = json!(effort);
                        row["busy"] = json!(self.locust.busy(&t.id));
                        // What a turn started from here would use, so the other app is never surprised by an edit.
                        row["runs_in"] = json!(if own && !t.runs_in.is_empty() { t.runs_in.as_str() } else { "ask" });
                        row
                    })
                    .collect();
                ToolResult::ok(Value::Array(rows).to_string())
            }
            "list_background_runs" => ToolResult::ok("[]"),
            "start_conversation" => {
                let (Some(who), Some(message)) = (text_arg(&args["teammate"], 200), text_arg(&args["message"], 8000)) else {
                    return ToolResult::refused(
                        "Choose a teammate and give it a message of up to 8,000 characters. Nothing was started.",
                    );
                };
                match self.find_teammate(who) {
                    Ok(t) => self.started(t, message, None, args, None),
                    Err(refused) => refused,
                }
            }
            "list_models" => {
                // Every agent this Locust can run, for carry_on. Its models are not listed: there is no catalog yet.
                if args["teammate"].is_null() {
                    let rows: Vec<Value> = self.locust.runtimes().iter().map(|r| json!({"runtime": r, "models": []})).collect();
                    return ToolResult::ok(Value::Array(rows).to_string());
                }
                ToolResult::refused("This server does not offer a choice of model.")
            }
            "carry_on" => self.carry_on(args),
            "list_routines" | "routine_status" => ToolResult::refused("This Locust does not offer routines to other apps."),
            "run_routine" => ToolResult::refused("This Locust does not offer routines to other apps. Nothing was started."),
            "list_conversations" => {
                if !self.locust.see_all() {
                    return ToolResult::ok(
                        "Off: this app sees only the conversations it started. The person can turn on \"Let apps see your conversations\" in Locust's Settings.",
                    );
                }
                let names: Vec<(Teammate, Option<String>)> = self.locust.teammates().unwrap_or_default();
                let rows: Vec<Value> = self
                    .locust
                    .conversations()
                    .iter()
                    .map(|turn| {
                        let owner = self.locust.owner(&turn.mission_id);
                        let teammate = owner.and_then(|o| names.iter().find(|(t, _)| t.id == o).map(|(t, _)| t.name.clone()));
                        let live = self.locust.live(&turn.mission_id);
                        let stopped = if live { None } else { stopped_reason(turn) };
                        let state = if live {
                            if self.locust.waiting(&turn.mission_id) { "waiting for the person in Locust" } else { "working" }
                        } else {
                            match stopped {
                                Some(crate::carry_on::Reason::Plan) => "stopped: its plan ran out",
                                Some(crate::carry_on::Reason::Provider) => "stopped: its provider failed",
                                Some(crate::carry_on::Reason::Process) => "stopped: its process failed",
                                None => turn.phase(),
                            }
                        };
                        let mut row = json!({
                            "conversation_id": turn.mission_id,
                            "teammate": teammate,
                            "title": turn.prompt.chars().take(80).collect::<String>(),
                            "started_by": if turn.from_app() { "an app" } else { "the person" },
                            "runtime": turn.runtime, "model": turn.model, "state": state,
                            "folder": self.turn_folder(turn),
                            "updated": turn.updated
                        });
                        if stopped.is_some() {
                            row["can_carry_on"] = json!(true);
                        }
                        row
                    })
                    .collect();
                ToolResult::ok(Value::Array(rows).to_string())
            }
            "stop_turn" => {
                let Some(id) = text_arg(&args["conversation_id"], 200) else {
                    return ToolResult::refused("Give a conversation id returned by start_conversation.");
                };
                let Some(turn) = self.lookup(id) else {
                    return ToolResult::refused(
                        "That conversation could not be read from the local ledger. Use an id returned by this server.",
                    );
                };
                // Only a turn an app started: the person may have gone on in Locust's window.
                if !turn.from_app() && !self.locust.see_all() {
                    return ToolResult::refused(
                        "The newest turn there was started by the person in Locust. Only they can stop it. Nothing was stopped.",
                    );
                }
                if !self.locust.live(&turn.mission_id) {
                    return ToolResult::ok("Nothing is running in that conversation now. Nothing was stopped.");
                }
                if self.locust.cancel(&turn.mission_id) {
                    ToolResult::ok("Stopping it. What it already changed stays; the person can undo that turn in Locust.")
                } else {
                    ToolResult::refused("Locust could not stop it from here. The person can stop it in Locust.")
                }
            }
            "send_message" | "read_reply" => self.send_or_read(name, args),
            _ => ToolResult::refused("That tool is not offered by Locust."),
        }
    }

    /// carry_on: a conversation whose turn stopped on its plan, provider or process, on to another agent.
    fn carry_on(&self, args: &Value) -> ToolResult {
        let Some(id) = text_arg(&args["conversation_id"], 200) else {
            return ToolResult::refused("Give a conversation id from start_conversation or list_conversations.");
        };
        let Some(turn) = self.lookup(id) else {
            return ToolResult::refused(
                "That conversation could not be read from the local ledger. Use an id returned by this server.",
            );
        };
        if self.locust.live(&turn.mission_id) {
            return ToolResult::refused("It is still working, so there is nothing to carry on. Nothing was started.");
        }
        // The stopped turn's own mode, as its card carries it on, but never above what an app's turns may run in.
        let mode = turn
            .mode
            .clone()
            .unwrap_or_else(|| if turn.sandbox.as_deref() == Some("read-only") { "ask".into() } else { "accept-edits".into() });
        if mode != "ask" && !self.locust.own_mode() {
            return ToolResult::refused(format!(
                "That turn ran in {mode}, not Ask. An app carries on only an Ask turn unless the person has turned on \"Use each teammate's own mode\" in Locust; they can carry it on from its card there. Nothing was started."
            ));
        }
        let runtimes = self.locust.runtimes();
        let Some(runtime) = text_arg(&args["runtime"], 40).filter(|r| runtimes.iter().any(|known| known == r)) else {
            let named = if runtimes.is_empty() { "none".to_owned() } else { runtimes.join(", ") };
            return ToolResult::refused(format!(
                "Name an AI agent this Locust can run, as list_models with no teammate lists them: {named}. Nothing was started."
            ));
        };
        let model = text_arg(&args["model"], 200);
        match self.locust.carry_on(&turn.mission_id, runtime, model) {
            Err(why) => ToolResult::refused(format!("{why} Nothing was started.")),
            Ok((carried, folder)) => {
                let mut reply = json!({"conversation_id": carried, "runtime": runtime});
                if let Some(m) = model {
                    reply["model"] = json!(m);
                }
                reply["mode"] = json!(mode);
                if let Some(f) = self.locust.read(&carried).and_then(|t| self.turn_folder(&t)).or(folder) {
                    reply["folder"] = json!(f);
                }
                ToolResult::ok(reply.to_string())
            }
        }
    }

    fn send_or_read(&self, name: &str, args: &Value) -> ToolResult {
        let Some(id) = text_arg(&args["conversation_id"], 200) else {
            return ToolResult::refused("Give a conversation id returned by start_conversation.");
        };
        let sending = name == "send_message";
        {
            let mut starting = self.starting.lock().unwrap_or_else(|e| e.into_inner());
            if starting.contains(id) {
                let mut r = ToolResult::ok("Still working: starting the next turn.");
                r.structured = Some(json!({"status": "working", "runtime": null, "model": null, "folder": null, "can_carry_on": false}));
                return r;
            }
            // Claim before the first read: two sends must not both continue the same old turn.
            if sending {
                starting.insert(id.to_owned());
            }
        }
        let result = self.send_or_read_claimed(id, sending, args);
        if sending {
            self.starting.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
        }
        result
    }

    fn send_or_read_claimed(&self, id: &str, sending: bool, args: &Value) -> ToolResult {
        let unreadable =
            || ToolResult::refused("That conversation could not be read from the local ledger. Use an id returned by this server.");
        let Some(mut turn) = self.lookup(id) else {
            return unreadable();
        };
        if !sending && let Some(wait) = args["wait_seconds"].as_f64().filter(|w| w.is_finite()) {
            let looking = turn.mission_id.clone();
            let mut left = (wait.floor().max(0.0) as u64).min(WAIT_LIMIT_S) * 2;
            while left > 0 && self.locust.live(&looking) && !self.locust.waiting(&looking) {
                self.locust.pause(500);
                left -= 1;
            }
            match self.lookup(id) {
                Some(t) => turn = t,
                None => return unreadable(),
            }
        }
        if self.locust.live(&turn.mission_id) {
            // This app cannot answer a card: there is no tool for it, by design.
            if self.locust.waiting(&turn.mission_id) {
                return self.with_status(
                    ToolResult::ok("Waiting for the person to approve or deny an action in Locust's window. This app cannot answer it; read_reply again later."),
                    &turn,
                );
            }
            let last = turn.events.iter().rev().find(|e| e["type"] == "tool.started" || e["type"] == "step.started");
            let doing = last.and_then(|e| {
                let p = &e["payload"];
                if e["type"] == "tool.started" {
                    p["title"].as_str().map(str::to_owned).or_else(|| p["name"].as_str().map(|n| format!("Using {n}")))
                } else {
                    p["message"].as_str().map(str::to_owned)
                }
            });
            let words = match doing.filter(|d| !d.is_empty()) {
                Some(d) => format!("Still working: {d}"),
                None => "Still working: waiting for the AI agent to answer.".into(),
            };
            return self.with_status(ToolResult::ok(words), &turn);
        }
        if sending {
            let Some(message) = text_arg(&args["message"], 8000) else {
                return ToolResult::refused("Give a message of up to 8,000 characters. Nothing was sent.");
            };
            let Some(owner) = self.locust.owner(&turn.mission_id) else {
                return ToolResult::refused("That conversation no longer has a teammate. Nothing was sent.");
            };
            let teammate = match self.locust.teammates() {
                Ok(list) => list.into_iter().find(|(t, _)| t.id == owner).map(|(t, _)| t),
                Err(why) => return ToolResult::refused(why),
            };
            let Some(teammate) = teammate else {
                return ToolResult::refused("That conversation no longer has a teammate. Nothing was sent.");
            };
            return self.started(teammate, message, Some(id), args, Some(&turn));
        }
        self.with_status(answer_of(&turn), &turn)
    }
}

#[cfg(test)]
#[path = "door_tools_tests.rs"]
mod tests;
