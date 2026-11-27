//! Antigravity over `agy acp` (27 Nov 2026): agy-acp-run.ts and antigravity-runs-over-acp.test.ts, through the caller --
//! `Antigravity` itself (spawn, start, next_event), as native.rs and run_cli.rs drive it -- against a stand-in `agy`
//! that speaks what agy 1.5.18 was measured speaking that day (~/work/scratch/agy-acp-probe/): string request ids,
//! `allow`/`deny` options, the command in `rawInput.CommandLine`, its question as an `elicitation/create` form, and
//! modes, model and effort as settings whose effort choices depend on the model.
use super::agent_support::*;
use super::*;
use crate::approval::{Answer, Channel, Decision, Kind, Mode};
use crate::runtime::antigravity::{self, Antigravity};

/// agy's settings and session, then `turn_start` on each prompt and `turn_reply` on each answer to what agy asked.
const AGY: &str = r#"
current = dict(model='gemini-4-neon', thought_level='high', mode='default')
def efforts():
    m = current['model']
    if m == 'gemini-3.8-flash': return ['low', 'medium', 'high']
    if m.startswith('gemini-pro-orchestrator') or m.startswith('gpt-oss'): return []
    return ['low', 'medium', 'high', 'xhigh']
def config():
    out = [dict(id='model', type='select', currentValue=current['model'], options=[dict(value=v) for v in ['gemini-4-neon', 'gemini-3.8-flash', 'gemini-pro-orchestrator-neon', 'gpt-oss-120b-medium']])]
    if efforts(): out.append(dict(id='thought_level', type='select', currentValue=current['thought_level'], options=[dict(value=v) for v in efforts()]))
    out.append(dict(id='mode', type='select', currentValue=current['mode'], options=[dict(value=v) for v in ['default', 'accept-edits', 'auto', 'plan']]))
    return out
RUN = dict(sessionUpdate='tool_call', toolCallId='call_1', name='run_command', status='pending', kind='execute', title='date > when.txt', rawInput=dict(CommandLine='date > when.txt'))
ALLOW_DENY = [dict(optionId='allow', name='Allow', kind='allow_once'), dict(optionId='deny', name='Deny', kind='reject_once')]
FORM = dict(sessionId='s1', toolCallId='call_q', mode='form', message='The agent has a question.', requestedSchema=dict(type='object', properties=dict(
    q1=dict(type='string', title='Which color?', oneOf=[dict(const='1', title='(Recommended) Red'), dict(const='2', title='Blue')]),
    q1_other=dict(type='string', title='Other', description='Optional free-form answer to: Which color?'))))
def said(text):
    update(dict(sessionUpdate='agent_message_chunk', content=dict(type='text', text=text)))
prompt = None
for m in messages():
    meth = m.get('method')
    if meth == 'initialize':
        answer(m, dict(protocolVersion=1, agentCapabilities=dict(loadSession=True)))
    elif meth in ('session/new', 'session/load'):
        answer(m, dict(sessionId='s1', configOptions=config()))
    elif meth == 'session/set_config_option':
        k, v = m['params']['configId'], m['params']['value']
        if k == 'thought_level' and v not in efforts():
            say(dict(id=m['id'], error=dict(code=-32602, message='Thinking level "%s" is not offered' % v)))
            continue
        current[k] = v
        answer(m, dict(configOptions=config()))
    elif meth == 'session/prompt':
        prompt = m
        TURN_START
    elif 'id' in m and 'method' not in m:
        TURN_REPLY
"#;

/// A stand-in agy, started as native.rs starts it: an executable named `agy`, its first argument `acp`.
fn agy(turn_start: &str, turn_reply: &str) -> (Agent, Command) {
    agy_from(&AGY.replace("TURN_START", turn_start).replace("TURN_REPLY", turn_reply))
}

fn agy_from(script: &str) -> (Agent, Command) {
    let agent = Agent::new(script);
    let launcher = agent.dir.join("agy");
    std::fs::write(&launcher, "#!/bin/sh\nexec python3 -u \"$(dirname \"$0\")/agent.py\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut command = Command::new(&launcher);
    command.arg("acp").current_dir(&agent.dir).env("LOCUST_SENT", agent.dir.join("sent.jsonl"));
    (agent, command)
}

/// agy asks to run one command, and says what it got back.
fn asking_to_run() -> (Agent, Command) {
    agy(
        "update(RUN); say(dict(id='perm-1', method='session/request_permission', params=dict(sessionId='s1', toolCall=RUN, options=ALLOW_DENY)))",
        "said('got: ' + json.dumps(m.get('result'), sort_keys=True)); ended(prompt)",
    )
}

fn agy_turn(folder: &std::path::Path, mode: Mode, model: &str, effort: Option<&str>, approvals: Option<Arc<Channel>>) -> Turn {
    let mut t = turn(folder, mode, None, approvals);
    t.model = Some(model.into());
    t.effort = effort.map(str::to_owned);
    t
}

/// The whole turn through `Antigravity`, `host` answering any card on its own thread.
fn run_agy(command: &mut Command, t: &Turn, host: Option<(Arc<Channel>, Box<dyn Fn(&crate::approval::Request) -> Answer + Send>)>) -> Vec<Event> {
    let mut rt = Antigravity::spawn(command).unwrap();
    assert!(matches!(rt, Antigravity::Acp(_)), "a command beginning `acp` runs over the protocol");
    let answering = host.map(|(channel, decide)| {
        std::thread::spawn(move || {
            let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
            while Instant::now() < deadline {
                if let Some(card) = channel.waiting().first() {
                    channel.decide(decide(card)).unwrap();
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        })
    });
    rt.start(t).unwrap();
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "the agy turn never ended: {:?}", kinds(&events));
        if let Some(e) = rt.next_event().unwrap() {
            let end = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            events.push(e);
            if end {
                break;
            }
        }
    }
    if let Some(a) = answering {
        a.join().unwrap();
    }
    events
}

fn reply_to(agent: &Agent, id: &str) -> Value {
    agent.sent().into_iter().find(|m| m["id"] == id && m["method"].is_null()).map(|m| m["result"].clone()).unwrap_or(Value::Null)
}

fn settings_set(agent: &Agent) -> Vec<(String, String)> {
    agent
        .sent()
        .iter()
        .filter(|m| m["method"] == "session/set_config_option")
        .map(|m| (m["params"]["configId"].as_str().unwrap().to_owned(), m["params"]["value"].as_str().unwrap().to_owned()))
        .collect()
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.to_owned(), b.to_owned())
}

#[test]
fn antigravity_acp_events_approval_puts_the_command_agy_asks_to_run_on_a_card_in_approve_each_and_the_yes_goes_back_as_agys_allow() {
    // case: puts the command agy asks to run on a card in Approve each, and the person's yes goes back as agy's own "allow"
    // case: reads agy's command from CommandLine and its file from Path, so the card says what it will do
    let (agent, mut command) = asking_to_run();
    let channel = Channel::new();
    let seen = Arc::new(Mutex::new(None::<crate::approval::Request>));
    let kept = seen.clone();
    let events = run_agy(
        &mut command,
        &agy_turn(&agent.dir, Mode::ApproveEach, "gemini-3.8-flash", Some("low"), Some(channel.clone())),
        Some((channel, Box::new(move |card| {
            *crate::invariants::lock(&kept) = Some(card.clone());
            Answer::Decision { approval_id: card.approval_id.clone(), decision: Decision::ApproveOnce, reason: None }
        }))),
    );
    let card = crate::invariants::lock(&seen).clone().expect("agy's request reached a card");
    assert_eq!(card.runtime, "antigravity");
    assert_eq!(card.kind, Kind::Command);
    assert!(card.detail.contains("date > when.txt"), "the card says the command agy named in CommandLine: {}", card.detail);
    assert_eq!(reply_to(&agent, "perm-1"), json!({"outcome":{"outcome":"selected","optionId":"allow"}}));
    assert_eq!(kinds(&events).last(), Some(&"run.completed"));
    // Approve each is agy's own `default` mode, which asks: nothing to switch.
    assert_eq!(settings_set(&agent), vec![pair("model", "gemini-3.8-flash"), pair("thought_level", "low")]);
    // And the activity row names the command too.
    let started = events.iter().find(|e| e.kind == "tool.started").expect("the command's row");
    assert_eq!(started.payload["command"], "date > when.txt");
}

#[test]
fn antigravity_acp_runs_each_mode_as_agys_and_answers_what_agy_asks_without_a_card() {
    // case: runs %s as agy's %s mode and answers what agy asks without a card
    // case: maps each Locust mode to agy's, and who answers what agy asks
    for (mode, agy_mode, answered) in [
        (Mode::Ask, "default", "deny"),
        (Mode::Plan, "plan", "deny"),
        (Mode::AcceptEdits, "accept-edits", "deny"),
        (Mode::Auto, "auto", "allow"),
    ] {
        let (agent, mut command) = asking_to_run();
        let channel = Channel::new();
        run_agy(&mut command, &agy_turn(&agent.dir, mode, "gemini-3.8-flash", Some("low"), Some(channel.clone())), None);
        assert!(channel.take_recorded_for("acp_case").is_empty(), "{agy_mode}: no card");
        let mut expected = vec![pair("model", "gemini-3.8-flash")];
        if agy_mode != "default" {
            expected.push(pair("mode", agy_mode));
        }
        expected.push(pair("thought_level", "low"));
        // The model first, then the mode (unless agy is in it already), then the effort the model lists.
        assert_eq!(settings_set(&agent), expected, "{agy_mode}");
        assert_eq!(reply_to(&agent, "perm-1"), json!({"outcome":{"outcome":"selected","optionId":answered}}), "{agy_mode}");
    }
}

#[test]
fn antigravity_acp_leaves_out_an_effort_the_model_does_not_list_and_asks_for_gpt_oss_by_agys_whole_id() {
    // case: leaves out an effort the model does not list, and asks for gpt-oss by agy's whole id
    // case: asks for a model by the id agy lists
    let (agent, mut command) = asking_to_run();
    let events = run_agy(&mut command, &agy_turn(&agent.dir, Mode::Ask, "gemini-3.8-flash", Some("xhigh"), None), None);
    assert_eq!(settings_set(&agent), vec![pair("model", "gemini-3.8-flash")]);
    assert_eq!(kinds(&events).last(), Some(&"run.completed"));
    let (agent, mut command) = asking_to_run();
    run_agy(&mut command, &agy_turn(&agent.dir, Mode::Ask, "gpt-oss-120b-medium", None, None), None);
    assert_eq!(settings_set(&agent), vec![pair("model", "gpt-oss-120b-medium")]);
}

/// An executable that prints `help` to stderr, as agy prints its usage.
fn fake_agy(help: &str) -> (Agent, std::path::PathBuf) {
    let agent = Agent::new("pass");
    let path = agent.dir.join("agy");
    std::fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{help}' >&2\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    (agent, path)
}

#[test]
fn antigravity_falls_back_to_agy_p_where_agy_has_no_acp() {
    // case: falls back to agy -p when agy has no acp
    // case: starts agy acp with the skills folder, and reference folders only in a read-only run
    let (_new, speaks) = fake_agy("Start the Agent Client Protocol (ACP) stdio server");
    let (_old, silent) = fake_agy("unknown command \"acp\" for \"agy\"");
    assert!(antigravity::agy_speaks_acp(&speaks));
    assert!(!antigravity::agy_speaks_acp(&silent));
    // What a sweep read off a binary is what the turn goes by, not asked again.
    let (_kept, remembered) = fake_agy("Start the Agent Client Protocol (ACP) stdio server");
    antigravity::remember_acp_help(&remembered, "Usage of agy: --help");
    assert!(!antigravity::agy_speaks_acp(&remembered));
    // A `-p` command runs on the print route; an `acp` one over the protocol.
    let mut print = Command::new(&silent);
    print.args(antigravity::print_args(Mode::AcceptEdits, Some("gemini-3.8-flash"), Some("low"), None, &[], None));
    assert!(matches!(Antigravity::spawn(&mut print).unwrap(), Antigravity::Print(_)));
    let mut acp = Command::new(&speaks);
    acp.args(antigravity::acp_args(Mode::Auto, &["/work/ref"], Some("/tmp/skills")));
    assert!(matches!(Antigravity::spawn(&mut acp).unwrap(), Antigravity::Acp(_)));
    // Auto over the protocol never needs the flag that lets `-p` allow everything; references only when read-only.
    let auto = antigravity::acp_args(Mode::Auto, &["/work/ref"], Some("/tmp/skills"));
    assert_eq!(auto, ["acp", "--add-dir", "/tmp/skills"]);
    assert_eq!(antigravity::acp_args(Mode::Ask, &["/work/ref"], None), ["acp", "--add-dir", "/work/ref"]);
}

#[test]
fn antigravity_command_for_takes_agy_acp_where_agy_speaks_it_and_p_where_it_does_not() {
    // case: refuses Approve each in words where agy has no acp, rather than run it as Edit
    let (_new, speaks) = fake_agy("Start the Agent Client Protocol (ACP) stdio server");
    let (_old, silent) = fake_agy("unknown command \"acp\" for \"agy\"");
    let folder = std::env::temp_dir();
    let args = |c: &Command| c.get_args().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>();
    // Closed in tests unless a test opens it: this machine's agy never decides a test.
    antigravity::open_acp_route_in_tests(false);
    assert!(antigravity::command_for(speaks.clone(), &folder, Mode::ApproveEach, None, None, None, &[], None).is_err());
    antigravity::open_acp_route_in_tests(true);
    let over = antigravity::command_for(speaks, &folder, Mode::ApproveEach, Some("gemini-3.8-flash"), Some("low"), Some("conv-1"), &[], None).unwrap();
    // The model, effort and conversation go over the protocol, never on argv.
    assert_eq!(args(&over), ["acp"]);
    let refused = antigravity::command_for(silent.clone(), &folder, Mode::ApproveEach, None, None, None, &[], None).unwrap_err();
    assert_eq!(refused.to_string(), antigravity::NO_APPROVE_EACH_WITHOUT_ACP, "-p takes no Approve each");
    assert!(refused.to_string().contains("agy update"));
    let print = antigravity::command_for(silent, &folder, Mode::AcceptEdits, Some("gemini-3.8-flash"), Some("low"), None, &[], None).unwrap();
    let print = args(&print);
    assert!(print.contains(&"-p=".to_owned()) && print.windows(2).any(|w| w == ["--mode", "accept-edits"]), "{print:?}");
    antigravity::open_acp_route_in_tests(false);
}
