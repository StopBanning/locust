//! Electron's `packages/runtime-adapters/test/opencode-serve-run.test.ts`, case by case: an OpenCode 1 turn through
//! its own server, through the caller. The command is the one the app builds (`command_for_version` for OpenCode
//! 1.18.27, in the teammate's folder, in the mode's config), started as the app starts it (`OpenCode::spawn`, `start`,
//! `next_event` to the end). Its program is a stand-in for `opencode`, in Python like Electron's `fakeServer`: it
//! answers `/session` with Electron's recorded session, streams Electron's own recorded turn
//! (`fixtures/opencode/serve-events.jsonl`, opencode 1.18.27) once the message is sent, and logs every request
//! (`calls.jsonl`, as `server.calls`) and how it was started (`spawn.json`: its arguments, folder and environment).
//! Like OpenCode's own server, and unlike Electron's stand-in, it streams nothing past a request it asked until that
//! request is answered: the native driver answers a request as it reads it, in order, and the recording's idle (the
//! end of a turn) comes only after the answer it waited on.
//!
//! Electron's `completion` is the run's last event's `process`, `server.killed` is the stand-in's process being gone
//! once the run ends, and `onPermission` is the person answering the card the run raises. Electron's `run.records` are
//! the driver's own records, which it keeps for tests (`Serve::records`: each record's type and part id).
use super::*;
use crate::approval::{Answer, Channel, Decision, Mode};
use crate::runtime::opencode_serve::OPENCODE_REFUSED_UNASKED;
use crate::store::Store;
use std::sync::Arc;
use serde_json::{Value, json};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const SESSION: &str = "ses_f27b5f958ffea6iVnmfRjD2imJ";

const SERVER: &str = r#"#!/usr/bin/env python3
import base64, json, os, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
HERE = os.path.dirname(os.path.abspath(__file__))
S = json.load(open(os.path.join(HERE, 'script.json')))
SESSION = 'ses_f27b5f958ffea6iVnmfRjD2imJ'
password = os.environ.get('OPENCODE_SERVER_PASSWORD', '')
auth = 'Basic ' + base64.b64encode(('opencode:' + password).encode()).decode()
json.dump({'argv': sys.argv[1:], 'cwd': os.getcwd(), 'password': password,
           'config': os.environ.get('OPENCODE_CONFIG_CONTENT')}, open(os.path.join(HERE, 'spawn.json'), 'w'))
open(os.path.join(HERE, 'pid'), 'w').write(str(os.getpid()))
prompted = threading.Event()
lock = threading.Lock()
replied = {}

def reply_to(id):
    with lock:
        return replied.setdefault(id, threading.Event())

def log(method, path, header, body):
    with lock, open(os.path.join(HERE, 'calls.jsonl'), 'a') as f:
        f.write(json.dumps({'method': method, 'path': path, 'auth': header, 'body': body}) + '\n')

class H(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def log_message(self, *a):
        pass
    def answer(self, value, code=200):
        data = b'' if value is None else json.dumps(value).encode()
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def do_GET(self):
        path = self.path.split('?')[0]
        log('GET', path, self.headers.get('Authorization'), None)
        if path == '/event':
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Connection', 'close')
            self.end_headers()
            self.wfile.flush()
            prompted.wait(30)
            time.sleep(S.get('delay', 0))
            for e in S['events']:
                self.wfile.write(('data: ' + e + '\n\n').encode())
                self.wfile.flush()
                # As OpenCode's own server does, the turn goes no further until a request it asked is answered.
                asked = json.loads(e)
                id = asked.get('properties', {}).get('id')
                if asked.get('type') == 'permission.asked' and id not in S.get('unanswered', []):
                    reply_to(id).wait(30)
            time.sleep(600)
            return
        self.answer(None, 404)
    def do_POST(self):
        path = self.path.split('?')[0]
        n = int(self.headers.get('Content-Length', '0'))
        body = json.loads(self.rfile.read(n)) if n else None
        log('POST', path, self.headers.get('Authorization'), body)
        if path == '/session':
            return self.answer({'id': SESSION})
        if path.endswith('/prompt_async'):
            prompted.set()
            return self.answer(None, 204)
        if path.endswith('/command'):
            prompted.set()
            return self.answer({'info': {}, 'parts': []})
        if '/permission/' in path:
            reply_to(path.split('/permission/')[1].split('/')[0]).set()
            return self.answer(True)
        self.answer(None, 404)

class Server(ThreadingHTTPServer):
    daemon_threads = True
    def handle_error(self, *a):
        pass

httpd = Server(('127.0.0.1', 0), H)
print('opencode server listening on http://127.0.0.1:%d' % httpd.server_address[1], flush=True)
httpd.serve_forever()
"#;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Electron's recorded turn, line by line.
fn recorded() -> Vec<String> {
    let path = format!(
        "{}/../../../packages/runtime-adapters/test/fixtures/opencode/serve-events.jsonl",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).unwrap().lines().filter(|l| !l.is_empty()).map(str::to_owned).collect()
}

/// Electron's `fakeServer(events)`: a stand-in `opencode` (in `bin/`) and the teammate's folder (`pebble/`).
struct Server {
    t: Temp,
    store: Store,
}
impl Server {
    fn new(events: &[String]) -> Self {
        Self::unanswered(events, &[])
    }
    /// A server whose stream does not wait for `unanswered`'s replies (requests that are not the run's).
    fn unanswered(events: &[String], unanswered: &[&str]) -> Self {
        let t = Temp(std::env::temp_dir().join(format!("locust-oc-serve-run-{}", crate::invariants::unique_nanos())));
        for dir in ["bin", "pebble"] {
            std::fs::create_dir_all(t.0.join(dir)).unwrap();
        }
        let exe = t.0.join("bin/opencode");
        std::fs::write(&exe, SERVER).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        // As a server's turn takes a moment (Electron's every-mode stand-in waits 30 ms before its events).
        let script = json!({"events": events, "unanswered": unanswered, "delay": 0.03});
        std::fs::write(t.0.join("bin/script.json"), script.to_string()).unwrap();
        let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
        std::fs::create_dir_all(store.electron()).unwrap();
        Self { t, store }
    }
    fn folder(&self) -> PathBuf {
        self.t.0.join("pebble")
    }
    /// `createOpenCodeServeCommand(EXECUTABLE, { workspacePath })`: the command the app builds for this mode.
    fn command(&self, mode: Mode) -> Command {
        command_for_version(self.t.0.join("bin/opencode"), "1.18.27", &self.folder(), mode, &self.store, false).unwrap()
    }
    fn calls(&self) -> Vec<Value> {
        std::fs::read_to_string(self.t.0.join("bin/calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
    /// The body of the first request whose path holds `part`.
    fn sent(&self, part: &str) -> Option<Value> {
        self.calls().into_iter().find(|c| c["path"].as_str().unwrap().contains(part)).map(|c| c["body"].clone())
    }
    fn spawned(&self) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.t.0.join("bin/spawn.json")).unwrap()).unwrap()
    }
    /// `server.killed`: the stand-in's process is gone.
    fn assert_killed(&self) {
        let pid = std::fs::read_to_string(self.t.0.join("bin/pid")).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while Path::new(&format!("/proc/{}", pid.trim())).exists() {
            assert!(Instant::now() < until, "the server was left running");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn turn(folder: &Path, prompt: &str, mode: Mode, approvals: Option<Arc<Channel>>) -> Turn {
    Turn {
        prompt: prompt.into(),
        folder: folder.to_path_buf(),
        model: None,
        effort: None,
        resume: None,
        mode,
        approvals,
        mission_id: format!("oc_serve_run_{}", crate::invariants::unique_nanos()),
        run_id: "opencode-serve-run-test".into(),
    }
}

fn drive(rt: &mut OpenCode) -> Vec<Event> {
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "the OpenCode turn never ended");
        if let Some(e) = rt.next_event().unwrap() {
            let end = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            events.push(e);
            if end {
                return events;
            }
        }
    }
}

/// A run in `mode` over `events`, a person answering `decision` when there is a window for them.
struct Ran {
    server: Server,
    events: Vec<Event>,
    records: Vec<(String, String)>,
    cards: Vec<crate::approval::Request>,
}

fn run(events: &[String], mode: Mode, answer: Option<(Decision, Option<&str>)>, shape: impl FnOnce(&mut Turn, OpenCode) -> OpenCode) -> Ran {
    run_on(Server::new(events), mode, answer, shape)
}

fn run_on(server: Server, mode: Mode, answer: Option<(Decision, Option<&str>)>, shape: impl FnOnce(&mut Turn, OpenCode) -> OpenCode) -> Ran {
    let channel = Channel::new();
    let mut t = turn(&server.folder(), "go", mode, answer.is_some().then(|| channel.clone()));
    let mut c = server.command(mode);
    let mut rt = shape(&mut t, OpenCode::spawn(&mut c).unwrap());
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let decider = answer.map(|(d, r)| person_until(&channel, d, r, done.clone()));
    rt.start(&t).unwrap();
    let events = drive(&mut rt);
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    let cards = decider.map(|h| h.join().unwrap()).unwrap_or_default();
    let records = match &rt.driver {
        Some(Driver::Serve(s)) => s.records.clone(),
        _ => panic!("an OpenCode 1 command ran without its server driver"),
    };
    // The password is the child's alone: never on the command the app built or keeps.
    for command in [&c, &rt.command] {
        assert!(!command.get_envs().any(|(k, _)| k == "OPENCODE_SERVER_PASSWORD"));
    }
    Ran { server, events, records, cards }
}

/// Electron's `onPermission`: every card the run raises is answered `decision` (with `reason`) until the run is
/// over, and each card is kept, in the order it came.
fn person_until(
    channel: &Arc<Channel>,
    decision: Decision,
    reason: Option<&str>,
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> std::thread::JoinHandle<Vec<crate::approval::Request>> {
    let (ch, reason) = (channel.clone(), reason.map(str::to_owned));
    std::thread::spawn(move || {
        let mut seen: Vec<crate::approval::Request> = vec![];
        while !done.load(std::sync::atomic::Ordering::Relaxed) {
            for card in ch.waiting() {
                if seen.iter().any(|s| s.approval_id == card.approval_id) {
                    continue;
                }
                ch.decide(Answer::Decision { approval_id: card.approval_id.clone(), decision: decision.clone(), reason: reason.clone() })
                    .unwrap();
                seen.push(card);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        seen
    })
}

/// Standard base64, for the `Basic` header Electron expects.
fn base64(bytes: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() { A[(n >> (18 - 6 * i)) as usize & 63] as char } else { '=' });
        }
    }
    out
}

fn said(events: &[Event]) -> String {
    events.iter().filter(|e| e.kind == "message.delta").filter_map(|e| e.payload["text"].as_str()).collect()
}

fn exit_code(events: &[Event]) -> Value {
    events.last().unwrap().payload["process"]["exitCode"].clone()
}

/// The permission replies, as Electron reads them: each request's id and what was sent.
fn replies(server: &Server) -> Vec<(String, Value)> {
    server
        .calls()
        .into_iter()
        .filter_map(|c| {
            let path = c["path"].as_str()?;
            let id = path.split("/permission/").nth(1)?;
            Some((id.to_owned(), c["body"].clone()))
        })
        .collect()
}

#[test]
fn starts_the_server_in_the_teammates_folder_never_where_the_app_itself_stands() {
    // case: starts the server IN the teammate's folder, never where the app itself stands (0.378)
    let ran = run(&recorded(), Mode::Auto, None, |_, rt| rt);
    let folder = ran.server.folder();
    assert_eq!(ran.server.command(Mode::Auto).get_current_dir(), Some(folder.as_path()));
    let cwd = PathBuf::from(ran.server.spawned()["cwd"].as_str().unwrap());
    assert_eq!(cwd.canonicalize().unwrap(), folder.canonicalize().unwrap());
    assert_ne!(cwd.canonicalize().unwrap(), std::env::current_dir().unwrap().canonicalize().unwrap());
    assert_eq!(ran.events.last().unwrap().kind, "run.completed");
}

#[test]
fn asks_the_person_sends_their_answer_back_and_reads_as_an_ordinary_opencode_run() {
    // case: asks the person, sends their answer back, and reads as an ordinary OpenCode run
    let ran = run(&recorded(), Mode::ApproveEach, Some((Decision::ApproveOnce, None)), |t, rt| {
        t.prompt = "Read README.md, then run echo SERVED.".into();
        t.model = Some("opencode/ling-3.0-flash-fin-free".into());
        t.effort = Some("high".into());
        rt
    });
    // The run's own records, in `run --format json`'s words: two steps, two tools, the answer, and never the
    // person's prompt echoed back.
    let types: Vec<&str> = ran.records.iter().map(|(kind, _)| kind.as_str()).collect();
    assert_eq!(types, ["step_start", "tool_use", "tool_use", "step_finish", "step_start", "text", "step_finish"]);
    // The shell call was put to the person, and their answer went back.
    let asked: Vec<(&str, &str)> = ran.cards.iter().map(|c| (c.kind.as_str(), c.detail.as_str())).collect();
    assert_eq!(asked, [("command", "echo SERVED")]);
    assert_eq!(replies(&ran.server), [("per_0d84a14d3001uIu5oHyC0O5Ij8/reply".to_owned(), json!({"reply": "once"}))]);
    // The model and effort as the server takes them.
    let prompt = ran.server.sent("/prompt_async").unwrap();
    assert_eq!(prompt["model"], json!({"providerID": "opencode", "modelID": "ling-3.0-flash-fin-free"}));
    assert_eq!(prompt["variant"], "high");
    // Every call carried the password, which is in the child's environment only (and `run` checks the commands).
    let password = ran.server.spawned()["password"].as_str().unwrap().to_owned();
    assert!(password.len() > 20, "{password:?}");
    let basic = format!("Basic {}", base64(format!("opencode:{password}").as_bytes()));
    let calls = ran.server.calls();
    assert!(!calls.is_empty());
    assert!(calls.iter().all(|c| c["auth"] == basic.as_str()), "{calls:?}");
    assert_eq!(ran.server.spawned()["argv"], json!(["serve", "--port", "0", "--hostname", "127.0.0.1"]));
    // And it ends like any OpenCode run that answered.
    assert_eq!(exit_code(&ran.events), 0);
    ran.server.assert_killed();
    assert!(said(&ran.events).contains("First line of the readme"), "{}", said(&ran.events));
    assert_eq!(ran.events.last().unwrap().kind, "run.completed");
}

/// QA-2026-09-29 round 2, R36: a subagent (the `task` tool) is a child session, and its request comes on this
/// stream under the child's id.
#[test]
fn asks_the_person_a_subagents_request_too_marked_as_the_subagents_and_answers_it() {
    // case: asks the person a subagent's request too, marked as the subagent's, and answers it
    let (child, grandchild) = ("ses_child_of_the_run", "ses_grandchild_of_the_run");
    let mut events = vec![
        json!({"type": "session.created", "properties": {"info": {"id": child, "parentID": SESSION}}}).to_string(),
        json!({"type": "session.created", "properties": {"info": {"id": grandchild, "parentID": child}}}).to_string(),
        json!({"type": "permission.asked", "properties": {"id": "per_child", "sessionID": grandchild, "permission": "bash", "patterns": ["echo FROM-SUBAGENT"], "metadata": {"command": "echo FROM-SUBAGENT"}, "always": ["echo *"]}}).to_string(),
        // Another session's request, not the run's nor a child of it, is not ours.
        json!({"type": "permission.asked", "properties": {"id": "per_stranger", "sessionID": "ses_someone_else", "permission": "bash", "patterns": ["rm -rf /"], "metadata": {"command": "rm -rf /"}, "always": []}}).to_string(),
    ];
    events.extend(recorded());
    let ran = run_on(Server::unanswered(&events, &["per_stranger"]), Mode::ApproveEach, Some((Decision::ApproveOnce, None)), |_, rt| rt);
    const BY_SUBAGENT: &str = ", asked by a subagent it started";
    let asked: Vec<(&str, bool)> = ran.cards.iter().map(|c| (c.detail.as_str(), c.summary.ends_with(BY_SUBAGENT))).collect();
    assert_eq!(asked, [("echo FROM-SUBAGENT", true), ("echo SERVED", false)]);
    let ids: Vec<String> = replies(&ran.server).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, ["per_child/reply", "per_0d84a14d3001uIu5oHyC0O5Ij8/reply"]);
}

#[test]
fn refuses_what_nobody_is_there_to_answer() {
    // case: refuses what nobody is there to answer
    // Approve each with no window to ask in: nobody was asked.
    let ran = run(&recorded(), Mode::ApproveEach, None, |_, rt| rt);
    assert_eq!(ran.server.sent("/permission/").unwrap(), json!({"reply": "reject", "message": OPENCODE_REFUSED_UNASKED}));
    // Never "the person declined" (0.822): nobody was asked.
    assert!(!OPENCODE_REFUSED_UNASKED.to_lowercase().contains("declined"));
}

#[test]
fn refuses_in_the_modes_own_words_when_it_is_given_them() {
    // case: refuses in the mode's own words when it is given them (0.822)
    let ran = run(&recorded(), Mode::Ask, None, |_, rt| rt);
    let refusal = "Locust refused this without asking anyone: this teammate is in Ask mode, which reads and changes nothing, so nothing was run.";
    assert_eq!(ran.server.sent("/permission/").unwrap(), json!({"reply": "reject", "message": refusal}));
}

#[test]
fn still_says_the_person_declined_when_a_person_answered_with_no_reason() {
    // case: still says the person declined when a person answered with no reason
    let ran = run(&recorded(), Mode::ApproveEach, Some((Decision::Deny, None)), |_, rt| rt);
    assert_eq!(ran.cards.len(), 1);
    assert_eq!(
        ran.server.sent("/permission/").unwrap(),
        json!({"reply": "reject", "message": "The person declined this in Locust."})
    );
}

#[test]
fn passes_on_why_the_person_declined_when_they_said() {
    // case: passes on why the person declined, when they said (0.374)
    let ran = run(&recorded(), Mode::ApproveEach, Some((Decision::Deny, Some("use the build script"))), |_, rt| rt);
    assert_eq!(
        ran.server.sent("/permission/").unwrap(),
        json!({"reply": "reject", "message": "The person declined this, and said: use the build script"})
    );
}

#[test]
fn sends_one_of_opencodes_own_commands_as_the_command_its_arguments_beside_it() {
    // case: sends one of OpenCode's own commands as the command, its arguments beside it, and reads the turn the same way (0.427)
    let ran = run(&recorded(), Mode::Auto, None, |t, rt| {
        t.prompt = "focus on the tests".into();
        t.model = Some("opencode/ling-3.0-flash-fin-free".into());
        rt.with_command(Some("review".into()))
    });
    assert!(!ran.server.calls().iter().any(|c| c["path"].as_str().unwrap().ends_with("/prompt_async")));
    assert_eq!(
        ran.server.sent(&format!("/session/{SESSION}/command")).unwrap(),
        json!({"command": "review", "arguments": "focus on the tests", "model": "opencode/ling-3.0-flash-fin-free"})
    );
    assert!(!ran.records.is_empty());
    assert_eq!(exit_code(&ran.events), 0);
    assert_eq!(ran.events.last().unwrap().kind, "run.completed");
}

#[test]
fn asks_for_every_action_in_its_config_and_never_announces_itself_on_the_network() {
    // case: asks for every action in its config, and never announces itself on the network
    // Electron's default sandbox, workspace-write with a person to ask, is Approve each.
    let server = Server::new(&[]);
    let command = server.command(Mode::ApproveEach);
    let raw = command
        .get_envs()
        .find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT")
        .and_then(|(_, v)| v)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let config: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(config["tools"], json!({"question": false}));
    assert_eq!(
        config["permission"],
        json!({
            // Every tool it does not name asks too (0.821).
            "*": "ask",
            // Reading is never a card; a `.env` file is the person's to open.
            "read": {"*": "allow", "*.env": "ask", "*.env.*": "ask", "*.env.example": "allow"},
            "glob": "allow",
            "grep": "allow",
            "list": "allow",
            "lsp": "allow",
            "task": "allow",
            "todowrite": "allow",
            "todoread": "allow",
            "skill": "allow",
            "edit": "ask",
            "bash": "ask",
            "webfetch": "ask",
            // Searches are not asked about in any mode.
            "websearch": "allow",
            "codesearch": "allow",
            "external_directory": "ask",
        })
    );
    // First, or OpenCode's last-match rule would make it outrank the reading tools after it.
    assert!(raw.contains(r#""permission":{"*":"ask""#), "{raw}");
    assert!(!command.get_args().any(|a| a == "--mdns"));
}

#[test]
fn turns_settled_parts_into_records_and_the_replys_text_while_it_is_written_into_partial_ones() {
    // case: turns settled parts into records, and the reply's text while it is written into partial ones (0.677)
    // Electron asks `runRecordFor` of each part alone; here each part comes on the stream, on a message whose role
    // the stream has already said, and the records the run made are read by part id.
    let message = |id: &str, role: &str| {
        json!({"type": "message.updated", "properties": {"sessionID": SESSION, "info": {"id": id, "role": role, "sessionID": SESSION}}}).to_string()
    };
    let part = |id: &str, on: &str, mut p: Value| {
        p["id"] = json!(id);
        p["messageID"] = json!(on);
        p["sessionID"] = json!(SESSION);
        json!({"type": "message.part.updated", "properties": {"sessionID": SESSION, "part": p}}).to_string()
    };
    let events = vec![
        message("msg_prompt", "user"),
        message("msg_reply", "assistant"),
        part("prt_prompt", "msg_prompt", json!({"type": "text", "text": "the prompt"})),
        part("prt_half", "msg_reply", json!({"type": "text", "text": "half", "time": {"start": 1}})),
        // Not the person's words, nor OpenCode's own notes, while they are written.
        part("prt_person_half", "msg_prompt", json!({"type": "text", "text": "half", "time": {"start": 1}})),
        part("prt_note_half", "msg_reply", json!({"type": "text", "synthetic": true, "text": "Contin", "time": {"start": 1}})),
        part("prt_done", "msg_reply", json!({"type": "text", "text": "done", "time": {"start": 1, "end": 2}})),
        // OpenCode's own continue note arrives on a message of its own; the normalizer needs it to see a compaction.
        message("msg_note", "user"),
        part("prt_continue", "msg_note", json!({"type": "text", "synthetic": true, "text": "Continue", "time": {"start": 1, "end": 1}})),
        part("prt_running", "msg_reply", json!({"type": "tool", "tool": "bash", "callID": "call_r", "state": {"status": "running"}})),
        part("prt_error", "msg_reply", json!({"type": "tool", "tool": "bash", "callID": "call_e", "state": {"status": "error", "error": "no"}})),
        json!({"type": "session.idle", "properties": {"sessionID": SESSION}}).to_string(),
    ];
    let ran = run(&events, Mode::Auto, None, |_, rt| rt);
    let made = |id: &str| -> Vec<&str> {
        ran.records.iter().filter(|(_, p)| p == id).map(|(kind, _)| kind.as_str()).collect::<Vec<_>>()
    };
    for none in ["prt_prompt", "prt_person_half", "prt_note_half", "prt_running"] {
        assert!(made(none).is_empty(), "{none}: {:?}", ran.records);
    }
    assert!(!made("prt_half").is_empty() && made("prt_half").iter().all(|k| *k == "text_partial"), "{:?}", ran.records);
    assert_eq!(made("prt_done"), ["text"]);
    assert_eq!(made("prt_continue"), ["text"]);
    assert_eq!(made("prt_error"), ["tool_use"]);
}

#[path = "opencode_every_mode_case_tests.rs"]
mod every_mode;

#[path = "opencode_commands_run_case_tests.rs"]
mod commands_run;
