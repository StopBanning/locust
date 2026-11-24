//! A stand-in `codex app-server` for the case tests that carry Electron's app-server tests (codex-app-server-run,
//! a-running-codex-turn-can-be-steered, app-server, codex-events): like Electron's `fakeServer`, it records every line
//! Locust writes and answers on cue. The cue is a plan: for each method Locust asks (or `method#2` for its second ask),
//! the steps to take; `$start` runs before anything is read, `answer` on each reply Locust sends to a server request.
//! A step is `{"result": ..}`, `{"error": "words"}` (the server's refusal), `{"send": message}`, `{"raw": "text"}`
//! (written as it is), `{"sleep": seconds}`, `{"stderr": "text"}`, `{"exit": code}`, or `{"silent": true}`.
use super::*;
use std::path::PathBuf;

const STAND_IN: &str = r#"
import json, os, sys, time
plan = json.load(open(os.environ['STAND_IN_PLAN'], encoding='utf-8'))
log = open(os.environ['STAND_IN_LOG'], 'a', encoding='utf-8')
THREAD = [{'result': {'thread': {'id': 'thread_9'}}}]
DEFAULTS = {'initialize': [{'result': {'userAgent': 'codex'}}], 'thread/start': THREAD, 'thread/resume': THREAD,
            'thread/fork': THREAD, 'turn/start': [{'result': {'turn': {'id': 'turn_1'}}}],
            'config/read': [{'result': {'config': {}}}]}
def out(text):
    sys.stdout.write(text); sys.stdout.flush()
def act(steps, msg):
    for a in steps:
        if 'result' in a: out(json.dumps({'jsonrpc': '2.0', 'id': msg.get('id'), 'result': a['result']}) + '\n')
        elif 'error' in a: out(json.dumps({'jsonrpc': '2.0', 'id': msg.get('id'), 'error': {'code': -32600, 'message': a['error']}}) + '\n')
        elif 'send' in a: out(json.dumps(a['send']) + '\n')
        elif 'raw' in a: out(a['raw'])
        elif 'sleep' in a: time.sleep(a['sleep'])
        elif 'stderr' in a: sys.stderr.write(a['stderr']); sys.stderr.flush()
        elif 'exit' in a: sys.exit(a['exit'])
seen = {}
act(plan.get('$start', []), {})
for line in sys.stdin:
    log.write(line); log.flush()
    msg = json.loads(line)
    method = msg.get('method')
    if method is None:
        act(plan.get('answer', []), msg)
        continue
    seen[method] = seen.get(method, 0) + 1
    key = '%s#%d' % (method, seen[method])
    key = key if key in plan else method
    if key in plan: act(plan[key], msg)
    elif 'id' in msg: act(DEFAULTS.get(method, [{'result': {}}]), msg)
"#;

/// What the stand-in was told, line by line, as it read it.
pub(super) struct Told {
    dir: PathBuf,
}

impl Drop for Told {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Told {
    /// Every line Locust wrote, exactly as written.
    pub(super) fn lines(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("told.jsonl"))
            .unwrap_or_default()
            .split_inclusive('\n')
            .map(str::to_owned)
            .collect()
    }
    pub(super) fn written(&self) -> Vec<Value> {
        self.lines().iter().map(|l| serde_json::from_str(l).unwrap()).collect()
    }
    pub(super) fn asked(&self) -> Vec<String> {
        self.written().iter().filter_map(|m| m["method"].as_str().map(str::to_owned)).collect()
    }
    /// The params of the first request with this method.
    pub(super) fn params_of(&self, method: &str) -> Option<Value> {
        self.written().into_iter().find(|m| m["method"] == method).map(|m| m["params"].clone())
    }
    pub(super) fn id_of(&self, method: &str) -> Option<Value> {
        self.written().into_iter().find(|m| m["method"] == method).map(|m| m["id"].clone())
    }
    /// Locust's replies to the server's own requests.
    pub(super) fn replies(&self) -> Vec<Value> {
        self.written().into_iter().filter(|m| m["method"].is_null()).collect()
    }
}

/// A Codex runtime whose `codex app-server` is the stand-in, playing `plan`.
pub(super) fn stand_in(plan: Value) -> (Codex, Told) {
    let dir = std::env::temp_dir().join(format!("locust-codex-stand-in-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("server.py"), STAND_IN).unwrap();
    std::fs::write(dir.join("plan.json"), plan.to_string()).unwrap();
    let mut command = Command::new("python3");
    command
        .arg(dir.join("server.py"))
        .env("STAND_IN_PLAN", dir.join("plan.json"))
        .env("STAND_IN_LOG", dir.join("told.jsonl"))
        .current_dir(&dir);
    (Codex::spawn(&mut command).unwrap(), Told { dir })
}

/// A turn as the host hands one to Codex: "count the files" in /work, in `mode`.
pub(super) fn turn_in(mode: Mode) -> Turn {
    Turn {
        prompt: "count the files".into(),
        folder: PathBuf::from("/work"),
        model: None,
        effort: None,
        resume: None,
        mode,
        approvals: None,
        mission_id: format!("codex-stand-in-{}", crate::invariants::unique_nanos()),
        run_id: "run_1".into(),
    }
}

/// The events of a run until its terminal one (inclusive), or every event read before `within` runs out.
pub(super) fn events_until_end(codex: &mut Codex, within: Duration) -> Vec<Event> {
    let deadline = Instant::now() + within;
    let mut events = vec![];
    while Instant::now() < deadline {
        if let Some(e) = codex.next_event().unwrap() {
            let end = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            events.push(e);
            if end {
                break;
            }
        }
    }
    events
}

/// The events of a run to its end, which must come within a generous wait.
pub(super) fn to_the_end(codex: &mut Codex) -> Vec<Event> {
    let events = events_until_end(codex, Duration::from_secs(20));
    assert!(
        events.last().is_some_and(|e| e.kind.starts_with("run.") && e.kind != "run.started"),
        "the run never ended: {:?}",
        events.iter().map(|e| &e.kind).collect::<Vec<_>>()
    );
    events
}

/// A turn whose server, once the turn is started, sends `then` (each a notification's method and params).
pub(super) fn turn_with(then: &[(&str, Value)]) -> (Codex, Told) {
    let sends: Vec<Value> = then.iter().map(|(m, p)| json!({"send": {"jsonrpc": "2.0", "method": m, "params": p}})).collect();
    let mut steps = vec![json!({"result": {"turn": {"id": "turn_1"}}})];
    steps.extend(sends);
    let (mut codex, told) = stand_in(json!({ "turn/start": steps }));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    (codex, told)
}

pub(super) fn kinds(events: &[Event]) -> Vec<&str> {
    events.iter().map(|e| e.kind.as_str()).collect()
}

pub(super) fn of<'a>(events: &'a [Event], kind: &str) -> Vec<&'a Event> {
    events.iter().filter(|e| e.kind == kind).collect()
}

/// Whether the process with this id is still there (`kill -0`).
pub(super) fn alive(pid: u32) -> bool {
    Command::new("kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}
