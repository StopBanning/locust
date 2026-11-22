//! THE EDITOR'S SESSIONS (locust-cli-sessions.ts): the `locust` command's and an editor's private API on the door's own
//! loopback listener. Its `cli/*` names never appear in the MCP bridge's tool list, and the bridge refuses them.
//!
//! A random, process-local capability owns one new conversation; neither a cancel nor a decision accepts a run or
//! conversation the client names. A conversation the window, the phone or another app started cannot be adopted. The
//! editor may answer only a waiting action of its own turn, once, through the window's own decision path, and only
//! while the person has turned on "Let editors answer cards for their own turns" (0.790). Any app connected to Locust
//! holds the key that opens a session, so until then every card waits in the window, as it always has.
//!
//! Each session holds a lease (30 s, renewed by every call it makes), a start deadline, and the turn's message text,
//! freed as the client acknowledges it, with at most 2 MiB not yet read. The timers are checked at every call and by a
//! sweeper once a second, so an editor that went away stops its turn without calling again.

use crate::door_tools::ToolResult;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

pub const LEASE_MS: u64 = 30_000;
pub const START_TIMEOUT_MS: u64 = 120_000;
const OUTPUT_CAP: usize = 2 * 1024 * 1024;
const EVENT_CAP: usize = 8_000;
const SESSION_CAP: usize = 128;
const TURN_CAP: usize = 1_024;
const CANCEL_CAP: usize = 100;
const ID_LIMIT: usize = 100;
const MESSAGE_LIMIT: usize = 8_000;

const NOT_CONNECTED: &str = "This editor session is no longer connected to Locust. Create a new session.";

/// A teammate as the editor sees it (`CliTeammate`): `runs_in` is the mode a turn started from here runs in.
#[derive(Debug, Clone, PartialEq)]
pub struct CliTeammate {
    pub id: String,
    pub name: String,
    pub runtime: Option<String>,
    pub model: Option<String>,
    pub runs_in: String,
}

impl CliTeammate {
    fn to_json(&self) -> Value {
        json!({"id": self.id, "name": self.name, "runtime": self.runtime, "model": self.model, "runs_in": self.runs_in})
    }
}

/// What a session asks Locust to start: a new conversation, or the session's own previous turn continued. The route
/// is always the teammate's saved one (`keepSavedRoute`); nothing the client sends changes it.
#[derive(Debug, Clone, PartialEq)]
pub struct CliStart {
    pub prompt: String,
    pub teammate_id: String,
    pub mode: String,
    pub follow_up_of: Option<String>,
}

/// A started turn's ids, as the start path returns them.
#[derive(Debug, Clone, PartialEq)]
pub struct CliBegun {
    pub mission_id: String,
    pub run_id: String,
}

/// What a running turn reports, while it runs.
#[derive(Debug, Clone, PartialEq)]
pub enum CliUpdate {
    /// The turn's ids, as soon as they are known.
    Run { mission_id: String, run_id: String },
    /// One runtime event: `{type, payload}`.
    Event(Value),
    /// The run's transport or its records failed.
    Error(String),
    /// Nothing new: only asks whether anyone still listens.
    Ping,
}

/// A card waiting in the window (`MissionApprovalRequest`).
#[derive(Debug, Clone, PartialEq)]
pub struct CliCard {
    pub approval_id: String,
    pub mission_id: String,
    pub run_id: String,
    pub kind: String,
    pub summary: String,
    pub detail: String,
    pub cwd: Option<String>,
    /// Where the card has to be answered when Locust cannot answer it itself.
    pub answer_in: Option<String>,
}

/// Feeds a running turn's updates in. Answers false once nobody listens any more (the session closed, or the turn
/// ended), so the one feeding it may stop.
pub type CliSink = Arc<dyn Fn(CliUpdate) -> bool + Send + Sync>;

/// What the sessions need of Locust. The live one is in `door.rs`; the tests use their own.
pub trait CliHost: Send + Sync {
    fn teammates(&self) -> Result<Vec<CliTeammate>, String>;
    /// The folder open in Locust, when one is chosen.
    fn workspace(&self) -> Option<String>;
    /// Start a turn through the native engine. May block; `update` is fed while the turn runs.
    fn start(&self, request: CliStart, update: CliSink) -> Result<CliBegun, String>;
    /// The window's own Stop.
    fn cancel(&self, run_id: &str);
    fn approvals(&self) -> Vec<CliCard>;
    /// The window's own guarded decision: approve once, or deny.
    fn answer(&self, approval_id: &str, approve: bool) -> bool;
    /// "Let editors answer cards for their own turns", read at every poll and decision.
    fn editor_answers(&self) -> bool;
    /// A session opened: from now on Settings shows that switch.
    fn opened(&self) {}
    fn status(&self) -> Result<Value, String>;
}

/// The clock, in milliseconds, and the two timers. The tests move their own clock.
pub struct CliOptions {
    pub now: Arc<dyn Fn() -> u64 + Send + Sync>,
    pub lease_ms: u64,
    pub start_timeout_ms: u64,
}

impl Default for CliOptions {
    fn default() -> Self {
        let started = Instant::now();
        Self { now: Arc::new(move || started.elapsed().as_millis() as u64), lease_ms: LEASE_MS, start_timeout_ms: START_TIMEOUT_MS }
    }
}

#[derive(Debug, Clone)]
struct Text {
    sequence: u64,
    item_id: String,
    operation: String,
    text: String,
}

#[derive(Debug)]
struct TurnState {
    request_id: String,
    phase: &'static str,
    mission_id: Option<String>,
    run_id: Option<String>,
    message: Option<String>,
    starting: bool,
    stopped: bool,
    finished: bool,
    events: Vec<Text>,
    sequence: u64,
    bytes: usize,
    acknowledged: u64,
    deadline: u64,
}

impl TurnState {
    fn terminal(&self) -> bool {
        !self.starting && self.finished
    }

    fn to_json(&self) -> Value {
        let mut out = json!({"requestId": self.request_id, "phase": self.phase});
        if let Some(m) = &self.mission_id {
            out["missionId"] = json!(m);
        }
        if let Some(r) = &self.run_id {
            out["runId"] = json!(r);
        }
        if let Some(m) = &self.message {
            out["message"] = json!(m);
        }
        out
    }

    /// The session's own cancel: the run to stop, when it has one.
    fn stop(&mut self) -> Option<String> {
        if self.terminal() {
            return None;
        }
        self.stopped = true;
        self.phase = "cancelled";
        self.run_id.clone()
    }
}

type Turn = Arc<Mutex<TurnState>>;

struct SessionState {
    cancelled_requests: HashSet<String>,
    /// Each accepted request id, with its message's digest.
    requests: HashMap<String, String>,
    expires: u64,
    closed: bool,
    mission_id: Option<String>,
    turn: Option<Turn>,
}

struct Session {
    id: String,
    key: String,
    teammate: CliTeammate,
    workspace: String,
    state: Mutex<SessionState>,
}

struct Shared {
    host: Arc<dyn CliHost>,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    disposed: AtomicBool,
    options: CliOptions,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn ok_json(value: Value) -> ToolResult {
    ToolResult::ok(value.to_string())
}

/// path.resolve, lexically: `.` and `..` folded, as Electron compares the editor's folder with Locust's.
fn normalized(path: &str) -> String {
    use std::path::Component;
    let absolute = std::path::absolute(path).unwrap_or_else(|_| std::path::PathBuf::from(path));
    let mut out = std::path::PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    let text = out.to_string_lossy().into_owned();
    if cfg!(windows) { text.to_lowercase() } else { text }
}

fn is_key(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The sessions on one door.
pub struct CliSessions {
    shared: Arc<Shared>,
}

impl CliSessions {
    /// Sessions under the given clock and timers, with no sweeper: the tests call [`CliSessions::sweep`].
    pub fn new(host: Arc<dyn CliHost>, options: CliOptions) -> Self {
        Self { shared: Arc::new(Shared { host, sessions: Mutex::new(HashMap::new()), disposed: AtomicBool::new(false), options }) }
    }

    /// The live sessions: real time, and a sweeper that ends an abandoned editor's turn.
    pub fn live(host: Arc<dyn CliHost>) -> Self {
        let sessions = Self::new(host, CliOptions::default());
        let weak: Weak<Shared> = Arc::downgrade(&sessions.shared);
        let every = Duration::from_millis(sessions.shared.options.lease_ms.clamp(1, 1_000));
        let _ = std::thread::Builder::new().name("locust-editor-sessions".into()).spawn(move || {
            loop {
                std::thread::sleep(every);
                let Some(shared) = weak.upgrade() else { return };
                if shared.disposed.load(Ordering::SeqCst) {
                    return;
                }
                expire(&shared);
            }
        });
        sessions
    }

    /// What the sweeper does: end every session whose lease ran out, and fail every start past its deadline.
    pub fn sweep(&self) {
        expire(&self.shared);
    }

    /// Locust is closing: every session closes, and only their own work is stopped, a late start included.
    pub fn dispose(&self) {
        self.shared.disposed.store(true, Ordering::SeqCst);
        let all: Vec<Arc<Session>> = lock(&self.shared.sessions).drain().map(|(_, s)| s).collect();
        let cancels: Vec<String> = all.iter().filter_map(|s| close(s)).collect();
        for run in cancels {
            self.shared.host.cancel(&run);
        }
    }

    /// One `cli/*` call: its name and arguments in, the MCP result out.
    pub fn call(&self, name: &str, input: &Value) -> ToolResult {
        let shared = &self.shared;
        expire(shared);
        if shared.disposed.load(Ordering::SeqCst) {
            return ToolResult::refused("Locust is closing. Open it again before sending.");
        }
        let empty = json!({});
        let args = if input.is_object() { input } else { &empty };
        match name {
            "cli/status" => match shared.host.status() {
                Ok(status) => ok_json(status),
                Err(why) => ToolResult::refused(why),
            },
            "cli/open" => self.open(args),
            _ => {
                let Some(session) = self.owned(args) else {
                    return ToolResult::refused(NOT_CONNECTED);
                };
                match name {
                    "cli/close" => {
                        lock(&shared.sessions).remove(&session.id);
                        if let Some(run) = close(&session) {
                            shared.host.cancel(&run);
                        }
                        ToolResult::ok("{\"ok\":true}")
                    }
                    "cli/keepalive" => ToolResult::ok("{\"ok\":true}"),
                    "cli/cancel" => self.cancel(&session, args),
                    "cli/decide" => self.decide_own_card(&session, args),
                    "cli/prompt" => self.prompt(&session, args),
                    "cli/poll" => self.poll(&session, args),
                    _ => ToolResult::refused("That command is not offered by Locust. Update the locust command and the app together."),
                }
            }
        }
    }

    fn open(&self, args: &Value) -> ToolResult {
        let shared = &self.shared;
        if lock(&shared.sessions).len() >= SESSION_CAP {
            return ToolResult::refused("Too many editor sessions are open. Close an editor session and try again.");
        }
        let Some(workspace) = shared.host.workspace() else {
            return ToolResult::refused("Choose a folder in Locust before sending a message.");
        };
        if let Some(cwd) = args.get("cwd")
            && !cwd.as_str().is_some_and(|c| normalized(c) == normalized(&workspace))
        {
            return ToolResult::refused(
                "The editor’s folder is not the folder open in Locust. Open that folder in Locust, then create a new editor session.",
            );
        }
        let team = match shared.host.teammates() {
            Ok(team) => team,
            Err(why) => return ToolResult::refused(why),
        };
        if shared.disposed.load(Ordering::SeqCst) {
            return ToolResult::refused("Locust is closing. Nothing was started.");
        }
        let matches: Vec<&CliTeammate> = match args.get("teammate") {
            None => team.iter().take(1).collect(),
            Some(given) => team
                .iter()
                .filter(|t| given.as_str().is_some_and(|g| g == t.id || g.to_lowercase() == t.name.to_lowercase()))
                .collect(),
        };
        if matches.len() != 1 {
            let listed: Vec<String> = team.iter().map(|t| format!("{} ({})", t.name, t.id)).collect();
            let listed = if listed.is_empty() { "add a teammate in Locust first".to_owned() } else { listed.join(", ") };
            return ToolResult::refused(format!("That teammate could not be identified. Choose one from locust teammates: {listed}. Nothing was started."));
        }
        let (Ok(id), Ok(key)) = (crate::door::random_hex(16), crate::door::random_hex(32)) else {
            return ToolResult::refused("Locust could not complete that request. Nothing was retried.");
        };
        // A random UUID's shape (randomUUID), version 4.
        let id = format!("{}-{}-4{}-{}{}-{}", &id[0..8], &id[8..12], &id[13..16], ["8", "9", "a", "b"][usize::from(id.as_bytes()[16] % 4)], &id[17..20], &id[20..32]);
        let session = Arc::new(Session {
            id: id.clone(),
            key: key.clone(),
            teammate: matches[0].clone(),
            workspace: workspace.clone(),
            state: Mutex::new(SessionState {
                cancelled_requests: HashSet::new(),
                requests: HashMap::new(),
                expires: (shared.options.now)() + shared.options.lease_ms,
                closed: false,
                mission_id: None,
                turn: None,
            }),
        });
        {
            let mut sessions = lock(&shared.sessions);
            if sessions.len() >= SESSION_CAP {
                return ToolResult::refused("Too many editor sessions are open. Close a session and try again.");
            }
            sessions.insert(id.clone(), session.clone());
        }
        shared.host.opened();
        ok_json(json!({"sessionId": id, "key": key, "teammate": session.teammate.to_json(), "workspace": workspace}))
    }

    /// The session these arguments hold the capability of, its lease renewed.
    fn owned(&self, args: &Value) -> Option<Arc<Session>> {
        let session = args["sessionId"].as_str().and_then(|id| lock(&self.shared.sessions).get(id).cloned())?;
        let key = args["key"].as_str().filter(|k| is_key(k))?;
        if !crate::door::same(key, &session.key) {
            return None;
        }
        lock(&session.state).expires = (self.shared.options.now)() + self.shared.options.lease_ms;
        Some(session)
    }

    fn cancel(&self, session: &Arc<Session>, args: &Value) -> ToolResult {
        let request = args["requestId"].as_str().filter(|r| r.encode_utf16().count() <= ID_LIMIT);
        let mut cancel = None;
        let mut too_many = false;
        {
            let mut state = lock(&session.state);
            let current = state.turn.as_ref().map(|t| lock(t).request_id.clone());
            match request.filter(|r| current.as_deref() != Some(*r)) {
                Some(r) => {
                    state.cancelled_requests.insert(r.to_owned());
                    too_many = state.cancelled_requests.len() > CANCEL_CAP;
                }
                None => cancel = state.turn.as_ref().and_then(|t| lock(t).stop()),
            }
        }
        if too_many {
            lock(&self.shared.sessions).remove(&session.id);
            cancel = close(session);
        }
        if let Some(run) = cancel {
            self.shared.host.cancel(&run);
        }
        ToolResult::ok("{\"ok\":true}")
    }

    /// The cards waiting for this session's current turn, and whether each must be answered in Locust.
    fn pending(&self, session: &Session) -> Vec<(CliCard, bool)> {
        let ids = {
            let state = lock(&session.state);
            state.turn.as_ref().and_then(|t| {
                let t = lock(t);
                if t.stopped || t.terminal() {
                    return None;
                }
                Some((t.mission_id.clone()?, t.run_id.clone()?))
            })
        };
        let Some((mission, run)) = ids else { return Vec::new() };
        let editors = self.shared.host.editor_answers();
        self.shared
            .host
            .approvals()
            .into_iter()
            .filter(|a| a.mission_id == mission && a.run_id == run)
            .map(|a| {
                let in_locust = a.kind == "question" || a.answer_in.is_some() || !editors;
                (a, in_locust)
            })
            .collect()
    }

    fn decide_own_card(&self, session: &Arc<Session>, args: &Value) -> ToolResult {
        if !self.shared.host.editor_answers() {
            return ToolResult::refused(
                "Answering cards from the editor is off, so this one waits in Locust. To answer them in the editor, turn on Let editors answer cards for their own turns in Locust’s Settings.",
            );
        }
        let card = self.pending(session).into_iter().find(|(a, _)| args["approvalId"].as_str() == Some(a.approval_id.as_str()));
        let (Some((card, false)), Some(approve)) = (card, args["approve"].as_bool()) else {
            return ToolResult::refused("That action is not waiting for this editor. Answer questions in Locust.");
        };
        let ok = self.shared.host.answer(&card.approval_id, approve);
        ok_json(json!({"ok": ok}))
    }

    fn prompt(&self, session: &Arc<Session>, args: &Value) -> ToolResult {
        let message = args["message"].as_str().filter(|m| {
            !m.trim().is_empty() && m.encode_utf16().count() <= MESSAGE_LIMIT && !m.contains('\0')
        });
        let request = args["requestId"].as_str().filter(|r| r.encode_utf16().count() <= ID_LIMIT);
        let (Some(message), Some(request)) = (message, request) else {
            return ToolResult::refused("Write a message of up to 8,000 characters. Nothing was sent.");
        };
        let digest: String = Sha256::digest(message.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        let turn = {
            let mut state = lock(&session.state);
            let previous = state.turn.clone();
            if let Some(seen) = state.requests.get(request) {
                let same_turn = previous.as_ref().is_some_and(|t| lock(t).request_id == request);
                return if same_turn && *seen == digest {
                    ToolResult::ok("{\"ok\":true}")
                } else {
                    ToolResult::refused("That request was already accepted. Check the conversation in Locust; it was not sent again.")
                };
            }
            if previous.as_ref().is_some_and(|t| !lock(t).terminal()) {
                return ToolResult::refused("This editor session is still working. Cancel it or wait for its reply.");
            }
            if state.requests.len() >= TURN_CAP {
                return ToolResult::refused("This editor session reached its turn limit. Create a new session in the editor.");
            }
            // Claimed here, before the roster lookup or the start: a cancel also owns a late start.
            let was_cancelled = state.cancelled_requests.remove(request);
            let turn: Turn = Arc::new(Mutex::new(TurnState {
                request_id: request.to_owned(),
                phase: if was_cancelled { "cancelled" } else { "starting" },
                mission_id: None,
                run_id: None,
                message: None,
                starting: !was_cancelled,
                stopped: was_cancelled,
                finished: was_cancelled,
                events: Vec::new(),
                sequence: 0,
                bytes: 0,
                acknowledged: 0,
                deadline: (self.shared.options.now)() + self.shared.options.start_timeout_ms,
            }));
            state.turn = Some(turn.clone());
            state.requests.insert(request.to_owned(), digest);
            (!was_cancelled).then_some(turn)
        };
        if let Some(turn) = turn {
            let (shared, session, message) = (self.shared.clone(), session.clone(), message.to_owned());
            let _ = std::thread::Builder::new().name("locust-editor-start".into()).spawn(move || start(&shared, &session, &turn, message));
        }
        ToolResult::ok("{\"ok\":true}")
    }

    fn poll(&self, session: &Arc<Session>, args: &Value) -> ToolResult {
        let invalid = || ToolResult::refused("The reply cursor is no longer valid. Read the conversation in Locust.");
        let Some(after) = args["after"].as_u64().filter(|a| *a <= (1u64 << 53) - 1) else {
            return invalid();
        };
        let turn = lock(&session.state).turn.clone();
        let (state, events, cursor) = match &turn {
            None => (None, Vec::new(), 0),
            Some(turn) => {
                let mut t = lock(turn);
                if after < t.acknowledged || after > t.sequence {
                    return invalid();
                }
                t.acknowledged = after;
                t.events.retain(|e| e.sequence > after);
                t.bytes = t.events.iter().map(|e| e.text.len()).sum();
                let events: Vec<Value> = t
                    .events
                    .iter()
                    .map(|e| json!({"sequence": e.sequence, "itemId": e.item_id, "operation": e.operation, "text": e.text}))
                    .collect();
                (Some(t.to_json()), events, t.sequence)
            }
        };
        let approvals: Vec<Value> = self
            .pending(session)
            .into_iter()
            .map(|(a, in_locust)| {
                let detail = if a.kind == "question" { String::new() } else { a.detail };
                json!({"approvalId": a.approval_id, "kind": a.kind, "summary": a.summary, "detail": detail, "cwd": a.cwd, "answerInLocust": in_locust})
            })
            .collect();
        let mut out = json!({});
        if let Some(state) = state {
            out["turn"] = state;
        }
        out["events"] = json!(events);
        out["cursor"] = json!(cursor);
        out["approvals"] = json!(approvals);
        ok_json(out)
    }
}

/// Close a session: its turn is stopped. Answers the run to stop. The caller has taken it out of the map.
fn close(session: &Session) -> Option<String> {
    let mut state = lock(&session.state);
    state.closed = true;
    state.turn.as_ref().and_then(|t| lock(t).stop())
}

/// End every session whose lease ran out, and fail every start past its deadline. Locust is asked to stop only after
/// every lock is let go.
fn expire(shared: &Shared) {
    let now = (shared.options.now)();
    let mut cancels = Vec::new();
    let expired: Vec<Arc<Session>> = {
        let mut sessions = lock(&shared.sessions);
        let gone: Vec<String> = sessions.iter().filter(|(_, s)| lock(&s.state).expires <= now).map(|(id, _)| id.clone()).collect();
        gone.iter().filter_map(|id| sessions.remove(id)).collect()
    };
    for session in &expired {
        cancels.extend(close(session));
    }
    let live: Vec<Arc<Session>> = lock(&shared.sessions).values().cloned().collect();
    for session in live {
        let Some(turn) = lock(&session.state).turn.clone() else { continue };
        let mut t = lock(&turn);
        if t.starting && !t.stopped && now >= t.deadline {
            t.stopped = true;
            t.phase = "failed";
            t.message = Some("The teammate did not finish starting in time. Check Locust before creating a new editor session; nothing was retried.".into());
            cancels.extend(t.run_id.clone());
        }
    }
    for run in cancels {
        shared.host.cancel(&run);
    }
}

/// The session's turn starts: the roster and folder checked again, then Locust's own start path.
fn start(shared: &Arc<Shared>, session: &Arc<Session>, turn: &Turn, message: String) {
    let host = shared.host.clone();
    let checked = host.teammates().and_then(|team| {
        let teammate = team
            .into_iter()
            .find(|t| t.id == session.teammate.id)
            .ok_or_else(|| "This teammate is no longer in Locust. Run locust teammates and choose another.".to_string())?;
        if host.workspace().as_deref() != Some(session.workspace.as_str()) {
            return Err("The folder open in Locust changed. Create a new editor session before sending.".to_string());
        }
        Ok(teammate)
    });
    let teammate = match checked {
        Ok(t) => t,
        Err(why) => {
            let mut t = lock(turn);
            t.starting = false;
            if t.run_id.is_none() {
                t.finished = true;
            }
            if !t.stopped {
                t.phase = "failed";
                t.message = Some(why);
            }
            return;
        }
    };
    let follow_up_of = {
        let state = lock(&session.state);
        let mut t = lock(turn);
        if t.stopped || state.closed || shared.disposed.load(Ordering::SeqCst) {
            t.starting = false;
            t.finished = true;
            return;
        }
        state.mission_id.clone()
    };
    let request = CliStart { prompt: message, teammate_id: teammate.id, mode: teammate.runs_in, follow_up_of };
    let result = host.start(request, sink(shared, session, turn));
    let cancel = {
        let mut state = lock(&session.state);
        let mut t = lock(turn);
        t.starting = false;
        match result {
            Err(why) => {
                t.finished = true;
                if !t.stopped {
                    t.phase = "failed";
                    t.message = Some(why);
                }
                None
            }
            Ok(begun) => {
                state.mission_id = Some(begun.mission_id.clone());
                t.mission_id = Some(begun.mission_id);
                t.run_id = Some(begun.run_id.clone());
                if !t.stopped && t.phase == "starting" {
                    t.phase = "running";
                }
                (t.stopped || state.closed || shared.disposed.load(Ordering::SeqCst)).then_some(begun.run_id)
            }
        }
    };
    if let Some(run) = cancel {
        host.cancel(&run);
    }
}

/// What a running turn's updates do to it: its ids, its message text (capped), and its end.
fn sink(shared: &Arc<Shared>, session: &Arc<Session>, turn: &Turn) -> CliSink {
    let (host, session, turn) = (Arc::downgrade(&shared.host), session.clone(), turn.clone());
    Arc::new(move |update: CliUpdate| {
        let mut cancels = Vec::new();
        let listening = {
            let state = lock(&session.state);
            if !state.turn.as_ref().is_some_and(|t| Arc::ptr_eq(t, &turn)) {
                return false;
            }
            let mut t = lock(&turn);
            match update {
                CliUpdate::Run { mission_id, run_id } => {
                    let first = t.run_id.is_none();
                    t.mission_id = Some(mission_id);
                    t.run_id = Some(run_id.clone());
                    if t.stopped && first {
                        cancels.push(run_id);
                    }
                }
                CliUpdate::Ping => {}
                CliUpdate::Error(why) => {
                    if !t.stopped && !state.closed {
                        t.phase = "failed";
                        t.message = Some(why);
                        cancels.extend(t.run_id.clone());
                    }
                }
                CliUpdate::Event(event) => {
                    let kind = event["type"].as_str().unwrap_or("");
                    if matches!(kind, "run.completed" | "run.cancelled" | "run.failed") {
                        t.finished = true;
                    }
                    if !t.stopped && !state.closed {
                        let payload = &event["payload"];
                        match kind {
                            "message.delta" => {
                                let text = payload["text"].as_str().unwrap_or("");
                                if t.bytes + text.len() > OUTPUT_CAP || t.events.len() >= EVENT_CAP {
                                    t.phase = "failed";
                                    t.message = Some("The editor did not keep up with the reply. Read it in Locust; nothing was resent.".into());
                                    t.stopped = true;
                                    cancels.extend(t.run_id.clone());
                                } else {
                                    t.bytes += text.len();
                                    t.sequence += 1;
                                    let item = Text {
                                        sequence: t.sequence,
                                        item_id: payload["itemId"].as_str().unwrap_or("").to_owned(),
                                        operation: if payload["operation"] == "replace" { "replace" } else { "append" }.to_owned(),
                                        text: text.to_owned(),
                                    };
                                    t.events.push(item);
                                }
                            }
                            "run.completed" => t.phase = "completed",
                            "run.cancelled" => t.phase = "cancelled",
                            "run.failed" => {
                                t.phase = "failed";
                                t.message = payload["message"].as_str().map(str::to_owned);
                            }
                            _ => {}
                        }
                    }
                }
            }
            !state.closed && !t.finished
        };
        if let Some(host) = host.upgrade() {
            for run in cancels {
                host.cancel(&run);
            }
        }
        listening
    })
}

#[cfg(test)]
#[path = "cli_sessions_tests.rs"]
mod cli_sessions_tests;
