//! Electron's `packages/runtime-adapters/test/acp-run.test.ts`, case by case, through the caller: `Acp` itself (spawn,
//! start, next_event, steer, stop) against an agent on the pipes that follows the case's script (acp_agent_support.rs),
//! as copilot.rs, opencode.rs and native.rs drive it. Electron's `records` are read here as the events the turn made,
//! and its `agent.sent` as what the agent wrote down it was sent.
use super::agent_support::*;
use super::*;
use crate::approval::{Answer, Channel, Decision, Mode};

const AGENT_MODE: &str = "https://agentclientprotocol.com/protocol/session-modes#agent";

/// `agentWith(turn)`: starts as the usual agent does and hands each prompt to `turn` (Python, the prompt is `m`).
fn agent_with(turn: &str) -> Agent {
    Agent::new(&format!(
        "for m in messages():\n    if usual(m): continue\n    if m.get('method') == 'session/prompt':\n{}\n",
        indent(turn, 8)
    ))
}

fn indent(body: &str, by: usize) -> String {
    body.lines().map(|l| format!("{}{l}", " ".repeat(by))).collect::<Vec<_>>().join("\n")
}

fn end_of(events: &[Event]) -> &Event {
    events.last().unwrap()
}

#[test]
fn offers_the_agent_nothing_starts_in_the_folder_it_names_and_records_the_turn() {
    // case: offers the agent nothing, starts in the folder it names, and records the turn
    let agent = agent_with("update(dict(sessionUpdate='agent_message_chunk', content=dict(type='text', text='Done.')))\nended(m)");
    let events = whole_turn(&agent, copilot(), Mode::Ask, None);
    let sent = agent.sent();
    let init = sent.iter().find(|m| m["method"] == "initialize").unwrap();
    assert_eq!(init["params"], json!({"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false}}));
    // The process is started IN the folder as well as told it: the command acp.rs builds for an added agent.
    let saved = &agent.dir;
    std::fs::write(saved.join("acp-agents.json"), r#"{"agents":[{"id":"acp_case","name":"Case","commandLine":"/usr/bin/true acp"}]}"#).unwrap();
    let built = super::command(saved, saved, Some("acp_case")).unwrap();
    assert_eq!(built.get_current_dir(), Some(saved.as_path()));
    let new = sent.iter().find(|m| m["method"] == "session/new").unwrap();
    assert_eq!(new["params"], json!({"cwd":agent.dir,"mcpServers":[]}));
    assert_eq!(kinds_of(&product(&events)), ["run.started", "message.delta", "message.delta", "run.completed"]);
    assert_eq!(said(&events), ["Done."]);
    let process = &end_of(&events).payload["process"];
    assert_eq!(process["exitCode"], 0);
    assert_eq!(process["cancelled"], false);
}

fn kinds_of<'a>(events: &[&'a Event]) -> Vec<&'a str> {
    events.iter().map(|e| e.kind.as_str()).collect()
}

#[test]
fn never_records_the_machines_own_commands_or_anything_said_for_another_session() {
    // case: never records the machine's own commands, or anything said for another session
    let agent = agent_with(
        "update(dict(sessionUpdate='available_commands_update', availableCommands=[dict(name='my-private-skill', description=\"the operator's own\")]))\nupdate(dict(sessionUpdate='agent_message_chunk', content=dict(type='text', text='not this run')), session='s2')\nupdate(dict(sessionUpdate='agent_message_chunk', content=dict(type='text', text='Done.')))\nended(m)",
    );
    let events = whole_turn(&agent, copilot(), Mode::Ask, None);
    let kept = shown(&events);
    assert!(!kept.contains("my-private-skill"));
    assert!(!kept.contains("not this run"));
    assert_eq!(said(&events), ["Done."]);
}

#[test]
fn continues_a_session_by_loading_it_and_keeps_none_of_what_the_load_replays() {
    // case: continues a session by loading it, and keeps none of what the load replays
    let agent = Agent::new(
        r#"
for m in messages():
    if usual(m): continue
    if m.get('method') == 'session/load':
        # As Copilot does: the whole conversation first, then the answer.
        update(dict(sessionUpdate='user_message_chunk', content=dict(type='text', text='Remember the word PLUM-3.')))
        update(dict(sessionUpdate='agent_message_chunk', content=dict(type='text', text='OK')))
        answer(m, dict(modes=SESSION['modes'], configOptions=SESSION['configOptions']))
    elif m.get('method') == 'session/prompt':
        update(dict(sessionUpdate='agent_message_chunk', content=dict(type='text', text='ACP-OK-7 PLUM-3')))
        ended(m)
"#,
    );
    let events = whole_turn(&agent, copilot(), Mode::Ask, Some("s1"));
    let load = agent.sent().into_iter().find(|m| m["method"] == "session/load").unwrap();
    assert_eq!(load["params"], json!({"sessionId":"s1","cwd":agent.dir,"mcpServers":[]}));
    assert_eq!(said(&events), ["ACP-OK-7 PLUM-3"]);
    assert!(!shown(&events).contains("PLUM-3."));
}

// Since 0.616 the run remembers nothing: every request goes to the host (the approval channel), which keeps the Always
// and decides after the rules.
#[test]
fn answers_by_the_kind_of_option_never_says_always_to_the_agent_and_asks_the_host_about_every_request() {
    // case: answers by the KIND of option, never says 'always' to the agent, and asks the host about every request
    let agent = agent_with(
        r#"def ask(i, command):
    return dict(id=i, method='session/request_permission', params=dict(sessionId='s1', toolCall=dict(toolCallId='t%d' % i, title='Run the tests', kind='execute', rawInput=dict(command=command)), options=OPTIONS))
say(ask(100, 'npm test')); say(ask(101, 'npm test')); say(ask(102, 'rm -rf build'))
answered = 0
for a in messages():
    if a.get('id') in (100, 101, 102) and 'result' in a: answered += 1
    if answered == 3: break
# The refusal is the call's: it fails, and the record calls it declined.
update(dict(sessionUpdate='tool_call_update', toolCallId='t102', kind='execute', status='failed', rawInput=dict(command='rm -rf build')))
ended(m)"#,
    );
    let channel = Channel::new();
    let mut acp = agent.spawn(added());
    acp.start(&turn(&agent.dir, Mode::ApproveEach, None, Some(channel.clone()))).unwrap();
    let host = channel.clone();
    let answering = std::thread::spawn(move || {
        let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
        let mut answered = 0;
        while answered < 2 && Instant::now() < deadline {
            if let Some(card) = host.waiting().first() {
                let tests = card.detail.contains("npm test") || card.summary.contains("npm test");
                host.decide(Answer::Decision {
                    approval_id: card.approval_id.clone(),
                    decision: if tests { Decision::ApproveAlways } else { Decision::Deny },
                    reason: None,
                })
                .unwrap();
                answered += 1;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let events = events_until_end(&mut acp, |_, _| {});
    answering.join().unwrap();
    let result = |id| agent.sent_with_id(id).unwrap()["result"].clone();
    assert_eq!(result(100), json!({"outcome":{"outcome":"selected","optionId":"yes"}}));
    // The second, identical, is put to the host too, and answered "once" again.
    assert_eq!(result(101), json!({"outcome":{"outcome":"selected","optionId":"yes"}}));
    assert_eq!(result(102), json!({"outcome":{"outcome":"selected","optionId":"no"}}));
    let asked = channel.take_recorded_for("acp_case");
    assert_eq!(asked.len(), 3, "the host decides every request");
    assert!(!format!("{}{}{}", result(100), result(101), result(102)).contains("yes-forever"));
    // The refusal is on the record, for the normalizer to call it declined.
    let refused = first(&events, "tool.failed");
    assert_eq!(refused["itemId"], "t102");
    assert_eq!(refused["status"], "declined");
}

/// An agent that starts a shell call, asks about it as request 7, and on the answer fails it and ends on `stop`.
fn refused_call(stop: &str) -> Agent {
    Agent::new(&format!(
        r#"
prompt = None
for m in messages():
    if usual(m): continue
    if m.get('method') == 'session/prompt':
        prompt = m
        update(dict(sessionUpdate='tool_call', toolCallId='t1', kind='execute', status='pending', rawInput=dict(command='echo hi')))
        say(dict(id=7, method='session/request_permission', params=dict(sessionId='s1', toolCall=dict(toolCallId='t1', kind='execute', rawInput=dict(command='echo hi')), options=OPTIONS)))
    elif m.get('id') == 7 and 'result' in m:
        update(dict(sessionUpdate='tool_call_update', toolCallId='t1', status='failed', rawOutput=dict(message='The user rejected this tool call.')))
        answer(prompt, dict(stopReason={stop:?}))
"#
    ))
}

#[test]
fn writes_a_refusal_down_before_the_answer_goes_back_so_it_comes_before_the_failed_call_it_explains() {
    // case: writes a refusal down BEFORE the answer goes back, so it comes before the failed call it explains
    // Ask: the host's own refusal. Written down first, the failed call that follows is the refused one.
    let agent = refused_call("end_turn");
    let events = whole_turn(&agent, added(), Mode::Ask, None);
    assert_eq!(agent.sent_with_id(7).unwrap()["result"], json!({"outcome":{"outcome":"selected","optionId":"no"}}));
    let failed = first(&events, "tool.failed");
    assert_eq!(failed["status"], "declined");
}

#[test]
fn a_turn_the_agent_ends_cancelled_on_the_hosts_own_refusal_finished_declined_it_was_not_stopped() {
    // case: a turn the agent ends 'cancelled' on the host's own refusal finished, declined -- it was not stopped (OpenCode 2.1.0)
    let events = whole_turn(&refused_call("cancelled"), opencode(), Mode::Ask, None);
    let end = end_of(&events);
    assert_eq!(end.payload["process"]["cancelled"], false);
    assert_eq!(end.payload["process"]["exitCode"], 0);
    assert!(events.iter().all(|e| e.kind != "run.cancelled"));
    assert_eq!(end.kind, "run.completed");
}

#[test]
fn a_turn_the_agent_ends_cancelled_with_nothing_refused_still_reads_as_stopped() {
    // case: a turn the agent ends 'cancelled' with nothing refused still reads as stopped (control)
    let events = whole_turn(&agent_with("ended(m, 'cancelled')"), opencode(), Mode::Ask, None);
    assert_eq!(end_of(&events).kind, "run.cancelled");
}

#[test]
fn refuses_anything_else_the_agent_asks_files_a_terminal_as_a_method_it_was_never_offered() {
    // case: refuses anything else the agent asks -- files, a terminal -- as a method it was never offered
    let agent = agent_with(
        r#"say(dict(id=50, method='fs/read_text_file', params=dict(sessionId='s1', path='C:\\Users\\person\\.ssh\\id_rsa')))
say(dict(id=51, method='terminal/create', params=dict(sessionId='s1', command='whoami')))
seen = 0
for a in messages():
    if a.get('id') in (50, 51): seen += 1
    if seen == 2: break
ended(m)"#,
    );
    whole_turn(&agent, copilot(), Mode::Ask, None);
    for id in [50, 51] {
        assert_eq!(agent.sent_with_id(id).unwrap()["error"]["code"], -32601);
    }
}

#[test]
fn on_stop_cancels_the_turn_the_protocols_way_and_answers_the_open_question_cancelled() {
    // case: on Stop, cancels the turn the protocol's way and answers the open question 'cancelled'
    // The prompt ends once the open question has been answered and the turn cancelled, as the spec has it.
    let agent = Agent::new(
        r#"
prompt, answered, cancelled = None, False, False
for m in messages():
    if usual(m): continue
    if m.get('method') == 'session/prompt':
        prompt = m
        say(dict(id=9, method='session/request_permission', params=dict(sessionId='s1', toolCall=dict(toolCallId='t9', kind='execute', rawInput=dict(command='npm publish')), options=OPTIONS)))
    if m.get('id') == 9 and 'result' in m: answered = True
    if m.get('method') == 'session/cancel': cancelled = True
    if answered and cancelled:
        ended(prompt, 'cancelled'); answered = False
"#,
    );
    let channel = Channel::new();
    let stop = Arc::new(AtomicBool::new(false));
    let mut acp = agent.spawn(added()).with_cancel(stop.clone());
    acp.start(&turn(&agent.dir, Mode::ApproveEach, None, Some(channel.clone()))).unwrap();
    // The card is up, and the person presses Stop instead of answering.
    let host = channel.clone();
    let pressing = std::thread::spawn(move || {
        crate::load_waits::wait_for("the card to be up", || !host.waiting().is_empty());
        stop.store(true, Ordering::Relaxed);
    });
    let events = events_until_end(&mut acp, |_, _| {});
    pressing.join().unwrap();
    let end = end_of(&events);
    assert_eq!(end.kind, "run.cancelled");
    assert_eq!(end.payload["process"]["cancelled"], true);
    assert_eq!(end.payload["process"]["exitCode"], Value::Null);
    assert!(agent.sent().iter().any(|m| m["method"] == "session/cancel" && m["params"]["sessionId"] == "s1"));
    assert_eq!(agent.sent_with_id(9).unwrap()["result"], json!({"outcome":{"outcome":"cancelled"}}));
}

#[test]
fn ends_a_stopped_turn_anyway_when_the_agent_never_says_it_stopped() {
    // case: ends a stopped turn anyway when the agent never says it stopped
    let agent = agent_with("pass");
    let mut acp = agent.spawn(copilot()).with_cancel_grace(Duration::from_millis(10));
    acp.start(&turn(&agent.dir, Mode::Ask, None, None)).unwrap();
    acp.stop().unwrap();
    let events = events_until_end(&mut acp, |_, _| {});
    assert_eq!(end_of(&events).kind, "run.cancelled");
    assert_eq!(end_of(&events).payload["process"]["cancelled"], true);
}

#[test]
fn puts_a_session_back_into_the_mode_that_asks_and_will_not_run_one_that_cannot_be() {
    // case: puts a session back into the mode that asks, and will not run one that cannot be
    let switched = Agent::new(
        r#"
autopilot = dict(SESSION, modes=dict(SESSION['modes'], currentModeId=AUTOPILOT))
for m in messages():
    if usual(m, session=autopilot): continue
    if m.get('method') == 'session/set_mode': answer(m, {})
    elif m.get('method') == 'session/prompt': ended(m)
"#,
    );
    whole_turn(&switched, copilot(), Mode::Ask, None);
    let sent = switched.sent();
    let set_mode = sent.iter().position(|m| m["method"] == "session/set_mode").unwrap();
    assert_eq!(sent[set_mode]["params"], json!({"sessionId":"s1","modeId":AGENT_MODE}));
    assert!(set_mode < sent.iter().position(|m| m["method"] == "session/prompt").unwrap());

    let refused = Agent::new(
        r#"
no_agent = dict(SESSION, modes=dict(currentModeId=AUTOPILOT, availableModes=[dict(id=AUTOPILOT, name='Autopilot')]))
for m in messages():
    if usual(m, session=no_agent): continue
"#,
    );
    let events = whole_turn(&refused, copilot(), Mode::Ask, None);
    let end = &end_of(&events).payload;
    assert_eq!(end["process"]["exitCode"], Value::Null);
    assert!(end["message"].as_str().unwrap().contains("did not offer the mode"), "{end}");
    assert!(!refused.methods().iter().any(|m| m == "session/prompt"));
}

#[test]
fn turns_allow_all_off_before_the_prompt_and_will_not_run_an_agent_that_keeps_it_on() {
    // case: turns allow-all off before the prompt, and will not run an agent that keeps it on
    let agent_setting = |after: &str| {
        Agent::new(&format!(
            r#"
def allow_all(value): return [dict(SESSION['configOptions'][0], currentValue=value)]
for m in messages():
    if usual(m, session=dict(SESSION, configOptions=allow_all('on'))): continue
    if m.get('method') == 'session/set_config_option': answer(m, dict(configOptions=allow_all({after:?})))
    elif m.get('method') == 'session/prompt': ended(m)
"#
        ))
    };
    let fixed = agent_setting("off");
    let events = whole_turn(&fixed, copilot(), Mode::Ask, None);
    assert_eq!(end_of(&events).payload["process"]["exitCode"], 0);
    let set = fixed.sent().into_iter().find(|m| m["method"] == "session/set_config_option").unwrap();
    assert_eq!(set["params"], json!({"sessionId":"s1","configId":"allow_all","value":"off"}));

    let stubborn = agent_setting("on");
    let events = whole_turn(&stubborn, copilot(), Mode::Ask, None);
    let end = &end_of(&events).payload;
    assert_eq!(end["process"]["exitCode"], Value::Null);
    assert!(end["message"].as_str().unwrap().contains("allow_all"), "{end}");
    assert!(!stubborn.methods().iter().any(|m| m == "session/prompt"));
}

/// The prompts the agent was sent, in order.
fn prompts(agent: &Agent) -> Vec<String> {
    agent
        .sent()
        .iter()
        .filter(|m| m["method"] == "session/prompt")
        .map(|m| m["params"]["prompt"][0]["text"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn sends_what_it_was_handed_during_a_turn_as_the_next_prompt_once_that_turn_ends() {
    // case: sends what it was handed during a turn as the next prompt, once that turn ends
    let agent = agent_with(
        "if text_of(m) != 'Change the port to 3001.':\n    update(dict(sessionUpdate='agent_message_chunk', content=dict(type='text', text='Running npm test.')))\nended(m)",
    );
    let mut acp = agent.spawn(copilot());
    acp.start(&turn(&agent.dir, Mode::Ask, None, None)).unwrap();
    // The person denies a step and says why while the turn is running.
    assert!(acp.steer("I declined that. Use npm test instead."));
    let events = events_until_end(&mut acp, |_, _| {});
    assert_eq!(prompts(&agent), ["Change the port to 3001.", "I declined that. Use npm test instead."]);
    assert_eq!(said(&events), ["Running npm test."]);
    assert_eq!(end_of(&events).kind, "run.completed");
    // Both prompts' ends are counted: each paid its 10 and 2.
    assert_eq!(end_of(&events).payload["usage"]["inputTokens"].as_f64(), Some(20.0));
    // Once it has ended there is no next prompt to carry anything.
    assert!(!acp.steer("too late"));
}

#[test]
fn still_sends_what_it_was_handed_when_the_turn_stops_at_a_limit() {
    // case: still sends what it was handed when the turn stops at a limit (2026-10-10 sweep)
    let agent = agent_with("ended(m, 'max_turn_requests' if text_of(m) == 'Change the port to 3001.' else 'end_turn')");
    let mut acp = agent.spawn(copilot());
    acp.start(&turn(&agent.dir, Mode::Ask, None, None)).unwrap();
    assert!(acp.steer("Then do the rest."));
    events_until_end(&mut acp, |_, _| {});
    assert_eq!(prompts(&agent), ["Change the port to 3001.", "Then do the rest."]);
}

#[test]
fn an_agent_that_dies_mid_turn_leaves_a_run_that_did_not_finish_saying_so() {
    // case: an agent that dies mid-turn leaves a run that did not finish, saying so
    let events = whole_turn(&agent_with("time.sleep(0.05)\nsys.exit(0)"), copilot(), Mode::Ask, None);
    let end = end_of(&events);
    assert_eq!(end.kind, "run.failed");
    assert_eq!(end.payload["process"]["exitCode"], Value::Null);
    assert_eq!(end.payload["process"]["cancelled"], false);
    assert_eq!(end.payload["message"], "The agent exited before its turn finished.");
}

#[test]
fn says_so_in_words_when_the_agent_cannot_continue_an_earlier_session_rather_than_asking_it_anyway() {
    // case: says so in words when the agent cannot continue an earlier session, rather than asking it anyway
    let agent = Agent::new(
        r#"
for m in messages():
    if m.get('method') == 'initialize': answer(m, dict(protocolVersion=1, agentCapabilities=dict(loadSession=False)))
    elif 'method' in m: say(dict(id=m['id'], error=dict(code=-32601, message='Method not found')))
"#,
    );
    let events = whole_turn(&agent, copilot(), Mode::Ask, Some("s1"));
    let end = &end_of(&events).payload;
    assert_eq!(end["process"]["exitCode"], Value::Null);
    assert_eq!(end["message"], "The agent cannot continue an earlier session.");
    assert_eq!(agent.methods(), ["initialize"]);
}

#[test]
fn will_not_speak_to_an_agent_on_another_version_of_the_protocol() {
    // case: will not speak to an agent on another version of the protocol
    let agent = Agent::new("for m in messages():\n    if m.get('method') == 'initialize': answer(m, dict(protocolVersion=2))\n");
    let events = whole_turn(&agent, copilot(), Mode::Ask, None);
    let end = &end_of(&events).payload;
    assert_eq!(end["process"]["exitCode"], Value::Null);
    assert!(end["message"].as_str().unwrap().contains("version 2"), "{end}");
    assert_eq!(agent.methods(), ["initialize"]);
}

// ---- what an ACP agent says it can do (W12)

fn read(said: [bool; 6]) -> Value {
    let keys = crate::acp_capabilities::KEYS;
    Value::Object(keys.iter().zip(said).map(|(k, v)| ((*k).to_owned(), json!(v))).collect())
}

#[test]
fn reads_the_protocols_own_keys_and_only_true_counts() {
    // case: reads the protocol's own keys, and only true counts
    let said = crate::acp_capabilities::capabilities_of(&json!({"loadSession":true,"promptCapabilities":{"image":true,"audio":false,"embeddedContext":true},"mcpCapabilities":{"http":true,"sse":"yes"}}));
    assert_eq!(
        read(said),
        json!({"continuesSessions":true,"images":true,"audio":false,"embeddedContext":true,"mcpOverHttp":true,"mcpOverSse":false})
    );
}

#[test]
fn reads_what_copilot_1_0_88_really_answered_its_session_capabilities_left_out() {
    // case: reads what Copilot 1.0.88 really answered (fixtures/acp), its sessionCapabilities left out
    let path = format!("{}/../../../packages/runtime-adapters/test/fixtures/acp/copilot-1.0.88-load-approved.jsonl", env!("CARGO_MANIFEST_DIR"));
    let line = std::fs::read_to_string(path).unwrap().lines().find(|l| l.contains("agentCapabilities")).unwrap().to_owned();
    let answer: Value = serde_json::from_str(&line).unwrap();
    let said = crate::acp_capabilities::capabilities_of(&answer["message"]["result"]["agentCapabilities"]);
    assert_eq!(
        read(said),
        json!({"continuesSessions":true,"images":true,"audio":false,"embeddedContext":true,"mcpOverHttp":true,"mcpOverSse":true})
    );
}

#[test]
fn leaves_out_keys_it_does_not_know_and_reads_nothing_from_a_malformed_answer() {
    // case: leaves out keys it does not know, and reads nothing from a malformed answer
    let read_one = read(crate::acp_capabilities::capabilities_of(
        &json!({"loadSession":true,"teleport":true,"_meta":{"anything":true},"promptCapabilities":{"video":true}}),
    ));
    let mut keys: Vec<_> = read_one.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["audio", "continuesSessions", "embeddedContext", "images", "mcpOverHttp", "mcpOverSse"]);
    assert_eq!(read_one["continuesSessions"], true);
    for malformed in [Value::Null, json!("all"), json!([true]), json!({"promptCapabilities":"image"})] {
        assert!(crate::acp_capabilities::capabilities_of(&malformed).iter().all(|v| !v), "{malformed}");
    }
}

/// Whether a turn of an agent answering `init` hands what it said to the store (acp.rs `begin`), seen as the turn
/// waiting on the store while this test holds it: a turn that never reaches the store finishes within `wait`.
fn reaches_the_store(runtime: &str, init: &str, wait: Duration) -> bool {
    let agent = Agent::new(&format!(
        "for m in messages():\n    if m.get('method') == 'initialize': answer(m, {init})\n    elif m.get('method') == 'session/new': answer(m, SESSION)\n    elif m.get('method') == 'session/prompt': ended(m)\n"
    ));
    let profile = Profile { runtime: runtime.into(), ..Default::default() };
    let store = crate::acp_capabilities::shared().expect("this app's data folder");
    let held = store.lock().unwrap();
    let (done, finished) = std::sync::mpsc::channel();
    let turn = std::thread::spawn(move || {
        whole_turn(&agent, profile, Mode::Ask, None);
        let _ = done.send(());
    });
    let ended_unheld = finished.recv_timeout(wait).is_ok();
    drop(held);
    turn.join().unwrap();
    !ended_unheld
}

#[test]
fn is_handed_over_once_the_agent_answers_on_this_protocols_version() {
    // case: is handed over once the agent answers on this protocol's version
    assert!(reaches_the_store("kilo", "dict(protocolVersion=1, agentCapabilities=dict(loadSession=True))", Duration::from_secs(2)));
    let mut seen = crate::acp_capabilities::said_by("kilo").expect("kept for Settings");
    seen.as_object_mut().unwrap().remove("toldAt");
    assert_eq!(
        seen,
        json!({"continuesSessions":true,"images":false,"audio":false,"embeddedContext":false,"mcpOverHttp":false,"mcpOverSse":false})
    );
}

#[test]
fn is_not_handed_over_from_an_agent_on_another_version_of_the_protocol() {
    // case: is not handed over from an agent on another version of the protocol
    assert!(!reaches_the_store("opencode", "dict(protocolVersion=2, agentCapabilities=dict(loadSession=True))", crate::load_waits::UNDER_LOAD));
}

#[test]
fn a_listener_that_throws_does_not_stop_the_turn() {
    // case: a listener that throws does not stop the turn
    // Rust's listener is the store's write (acp_capabilities::record_said), which never fails a turn. A runtime it
    // does not keep, and one it does, both finish.
    for runtime in ["acp_case", "copilot"] {
        let agent = agent_with("ended(m)");
        let profile = if runtime == "copilot" { copilot() } else { Profile { runtime: runtime.into(), ..Default::default() } };
        let events = whole_turn(&agent, profile, Mode::Ask, None);
        assert_eq!(end_of(&events).payload["process"]["exitCode"], 0, "{runtime}");
    }
}

// ---- what Copilot is started with, to ask

#[test]
fn acp_and_nothing_that_stops_it_asking_no_allow_all_tools_no_prompt_no_session_on_the_argv() {
    // case: --acp and nothing that stops it asking: no --allow-all-tools, no prompt, no session on the argv
    use super::super::copilot::acp_args;
    assert_eq!(acp_args(None, None, None), ["--acp", "--excluded-tools=session_store_sql"]);
    assert_eq!(acp_args(Some("gpt-5.6-luna"), None, None), ["--acp", "--excluded-tools=session_store_sql", "--model", "gpt-5.6-luna"]);
    // The effort the person chose rides on this route too (the 2026-10-10 sweep: it never did).
    assert_eq!(
        acp_args(Some("gpt-5.6-luna"), Some("high"), None),
        ["--acp", "--excluded-tools=session_store_sql", "--model", "gpt-5.6-luna", "--effort", "high"]
    );
    // And the command the engine launches for Approve each (native.rs calls copilot::command) carries them: the CLI,
    // in the workspace, with Electron's argv and nothing else on it.
    let fake = std::path::PathBuf::from("/fake/bin/copilot");
    super::super::copilot::FAKE_LAUNCHER.with(|f| *f.borrow_mut() = Some(fake.clone()));
    let built = super::super::copilot::command(
        Path::new("/work"),
        "go",
        Mode::ApproveEach,
        Some("gpt-5.6-luna"),
        Some("high"),
        None,
        None,
        &[],
        None,
    );
    super::super::copilot::FAKE_LAUNCHER.with(|f| *f.borrow_mut() = None);
    let built = built.unwrap();
    assert_eq!(Path::new(built.get_program()), fake.as_path());
    assert_eq!(built.get_current_dir(), Some(Path::new("/work")));
    let argv: Vec<String> = built.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    assert_eq!(argv, ["--acp", "--excluded-tools=session_store_sql", "--model", "gpt-5.6-luna", "--effort", "high"]);
}

#[test]
fn holds_the_session_to_the_mode_that_asks_with_allow_all_off_by_copilots_own_ids() {
    // case: holds the session to the mode that asks, with allow-all off -- by Copilot's own ids
    let agent = agent_with("ended(m)");
    let mut command = agent.command();
    command.arg("--acp");
    let super::super::copilot::Copilot::Acp(acp) = super::super::copilot::Copilot::spawn(&mut command).unwrap() else {
        panic!("Copilot with --acp is the ACP driver");
    };
    assert_eq!(acp.profile.runtime, "copilot");
    assert_eq!(acp.profile.required, [("allow_all".to_owned(), "off".to_owned())]);
    // The mode it holds the session to is Copilot's agent mode: a session already in it is sent no set_mode.
    let mut acp = acp;
    acp.start(&turn(&agent.dir, Mode::Ask, None, None)).unwrap();
    events_until_end(&mut acp, |_, _| {});
    assert!(!agent.methods().iter().any(|m| m == "session/set_mode"));
    assert_eq!(AGENT_MODE, "https://agentclientprotocol.com/protocol/session-modes#agent");
}
