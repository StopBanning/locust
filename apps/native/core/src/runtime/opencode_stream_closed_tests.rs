//! Electron's `packages/runtime-adapters/test/a-message-taken-and-never-begun-ends-the-run.test.ts` past its first
//! three cases (those are opencode_tests.rs `a_message_opencode_took_*`): an OpenCode 1 turn whose event stream closes
//! before the turn finishes, through the caller (`OpenCode::spawn` on a `serve` command, `start`, `next_event` to the
//! end). The server is a scripted stand-in in Python, like Electron's `fakeServer`: each test writes the events it
//! streams after the message, whether the stream then closes, and what `/session/status` and the session's messages
//! answer (in order, or after a delay, or never). It logs every request it is sent (`calls.jsonl`), as `server.calls`.
//!
//! Electron's `completion` is the run's last event's `process` (its exit code and stderr), `cancelled` is a
//! `run.cancelled` end, and `server.kill` is the stand-in's process being gone once the run ends. Electron's raw
//! records are not kept natively, so where a case reads them the test reads the events they become (a `tool_use` is a
//! `tool.started` and a `tool.completed` or `tool.failed` named by its call id; a final `text` a final `message.delta`).
use super::*;
use crate::approval::Mode;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const SESSION: &str = "ses_dropped";

const SERVER: &str = r#"
import base64, json, os, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
S = json.load(open('script.json'))
SESSION = 'ses_dropped'
auth = 'Basic ' + base64.b64encode(('opencode:' + os.environ.get('OPENCODE_SERVER_PASSWORD', '')).encode()).decode()
open('pid', 'w').write(str(os.getpid()))
prompted, status_asked = threading.Event(), threading.Event()
state = {'reads': 0, 'first': None}
lock = threading.Lock()
SPECIAL = ('json', 'http', 'hang', 'endless', 'exit', 'delay', 'drop')

def log(method, path, ok):
    with lock, open('calls.jsonl', 'a') as f:
        f.write(json.dumps({'method': method, 'path': path, 'auth': ok, 'at': time.time()}) + '\n')

class H(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def log_message(self, *a):
        pass
    def answer(self, spec):
        if not (isinstance(spec, dict) and any(k in spec for k in SPECIAL)):
            spec = {'json': spec}
        time.sleep(spec.get('delay', 0))
        if spec.get('exit'):
            os._exit(0)
        if spec.get('hang'):
            time.sleep(600)
            return
        if spec.get('drop'):
            self.close_connection = True
            return
        if spec.get('endless'):
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', '100')
            self.end_headers()
            self.wfile.write(b'[')
            self.wfile.flush()
            time.sleep(600)
            return
        data = json.dumps(spec['json']).encode() if 'json' in spec else b''
        self.send_response(spec.get('http', 200))
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def do_GET(self):
        path = self.path.split('?')[0]
        log('GET', path, self.headers.get('Authorization') == auth)
        if path == '/event':
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Connection', 'close')
            self.end_headers()
            self.wfile.flush()
            if not S.get('close_before_session'):
                prompted.wait(30)
                if S.get('gate') == 'status':
                    status_asked.wait(30)
                time.sleep(S.get('event_delay', 0))
            for e in S.get('events', []):
                self.wfile.write(('data: ' + (e if isinstance(e, str) else json.dumps(e)) + '\n\n').encode())
            self.wfile.flush()
            if not S.get('close'):
                time.sleep(600)
            return
        if path == '/session/status':
            with lock:
                n = state['reads']
                state['reads'] += 1
                if state['first'] is None:
                    state['first'] = time.time()
                first = state['first']
            status_asked.set()
            switch = S.get('status_switch')
            if switch:
                spec = switch['before'] if time.time() - first < switch['for'] else switch['after']
            else:
                statuses = S.get('statuses', [{}])
                spec = statuses[min(n, len(statuses) - 1)]
            return self.answer(spec)
        if path == '/session/' + SESSION + '/message':
            return self.answer(S.get('messages', []))
        self.answer({'http': 404})
    def do_POST(self):
        path = self.path.split('?')[0]
        n = int(self.headers.get('Content-Length', '0'))
        if n:
            self.rfile.read(n)
        log('POST', path, self.headers.get('Authorization') == auth)
        if path == '/session':
            time.sleep(S.get('session_delay', 0))
            return self.answer({'id': SESSION})
        if path.endswith('/prompt_async') or path.endswith('/command'):
            prompted.set()
            return self.answer({'http': 200})
        if path.endswith('/abort'):
            return self.answer(S.get('abort', {'http': 200}))
        self.answer({'http': 404})

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

/// Electron's `fakeServer(options)`: the script the stand-in follows.
fn server(script: Value) -> Temp {
    let t = Temp(std::env::temp_dir().join(format!("locust-oc-closed-{}", crate::invariants::unique_nanos())));
    std::fs::create_dir_all(&t.0).unwrap();
    std::fs::write(t.0.join("server.py"), SERVER).unwrap();
    std::fs::write(t.0.join("script.json"), script.to_string()).unwrap();
    t
}

/// Electron's `fakeClosedServer`: the model begins (`assistant`), then the stream closes.
fn closed(more: Value) -> Value {
    let mut script = json!({"events":[assistant()],"close":true});
    for (k, v) in more.as_object().unwrap() {
        script[k] = v.clone();
    }
    script
}

fn opencode(folder: &Path) -> OpenCode {
    let mut c = Command::new("python3");
    c.arg(folder.join("server.py"))
        .arg("serve")
        .current_dir(folder)
        .env("LOCUST_NATIVE_OPENCODE_PROTOCOL", "serve");
    OpenCode::spawn(&mut c).unwrap()
}

fn turn(folder: &Path, resume: bool) -> Turn {
    Turn {
        prompt: "Read the project summary".into(),
        folder: folder.to_path_buf(),
        model: None,
        effort: None,
        resume: resume.then(|| SESSION.to_owned()),
        mode: Mode::Ask,
        approvals: None,
        mission_id: format!("oc_closed_{}", crate::invariants::unique_nanos()),
        run_id: "opencode-closed-test".into(),
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

/// Electron's `inspect`: one turn through the stand-in, to its end.
fn inspect(script: Value, resume: bool, shape: impl FnOnce(OpenCode) -> OpenCode) -> (Temp, Vec<Event>) {
    let t = server(script);
    let mut rt = shape(opencode(&t.0));
    rt.start(&turn(&t.0, resume)).unwrap();
    let events = drive(&mut rt);
    (t, events)
}

/// Electron's `completion`.
fn completion(events: &[Event]) -> &Value {
    &events.last().unwrap().payload["process"]
}
fn exit_code(events: &[Event]) -> Value {
    completion(events)["exitCode"].clone()
}
fn stderr(events: &[Event]) -> &str {
    completion(events)["stderr"].as_str().unwrap()
}
fn cancelled(events: &[Event]) -> bool {
    events.last().unwrap().kind == "run.cancelled"
}

fn calls(t: &Temp) -> Vec<Value> {
    std::fs::read_to_string(t.0.join("calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
fn count(t: &Temp, f: impl Fn(&str) -> bool) -> usize {
    calls(t).iter().filter(|c| f(c["path"].as_str().unwrap())).count()
}
fn statuses(t: &Temp) -> usize {
    count(t, |p| p == "/session/status")
}
fn message_reads(t: &Temp) -> usize {
    count(t, |p| p.ends_with("/message"))
}
fn prompts(t: &Temp) -> usize {
    count(t, |p| p.ends_with("/prompt_async"))
}
fn sessions_made(t: &Temp) -> usize {
    count(t, |p| p == "/session")
}
fn all_authenticated(t: &Temp) -> bool {
    calls(t).iter().all(|c| c["auth"] == true)
}

/// Electron's `server.kill` called: the stand-in's process is gone once the run has ended.
fn assert_stopped(t: &Temp) {
    let pid = std::fs::read_to_string(t.0.join("pid")).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while Path::new(&format!("/proc/{}", pid.trim())).exists() {
        assert!(Instant::now() < until, "the server was left running");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A stop that comes as the server is first asked for `path` (Electron's `onPrompt` / `onStatus`).
fn stop_on(folder: &Path, path: &'static str) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let (set, folder) = (flag.clone(), folder.to_path_buf());
    std::thread::spawn(move || {
        let until = Instant::now() + crate::load_waits::UNDER_LOAD;
        while Instant::now() < until {
            let seen = std::fs::read_to_string(folder.join("calls.jsonl")).unwrap_or_default();
            if seen.lines().any(|l| serde_json::from_str::<Value>(l).is_ok_and(|c| c["path"].as_str().is_some_and(|p| p.ends_with(path)))) {
                set.store(true, Ordering::Relaxed);
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    });
    flag
}

fn kinds<'a>(events: &'a [Event], kind: &str) -> Vec<&'a Event> {
    events.iter().filter(|e| e.kind == kind).collect()
}
fn finals(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e.kind == "message.delta" && e.payload["final"] == true)
        .map(|e| e.payload["text"].as_str().unwrap().to_owned())
        .collect()
}
/// The call ids of the tools that settled, in order (Electron's `tool_use` records).
fn settled_tools(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e.kind == "tool.completed" || e.kind == "tool.failed")
        .map(|e| e.payload["itemId"].as_str().unwrap().to_owned())
        .collect()
}
/// The turn's steps, tools and final text, in order: what Electron's raw records become (a step's end with no step
/// begun, like a final answer's, shows as no step).
fn replay(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e.kind.as_str() {
            "step.started" | "step.completed" => Some(e.kind.clone()),
            "tool.completed" | "tool.failed" => Some(format!("{} {}", e.kind, e.payload["itemId"].as_str().unwrap())),
            "message.delta" if e.payload["final"] == true => Some("text".into()),
            _ => None,
        })
        .collect()
}

fn assistant() -> Value {
    json!({"type":"message.updated","properties":{"sessionID":SESSION,"info":{"id":"msg_answer","role":"assistant"}}})
}
fn idle() -> Value {
    json!({"type":"session.idle","properties":{"sessionID":SESSION}})
}
fn status(kind: &str) -> Value {
    json!({SESSION: {"type": kind}})
}
fn finished_message(id: &str) -> Value {
    json!({
        "info": {"id":id,"role":"assistant","sessionID":SESSION,"finish":"stop","time":{"created":1,"completed":2}},
        "parts": [
            {"id":format!("{id}_text"),"type":"text","messageID":id,"sessionID":SESSION,"text":"Recovered final answer","time":{"start":1,"end":2}},
            {"id":format!("{id}_end"),"type":"step-finish","messageID":id,"sessionID":SESSION,"reason":"stop","cost":0,"tokens":{"input":1,"output":1,"reasoning":0,"cache":{"read":0,"write":0}}},
        ],
    })
}
fn prompt_message(id: &str, session: &str) -> Value {
    json!({"type":"message.updated","properties":{"sessionID":session,"info":{"id":id,"sessionID":session,"role":"user"}}})
}
fn prompt() -> Value {
    prompt_message("msg_current_prompt", SESSION)
}
fn answer_to(parent: &str, id: &str) -> Value {
    let mut m = finished_message(id);
    m["info"]["parentID"] = json!(parent);
    m
}
fn current_answer() -> Value {
    answer_to("msg_current_prompt", "msg_current_answer")
}
fn tool_step(id: &str, created: u64) -> Value {
    json!({
        "info": {"id":id,"role":"assistant","sessionID":SESSION,"parentID":"msg_current_prompt","finish":"tool-calls","time":{"created":created,"completed":created + 1}},
        "parts": [
            {"id":format!("{id}_001"),"type":"step-start","messageID":id,"sessionID":SESSION},
            {"id":format!("{id}_002"),"type":"tool","messageID":id,"sessionID":SESSION,"tool":"bash","callID":format!("call_{id}"),"state":{"status":"completed","input":{"command":format!("echo {id}")},"output":format!("output {id}"),"time":{"start":created,"end":created + 1}}},
            {"id":format!("{id}_003"),"type":"step-finish","messageID":id,"sessionID":SESSION,"reason":"tool-calls","cost":0,"tokens":{"input":2,"output":1,"reasoning":0,"cache":{"read":0,"write":0}}},
        ],
    })
}
fn final_step(created: u64) -> Value {
    let mut m = current_answer();
    m["info"]["time"] = json!({"created":created,"completed":created + 1});
    for (i, part) in m["parts"].as_array_mut().unwrap().iter_mut().enumerate() {
        part["id"] = json!(format!("prt_final_00{}", i + 1));
    }
    m
}
fn message_event(info: &Value) -> Value {
    json!({"type":"message.updated","properties":{"sessionID":SESSION,"info":info}})
}
fn part_event(part: &Value) -> Value {
    json!({"type":"message.part.updated","properties":{"sessionID":SESSION,"part":part}})
}

// An OpenCode event stream that closes before the turn finishes.

#[test]
fn fails_a_begun_run_and_ends_its_records_when_the_stream_closes_without_idle() {
    let (t, events) = inspect(closed(json!({})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert!(!cancelled(&events));
    assert!(stderr(&events).contains("event stream closed before the turn finished"), "{}", stderr(&events));
    assert_stopped(&t);
    assert_eq!(events.last().unwrap().kind, "run.failed");
    assert_eq!(prompts(&t), 1);
    assert!(all_authenticated(&t));
}

#[test]
fn flushes_paced_reply_text_before_reporting_the_lost_stream() {
    let part = |text: &str| json!({"type":"message.part.updated","properties":{"sessionID":SESSION,"part":{"id":"part_1","type":"text","messageID":"msg_answer","text":text}}});
    let (_t, events) = inspect(closed(json!({"events":[assistant(), part("Hello"), part("Hello there")]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    // The paced reply as it stood when the stream was lost (Electron's last `text_partial`).
    let mut shown = String::new();
    for e in events.iter().filter(|e| e.kind == "message.delta" && e.payload["final"] == false) {
        if e.payload["operation"] == "replace" {
            shown.clear();
        }
        shown.push_str(e.payload["text"].as_str().unwrap());
    }
    assert_eq!(shown, "Hello there");
}

#[test]
fn never_sends_a_prompt_after_the_stream_was_lost_during_session_creation() {
    let (t, events) = inspect(closed(json!({"close_before_session":true,"session_delay":0.3,"events":[]})), false, |rt| rt);
    assert_eq!(events.last().unwrap().kind, "run.failed");
    assert_eq!(sessions_made(&t), 1);
    assert_eq!(count(&t, |p| p.ends_with("/prompt_async") || p.ends_with("/command")), 0);
    assert_stopped(&t);
}

#[test]
fn keeps_a_normal_idle_followed_by_eof_successful() {
    let (t, events) = inspect(closed(json!({"events":[assistant(), idle()]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert!(!cancelled(&events));
    assert_eq!(stderr(&events), "");
    assert_stopped(&t);
}

#[test]
fn fails_an_empty_stream_without_waiting_for_the_dropped_start_watchdog() {
    let began = Instant::now();
    let (t, events) = inspect(closed(json!({"events":[]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert_eq!(statuses(&t), 1);
    // The watchdog's own wait is a minute; the closed stream is asked about at once.
    assert!(began.elapsed() < Duration::from_secs(30));
}

#[test]
fn preserves_cancellation_when_eof_arrives_after_the_person_stops() {
    let t = server(closed(json!({"event_delay":0.3})));
    let stop = stop_on(&t.0, "/prompt_async");
    let mut rt = opencode(&t.0).with_cancel(stop);
    rt.start(&turn(&t.0, false)).unwrap();
    let events = drive(&mut rt);
    assert!(cancelled(&events));
    assert_eq!(exit_code(&events), Value::Null);
    assert_eq!(stderr(&events), "");
    assert_stopped(&t);
}

#[test]
fn fails_a_resumed_conversation_when_its_new_stream_closes_prematurely() {
    let (t, events) = inspect(closed(json!({})), true, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert_eq!(sessions_made(&t), 0);
    assert_eq!(prompts(&t), 1);
}

#[test]
fn fails_a_command_turn_on_eof_without_replaying_its_command() {
    let (t, events) = inspect(closed(json!({})), false, |rt| rt.with_command(Some("review".into())));
    assert_eq!(exit_code(&events), 1);
    assert_eq!(count(&t, |p| p.ends_with("/command")), 1);
    assert_eq!(prompts(&t), 0);
}

// An OpenCode session polled after its event stream closes.

#[test]
fn polls_busy_and_retry_states_before_recovering_a_verified_completed_answer() {
    let script = closed(json!({"statuses":[status("busy"), status("retry"), {}],"messages":[finished_message("msg_answer")]}));
    let (t, events) = inspect(script, false, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert!(!cancelled(&events));
    assert_eq!(stderr(&events), "");
    assert_eq!(statuses(&t), 3);
    assert_eq!(message_reads(&t), 1);
    assert_eq!(prompts(&t), 1);
    assert!(all_authenticated(&t));
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert!(finals(&events).contains(&"Recovered final answer".to_owned()));
    assert_stopped(&t);
}

#[test]
fn does_not_emit_already_received_final_text_twice_during_recovery() {
    let message = finished_message("msg_answer");
    let script = closed(json!({"events":[assistant(), part_event(&message["parts"][0])],"messages":[message]}));
    let (_t, events) = inspect(script, false, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(finals(&events).iter().filter(|s| *s == "Recovered final answer").count(), 1);
}

#[test]
fn does_not_treat_an_older_unobserved_answer_as_this_turns_completion() {
    let (t, events) = inspect(closed(json!({"messages":[finished_message("msg_old_answer")]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert_eq!(prompts(&t), 1);
}

#[test]
fn does_not_infer_completion_from_idle_with_no_final_answer() {
    let (t, events) = inspect(closed(json!({"statuses":[status("idle")],"messages":[]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert_stopped(&t);
}

#[test]
fn refuses_recovery_without_a_completed_timestamp() {
    let mut message = finished_message("msg_answer");
    message["info"]["time"] = json!({"created":1});
    let (_t, events) = inspect(closed(json!({"messages":[message]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn refuses_recovery_without_a_terminal_stop_part() {
    let mut message = finished_message("msg_answer");
    message["parts"].as_array_mut().unwrap().truncate(1);
    let (_t, events) = inspect(closed(json!({"messages":[message]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn refuses_a_foreign_sessions_completed_answer() {
    let mut message = finished_message("msg_answer");
    message["info"]["sessionID"] = json!("ses_other");
    let (_t, events) = inspect(closed(json!({"messages":[message]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn does_not_infer_a_resumed_turns_completion_from_historical_messages() {
    let (t, events) = inspect(closed(json!({"messages":[finished_message("msg_answer")]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert_eq!(sessions_made(&t), 0);
}

#[test]
fn bounds_a_session_that_remains_busy_after_the_stream_closes() {
    // Electron's `streamCloseTimeoutMs: 30` for its 300 s default; the native budget is shortened the same way.
    let (t, events) = inspect(closed(json!({"statuses":[status("busy")]})), false, |rt| rt.with_recovery_budget(Duration::from_millis(600)));
    assert_eq!(exit_code(&events), 1);
    assert!(stderr(&events).contains("session was still busy"), "{}", stderr(&events));
    assert!(statuses(&t) > 1);
    assert_eq!(message_reads(&t), 0);
    assert_stopped(&t);
}

#[test]
fn bounds_and_aborts_a_status_request_that_never_answers() {
    let (t, events) = inspect(closed(json!({"statuses":[{"hang":true}]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert_stopped(&t);
}

#[test]
fn bounds_a_status_response_whose_body_never_ends() {
    let (t, events) = inspect(closed(json!({"statuses":[{"endless":true}]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert_stopped(&t);
}

#[test]
fn keeps_cancellation_while_polling_as_cancelled() {
    let t = server(closed(json!({"statuses":[{"delay":0.5,"json":{}}]})));
    let stop = stop_on(&t.0, "/session/status");
    let mut rt = opencode(&t.0).with_cancel(stop);
    rt.start(&turn(&t.0, false)).unwrap();
    let events = drive(&mut rt);
    assert!(cancelled(&events));
    assert_eq!(exit_code(&events), Value::Null);
    assert_eq!(stderr(&events), "");
    assert_eq!(prompts(&t), 1);
}

#[test]
fn fails_a_rejected_status_request() {
    let (_t, events) = inspect(closed(json!({"statuses":[{"drop":true}]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn fails_an_http_error_from_the_status_endpoint() {
    let (_t, events) = inspect(closed(json!({"statuses":[{"http":500}]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn fails_a_malformed_status_response() {
    let (_t, events) = inspect(closed(json!({"statuses":["unreadable"]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn fails_an_unknown_session_status_without_guessing_success() {
    let (t, events) = inspect(closed(json!({"statuses":[status("unknown")]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert_eq!(message_reads(&t), 0);
}

#[test]
fn fails_an_unreadable_message_response() {
    let (_t, events) = inspect(closed(json!({"messages":{"http":503}})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

// An OpenCode recovery that loses the server or its response.

#[test]
fn bounds_a_message_response_whose_body_never_ends() {
    let (t, events) = inspect(closed(json!({"messages":{"endless":true}})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert_stopped(&t);
}

#[test]
fn does_not_recover_a_delayed_answer_after_the_server_exits() {
    let (t, events) = inspect(closed(json!({"messages":{"delay":0.3,"exit":true}})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert!(!finals(&events).contains(&"Recovered final answer".to_owned()));
    assert_stopped(&t);
}

#[test]
fn does_not_recover_a_completed_message_that_reports_an_error() {
    let mut message = finished_message("msg_answer");
    message["info"]["error"] = json!({"name":"APIError","data":{"message":"provider failed"}});
    let (_t, events) = inspect(closed(json!({"messages":[message]})), false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

// The shared OpenCode session check.

#[test]
fn reuses_an_in_flight_watchdog_query_when_the_stream_closes() {
    // The stream closes while the watchdog's status check is out; that one check serves for the closed stream.
    let script = closed(json!({"events":[],"gate":"status","statuses":[{"delay":0.4,"json":{}}]}));
    let (t, events) = inspect(script, false, |rt| rt.with_begin_check(Duration::from_millis(1)));
    assert_eq!(statuses(&t), 1);
    assert_eq!(exit_code(&events), 1);
    assert!(stderr(&events).contains("event stream closed before the turn finished"), "{}", stderr(&events));
    assert_eq!(message_reads(&t), 1);
}

#[test]
fn retries_a_failed_watchdog_query_while_its_event_stream_remains_open() {
    let script = json!({"events":[],"close":false,"statuses":[{"http":500},{}],"messages":[]});
    let (t, events) = inspect(script, false, |rt| rt.with_begin_check(Duration::from_millis(50)));
    assert_eq!(exit_code(&events), 1);
    assert!(stderr(&events).contains("took the message but never began"), "{}", stderr(&events));
    assert_eq!(statuses(&t), 2);
}

#[test]
fn ends_cancellation_even_when_the_server_never_answers_abort() {
    let t = server(closed(json!({"statuses":[{"delay":0.5,"json":{}}],"abort":{"hang":true}})));
    let stop = stop_on(&t.0, "/session/status");
    let mut rt = opencode(&t.0).with_cancel(stop);
    rt.start(&turn(&t.0, false)).unwrap();
    let events = drive(&mut rt);
    assert!(cancelled(&events));
    assert_eq!(exit_code(&events), Value::Null);
    assert_eq!(stderr(&events), "");
    assert_eq!(count(&t, |p| p.ends_with("/abort")), 1);
    assert_stopped(&t);
}

// An OpenCode follow-up recovered by its prompt ID.

#[test]
fn recovers_a_resumed_turn_by_parentid_even_when_its_assistant_event_was_lost() {
    let (t, events) = inspect(closed(json!({"events":[prompt()],"messages":[current_answer()]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert!(!cancelled(&events));
    assert_eq!(stderr(&events), "");
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert!(finals(&events).contains(&"Recovered final answer".to_owned()));
    assert_eq!(sessions_made(&t), 0);
    assert_eq!(prompts(&t), 1);
    assert!(all_authenticated(&t));
    assert_stopped(&t);
}

#[test]
fn selects_the_current_answer_instead_of_an_observed_historical_answer() {
    let historical = answer_to("msg_previous_prompt", "msg_previous_answer");
    let current = current_answer();
    let script = closed(json!({"events":[prompt(), message_event(&historical["info"])],"messages":[historical, current]}));
    let (_t, events) = inspect(script, true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    // Only the current answer (Electron: the text and step end records are both `msg_current_answer`'s; an end with
    // no step begun shows as no step).
    assert_eq!(replay(&events), ["text"]);
    assert_eq!(finals(&events).len(), 1);
}

#[test]
fn rejects_a_resumed_answer_whose_parent_is_not_the_current_prompt() {
    // case: rejects a resumed answer with parentID %s
    for parent in [json!("msg_previous_prompt"), json!(""), Value::Null] {
        let mut message = current_answer();
        if parent.is_null() {
            message["info"].as_object_mut().unwrap().remove("parentID");
        } else {
            message["info"]["parentID"] = parent.clone();
        }
        let script = closed(json!({"events":[prompt(), message_event(&message["info"])],"messages":[message]}));
        let (_t, events) = inspect(script, true, |rt| rt);
        assert_eq!(exit_code(&events), 1, "parentID {parent}");
        assert!(replay(&events).is_empty(), "parentID {parent}: {:?}", replay(&events));
    }
}

#[test]
fn requires_the_current_prompt_receipt_before_recovering_a_resumed_answer() {
    let message = current_answer();
    let (_t, events) = inspect(closed(json!({"events":[message_event(&message["info"])],"messages":[message]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn ignores_a_prompt_receipt_belonging_to_another_session() {
    let script = closed(json!({"events":[prompt_message("msg_current_prompt", "ses_other")],"messages":[current_answer()]}));
    let (_t, events) = inspect(script, true, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn rejects_a_prompt_whose_message_session_conflicts_with_its_event_session() {
    let mut conflicting = prompt_message("msg_current_prompt", "ses_other");
    conflicting["properties"]["sessionID"] = json!(SESSION);
    let (_t, events) = inspect(closed(json!({"events":[conflicting],"messages":[current_answer()]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn keeps_the_first_prompt_receipt_when_later_user_events_arrive() {
    let script = closed(json!({"events":[prompt(), prompt_message("msg_other_prompt", SESSION)],"messages":[answer_to("msg_other_prompt", "msg_current_answer")]}));
    let (_t, events) = inspect(script, true, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn does_not_fall_back_to_an_observed_first_turn_answer_with_a_mismatched_parentid() {
    let script = closed(json!({"events":[prompt(), assistant()],"messages":[answer_to("msg_previous_prompt", "msg_answer")]}));
    let (_t, events) = inspect(script, false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn still_requires_a_completed_answer_and_a_terminal_stop_part_after_matching_parentid() {
    let mut message = current_answer();
    message["parts"].as_array_mut().unwrap().truncate(1);
    let (_t, events) = inspect(closed(json!({"events":[prompt()],"messages":[message]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 1);
}

#[test]
fn does_not_duplicate_a_resumed_answer_that_already_streamed_its_final_text() {
    let message = current_answer();
    let script = closed(json!({"events":[prompt(), message_event(&message["info"]), part_event(&message["parts"][0])],"messages":[message]}));
    let (_t, events) = inspect(script, true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(finals(&events).len(), 1);
}

// An OpenCode recovery that is still active after ten seconds.

#[test]
fn keeps_polling_a_busy_or_retrying_session_beyond_ten_seconds_and_then_recovers() {
    // case: keeps polling %s beyond ten seconds and then recovers
    let runs: Vec<_> = ["busy", "retry"]
        .into_iter()
        .map(|kind| {
            std::thread::spawn(move || {
                let script = closed(json!({"events":[prompt()],"status_switch":{"for":10.3,"before":status(kind),"after":{}},"messages":[current_answer()]}));
                let (t, events) = inspect(script, true, |rt| rt);
                (kind, t, events)
            })
        })
        .collect();
    for run in runs {
        let (kind, t, events) = run.join().unwrap();
        let polls = calls(&t);
        let first = polls.iter().find(|c| c["path"] == "/session/status").unwrap()["at"].as_f64().unwrap();
        let in_ten = polls.iter().filter(|c| c["path"] == "/session/status" && c["at"].as_f64().unwrap() - first < 10.0).count();
        assert!(in_ten > 2, "{kind}: {in_ten} checks in the first ten seconds");
        assert_eq!(exit_code(&events), 0, "{kind}: {}", stderr(&events));
        assert_eq!(stderr(&events), "", "{kind}");
        assert_eq!(prompts(&t), 1, "{kind}");
    }
}

#[test]
fn stops_a_perpetually_busy_recovery_at_the_five_minute_overall_boundary() {
    // The native boundary is 300 s (opencode_serve.rs `recovery_budget`); the test shortens it as Electron's fake
    // clock does.
    let (t, events) = inspect(closed(json!({"statuses":[status("busy")]})), false, |rt| rt.with_recovery_budget(Duration::from_millis(500)));
    assert_eq!(exit_code(&events), 1);
    assert!(!cancelled(&events));
    assert!(stderr(&events).contains("recovery deadline"), "{}", stderr(&events));
    assert_stopped(&t);
    let asked = calls(&t).len();
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(calls(&t).len(), asked);
}

#[test]
fn bounds_an_unresponsive_poll_at_ten_seconds_even_after_earlier_busy_responses() {
    let began = Instant::now();
    let (t, events) = inspect(closed(json!({"statuses":[status("busy"), {"hang":true}]})), false, |rt| rt);
    let took = began.elapsed();
    assert!(took >= Duration::from_secs(10) && took < Duration::from_secs(20), "ended after {took:?}");
    assert_eq!(exit_code(&events), 1);
    assert_eq!(statuses(&t), 2);
    assert_stopped(&t);
}

#[test]
fn fails_a_hard_status_error_after_extended_busy_polling() {
    let script = closed(json!({"status_switch":{"for":10.0,"before":status("busy"),"after":{"http":503}}}));
    let (t, events) = inspect(script, false, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert!(!cancelled(&events));
    assert!(statuses(&t) > 2);
    assert_eq!(message_reads(&t), 0);
}

#[test]
fn settles_a_timed_out_check_even_if_the_fetch_implementation_ignores_abort() {
    let began = Instant::now();
    let (t, events) = inspect(closed(json!({"statuses":[{"hang":true}]})), false, |rt| rt);
    assert!(began.elapsed() < Duration::from_secs(20));
    assert_eq!(exit_code(&events), 1);
    assert_stopped(&t);
}

// An OpenCode recovery that replays the current turn's tool steps.

#[test]
fn replays_both_disconnected_tool_steps_in_order_before_the_final_answer_and_completion() {
    let (first, second) = (tool_step("msg_step_1", 10), tool_step("msg_step_2", 20));
    let (t, events) = inspect(closed(json!({"events":[prompt()],"messages":[first, second, final_step(100)]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(stderr(&events), "");
    assert_eq!(settled_tools(&events), ["call_msg_step_1", "call_msg_step_2"]);
    let text_at = events.iter().position(|e| e.kind == "message.delta").unwrap();
    let last_tool = events.iter().rposition(|e| e.kind == "tool.completed").unwrap();
    assert!(text_at > last_tool);
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert_eq!(prompts(&t), 1);
}

#[test]
fn does_not_replay_a_tool_step_already_settled_on_the_live_stream() {
    let (first, second) = (tool_step("msg_streamed_step", 10), tool_step("msg_missing_step", 20));
    let mut live = vec![prompt(), message_event(&first["info"])];
    live.extend(first["parts"].as_array().unwrap().iter().map(part_event));
    let (_t, events) = inspect(closed(json!({"events":live,"messages":[first, second, final_step(100)]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(settled_tools(&events), ["call_msg_streamed_step", "call_msg_missing_step"]);
}

#[test]
fn replays_a_tool_that_was_still_running_when_the_stream_disconnected_exactly_once() {
    let step = tool_step("msg_running_step", 10);
    let mut running = step["parts"][1].clone();
    running["state"] = json!({"status":"running","input":{"command":"echo msg_running_step"},"time":{"start":10}});
    let live = json!([prompt(), message_event(&step["info"]), part_event(&step["parts"][0]), part_event(&running), part_event(&running)]);
    let (_t, events) = inspect(closed(json!({"events":live,"messages":[step, final_step(100)]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(settled_tools(&events), ["call_msg_running_step"]);
    assert_eq!(kinds(&events, "step.started").len(), 1);
    assert_eq!(kinds(&events, "tool.started").len(), 1);
    assert_eq!(&replay(&events)[..3], ["step.started", "tool.completed call_msg_running_step", "step.completed"]);
}

#[test]
fn orders_recovered_messages_by_creation_time_and_then_message_id_instead_of_response_order() {
    let (first, second, third) = (tool_step("msg_earliest", 10), tool_step("msg_same_time_a", 20), tool_step("msg_same_time_b", 20));
    let (_t, events) = inspect(closed(json!({"events":[prompt()],"messages":[final_step(100), third, first, second]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(settled_tools(&events), ["call_msg_earliest", "call_msg_same_time_a", "call_msg_same_time_b"]);
    let order = replay(&events);
    assert_eq!(order.last().unwrap(), "text");
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn orders_shuffled_step_markers_and_tools_by_their_chronological_part_ids() {
    let step = tool_step("msg_shuffled_parts", 10);
    let mut parts = step["parts"].as_array().unwrap().clone();
    for (part, id) in parts.iter_mut().zip(["prt_001", "prt_002", "prt_003"]) {
        part["id"] = json!(id);
    }
    parts.reverse();
    let shuffled = json!({"info":step["info"],"parts":parts});
    let (_t, events) = inspect(closed(json!({"events":[prompt()],"messages":[shuffled, final_step(100)]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(&replay(&events)[..3], ["step.started", "tool.completed call_msg_shuffled_parts", "step.completed"]);
}

#[test]
fn uses_tool_timestamps_to_order_recovered_parts_when_their_ids_are_absent() {
    let step = tool_step("msg_no_part_ids", 10);
    let mut first = step["parts"][1].clone();
    first.as_object_mut().unwrap().remove("id");
    let mut second = first.clone();
    second["callID"] = json!("call_second_no_id");
    second["state"] = json!({"status":"completed","input":{"command":"echo second"},"output":"second output","time":{"start":20,"end":21}});
    let message = json!({"info":step["info"],"parts":[second, first]});
    let (_t, events) = inspect(closed(json!({"events":[prompt()],"messages":[message, final_step(100)]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(settled_tools(&events), ["call_msg_no_part_ids", "call_second_no_id"]);
}

#[test]
fn keeps_multiple_imported_step_boundaries_in_order_while_recovering_one_assistant_message() {
    let step = tool_step("msg_imported_steps", 10);
    let p = step["parts"].as_array().unwrap();
    let with = |part: &Value, id: &str, more: Value| {
        let mut part = part.clone();
        part["id"] = json!(id);
        for (k, v) in more.as_object().unwrap() {
            part[k] = v.clone();
        }
        part
    };
    let tokens = |i: u64, o: u64| json!({"tokens":{"input":i,"output":o,"reasoning":0,"cache":{"read":0,"write":0}}});
    let ordered = json!([
        with(&p[0], "imported_start_1", json!({})),
        with(&p[1], "imported_tool_1", json!({})),
        with(&p[2], "imported_finish_1", tokens(2, 3)),
        with(&p[0], "imported_start_2", json!({})),
        with(&p[1], "imported_tool_2", json!({"callID":"call_second_imported","state":{"status":"completed","input":{"command":"echo second"},"output":"second output","time":{"start":20,"end":21}}})),
        with(&p[2], "imported_finish_2", tokens(5, 7)),
    ]);
    let message = json!({"info":step["info"],"parts":ordered});
    let (_t, events) = inspect(closed(json!({"events":[prompt()],"messages":[message, final_step(100)]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(
        &replay(&events)[..6],
        ["step.started", "tool.completed call_msg_imported_steps", "step.completed", "step.started", "tool.completed call_second_imported", "step.completed"]
    );
    assert_eq!(kinds(&events, "step.started").len(), 2);
    assert_eq!(kinds(&events, "step.completed").len(), 2);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.completed");
    assert_eq!(last.payload["usage"]["inputTokens"], 8.0);
    assert_eq!(last.payload["usage"]["outputTokens"], 11.0);
}

#[test]
fn replays_a_failed_tool_with_its_complete_diagnostics_before_the_final_answer() {
    let step = tool_step("msg_failed_tool", 10);
    let error = format!("Build failed: src/widgets.ts:27\nTypeError: unexpected response\n{}", "diagnostic detail ".repeat(100));
    let mut failed = step["parts"][1].clone();
    failed["state"] = json!({"status":"error","input":{"command":"npm run build"},"error":error,"metadata":{"exit":2},"time":{"start":10,"end":11}});
    let message = json!({"info":step["info"],"parts":[step["parts"][0], failed, step["parts"][2]]});
    let (_t, events) = inspect(closed(json!({"events":[prompt()],"messages":[message, final_step(100)]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    let failures = kinds(&events, "tool.failed");
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].payload["itemId"], "call_msg_failed_tool");
    assert_eq!(failures[0].payload["command"], "npm run build");
    let order = replay(&events);
    let tool_at = order.iter().position(|k| k.starts_with("tool.failed")).unwrap();
    let text_at = order.iter().position(|k| k == "text").unwrap();
    assert!(tool_at < text_at);
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn excludes_historical_turns_foreign_sessions_and_mismatched_parts_from_tool_replay() {
    let current = tool_step("msg_current_tool", 10);
    let mut historical = tool_step("msg_old_tool", 1);
    historical["info"]["parentID"] = json!("msg_previous_prompt");
    let mut foreign = tool_step("msg_foreign_tool", 2);
    foreign["info"]["sessionID"] = json!("ses_other");
    let mut as_user = historical.clone();
    as_user["info"]["role"] = json!("user");
    let mut with_strays = current.clone();
    let mut stray_session = current["parts"][1].clone();
    stray_session["id"] = json!("foreign_session_part");
    stray_session["sessionID"] = json!("ses_other");
    let mut stray_message = current["parts"][1].clone();
    stray_message["id"] = json!("foreign_message_part");
    stray_message["messageID"] = json!("msg_other");
    with_strays["parts"].as_array_mut().unwrap().extend([stray_session, stray_message]);
    let mut no_id = current.clone();
    no_id["info"].as_object_mut().unwrap().remove("id");
    let other = json!({"id":"msg_other","role":"assistant","sessionID":SESSION,"parentID":"msg_previous_prompt"});
    let script = closed(json!({
        "events":[prompt(), message_event(&other)],
        "messages":[historical, foreign, as_user, with_strays, null, 42, {"info":null,"parts":[]}, no_id, final_step(100)],
    }));
    let (_t, events) = inspect(script, true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(settled_tools(&events), ["call_msg_current_tool"]);
    // The current step (its start, tool and end) and the final answer (its text and end), and nothing else.
    assert_eq!(replay(&events), ["step.started", "tool.completed call_msg_current_tool", "step.completed", "text"]);
}

#[test]
fn emits_duplicate_recovered_messages_and_parts_only_once() {
    let step = tool_step("msg_duplicated_tool", 10);
    let final_answer = final_step(100);
    let mut repeated = step.clone();
    let twice: Vec<Value> = step["parts"].as_array().unwrap().iter().chain(step["parts"].as_array().unwrap()).cloned().collect();
    repeated["parts"] = json!(twice);
    let script = closed(json!({"events":[prompt()],"messages":[repeated, step, final_answer, final_answer]}));
    let (_t, events) = inspect(script, true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(replay(&events), ["step.started", "tool.completed call_msg_duplicated_tool", "step.completed", "text"]);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.completed");
    assert_eq!(last.payload["usage"]["inputTokens"], 3.0);
    assert_eq!(last.payload["usage"]["outputTokens"], 2.0);
}

#[test]
fn keeps_distinct_no_id_tool_payloads_beyond_the_old_80_character_prefix_and_deduplicates_equivalent_payloads() {
    let step = tool_step("msg_payload_fallback", 10);
    let command = format!("echo {}", "shared long prefix ".repeat(20));
    let first = json!({"type":"tool","messageID":"msg_payload_fallback","sessionID":SESSION,"tool":"bash","state":{"status":"completed","input":{"command":command},"output":"first output","time":{"start":10,"end":11}},"callID":"call_no_id_first"});
    let mut second = first.clone();
    second["state"]["output"] = json!("second output");
    second["state"]["time"] = json!({"start":20,"end":21});
    second["callID"] = json!("call_no_id_second");
    // The same payload as `first`, its keys in another order.
    let duplicate = serde_json::from_str::<Value>(&format!(
        r#"{{"callID":"call_no_id_first","state":{{"time":{{"start":10,"end":11}},"output":"first output","input":{{"command":{}}},"status":"completed"}},"tool":"bash","sessionID":"{SESSION}","messageID":"msg_payload_fallback","type":"tool"}}"#,
        json!(command)
    ))
    .unwrap();
    let message = json!({"info":step["info"],"parts":[duplicate, second, first]});
    let live = json!([prompt(), message_event(&step["info"]), part_event(&first)]);
    let (_t, events) = inspect(closed(json!({"events":live,"messages":[message, final_step(100)]})), true, |rt| rt);
    assert_eq!(exit_code(&events), 0);
    assert_eq!(settled_tools(&events), ["call_no_id_first", "call_no_id_second"]);
}

#[test]
fn rejects_the_unsupported_working_status_instead_of_polling_it_as_active() {
    let script = closed(json!({"events":[prompt()],"statuses":[status("working")],"messages":[final_step(100)]}));
    let (t, events) = inspect(script, true, |rt| rt.with_recovery_budget(Duration::from_secs(5)));
    assert_eq!(exit_code(&events), 1);
    assert_eq!(statuses(&t), 1);
    assert_eq!(message_reads(&t), 0);
    assert!(replay(&events).is_empty());
}

#[test]
fn does_not_settle_on_an_earlier_stop_while_the_latest_matching_message_is_unfinished() {
    let script = closed(json!({"events":[prompt()],"messages":[tool_step("msg_newer_incomplete", 20), final_step(10)]}));
    let (t, events) = inspect(script, true, |rt| rt);
    assert_eq!(exit_code(&events), 1);
    assert_stopped(&t);
    assert_eq!(prompts(&t), 1);
}
