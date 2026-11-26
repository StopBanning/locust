//! Electron's `packages/runtime-adapters/test/opencode-acp-profile.test.ts`, case by case, through the caller: each
//! case starts OpenCode 2 as the app does (`OpenCode::spawn` on an `acp` command, `start`, `next_event` to the end),
//! whose `opencode` is a scripted ACP v1 peer over stdio in Python, like Electron's scripted agent. The peer writes
//! each process it was started as (`spawns.log`) and every message it was sent (`sent.jsonl`), as Electron's fixture
//! keeps `spawned` and `sent`. Electron's structured `failure` is the `opencode.<code>` diagnostic the native driver
//! writes before the run ends (opencode.rs `failure`), with the same code, stage and attempt count.
//!
//! Electron's timings are its defaults, since the native driver has no options for them: 250 ms before the first
//! restart, two restarts at most (`bound(options.maxStartupRetries, 2, 3, 0)`) and a 60 s startup budget, which the
//! tests shorten through `with_startup_budget`.
use super::*;
use crate::approval::Mode;
use crate::store::Store;
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

const SESSION: &str = "session-current";

const PEER: &str = r#"
import json, os, sys, time
scenario = [a for a in sys.argv[1:] if a != 'acp'][0]
with open('spawns.log', 'a') as f:
    f.write(json.dumps({'at': time.time(), 'argv': sys.argv[1:], 'cwd': os.getcwd(),
        'env': {k: os.environ.get(k) for k in ('OPENCODE_CONFIG_CONTENT', 'OPENCODE_CONFIG_PROJECT_DISABLE')}}) + '\n')
attempt = sum(1 for _ in open('spawns.log'))
SESSION = 'session-current'
options = [{'id': 'mode', 'currentValue': 'build'}]
if scenario == 'refuse-config':
    options.append({'id': 'model', 'type': 'select', 'currentValue': 'provider/other'})
if scenario in ('effort-after-model', 'abandoned'):
    options.append({'id': 'model', 'currentValue': 'provider/old'})
prompt = None
def send(v):
    sys.stdout.write(json.dumps(dict(jsonrpc='2.0', **v)) + '\n'); sys.stdout.flush()
def answer(m, r):
    send({'id': m['id'], 'result': r})
def update(u):
    send({'method': 'session/update', 'params': {'sessionId': SESSION, 'update': u}})
def text(t, mid='answer-current'):
    update({'sessionUpdate': 'agent_message_chunk', 'messageId': mid, 'content': {'type': 'text', 'text': t}})
for line in sys.stdin:
    m = json.loads(line)
    with open('sent.jsonl', 'a') as f:
        f.write(json.dumps({'attempt': attempt, 'message': m}) + '\n')
    method = m.get('method')
    if method == 'initialize':
        if scenario == 'close-always' or (scenario == 'close-first' and attempt == 1):
            os._exit(0)
        if scenario == 'auth':
            send({'id': m['id'], 'error': {'code': -32000, 'message': 'Sign in to OpenCode before starting ACP'}})
        elif scenario == 'v2':
            answer(m, {'protocolVersion': 2})
        else:
            answer(m, {'protocolVersion': 1, 'agentCapabilities': {'loadSession': True}})
    elif method == 'session/new':
        sid = 'abandoned-session' if scenario == 'abandoned' and attempt == 1 else SESSION
        answer(m, {'sessionId': sid, 'configOptions': options})
    elif method == 'session/load':
        text('historical answer must stay out', 'history')
        update({'sessionUpdate': 'user_message_chunk', 'content': {'type': 'text', 'text': 'historical prompt'}})
        answer(m, {'sessionId': SESSION, 'configOptions': options})
    elif method == 'session/set_config_option':
        if scenario == 'abandoned' and attempt == 1:
            os._exit(0)
        if scenario != 'refuse-config' or m['params']['configId'] != 'model':
            for row in options:
                if row['id'] == m['params']['configId']:
                    row['currentValue'] = m['params']['value']
            if scenario == 'effort-after-model' and m['params']['configId'] == 'model':
                options.append({'id': 'effort', 'currentValue': 'low'})
        answer(m, {'configOptions': options})
    elif method == 'session/prompt':
        prompt = m
        if scenario == 'tools':
            update({'sessionUpdate': 'tool_call', 'toolCallId': 'tool-ok', 'kind': 'execute', 'title': 'Run the tests', 'status': 'pending', 'rawInput': {'command': 'npm test'}})
            send({'id': 90, 'method': 'session/request_permission', 'params': {'sessionId': SESSION,
                'toolCall': {'toolCallId': 'tool-ok', 'kind': 'execute', 'rawInput': {'command': 'npm test'}},
                'options': [{'optionId': 'once', 'kind': 'allow_once', 'name': 'Allow once'}, {'optionId': 'always', 'kind': 'allow_always', 'name': 'Always'}]}})
            continue
        if scenario == 'prompt-close':
            update({'sessionUpdate': 'tool_call', 'toolCallId': 'once-only', 'kind': 'execute', 'status': 'in_progress', 'rawInput': {'command': 'publish-once'}})
            os._exit(0)
        if scenario == 'slow':
            time.sleep(float(os.environ['LOCUST_ACP_SLOW']))
        text('Recovered.' if scenario == 'close-first' else 'Current answer.')
        answer(m, {'stopReason': 'end_turn', 'usage': {'inputTokens': 7, 'outputTokens': 3}})
    elif m.get('id') == 90 and 'result' in m:
        update({'sessionUpdate': 'tool_call_update', 'toolCallId': 'tool-ok', 'status': 'completed', 'content': [{'type': 'content', 'content': {'type': 'text', 'text': 'All tests passed'}}]})
        update({'sessionUpdate': 'tool_call', 'toolCallId': 'tool-error', 'kind': 'execute', 'status': 'in_progress', 'rawInput': {'command': 'read missing-file'}})
        update({'sessionUpdate': 'tool_call_update', 'toolCallId': 'tool-error', 'status': 'failed', 'content': [{'type': 'content', 'content': {'type': 'text', 'text': 'ENOENT: missing-file does not exist'}}]})
        text('Tests passed; the missing file was reported.')
        answer(prompt, {'stopReason': 'end_turn'})
"#;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn temp() -> Temp {
    let t = Temp(std::env::temp_dir().join(format!("locust-oc-acp-profile-{}", crate::invariants::unique_nanos())));
    std::fs::create_dir_all(&t.0).unwrap();
    std::fs::write(t.0.join("opencode.py"), PEER).unwrap();
    t
}

/// The `opencode acp` command the app builds, pointed at the scripted peer.
fn command(folder: &Path, scenario: &str) -> Command {
    let mut c = Command::new("python3");
    c.arg(folder.join("opencode.py")).arg("acp").arg(scenario).current_dir(folder);
    c
}

fn turn(folder: &Path) -> Turn {
    Turn {
        prompt: "Inspect the project.".into(),
        folder: folder.to_path_buf(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: format!("oc_acp_{}", crate::invariants::unique_nanos()),
        run_id: "opencode-acp-test".into(),
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

fn run(folder: &Path, scenario: &str, t: &Turn, shape: impl FnOnce(OpenCode) -> OpenCode) -> Vec<Event> {
    let mut rt = shape(OpenCode::spawn(&mut command(folder, scenario)).unwrap());
    rt.start(t).unwrap();
    drive(&mut rt)
}

fn spawns(folder: &Path) -> Vec<Value> {
    lines(&folder.join("spawns.log"))
}
fn sent(folder: &Path) -> Vec<Value> {
    lines(&folder.join("sent.jsonl"))
}
fn lines(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
fn prompts(folder: &Path) -> Vec<Value> {
    sent(folder).into_iter().filter(|m| m["message"]["method"] == "session/prompt").collect()
}

/// Electron's `failure`: the code, stage, attempts and words the driver reports before the run ends.
fn failure(events: &[Event]) -> Option<Value> {
    events
        .iter()
        .find(|e| e.kind == "adapter.diagnostic" && e.payload["code"].as_str().is_some_and(|c| c.starts_with("opencode.")))
        .map(|e| e.payload.clone())
}
fn assert_failure(events: &[Event], code: &str, stage: &str, attempts: u64) -> String {
    let f = failure(events).unwrap_or_else(|| panic!("no failure in {}", shown(events)));
    assert_eq!(f["code"], format!("opencode.{code}"), "{f}");
    assert_eq!(f["stage"], stage, "{f}");
    assert_eq!(f["attempts"], attempts, "{f}");
    f["message"].as_str().unwrap().to_owned()
}

fn shown(events: &[Event]) -> String {
    events.iter().map(|e| format!("{} {}", e.kind, e.payload)).collect::<Vec<_>>().join("\n")
}

fn finals(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e.kind == "message.delta" && e.payload["final"] == true)
        .map(|e| e.payload["text"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn passes_the_desktop_launch_environment_and_permission_receipt_through_the_profile() {
    let t = temp();
    let mut c = command(&t.0, "ordinary");
    c.env("OPENCODE_CONFIG_CONTENT", r#"{"agents":{"locust":{}}}"#).env("OPENCODE_CONFIG_PROJECT_DISABLE", "1");
    let mut rt = OpenCode::spawn(&mut c).unwrap();
    rt.start(&turn(&t.0)).unwrap();
    let events = drive(&mut rt);
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert_eq!(events.last().unwrap().payload["process"]["exitCode"], 0);
    assert_eq!(
        spawns(&t.0)[0]["env"],
        json!({"OPENCODE_CONFIG_CONTENT":r#"{"agents":{"locust":{}}}"#,"OPENCODE_CONFIG_PROJECT_DISABLE":"1"})
    );
}

#[test]
fn reads_effort_options_advertised_after_switching_the_selected_model() {
    let t = temp();
    let mut asked = turn(&t.0);
    asked.model = Some("provider/new".into());
    asked.effort = Some("high".into());
    let events = run(&t.0, "effort-after-model", &asked, |rt| rt);
    assert_eq!(events.last().unwrap().kind, "run.completed");
    // The native driver sets OpenCode's `locust` mode first; the model and the effort follow as in Electron.
    let set: Vec<_> = sent(&t.0)
        .into_iter()
        .filter(|m| m["message"]["method"] == "session/set_config_option")
        .map(|m| m["message"]["params"]["configId"].as_str().unwrap().to_owned())
        .filter(|id| id != "mode")
        .collect();
    assert_eq!(set, ["model", "effort"]);
    assert_eq!(prompts(&t.0).len(), 1);
}

#[test]
fn keeps_opencode_identity_launch_prefix_and_environment_on_the_stdio_command() {
    // The command the app builds for an installed OpenCode 2: its own path, `acp`, the teammate's folder.
    let t = temp();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let built = command_for_version(PathBuf::from("/installed/opencode"), "2.0.0", &t.0, Mode::Ask, &store, false).unwrap();
    assert_eq!(built.get_program(), "/installed/opencode");
    assert_eq!(built.get_args().collect::<Vec<_>>(), ["acp"]);
    assert_eq!(built.get_current_dir(), Some(t.0.as_path()));
    assert!(uses_acp(&built));
    assert!(built.get_envs().any(|(k, v)| k == "OPENCODE_CONFIG_CONTENT" && v.is_some()));
    // And that command is the one started: its arguments, folder and environment reach the process unchanged.
    let mut c = command(&t.0, "ordinary");
    c.env("OPENCODE_CONFIG_PROJECT_DISABLE", "1");
    let events = run(&t.0, "ordinary", &turn(&t.0), |_| OpenCode::spawn(&mut c).unwrap());
    assert_eq!(events.last().unwrap().kind, "run.completed");
    let started = spawns(&t.0);
    assert_eq!(started.len(), 1);
    assert_eq!(started[0]["argv"], json!(["acp", "ordinary"]));
    assert_eq!(Path::new(started[0]["cwd"].as_str().unwrap()).canonicalize().unwrap(), t.0.canonicalize().unwrap());
    assert_eq!(started[0]["env"]["OPENCODE_CONFIG_PROJECT_DISABLE"], "1");
    let new = sent(&t.0).into_iter().find(|m| m["message"]["method"] == "session/new").unwrap();
    assert!(new["message"]["params"]["mcpServers"].is_array());
    assert!(failure(&events).is_none());
}

#[test]
fn loads_an_acp_v1_session_without_recording_its_replayed_history() {
    let t = temp();
    let mut resumed = turn(&t.0);
    resumed.resume = Some(SESSION.into());
    let events = run(&t.0, "load", &resumed, |rt| rt);
    let messages: Vec<_> = sent(&t.0).into_iter().map(|m| m["message"].clone()).collect();
    assert_eq!(
        messages.iter().find(|m| m["method"] == "initialize").unwrap()["params"],
        json!({"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false}})
    );
    assert_eq!(
        messages.iter().find(|m| m["method"] == "session/load").unwrap()["params"],
        json!({"sessionId":SESSION,"cwd":t.0,"mcpServers":[]})
    );
    assert!(!messages.iter().any(|m| m["method"] == "session/new"));
    let all = serde_json::to_string(&events.iter().map(|e| e.payload.clone()).collect::<Vec<_>>()).unwrap();
    assert!(!all.contains("historical"));
    assert_eq!(finals(&events), ["Current answer."]);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.completed");
    assert_eq!(last.payload["runtimeThreadId"], SESSION);
    assert_eq!(last.payload["usage"], json!({"inputTokens":7.0,"outputTokens":3.0}));
    assert!(failure(&events).is_none());
}

#[test]
fn maps_permissions_and_successful_and_failed_tool_calls_through_the_existing_normalizer() {
    // Auto answers the agent's question itself, with the one-time choice, never "always".
    let t = temp();
    let mut auto = turn(&t.0);
    auto.mode = Mode::Auto;
    let events = run(&t.0, "tools", &auto, |rt| rt);
    let reply = sent(&t.0).into_iter().find(|m| m["message"]["id"] == 90).unwrap();
    assert_eq!(reply["message"]["result"], json!({"outcome":{"outcome":"selected","optionId":"once"}}));
    let started: Vec<_> = events.iter().filter(|e| e.kind == "tool.started").map(|e| e.payload["itemId"].clone()).collect();
    assert_eq!(started, [json!("tool-ok"), json!("tool-error")]);
    let done = &events.iter().find(|e| e.kind == "tool.completed").unwrap().payload;
    assert_eq!((&done["itemId"], &done["status"], &done["output"]), (&json!("tool-ok"), &json!("completed"), &json!("All tests passed")));
    let failed = &events.iter().find(|e| e.kind == "tool.failed").unwrap().payload;
    assert_eq!(
        (&failed["itemId"], &failed["status"], &failed["output"]),
        (&json!("tool-error"), &json!("failed"), &json!("ENOENT: missing-file does not exist"))
    );
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert_eq!(prompts(&t.0).len(), 1);
    assert!(failure(&events).is_none());
}

#[test]
fn retries_a_pre_prompt_stdout_close_only_after_bounded_startup_backoff() {
    let t = temp();
    let events = run(&t.0, "close-first", &turn(&t.0), |rt| rt);
    let started = spawns(&t.0);
    assert_eq!(started.len(), 2);
    let waited = started[1]["at"].as_f64().unwrap() - started[0]["at"].as_f64().unwrap();
    assert!(waited >= 0.25, "restarted after {waited} s, before the 250 ms backoff");
    let asked = prompts(&t.0);
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0]["attempt"], 2);
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert_eq!(finals(&events), ["Recovered."]);
    assert!(failure(&events).is_none());
}

#[test]
fn exhausts_capped_startup_retries_with_a_structured_transport_failure() {
    let t = temp();
    let events = run(&t.0, "close-always", &turn(&t.0), |rt| rt);
    // Electron's default: two restarts (`bound(options.maxStartupRetries, 2, 3, 0)`), so three processes in all.
    assert_failure(&events, "transport-closed", "startup", 3);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert_eq!(last.payload["process"]["exitCode"], Value::Null);
    assert_eq!(last.payload["process"]["cancelled"], false);
    assert_eq!(last.payload["process"]["terminationUnconfirmed"], false);
    assert_eq!(spawns(&t.0).len(), 3);
    assert!(prompts(&t.0).is_empty());
}

#[test]
fn cancels_during_startup_backoff_without_creating_a_replacement_process() {
    let t = temp();
    let cancel = Arc::new(AtomicBool::new(false));
    let (flag, folder) = (cancel.clone(), t.0.clone());
    let stopper = std::thread::spawn(move || {
        let until = Instant::now() + crate::load_waits::UNDER_LOAD;
        while !sent(&folder).iter().any(|m| m["message"]["method"] == "initialize") {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(5));
        }
        // The first process is gone the moment it reads initialize; stop inside the 250 ms before its replacement.
        std::thread::sleep(Duration::from_millis(60));
        flag.store(true, Ordering::Relaxed);
    });
    let events = run(&t.0, "close-always", &turn(&t.0), |rt| rt.with_cancel(cancel));
    stopper.join().unwrap();
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.cancelled");
    assert_eq!(last.payload["process"]["exitCode"], Value::Null);
    assert_eq!(spawns(&t.0).len(), 1);
    assert!(prompts(&t.0).is_empty());
    assert!(failure(&events).is_none(), "a stopped run is not a failure: {}", shown(&events));
}

#[test]
fn bounds_the_total_startup_deadline_even_when_retry_backoff_remains() {
    let t = temp();
    let events = run(&t.0, "close-always", &turn(&t.0), |rt| rt.with_startup_budget(Duration::from_millis(200)));
    assert_failure(&events, "startup-timeout", "startup", 1);
    let last = events.last().unwrap();
    assert_eq!(last.payload["process"]["exitCode"], Value::Null);
    assert_eq!(last.payload["process"]["cancelled"], false);
    assert_eq!(spawns(&t.0).len(), 1);
    assert!(prompts(&t.0).is_empty());
}

#[test]
fn never_resends_an_admitted_prompt_after_stdout_closes_during_its_tool_call() {
    let t = temp();
    let events = run(&t.0, "prompt-close", &turn(&t.0), |rt| rt);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert_eq!(last.payload["process"]["exitCode"], Value::Null);
    assert_eq!(last.payload["process"]["cancelled"], false);
    assert_eq!(spawns(&t.0).len(), 1);
    assert_eq!(prompts(&t.0).len(), 1);
    assert_eq!(events.iter().filter(|e| e.kind == "tool.started").count(), 1);
}

#[test]
fn reports_authentication_errors_without_treating_them_as_reconnectable_transport_loss() {
    let t = temp();
    let events = run(&t.0, "auth", &turn(&t.0), |rt| rt);
    assert_eq!(assert_failure(&events, "agent-error", "startup", 1), "Sign in to OpenCode before starting ACP");
    assert_eq!(spawns(&t.0).len(), 1);
    assert!(prompts(&t.0).is_empty());
}

#[test]
fn rejects_unsupported_acp_protocol_versions_without_reconnecting() {
    let t = temp();
    let events = run(&t.0, "v2", &turn(&t.0), |rt| rt);
    assert!(assert_failure(&events, "agent-error", "startup", 1).contains("version 2"));
    assert_eq!(spawns(&t.0).len(), 1);
    assert!(prompts(&t.0).is_empty());
}

#[test]
fn fails_a_refused_required_configuration_without_sending_a_prompt_or_retrying() {
    let t = temp();
    let mut chosen = turn(&t.0);
    chosen.model = Some("provider/selected".into());
    let events = run(&t.0, "refuse-config", &chosen, |rt| rt);
    assert!(assert_failure(&events, "agent-error", "startup", 1).contains("would not set model to provider/selected"));
    assert_eq!(spawns(&t.0).len(), 1);
    assert!(prompts(&t.0).is_empty());
    let set = sent(&t.0)
        .into_iter()
        .find(|m| m["message"]["method"] == "session/set_config_option" && m["message"]["params"]["configId"] == "model")
        .unwrap();
    assert_eq!(set["message"]["params"], json!({"sessionId":SESSION,"configId":"model","value":"provider/selected"}));
}

#[test]
fn does_not_spawn_a_process_for_an_already_cancelled_run() {
    let t = temp();
    let events = run(&t.0, "ordinary", &turn(&t.0), |rt| rt.with_cancel(Arc::new(AtomicBool::new(true))));
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.cancelled");
    assert_eq!(last.payload["process"]["recordCount"], 0);
    assert!(spawns(&t.0).is_empty());
    assert!(failure(&events).is_none(), "a stopped run is not a failure: {}", shown(&events));
}

#[test]
fn reports_a_failed_spawn_once_without_retrying_an_invalid_command() {
    let t = temp();
    let mut missing = Command::new(t.0.join("no-such-opencode"));
    missing.arg("acp").current_dir(&t.0);
    let mut rt = OpenCode::spawn(&mut missing).unwrap();
    rt.start(&turn(&t.0)).unwrap();
    let events = drive(&mut rt);
    let said = assert_failure(&events, "process-start-failed", "startup", 1);
    assert!(said.contains("could not start"));
    let last = events.last().unwrap();
    assert_eq!(last.payload["process"]["exitCode"], Value::Null);
    assert_eq!(last.payload["process"]["terminationUnconfirmed"], false);
}

#[test]
fn clears_the_startup_deadline_when_a_long_running_prompt_is_admitted() {
    let t = temp();
    let mut c = command(&t.0, "slow");
    c.env("LOCUST_ACP_SLOW", "3");
    let mut rt = OpenCode::spawn(&mut c).unwrap().with_startup_budget(Duration::from_secs(2));
    rt.start(&turn(&t.0)).unwrap();
    let events = drive(&mut rt);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.completed");
    assert_eq!(last.payload["process"]["cancelled"], false);
    assert!(failure(&events).is_none());
    assert_eq!(prompts(&t.0).len(), 1);
}

#[test]
fn refuses_an_unadvertised_model_instead_of_silently_changing_the_requested_route() {
    let t = temp();
    let mut chosen = turn(&t.0);
    chosen.model = Some("provider/selected".into());
    let events = run(&t.0, "ordinary", &chosen, |rt| rt);
    assert!(assert_failure(&events, "agent-error", "startup", 1).contains("did not offer the required model"));
    assert_eq!(events.last().unwrap().payload["process"]["exitCode"], Value::Null);
    assert!(prompts(&t.0).is_empty());
    assert_eq!(spawns(&t.0).len(), 1);
}

/// The half of "keeps queued steering and excludes abandoned session receipts on a startup retry" that holds
/// natively: the session a failed attempt opened is never this turn's. The queued steering half does not (the
/// native OpenCode driver offers no steering), so the case is not claimed.
#[test]
fn a_session_a_failed_startup_opened_is_left_out_of_the_retry() {
    let t = temp();
    let mut chosen = turn(&t.0);
    chosen.model = Some("provider/selected".into());
    let events = run(&t.0, "abandoned", &chosen, |rt| rt);
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert_eq!(spawns(&t.0).len(), 2);
    let asked = prompts(&t.0);
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0]["message"]["params"]["prompt"], json!([{"type":"text","text":"Inspect the project."}]));
    let all = serde_json::to_string(&events.iter().map(|e| (&e.payload, &e.thread)).collect::<Vec<_>>()).unwrap();
    assert!(!all.contains("abandoned-session"));
    assert_eq!(events.iter().filter(|e| e.payload["code"] == "acp.session").count(), 1);
}
