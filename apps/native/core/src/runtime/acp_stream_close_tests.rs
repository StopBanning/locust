//! Electron's `packages/runtime-adapters/test/an-acp-output-stream-closing-ends-the-turn.test.ts`, through the caller:
//! `Acp` itself (spawn, start, next_event, steer) against an agent on the pipes that closes its output, holds an answer,
//! or exits, as the case has it (acp_agent_support.rs). Electron's fake timers and kill counts have no Rust twin: what
//! each case decides -- how the turn ends, and what the agent was sent -- is read here.
use super::agent_support::*;
use super::*;
use crate::approval::{Channel, Mode};

/// The usual handshake (`session-one`), then `prompt` on the prompt (Python, the prompt is `m`).
fn agent(prompt: &str) -> Agent {
    Agent::new(&format!(
        "for m in messages():\n    if m.get('method') == 'initialize': answer(m, dict(protocolVersion=1, agentCapabilities=dict(loadSession=True)))\n    elif m.get('method') == 'session/new': answer(m, dict(sessionId='session-one'))\n    elif m.get('method') == 'session/prompt':\n{}\n",
        prompt.lines().map(|l| format!("        {l}")).collect::<Vec<_>>().join("\n")
    ))
}

/// Closes its output and stays alive, reading on.
const CLOSE: &str = "sys.stdout.flush(); os.close(1)";

#[test]
fn fails_a_live_child_once_rather_than_waiting_for_the_prompt_timeout() {
    // case: fails a live child once rather than waiting for the prompt timeout
    let agent = agent(CLOSE);
    let mut acp = agent.spawn(added());
    acp.start(&turn(&agent.dir, Mode::Ask, None, None)).unwrap();
    let events = events_until_end(&mut acp, |_, _| {});
    assert_eq!(agent.methods().iter().filter(|m| *m == "session/prompt").count(), 1);
    let end = events.last().unwrap();
    assert_eq!(end.kind, "run.failed");
    assert_eq!(end.payload["process"]["exitCode"], Value::Null);
    assert_eq!(end.payload["process"]["cancelled"], false);
    assert!(end.payload["message"].as_str().unwrap().contains("output stream closed"), "{}", end.payload);
    assert_eq!(events.iter().filter(|e| e.kind.starts_with("run.") && e.kind != "run.started").count(), 1);
    assert!(!acp.steer("Try again"));
}

#[test]
fn reports_an_output_error_without_needing_a_process_exit() {
    // case: reports an output error without needing a process exit
    // A pipe on this machine cannot be made to fail a read, so the reader's own reading of one is checked: the line the
    // reader thread (Acp::spawn) hands on, which ends the turn with these words as a closed stream's do.
    let failed = record_from(Err(io::Error::other("broken pipe")), b"").unwrap().unwrap_err();
    assert_eq!(failed.to_string(), "The agent's output stream failed before its turn finished.");
    let closed = record_from(Ok(0), b"").unwrap().unwrap_err();
    assert_eq!(closed.to_string(), "The agent's output stream closed before its turn finished.");
}

#[test]
fn keeps_the_final_prompt_answer_in_the_last_chunk_before_eof() {
    // case: keeps the final prompt answer in the last chunk before EOF
    let agent = agent(&format!("answer(m, dict(stopReason='end_turn', usage=dict(outputTokens=3)))\n{CLOSE}"));
    let mut acp = agent.spawn(added());
    acp.start(&turn(&agent.dir, Mode::Ask, None, None)).unwrap();
    let events = events_until_end(&mut acp, |_, _| {});
    let end = events.last().unwrap();
    assert_eq!(end.kind, "run.completed");
    assert_eq!(end.payload["process"]["exitCode"], 0);
    assert_eq!(end.payload["process"]["stderr"], "");
    assert_eq!(end.payload["usage"]["outputTokens"].as_f64(), Some(3.0));
}

#[test]
fn does_not_resurrect_a_handshake_after_cancellation_or_its_late_answer() {
    // case: does not resurrect a handshake after cancellation or its late answer
    // The agent answers initialize late, once the run has been stopped.
    let agent = Agent::new(
        "for m in messages():\n    if m.get('method') == 'initialize':\n        while not os.path.exists('stopped'): time.sleep(0.01)\n        answer(m, dict(protocolVersion=1))\n",
    );
    let stop = Arc::new(AtomicBool::new(false));
    let mut acp = agent.spawn(added()).with_cancel(stop.clone());
    let (dir, sent, flag) = (agent.dir.clone(), agent.dir.join("sent.jsonl"), stop.clone());
    let stopping = std::thread::spawn(move || {
        crate::load_waits::wait_for("initialize to be sent", || std::fs::read_to_string(&sent).unwrap_or_default().contains("initialize"));
        flag.store(true, Ordering::Relaxed);
        std::fs::write(dir.join("stopped"), "").unwrap();
    });
    acp.start(&turn(&agent.dir, Mode::Ask, None, None)).unwrap();
    let events = events_until_end(&mut acp, |_, _| {});
    stopping.join().unwrap();
    let end = events.last().unwrap();
    assert_eq!(end.kind, "run.cancelled");
    assert_eq!(end.payload["process"]["exitCode"], Value::Null);
    assert_eq!(end.payload["process"]["cancelled"], true);
    assert_eq!(agent.methods(), ["initialize"]);
}

/// A run stopped once the agent has been sent `method`, whose answer the agent holds until then.
fn stopped_at(method: &str) -> (Agent, Vec<Event>) {
    let agent = Agent::new(&format!(
        r#"
for m in messages():
    if m.get('method') == {method:?}:
        while not os.path.exists('stopped'): time.sleep(0.01)
    if m.get('method') == 'initialize': answer(m, dict(protocolVersion=1, agentCapabilities=dict(loadSession=True)))
    elif m.get('method') == 'session/new': answer(m, dict(sessionId='session-one'))
"#
    ));
    let stop = Arc::new(AtomicBool::new(false));
    let mut acp = agent.spawn(added()).with_cancel(stop.clone());
    let (dir, sent, flag, wanted) = (agent.dir.clone(), agent.dir.join("sent.jsonl"), stop.clone(), method.to_owned());
    let stopping = std::thread::spawn(move || {
        crate::load_waits::wait_for("the held request to be sent", || {
            std::fs::read_to_string(&sent).unwrap_or_default().contains(&format!("\"{wanted}\""))
        });
        flag.store(true, Ordering::Relaxed);
        std::fs::write(dir.join("stopped"), "").unwrap();
    });
    acp.start(&turn(&agent.dir, Mode::Ask, None, None)).unwrap();
    let events = events_until_end(&mut acp, |_, _| {});
    stopping.join().unwrap();
    (agent, events)
}

#[test]
fn does_not_send_a_prompt_if_cancellation_races_the_session_answer() {
    // case: does not send a prompt if cancellation races the session answer
    let (agent, events) = stopped_at("session/new");
    assert_eq!(events.last().unwrap().kind, "run.cancelled");
    assert_eq!(agent.methods(), ["initialize", "session/new"]);
}

#[test]
fn does_not_begin_a_session_after_the_capabilities_callback_cancels_the_run() {
    // case: does not begin a session after the capabilities callback cancels the run
    // Rust has no capabilities callback of the host's (they go to the store, which never stops a turn); a stop that
    // comes with the capabilities' answer is the same moment, and begins no session either.
    let (agent, events) = stopped_at("initialize");
    assert_eq!(events.last().unwrap().payload["process"]["cancelled"], true);
    assert_eq!(agent.methods(), ["initialize"]);
}

#[test]
fn does_not_start_a_session_after_timeout_exit_or_an_already_aborted_signal() {
    // case: does not start a session after timeout, exit, or an already aborted signal
    let silent = Agent::new("for m in messages(): pass\n");
    // Long enough for the agent to have started and written down what it was sent, under load; it still never answers.
    let mut timed = silent.spawn(Profile { deadline: Some(Instant::now() + Duration::from_secs(3)), ..Default::default() });
    timed.start(&turn(&silent.dir, Mode::Ask, None, None)).unwrap();
    let end = events_until_end(&mut timed, |_, _| {}).pop().unwrap();
    assert_eq!(end.payload["process"]["exitCode"], Value::Null);
    assert_eq!(end.payload["message"], "The agent did not answer in time.");

    let gone = Agent::new("for m in messages():\n    if m.get('method') == 'initialize': sys.exit(0)\n");
    let end = whole_turn(&gone, added(), Mode::Ask, None).pop().unwrap();
    assert_eq!(end.payload["process"]["exitCode"], Value::Null);

    let stopped = Agent::new("for m in messages():\n    if m.get('method') == 'initialize': answer(m, dict(protocolVersion=1))\n");
    let mut acp = stopped.spawn(added()).with_cancel(Arc::new(AtomicBool::new(true)));
    acp.start(&turn(&stopped.dir, Mode::Ask, None, None)).unwrap();
    let end = events_until_end(&mut acp, |_, _| {}).pop().unwrap();
    assert_eq!(end.payload["process"]["cancelled"], true);

    assert_eq!(silent.methods(), ["initialize"]);
    assert_eq!(gone.methods(), ["initialize"]);
    assert!(stopped.sent().is_empty());
}

#[test]
fn releases_queued_permission_waits_when_eof_ends_the_run() {
    // case: releases queued permission waits when EOF ends the run
    let agent = agent(
        r#"for i in (80, 81):
    say(dict(id=i, method='session/request_permission', params=dict(sessionId='session-one', toolCall=dict(toolCallId=str(i), kind='execute', rawInput=dict(command='publish')), options=[dict(optionId='yes', kind='allow_once', name='Allow')])))
while not os.path.exists('asked'): time.sleep(0.01)
sys.stdout.flush(); os.close(1)"#,
    );
    let channel = Channel::new();
    let mut acp = agent.spawn(added());
    acp.start(&turn(&agent.dir, Mode::ApproveEach, None, Some(channel.clone()))).unwrap();
    // The host never answers: the card stays up until the agent's output ends.
    let (host, dir) = (channel.clone(), agent.dir.clone());
    let watching = std::thread::spawn(move || {
        let mut cards = std::collections::HashSet::new();
        crate::load_waits::wait_for("the first card", || !host.waiting().is_empty());
        std::fs::write(dir.join("asked"), "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            cards.extend(host.waiting().into_iter().map(|c| c.approval_id));
            std::thread::sleep(Duration::from_millis(5));
        }
        cards.len()
    });
    let events = events_until_end(&mut acp, |_, _| {});
    assert_eq!(events.last().unwrap().kind, "run.failed");
    assert_eq!(watching.join().unwrap(), 1, "the host was asked once");
    assert!(channel.waiting().is_empty());
    assert!(agent.sent().iter().all(|m| m["id"] != 80 && m["id"] != 81));
}

#[test]
fn clears_the_cancellation_grace_timer_when_the_prompt_answers() {
    // case: clears the cancellation grace timer when the prompt answers
    let agent = Agent::new(
        r#"
prompt = None
for m in messages():
    if m.get('method') == 'initialize': answer(m, dict(protocolVersion=1, agentCapabilities=dict(loadSession=True)))
    elif m.get('method') == 'session/new': answer(m, dict(sessionId='session-one'))
    elif m.get('method') == 'session/prompt': prompt = m
    elif m.get('method') == 'session/cancel': answer(prompt, dict(stopReason='cancelled'))
"#,
    );
    // A grace far longer than the test: the turn ends on the prompt's answer, not on the grace running out.
    let mut acp = agent.spawn(added()).with_cancel_grace(Duration::from_secs(600));
    acp.start(&turn(&agent.dir, Mode::Ask, None, None)).unwrap();
    acp.stop().unwrap();
    let began = Instant::now();
    let events = events_until_end(&mut acp, |_, _| {});
    assert!(began.elapsed() < Duration::from_secs(60));
    assert_eq!(events.last().unwrap().kind, "run.cancelled");
    assert!(agent.methods().iter().any(|m| m == "session/cancel"));
}
