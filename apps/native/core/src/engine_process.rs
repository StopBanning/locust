//! THE ENGINE AS A PROCESS (plan E2).
//!
//! `locust-native --engine` runs the engine with no window. It answers [`EngineApi`] on a Unix socket, `engine.sock`, in
//! the native data folder. The socket file is the owner's alone (0600). [`Client`] is the same trait spoken over that
//! socket, so E3 can make the window, Quick Ask and `--ask` use one engine instead of each keeping its own turns.
//!
//! - **One engine at a time.** The server holds an exclusive lock on `engine.lock` for as long as it runs. A second
//!   engine finds the lock taken and refuses to start. A stale socket left by a crash is replaced only by the process
//!   that holds the lock.
//! - **Only state crosses the socket.** Windows still follow the ledger files for streaming.
//! - **One request per connection.** One JSON line goes in and one comes out, so a half-read reply can never be
//!   mistaken for the next one. A send is never retried here: an error says what happened, and the caller decides.

use crate::{
    approval::{Decision, Kind, Mode, Request},
    approval_rules::RuleEffect,
    engine::{Started, Teammate},
    engine_api::{DoorChange, EngineApi, InProcess},
    native::{Origin, Send},
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub const SOCKET: &str = "engine.sock";
pub const LOCK: &str = "engine.lock";
/// Long enough for a shutdown to write every turn's ending (`native::shutdown_in_background` allows 8 s).
const REPLY_WAIT: Duration = Duration::from_secs(30);

// ---- the wire ---------------------------------------------------------------------------------------------------

fn text(v: &Value, key: &str) -> Option<String> {
    v[key].as_str().map(str::to_owned)
}

fn needed(v: &Value, key: &str) -> Result<String, String> {
    text(v, key).ok_or_else(|| format!("The engine's message has no {key}."))
}

fn teammate_json(t: &Teammate) -> Value {
    json!({"id": t.id, "name": t.name, "role": t.role, "runtime": t.runtime, "model": t.model, "account": t.account, "busy": t.busy,
        "runsIn": t.runs_in, "folder": t.folder})
}

fn teammate_from(v: &Value) -> Result<Teammate, String> {
    Ok(Teammate {
        id: needed(v, "id")?,
        name: needed(v, "name")?,
        role: text(v, "role").unwrap_or_default(),
        runtime: text(v, "runtime"),
        model: text(v, "model"),
        account: text(v, "account"),
        busy: v["busy"].as_bool().unwrap_or(false),
        runs_in: text(v, "runsIn").unwrap_or_default(),
        folder: text(v, "folder"),
    })
}

fn send_json(job: &Send) -> Value {
    json!({"teammate": teammate_json(&job.teammate), "conversation": job.conversation, "prompt": job.prompt,
        "model": job.model, "effort": job.effort})
}

fn send_from(v: &Value) -> Result<Send, String> {
    Ok(Send {
        teammate: teammate_from(&v["teammate"])?,
        conversation: text(v, "conversation"),
        prompt: needed(v, "prompt")?,
        model: text(v, "model"),
        effort: text(v, "effort"),
    })
}

pub(crate) fn origin_json(o: &Origin) -> Value {
    let mut obj = json!({"mode": o.mode.map(Mode::as_str), "folder": o.folder.as_ref().map(|f| f.to_string_lossy()),
        "startedBy": o.started_by, "runtime": o.runtime, "referenceIds": o.reference_ids});
    if let Some(acc) = &o.account {
        obj["accountOverride"] = json!(acc);
    }
    if !o.include_messages.is_empty() {
        obj["includeMessages"] = json!(o.include_messages);
    }
    if let Some(relay) = &o.relay {
        obj["relay"] = relay.clone();
    }
    if !o.left_out_by_you.is_empty() {
        obj["leftOutByYou"] = json!(o.left_out_by_you);
    }
    if let Some(hist) = &o.room_history {
        obj["roomHistory"] = json!({
            "roomName": hist.room_name,
            "posts": hist.posts.iter().map(|p| json!({
                "text": p.text,
                "to": p.to.iter().map(|r| json!({"teammateId": r.teammate_id, "name": r.name})).collect::<Vec<_>>(),
                "answers": p.answers.iter().map(|a| json!({"teammateId": a.teammate_id, "name": a.name, "text": a.text})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        });
    }
    if let Some(rewind) = &o.rewind {
        obj["rewind"] = json!({"tip": rewind.tip, "before": rewind.before, "setAside": rewind.set_aside});
    }
    if let Some(slot) = &o.slot {
        obj["slot"] = json!({
            "key": slot.key,
            "cwd": slot.cwd.as_ref().map(|p| p.to_string_lossy()),
            "instructions": slot.instructions,
            "sealed": slot.sealed,
        });
    }
    obj
}

fn room_history_from(v: &Value) -> Option<crate::room_history::RoomHistory> {
    let obj = v.as_object()?;
    let room_name = obj.get("roomName")?.as_str()?.to_owned();
    let posts = obj
        .get("posts")?
        .as_array()?
        .iter()
        .filter_map(|p| {
            let text = p.get("text")?.as_str()?.to_owned();
            let to = p
                .get("to")
                .and_then(|t| t.as_array())
                .into_iter()
                .flatten()
                .filter_map(|r| {
                    Some(crate::room_history::RoomHistoryRecipient {
                        teammate_id: r.get("teammateId")?.as_str()?.to_owned(),
                        name: r.get("name")?.as_str()?.to_owned(),
                    })
                })
                .collect();
            let answers = p
                .get("answers")
                .and_then(|a| a.as_array())
                .into_iter()
                .flatten()
                .filter_map(|a| {
                    Some(crate::room_history::RoomHistoryAnswer {
                        teammate_id: a.get("teammateId")?.as_str()?.to_owned(),
                        name: a.get("name")?.as_str()?.to_owned(),
                        text: a.get("text")?.as_str()?.to_owned(),
                    })
                })
                .collect();
            Some(crate::room_history::RoomHistoryPost { text, to, answers })
        })
        .collect();
    Some(crate::room_history::RoomHistory { room_name, posts })
}

pub(crate) fn origin_from(v: &Value) -> Result<Origin, String> {
    let mode = match text(v, "mode") {
        None => None,
        Some(m) => Some(Mode::parse(&m).ok_or_else(|| format!("The engine does not know the mode {m}."))?),
    };
    let account = v.get("accountOverride").map(|a| a.as_str().map(str::to_owned));
    let include_messages = v["includeMessages"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|i| i.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    let left_out_by_you = v["leftOutByYou"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|i| i.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    let rewind = v.get("rewind").filter(|r| r.is_object()).map(|r| crate::native::Rewind {
        tip: r["tip"].as_str().unwrap_or_default().to_owned(),
        before: r["before"].as_str().map(str::to_owned),
        set_aside: r["setAside"].as_array().map(|ids| ids.iter().filter_map(|i| i.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
    });
    let slot = v.get("slot").filter(|s| s.is_object()).and_then(|s| {
        Some(crate::native::SlotOrigin {
            key: s.get("key")?.as_str()?.to_owned(),
            cwd: text(s, "cwd").map(PathBuf::from),
            instructions: text(s, "instructions"),
            skill: None,
            sealed: s.get("sealed").and_then(Value::as_bool) == Some(true),
        })
    });
    Ok(Origin {
        mode,
        folder: text(v, "folder").map(PathBuf::from),
        started_by: Some(v["startedBy"].clone()).filter(|b| !b.is_null()),
        runtime: text(v, "runtime"),
        account,
        reference_ids: v["referenceIds"]
            .as_array()
            .map(|ids| ids.iter().filter_map(|i| i.as_str().map(str::to_owned)).collect()),
        slot,
        include_messages,
        relay: v.get("relay").cloned().filter(|r| !r.is_null()),
        left_out_by_you,
        room_history: v.get("roomHistory").and_then(room_history_from),
        rewind,
    })
}

fn decision_word(d: &Decision) -> &'static str {
    match d {
        Decision::ApproveOnce => "approve-once",
        Decision::ApproveAlways => "approve-always",
        Decision::Deny => "deny",
    }
}

fn decision_from(word: &str) -> Result<Decision, String> {
    match word {
        "approve-once" => Ok(Decision::ApproveOnce),
        "approve-always" => Ok(Decision::ApproveAlways),
        "deny" => Ok(Decision::Deny),
        other => Err(format!("The engine does not know the answer {other}.")),
    }
}

fn effect_word(e: &RuleEffect) -> &'static str {
    match e {
        RuleEffect::Allow => "allow",
        RuleEffect::Deny => "deny",
    }
}

fn effect_from(word: &str) -> Result<RuleEffect, String> {
    match word {
        "allow" => Ok(RuleEffect::Allow),
        "deny" => Ok(RuleEffect::Deny),
        other => Err(format!("The engine does not know the rule {other}.")),
    }
}

fn kind_from(word: &str) -> Result<Kind, String> {
    [Kind::Command, Kind::FileChange, Kind::Question, Kind::Connector]
        .into_iter()
        .find(|k| k.as_str() == word)
        .ok_or_else(|| format!("The engine does not know the card kind {word}."))
}

fn card_json(c: &Request) -> Value {
    json!({"approvalId": c.approval_id, "missionId": c.mission_id, "runtime": c.runtime, "kind": c.kind.as_str(),
        "summary": c.summary, "detail": c.detail, "cwd": c.cwd, "requestedAt": c.requested_at,
        "alwaysCovers": c.always_covers, "alwaysKey": c.always_key, "dataSentSays": c.data_sent_says,
        "reversibleSays": c.reversible_says, "claudeInput": c.claude_input,
        "questions": crate::approval::questions_json(&c.questions)})
}

fn card_from(v: &Value) -> Result<Request, String> {
    Ok(Request {
        approval_id: needed(v, "approvalId")?,
        mission_id: needed(v, "missionId")?,
        runtime: needed(v, "runtime")?,
        kind: kind_from(&needed(v, "kind")?)?,
        summary: text(v, "summary").unwrap_or_default(),
        detail: text(v, "detail").unwrap_or_default(),
        cwd: text(v, "cwd"),
        requested_at: text(v, "requestedAt").unwrap_or_default(),
        always_covers: text(v, "alwaysCovers"),
        always_key: text(v, "alwaysKey"),
        data_sent_says: text(v, "dataSentSays"),
        reversible_says: text(v, "reversibleSays"),
        claude_input: Some(v["claudeInput"].clone()).filter(|i| !i.is_null()),
        questions: crate::approval::questions_of(&json!({"questions": v["questions"]})),
    })
}

fn door_state_json(d: &crate::door::State) -> Value {
    json!({"settings": {"enabled": d.settings.enabled, "ownMode": d.settings.own_mode, "conversations": d.settings.conversations},
        "claudeCommand": d.claude_command, "codexConfig": d.codex_config, "message": d.message})
}

fn door_state_from(v: &Value) -> crate::door::State {
    crate::door::State {
        settings: crate::door::Settings {
            enabled: v["settings"]["enabled"].as_bool().unwrap_or(false),
            own_mode: v["settings"]["ownMode"].as_bool().unwrap_or(false),
            conversations: v["settings"]["conversations"].as_bool().unwrap_or(false),
        },
        claude_command: text(v, "claudeCommand"),
        codex_config: text(v, "codexConfig"),
        message: text(v, "message"),
    }
}

fn door_change_json(c: DoorChange) -> Value {
    match c {
        DoorChange::Load => json!({"change": "load"}),
        DoorChange::Read => json!({"change": "read"}),
        DoorChange::Enabled(on) => json!({"change": "enabled", "on": on}),
        DoorChange::OwnMode(on) => json!({"change": "own-mode", "on": on}),
        DoorChange::Conversations(on) => json!({"change": "conversations", "on": on}),
    }
}

fn door_change_from(v: &Value) -> Result<DoorChange, String> {
    let on = v["on"].as_bool().unwrap_or(false);
    match v["change"].as_str().unwrap_or("") {
        "load" => Ok(DoorChange::Load),
        "read" => Ok(DoorChange::Read),
        "enabled" => Ok(DoorChange::Enabled(on)),
        "own-mode" => Ok(DoorChange::OwnMode(on)),
        "conversations" => Ok(DoorChange::Conversations(on)),
        other => Err(format!("The engine does not know the door change {other}.")),
    }
}

/// The door for other apps, served by the engine process (E4), over the data folder its turns use. It keeps its own
/// switch, token and tools; none of its tools answers a card, so no app can.
static DOOR: std::sync::OnceLock<crate::door::Door> = std::sync::OnceLock::new();

fn engine_door() -> Result<&'static crate::door::Door, String> {
    if let Some(door) = DOOR.get() {
        return Ok(door);
    }
    let store = crate::native::store().ok_or("The native data folder could not be determined.")?;
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let _ = DOOR.set(crate::door::Door::for_store(store, &exe));
    DOOR.get().ok_or_else(|| "The door could not be made.".to_string())
}

fn change_door(change: DoorChange) -> Result<crate::door::State, String> {
    let door = engine_door()?;
    Ok(match change {
        DoorChange::Load => door.load(),
        DoorChange::Read => door.state(None),
        DoorChange::Enabled(on) => door.set_enabled(on),
        DoorChange::OwnMode(on) => door.set_own_mode(on),
        DoorChange::Conversations(on) => door.set_conversations(on),
    })
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_owned)).collect()
}

// ---- the server -------------------------------------------------------------------------------------------------

/// One request, answered by the engine in this process. Errors become `{"error": ...}`.
fn answer(engine: &dyn EngineApi, request: &Value) -> Value {
    let result: Result<Value, String> = (|| match request["op"].as_str().unwrap_or("") {
        "send" => {
            let started = engine.send(send_from(&request["job"])?, origin_from(&request["origin"])?)?;
            Ok(json!({"conversationId": started.conversation_id, "mode": started.mode}))
        }
        "stop" => engine.stop(&needed(request, "conversation")?).map(Value::from),
        "owns" => Ok(Value::from(engine.owns(&needed(request, "conversation")?))),
        "running" => Ok(json!(engine.running())),
        "running_checks" => Ok(json!(engine.running_checks())),
        "waiting" => Ok(Value::Array(engine.waiting().iter().map(card_json).collect())),
        "decide" => engine
            .decide(&needed(request, "approvalId")?, decision_from(&needed(request, "decision")?)?, text(request, "reason"))
            .map(|()| Value::Null),
        "answer" => {
            // Bounded as Electron bounds them: words a person typed, not a file.
            let answers: Vec<(String, Vec<String>)> = request["answers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|pair| {
                    let id = pair[0].as_str()?.to_owned();
                    let given = pair[1].as_array()?.iter().take(64).map(|g| g.as_str().map(|s| s.chars().take(4_000).collect::<String>())).collect::<Option<Vec<_>>>()?;
                    Some((id, given))
                })
                .collect();
            engine.answer(&needed(request, "approvalId")?, answers).map(|()| Value::Null)
        }
        "save-rule-and-decide" => engine
            .save_rule_and_decide(&card_from(&request["card"])?, effect_from(&needed(request, "effect")?)?)
            .map(|()| Value::Null),
        "carry" => {
            let (conversation, folder, mode) = engine.carry(
                &needed(request, "mission")?,
                &needed(request, "runtime")?,
                text(request, "model").as_deref(),
            )?;
            Ok(json!({"conversation": conversation, "folder": folder, "mode": mode}))
        }
        "shut-down" => {
            engine.shut_down();
            engine.wait_for_shutdown();
            // The door closes with the engine: its connection file goes, so no app reaches a Locust that is not there.
            if let Some(door) = DOOR.get() {
                door.dispose();
            }
            Ok(Value::Null)
        }
        "door" => change_door(door_change_from(&request["door"])?).map(|state| door_state_json(&state)),
        "room-notices" => {
            let list: Vec<Value> = engine
                .room_notices()
                .into_iter()
                .map(|n| json!({"roomId": n.room_id, "roomName": n.room_name, "message": n.message}))
                .collect();
            Ok(Value::Array(list))
        }
        "ping" => Ok(Value::from("engine")),
        other => Err(format!("The engine does not know the request {other}.")),
    })();
    match result {
        Ok(value) => json!({"ok": value}),
        Err(error) => json!({"error": error}),
    }
}

fn serve_one(engine: &dyn EngineApi, stream: UnixStream) {
    let _ = stream.set_read_timeout(Some(REPLY_WAIT));
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
        return;
    }
    let reply = match serde_json::from_str::<Value>(&line) {
        Ok(request) => answer(engine, &request),
        Err(_) => json!({"error": "The engine could not read that request."}),
    };
    let mut out = &stream;
    let _ = out.write_all(format!("{reply}\n").as_bytes());
    // Decision 3's default: quitting the window stops its turns. Once their endings are written and the window told,
    // `locust-native --engine` has nothing left to run, so it ends; the next window, Quick Ask or `--ask` starts one.
    if EXIT_ON_SHUTDOWN.load(Ordering::Relaxed) && request_was(&line, "shut-down") {
        std::process::exit(0);
    }
}

static EXIT_ON_SHUTDOWN: AtomicBool = AtomicBool::new(false);

fn request_was(line: &str, op: &str) -> bool {
    serde_json::from_str::<Value>(line).ok().is_some_and(|r| r["op"] == op)
}

/// A running engine: the lock it holds and the socket it answers on.
pub struct Server {
    socket: PathBuf,
    stopping: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    // Held for the server's life: dropping it lets another engine start.
    _lock: std::fs::File,
}

impl Server {
    /// Take the lock in `dir`, bind `engine.sock`, and answer on a thread. Refused while another engine holds the lock.
    pub fn start(dir: &Path) -> Result<Server, String> {
        Self::start_with(dir, Arc::new(InProcess))
    }

    fn start_with(dir: &Path, engine: Arc<dyn EngineApi>) -> Result<Server, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("The engine's folder could not be made: {e}"))?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(LOCK))
            .map_err(|e| format!("The engine's lock could not be opened: {e}"))?;
        // SAFETY: flock on a descriptor this function owns; it only reads the descriptor.
        let taken = unsafe { libc::flock(std::os::fd::AsRawFd::as_raw_fd(&lock), libc::LOCK_EX | libc::LOCK_NB) };
        if taken != 0 {
            return Err("Another Locust engine is already running.".into());
        }
        // A restore asked for at the last Restore and restart is applied here, holding the lock, before the engine reads
        // any store (profile-backup.ts: applied at the next start, before any store is read).
        let _ = crate::profile_backup::apply_pending_restore(dir, env!("CARGO_PKG_VERSION"), chrono::Local::now());
        let socket = dir.join(SOCKET);
        // Holding the lock, any socket file left here is a crashed engine's.
        let _ = std::fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket).map_err(|e| format!("The engine could not listen: {e}"))?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("The engine's socket could not be made private: {e}"))?;
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = stopping.clone();
        let thread = std::thread::Builder::new()
            .name("locust-engine".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Ok(stream) = stream {
                        let engine = engine.clone();
                        let _ = std::thread::Builder::new()
                            .name("locust-engine-call".into())
                            .spawn(move || serve_one(engine.as_ref(), stream));
                    }
                }
            })
            .map_err(|e| format!("The engine could not start: {e}"))?;
        Ok(Server { socket, stopping, thread: Some(thread), _lock: lock })
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Run until the process ends (`locust-native --engine`): until a shut-down has written every ending.
    pub fn run(mut self) {
        EXIT_ON_SHUTDOWN.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        // Wake the accept loop so it sees the flag.
        let _ = UnixStream::connect(&self.socket);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(&self.socket);
    }
}

// ---- the client -------------------------------------------------------------------------------------------------

/// [`EngineApi`] over the socket of an engine in another process.
/// How a client starts the engine again when it finds it gone.
pub type Restart = Arc<dyn Fn() -> Result<(), String> + std::marker::Send + Sync>;

/// After a restart was tried, how long before another may be.
const RESTART_PAUSE: Duration = Duration::from_secs(10);

pub struct Client {
    socket: PathBuf,
    shutting: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
    restart: Option<Restart>,
    last_restart: std::sync::Mutex<Option<std::time::Instant>>,
    restarted: AtomicBool,
}

impl Client {
    pub fn new(socket: PathBuf) -> Self {
        Client { socket, shutting: std::sync::Mutex::new(None), restart: None, last_restart: std::sync::Mutex::new(None), restarted: AtomicBool::new(false) }
    }

    /// A client that starts the engine again when it finds it gone (E5): at most once every ten seconds.
    pub fn restarting(socket: PathBuf, restart: Restart) -> Self {
        Client { restart: Some(restart), ..Client::new(socket) }
    }

    /// Does an engine answer on this socket?
    pub fn answers(&self) -> bool {
        self.call(json!({"op": "ping"})).ok().and_then(|v| v.as_str().map(|s| s == "engine")).unwrap_or(false)
    }

    /// A connection to the engine. When none answers and this client may restart it, start it once and connect again.
    /// That is no retry of a request: a request that found no engine never reached one, so nothing it asked happened.
    fn connect(&self) -> Result<UnixStream, String> {
        if let Ok(stream) = UnixStream::connect(&self.socket) {
            return Ok(stream);
        }
        let gone = || "Locust's engine is not running.".to_string();
        let Some(restart) = &self.restart else { return Err(gone()) };
        {
            let mut last = crate::invariants::lock(&self.last_restart);
            if last.is_some_and(|t| t.elapsed() < RESTART_PAUSE) {
                return Err(gone());
            }
            *last = Some(std::time::Instant::now());
        }
        restart()?;
        let stream = UnixStream::connect(&self.socket).map_err(|_| gone())?;
        self.restarted.store(true, Ordering::Relaxed);
        Ok(stream)
    }

    /// One request, one reply. Never retried: whether a send that failed midway started a turn is for the ledger to
    /// say, not for a second attempt to find out.
    fn call(&self, request: Value) -> Result<Value, String> {
        let stream = self.connect()?;
        let _ = stream.set_read_timeout(Some(REPLY_WAIT));
        let mut out = &stream;
        out.write_all(format!("{request}\n").as_bytes())
            .map_err(|e| format!("The request could not reach Locust's engine: {e}"))?;
        let mut line = String::new();
        BufReader::new(&stream)
            .read_line(&mut line)
            .map_err(|e| format!("Locust's engine did not answer: {e}"))?;
        let reply: Value =
            serde_json::from_str(&line).map_err(|_| "Locust's engine gave an answer that could not be read.".to_string())?;
        match reply.get("error").and_then(Value::as_str) {
            Some(error) => Err(error.to_owned()),
            None => Ok(reply["ok"].clone()),
        }
    }
}

impl EngineApi for Client {
    fn send(&self, job: Send, origin: Origin) -> Result<Started, String> {
        let v = self.call(json!({"op": "send", "job": send_json(&job), "origin": origin_json(&origin)}))?;
        Ok(Started { conversation_id: needed(&v, "conversationId")?, mode: text(&v, "mode").unwrap_or_default() })
    }
    fn stop(&self, conversation: &str) -> Result<String, String> {
        Ok(self.call(json!({"op": "stop", "conversation": conversation}))?.as_str().unwrap_or_default().to_owned())
    }
    fn owns(&self, conversation: &str) -> bool {
        self.call(json!({"op": "owns", "conversation": conversation})).ok().and_then(|v| v.as_bool()).unwrap_or(false)
    }
    fn running(&self) -> Vec<String> {
        self.call(json!({"op": "running"})).map(|v| strings(&v)).unwrap_or_default()
    }
    fn running_checks(&self) -> Vec<(String, String)> {
        self.call(json!({"op": "running_checks"}))
            .ok()
            .and_then(|v| {
                v.as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|pair| {
                            let a = pair.as_array()?;
                            Some((a.first()?.as_str()?.to_owned(), a.get(1)?.as_str()?.to_owned()))
                        })
                        .collect()
                })
            })
            .unwrap_or_default()
    }
    fn waiting(&self) -> Vec<Request> {
        self.call(json!({"op": "waiting"}))
            .map(|v| v.as_array().into_iter().flatten().filter_map(|c| card_from(c).ok()).collect())
            .unwrap_or_default()
    }
    fn decide(&self, approval_id: &str, decision: Decision, reason: Option<String>) -> Result<(), String> {
        self.call(json!({"op": "decide", "approvalId": approval_id, "decision": decision_word(&decision), "reason": reason}))
            .map(|_| ())
    }
    fn answer(&self, approval_id: &str, answers: Vec<(String, Vec<String>)>) -> Result<(), String> {
        let answers: Vec<Value> = answers.into_iter().map(|(id, given)| json!([id, given])).collect();
        self.call(json!({"op": "answer", "approvalId": approval_id, "answers": answers})).map(|_| ())
    }
    fn save_rule_and_decide(&self, card: &Request, effect: RuleEffect) -> Result<(), String> {
        self.call(json!({"op": "save-rule-and-decide", "card": card_json(card), "effect": effect_word(&effect)}))
            .map(|_| ())
    }
    fn carry(&self, mission: &str, runtime: &str, model: Option<&str>) -> Result<(String, Option<String>, String), String> {
        let v = self.call(json!({"op": "carry", "mission": mission, "runtime": runtime, "model": model}))?;
        Ok((needed(&v, "conversation")?, text(&v, "folder"), text(&v, "mode").unwrap_or_default()))
    }
    fn shut_down(&self) {
        // Asked from the UI thread as the window closes, which must not wait: the request goes on its own thread, and
        // its reply (once every ending is written) is what `wait_for_shutdown` waits for.
        let socket = self.socket.clone();
        let handle = std::thread::Builder::new()
            .name("locust-engine-shut-down".into())
            .spawn(move || {
                let _ = Client::new(socket).call(json!({"op": "shut-down"}));
            })
            .ok();
        *crate::invariants::lock(&self.shutting) = handle;
    }
    fn wait_for_shutdown(&self) {
        if let Some(handle) = crate::invariants::lock(&self.shutting).take() {
            let _ = handle.join();
        }
    }
    fn door(&self, change: DoorChange) -> Result<crate::door::State, String> {
        self.call(json!({"op": "door", "door": door_change_json(change)})).map(|v| door_state_from(&v))
    }
    fn notice(&self) -> Option<String> {
        self.restarted.swap(false, Ordering::Relaxed).then(|| {
            "Locust's engine had stopped, so it was started again. A turn it was running shows where it stopped.".to_string()
        })
    }
    fn room_notices(&self) -> Vec<crate::room_tasks::RoomNotice> {
        self.call(json!({"op": "room-notices"}))
            .map(|v| {
                v.as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|n| {
                        Some(crate::room_tasks::RoomNotice {
                            room_id: n.get("roomId")?.as_str()?.to_owned(),
                            room_name: n.get("roomName")?.as_str()?.to_owned(),
                            message: n.get("message")?.as_str()?.to_owned(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The engine for a window, Quick Ask or `--ask` (plan E3): the one answering in `dir`, else a new
/// `<exe> --engine` started there, waited for up to five seconds. Two callers starting at once both end up on the one
/// that won the lock.
pub fn join_or_start(dir: &Path, exe: &Path) -> Result<Arc<dyn EngineApi>, String> {
    let (dir_for, exe_for) = (dir.to_path_buf(), exe.to_path_buf());
    let restart: Restart = Arc::new(move || start_engine(&dir_for, &exe_for));
    let client = Client::restarting(dir.join(SOCKET), restart);
    if client.answers() {
        return Ok(Arc::new(client));
    }
    start_engine(dir, exe)?;
    // Started by joining, not found gone: nothing to tell the window.
    client.restarted.store(false, Ordering::Relaxed);
    Ok(Arc::new(client))
}

/// Start `<exe> --engine` in `dir` and wait up to five seconds for it to answer.
fn start_engine(dir: &Path, exe: &Path) -> Result<(), String> {
    let probe = Client::new(dir.join(SOCKET));
    if probe.answers() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("The engine's folder could not be made: {e}"))?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("engine.log"))
        .map_err(|e| format!("The engine's log could not be opened: {e}"))?;
    std::process::Command::new(exe)
        .arg("--engine")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(log)
        .spawn()
        .map_err(|e| format!("Locust's engine could not be started: {e}"))?;
    let end = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < end {
        if probe.answers() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("Locust's engine did not start in time.".into())
}

#[cfg(test)]
#[path = "engine_process_tests.rs"]
mod tests;
