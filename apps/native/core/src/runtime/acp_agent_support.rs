//! A scripted ACP agent for the case-by-case ports of Electron's ACP tests (acp-events, acp-run,
//! an-acp-output-stream-closing-ends-the-turn). Electron's fakes are closures over a fake transport; here each one is a
//! few lines of Python on the real pipes, so the turn is driven through `Acp` itself (spawn, start, next_event, steer,
//! stop), as copilot.rs, opencode.rs and native.rs drive it. Everything the agent was sent is written down, one JSON
//! message a line, for the test to read back as Electron's `agent.sent`.
use super::*;
use crate::approval::{Channel, Mode};
use std::path::PathBuf;

/// What every agent can say: Electron's `say`, `answer`, `update` and `ended`, its SESSION and OPTIONS, and
/// `messages()`, which yields each message Locust sent once it is written down.
const PRELUDE: &str = r#"
import json, os, sys, time
_sent = open(os.environ['LOCUST_SENT'], 'a')
AGENT_MODE = 'https://agentclientprotocol.com/protocol/session-modes#agent'
AUTOPILOT = 'https://agentclientprotocol.com/protocol/session-modes#autopilot'
SESSION = dict(sessionId='s1', modes=dict(currentModeId=AGENT_MODE, availableModes=[dict(id=AGENT_MODE, name='Agent'), dict(id=AUTOPILOT, name='Autopilot')]),
    configOptions=[dict(type='select', id='allow_all', currentValue='off', options=[dict(value='on'), dict(value='off')])])
OPTIONS = [dict(optionId='yes', kind='allow_once', name='Allow once'), dict(optionId='yes-forever', kind='allow_always', name='Always allow'), dict(optionId='no', kind='reject_once', name='Deny')]
def say(m):
    sys.stdout.write(json.dumps(dict(jsonrpc='2.0', **m)) + '\n'); sys.stdout.flush()
def answer(to, result):
    say(dict(id=to['id'], result=result))
def update(u, session='s1'):
    say(dict(method='session/update', params=dict(sessionId=session, update=u)))
def ended(prompt, stop='end_turn'):
    answer(prompt, dict(stopReason=stop, usage=dict(inputTokens=10, outputTokens=2)))
def text_of(prompt):
    return prompt['params']['prompt'][0]['text']
def messages():
    for line in sys.stdin:
        m = json.loads(line)
        _sent.write(json.dumps(m) + '\n'); _sent.flush()
        yield m
def usual(m, session=SESSION):
    if m.get('method') == 'initialize':
        answer(m, dict(protocolVersion=1, agentCapabilities=dict(loadSession=True))); return True
    if m.get('method') == 'session/new':
        answer(m, session); return True
    return False
"#;

/// An agent that plays back one of Electron's measured transcripts (`test/fixtures/acp/`): each request Locust sends
/// is matched to the next recorded one of its method, and what the agent said after it is said again, its answer
/// carrying the id Locust used. A request the transcript never saw is answered with an empty result.
pub const REPLAY: &str = r#"
lines = [json.loads(l) for l in open(os.environ['LOCUST_ACP_FIXTURE']) if l.strip()]
lines = [l for l in lines if l.get('dir') in ('in', 'out') and isinstance(l.get('message'), dict)]
pos, ids = 0, {}
for m in messages():
    if 'method' in m:
        j = next((k for k in range(pos, len(lines)) if lines[k]['dir'] == 'out' and lines[k]['message'].get('method') == m['method']), None)
        if j is None:
            if 'id' in m: say(dict(id=m['id'], result={}))
            continue
        if 'id' in lines[j]['message']: ids[lines[j]['message']['id']] = m.get('id')
    else:
        j = next((k for k in range(pos, len(lines)) if lines[k]['dir'] == 'out' and 'method' not in lines[k]['message'] and lines[k]['message'].get('id') == m.get('id')), None)
        if j is None: continue
    pos = j + 1
    while pos < len(lines) and lines[pos]['dir'] == 'in':
        said = dict(lines[pos]['message']); pos += 1
        said.pop('jsonrpc', None)
        if 'method' not in said and said.get('id') in ids: said['id'] = ids[said['id']]
        say(said)
"#;

static AGENTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub struct Agent {
    pub dir: PathBuf,
    script: PathBuf,
    sent: PathBuf,
    env: Vec<(String, String)>,
}
impl Drop for Agent {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
impl Agent {
    /// An agent whose behaviour is `body`, after the prelude.
    pub fn new(body: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "locust-acp-agent-{}-{}-{}",
            std::process::id(),
            AGENTS.fetch_add(1, Ordering::Relaxed),
            crate::invariants::unique_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("agent.py");
        std::fs::write(&script, format!("{PRELUDE}\n{body}")).unwrap();
        let sent = dir.join("sent.jsonl");
        Self { dir, script, sent, env: vec![] }
    }
    /// The agent that plays back Electron's `fixtures/acp/<name>`.
    pub fn replaying(name: &str) -> Self {
        let mut agent = Self::new(REPLAY);
        agent.env.push((
            "LOCUST_ACP_FIXTURE".into(),
            format!("{}/../../../packages/runtime-adapters/test/fixtures/acp/{name}", env!("CARGO_MANIFEST_DIR")),
        ));
        agent
    }
    pub fn command(&self) -> Command {
        let mut c = Command::new("python3");
        c.arg("-u").arg(&self.script).current_dir(&self.dir).env("LOCUST_SENT", &self.sent);
        for (k, v) in &self.env {
            c.env(k, v);
        }
        c
    }
    pub fn spawn(&self, profile: Profile) -> Acp {
        let mut acp = Acp::spawn(&mut self.command()).unwrap();
        acp.set_profile(profile);
        acp
    }
    /// Everything Locust sent the agent, in order (Electron's `agent.sent`).
    pub fn sent(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.sent)
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
    pub fn methods(&self) -> Vec<String> {
        self.sent().iter().filter_map(|m| m["method"].as_str().map(str::to_owned)).collect()
    }
    pub fn sent_with_id(&self, id: u64) -> Option<Value> {
        self.sent().into_iter().find(|m| m["id"] == id && m["method"].is_null())
    }
}

/// Copilot as copilot.rs starts it over ACP: Agent mode, allow-all off, every card the mode's own.
pub fn copilot() -> Profile {
    Profile {
        runtime: "copilot".into(),
        required: vec![("allow_all".into(), "off".into())],
        strict: false,
        named: true,
        ..Default::default()
    }
}

/// OpenCode as opencode.rs starts it over ACP (no model or effort chosen).
pub fn opencode() -> Profile {
    Profile { runtime: "opencode".into(), strict: true, named: true, ..Default::default() }
}

/// An agent added in Settings > AI agents, as native.rs starts it: every request goes to the host's card.
pub fn added() -> Profile {
    Profile::default()
}

pub fn turn(folder: &std::path::Path, mode: Mode, resume: Option<&str>, approvals: Option<Arc<Channel>>) -> Turn {
    Turn {
        prompt: "Change the port to 3001.".into(),
        folder: folder.to_path_buf(),
        model: None,
        effort: None,
        resume: resume.map(str::to_owned),
        mode,
        approvals,
        mission_id: "acp_case".into(),
        run_id: "run_acp_case".into(),
    }
}

/// The turn's events until its end, `tick` called between reads (to steer, stop or answer a card).
pub fn events_until_end(acp: &mut Acp, mut tick: impl FnMut(&mut Acp, &[Event])) -> Vec<Event> {
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "the ACP turn never ended: {:?}", kinds(&events));
        tick(acp, &events);
        if let Some(e) = acp.next_event().unwrap() {
            let end = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            events.push(e);
            if end {
                return events;
            }
        }
    }
}

/// A whole turn: spawned with `profile`, started, read to its end.
pub fn whole_turn(agent: &Agent, profile: Profile, mode: Mode, resume: Option<&str>) -> Vec<Event> {
    let mut acp = agent.spawn(profile);
    acp.start(&turn(&agent.dir, mode, resume, None)).unwrap();
    events_until_end(&mut acp, |_, _| {})
}

pub fn kinds(events: &[Event]) -> Vec<&str> {
    events.iter().map(|e| e.kind.as_str()).collect()
}

/// The events Electron's normalizer would make: Rust also says, first, that the session opened (`acp.session`), so a
/// turn whose start fails still keeps its identity. Electron keeps that as a record of its own, not an event.
pub fn product(events: &[Event]) -> Vec<&Event> {
    events.iter().filter(|e| e.payload["code"] != "acp.session").collect()
}

/// What the run SAID: each message as it was finally marked whole.
pub fn said(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e.kind == "message.delta" && e.payload["final"] == true)
        .map(|e| e.payload["text"].as_str().unwrap().to_owned())
        .collect()
}

pub fn first<'a>(events: &'a [Event], kind: &str) -> &'a Value {
    &events.iter().find(|e| e.kind == kind).unwrap_or_else(|| panic!("no {kind} in {:?}", kinds(events))).payload
}

/// The events' payloads, as text (Event is not Debug).
pub fn shown(events: &[Event]) -> String {
    serde_json::to_string(&events.iter().map(|e| (&e.kind, &e.payload)).collect::<Vec<_>>()).unwrap()
}
