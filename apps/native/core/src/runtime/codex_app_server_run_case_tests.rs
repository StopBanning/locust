//! Electron's `packages/runtime-adapters/test/codex-app-server-run.test.ts`, case by case, through the caller: each turn
//! is a Codex turn as the host runs one (`Codex::spawn`, `start`, `next_event`, `steer`, `stop`), whose `codex
//! app-server` is the stand-in (codex_stand_in.rs), recording what it was told and answering on cue, as Electron's
//! `fakeServer` does. Electron's `codexAppServerPolicy`, `codexPlanToolArguments` and `createCodexAppServerCommand` are
//! `approval::codex_app_server_policy`, `plan_tool_args` and `app_server_args`, read through the turn's own requests
//! where the turn reads them. Electron's thread "t1" is the stand-in's "thread_9": a turn here starts on the thread its
//! `thread/start` answered.
use super::codex_stand_in::*;
use super::*;

/// The requests a turn in `mode` makes, as the stand-in heard them.
fn started_with(mut turn: Turn, codex: impl FnOnce(Codex) -> Codex) -> Told {
    let (runtime, told) = stand_in(json!({}));
    let mut runtime = codex(runtime);
    turn.mission_id = format!("codex-run-case-{}", crate::invariants::unique_nanos());
    runtime.start(&turn).unwrap();
    told
}

fn started(turn: Turn) -> Told {
    started_with(turn, |c| c)
}

fn resuming(mode: Mode, model: Option<&str>) -> Turn {
    Turn { resume: Some("thread_earlier".into()), model: model.map(str::to_owned), ..turn_in(mode) }
}

// case: asks for danger-full-access only when the mission really has full access
#[test]
fn danger_full_access_only_for_a_mission_with_full_access() {
    let sandbox_of = |mode| started(turn_in(mode)).params_of("thread/start").unwrap()["sandbox"].clone();
    assert_eq!(sandbox_of(Mode::Auto), "danger-full-access");
    assert_eq!(sandbox_of(Mode::AcceptEdits), "workspace-write");
    assert_eq!(sandbox_of(Mode::Ask), "read-only");
    // A mission with no sandbox of its own reads as read-only, never wider.
    assert_eq!(sandbox_of(Mode::default()), "read-only");
    // The dangerous value comes only from the one sandbox that means it.
    assert_eq!(crate::approval::codex_app_server_policy(Mode::Auto, Some("workspace-write")).sandbox, "workspace-write");
    assert_eq!(crate::approval::codex_app_server_policy(Mode::Auto, Some("read-only")).sandbox, "read-only");
}

// case: never asks anything, because the exec transport it replaces never could
#[test]
fn never_asks_outside_approve_each() {
    for mode in [Mode::Ask, Mode::Plan, Mode::AcceptEdits, Mode::Auto] {
        let told = started(turn_in(mode));
        assert_eq!(told.params_of("thread/start").unwrap()["approvalPolicy"], "never", "{mode:?}");
        assert_eq!(told.params_of("turn/start").unwrap()["approvalPolicy"], "never", "{mode:?}");
    }
}

// case: is `codex app-server`, with the prompt in neither argv nor stdin
#[test]
fn is_codex_app_server_with_the_prompt_only_in_the_protocol() {
    assert_eq!(app_server_args(None), vec!["app-server".to_string()]);
    let told = started(turn_in(Mode::AcceptEdits));
    // stdin carries the protocol and nothing else: every line one JSON-RPC message, the prompt inside turn/start alone.
    for line in told.lines() {
        assert!(line.ends_with('\n'));
        let message: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(message["jsonrpc"], "2.0", "{line}");
        assert!(message["method"] == "turn/start" || !line.contains("count the files"), "{line}");
    }
    assert_eq!(told.params_of("turn/start").unwrap()["input"], json!([{"type": "text", "text": "count the files"}]));
}

// case: switches the plan tool on from 0.153.0, where it was measured, and not below
#[test]
fn the_plan_tool_is_on_from_0_153_0_and_not_below() {
    let on = vec!["-c".to_string(), "tools.update_plan.enabled=true".to_string()];
    for version in ["0.153.0", "0.156.1", "1.0.0"] {
        assert_eq!(plan_tool_args(Some(version)), on, "{version}");
    }
    for version in [Some("0.152.9"), Some("0.151.0-alpha.7.2"), Some("not a version"), None] {
        assert!(plan_tool_args(version).is_empty(), "{version:?}");
    }
    let mut with = vec!["app-server".to_string()];
    with.extend(on);
    assert_eq!(app_server_args(Some("0.156.1")), with);
}

// case: hands the folder, the policy, the model and the prompt to the right calls
#[test]
fn hands_folder_policy_model_and_prompt_to_the_right_calls() {
    let told = started(Turn { model: Some("gpt-5.6-luna".into()), effort: Some("low".into()), ..turn_in(Mode::AcceptEdits) });
    let thread = told.params_of("thread/start").unwrap();
    assert_eq!((&thread["cwd"], &thread["sandbox"], &thread["approvalPolicy"]), (&json!("/work"), &json!("workspace-write"), &json!("never")));
    let turn = told.params_of("turn/start").unwrap();
    assert_eq!(turn["threadId"], "thread_9");
    assert_eq!(turn["model"], "gpt-5.6-luna");
    assert_eq!(turn["effort"], "low");
    assert_eq!(turn["input"], json!([{"type": "text", "text": "count the files"}]));
}

// case: resumes a prior thread rather than starting one, which is what keeps a follow-up warm
#[test]
fn resumes_a_prior_thread_rather_than_starting_one() {
    let told = started(resuming(Mode::AcceptEdits, None));
    assert_eq!(told.id_of("thread/start"), None);
    let resume = told.params_of("thread/resume").unwrap();
    assert_eq!((&resume["threadId"], &resume["cwd"], &resume["excludeTurns"]), (&json!("thread_earlier"), &json!("/work"), &json!(true)));
}

// case: reopens a thread on the turn's own model, so Codex does not warn it is resuming on another (0.864)
#[test]
fn reopens_a_thread_on_the_turns_own_model() {
    let resumed = started(Turn { effort: Some("high".into()), ..resuming(Mode::AcceptEdits, Some("gpt-6.1-sol")) });
    let params = resumed.params_of("thread/resume").unwrap();
    assert_eq!((&params["threadId"], &params["model"]), (&json!("thread_earlier"), &json!("gpt-6.1-sol")));
    assert!(params["config"].get("model_reasoning_effort").is_none(), "effort stays the turn's own: {params}");
    let forked = started_with(resuming(Mode::AcceptEdits, Some("gpt-6.1-sol")), |c| c.with_fork(true));
    assert_eq!(forked.params_of("thread/fork").unwrap()["model"], "gpt-6.1-sol");
    let fresh = started(Turn { model: Some("gpt-6.1-sol".into()), ..turn_in(Mode::AcceptEdits) });
    assert!(fresh.params_of("thread/start").unwrap().get("model").is_none());
    let unnamed = started(resuming(Mode::AcceptEdits, None));
    assert!(unnamed.params_of("thread/resume").unwrap().get("model").is_none());
}

// case: forks the thread for a question on the side, a copy the side can fork again (0.461)
#[test]
fn forks_the_thread_for_a_side_question() {
    let told = started_with(resuming(Mode::Ask, None), |c| c.with_fork(true));
    assert_eq!((told.id_of("thread/resume"), told.id_of("thread/start")), (None, None));
    let fork = told.params_of("thread/fork").unwrap();
    assert_eq!(fork["threadId"], "thread_earlier");
    assert_eq!(fork["sandbox"], "read-only");
    assert_eq!(fork["cwd"], "/work");
    assert_eq!(fork["excludeTurns"], true);
    assert!(fork.get("ephemeral").is_none());
}

// case: lays the thread's own config over the person's, on a start, a resume and a fork alike (0.720)
#[test]
fn lays_the_threads_own_config_on_start_resume_and_fork() {
    let config = json!({"bypass_hook_trust": true, "hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "guard"}]}]}});
    let with = |c: Codex| c.with_thread_config(Some(config.clone()));
    assert_eq!(started_with(turn_in(Mode::AcceptEdits), with).params_of("thread/start").unwrap()["config"], config);
    assert_eq!(started_with(resuming(Mode::AcceptEdits, None), with).params_of("thread/resume").unwrap()["config"], config);
    let forked = started_with(resuming(Mode::AcceptEdits, None), |c| with(c).with_fork(true));
    assert_eq!(forked.params_of("thread/fork").unwrap()["config"], config);
    // A typed command's effort travels in the same config, and neither drops the other.
    let typed = started_with(Turn { model: Some("gpt-6-luna".into()), effort: Some("low".into()), ..turn_in(Mode::AcceptEdits) }, |c| {
        with(c).with_command(Some("compact".into()))
    })
    .params_of("thread/start")
    .unwrap();
    assert_eq!(typed["model"], "gpt-6-luna");
    let mut both = config.clone();
    both["model_reasoning_effort"] = json!("low");
    assert_eq!(typed["config"], both);
    // Without one, no config is sent at all.
    assert!(started(turn_in(Mode::AcceptEdits)).params_of("thread/start").unwrap().get("config").is_none());
}

// case: reads the person's settings before the thread starts, and lays what it makes under Locust's own config (0.824)
#[test]
fn reads_the_settings_first_and_lays_locusts_own_config_over_them() {
    let (codex, told) = stand_in(json!({"config/read": [{"result": {"config": {"mcp_servers": {"notes": {"command": "notes-server"}}}}}]}));
    let mut codex = codex.with_thread_config(Some(json!({"mcp_servers.locust_browser.enabled": true})));
    codex.start(&turn_in(Mode::Ask)).unwrap();
    let asked = told.asked();
    let at = |m: &str| asked.iter().position(|a| a == m).unwrap();
    assert!(at("config/read") < at("thread/start"), "{asked:?}");
    assert_eq!(told.params_of("config/read").unwrap(), json!({"cwd": "/work"}));
    let config = told.params_of("thread/start").unwrap()["config"].clone();
    assert_eq!(config["features.plugins"], false);
    assert_eq!(config["mcp_servers.notes.enabled"], false);
    assert_eq!(config["mcp_servers.locust_browser.enabled"], true);
}

// case: starts no thread when the settings refuse it, and ends with the refusal's own words (0.824)
#[test]
fn starts_no_thread_when_the_settings_refuse_it() {
    let (mut codex, told) = stand_in(json!({
        "$start": [{"stderr": "some unrelated server line\n"}],
        "config/read": [{"result": {"not": "settings"}}],
    }));
    let refused = codex.start(&turn_in(Mode::Ask)).unwrap_err().to_string();
    assert!(refused.starts_with("Locust could not read your Codex settings, so it did not start this turn."), "{refused}");
    assert!(!refused.contains("unrelated server line"), "{refused}");
    assert_eq!(told.id_of("thread/start"), None);
}

// case: hands on Codex's own words when it refuses the read, so an old Codex can be told to update (0.825)
#[test]
fn hands_on_codexs_own_words_when_it_refuses_the_read() {
    let words = "Invalid request: unknown variant `config/read`, expected one of `initialize`, `thread/start`";
    let (mut codex, told) = stand_in(json!({"config/read": [{"error": words}]}));
    let refused = codex.start(&turn_in(Mode::Ask)).unwrap_err().to_string();
    assert!(refused.ends_with("Update Codex, then send it again."), "{refused}");
    assert_eq!(told.id_of("thread/start"), None);
}

// case: streams every notification as a record and ends when the turn does
#[test]
fn streams_every_notification_and_ends_when_the_turn_does() {
    let (mut codex, _told) = turn_with(&[
        ("item/agentMessage/delta", json!({"itemId": "m1", "delta": "Cach"})),
        ("item/agentMessage/delta", json!({"itemId": "m1", "delta": "ing"})),
        ("turn/completed", json!({"threadId": "thread_9"})),
    ]);
    let events = to_the_end(&mut codex);
    assert_eq!(kinds(&events), ["message.delta", "message.delta", "run.completed"]);
    assert_eq!(events[0].payload["text"], "Cach");
    let done = &events[2].payload["process"];
    assert_eq!((&done["exitCode"], &done["forcedTerminationAttempted"]), (&json!(0), &json!(false)));
    // The server is not left running once its turn is over: the host drops the run at its terminal event.
    let pid = codex.child.id();
    drop(codex);
    assert!(!alive(pid));
}

// case: reports a server that dies mid-turn as a lost transport, not a clean finish
#[test]
fn a_server_that_dies_mid_turn_is_a_lost_transport() {
    let (mut codex, _told) = stand_in(json!({"turn/start": [
        {"result": {"turn": {"id": "turn_1"}}},
        {"send": {"jsonrpc": "2.0", "method": "item/agentMessage/delta", "params": {"itemId": "m1", "delta": "half a th"}}},
        {"exit": 1},
    ]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    let events = to_the_end(&mut codex);
    let end = events.last().unwrap();
    assert_eq!(end.kind, "run.failed");
    assert_eq!(end.payload["process"]["exitCode"], Value::Null);
    assert!(end.payload["process"]["stderr"].as_str().unwrap().starts_with("Codex stdout closed before the turn completed"));
}

// case: keeps what a dying server wrote to stderr, as its last line
#[test]
fn keeps_a_dying_servers_stderr_as_its_last_line() {
    let (mut codex, _told) = stand_in(json!({"turn/start": [
        {"result": {"turn": {"id": "turn_1"}}},
        {"stderr": "thread 'main' panicked\nError: sandbox could not start: seatbelt denied\n"},
        {"exit": 1},
    ]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    let events = to_the_end(&mut codex);
    let stderr = events.last().unwrap().payload["process"]["stderr"].as_str().unwrap().to_owned();
    let lines: Vec<&str> = stderr.trim().split('\n').collect();
    assert!(lines[0].contains("closed before the turn completed"), "{stderr}");
    assert_eq!(*lines.last().unwrap(), "Error: sandbox could not start: seatbelt denied");
}

fn interruptible() -> (Codex, Told) {
    let (mut codex, told) = stand_in(json!({"turn/interrupt": [
        {"result": {}},
        {"send": {"jsonrpc": "2.0", "method": "turn/completed", "params": {"threadId": "thread_9", "turn": {"id": "turn_1", "status": "interrupted"}}}},
    ]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    (codex, told)
}

// case: ends the run when the mission is cancelled
#[test]
fn ends_the_run_when_the_mission_is_cancelled() {
    let (mut codex, told) = interruptible();
    codex.stop().unwrap();
    let events = to_the_end(&mut codex);
    let end = events.last().unwrap();
    assert_eq!(end.kind, "run.cancelled");
    assert_eq!(end.payload["process"]["exitCode"], Value::Null);
    assert_eq!(told.params_of("turn/interrupt").unwrap(), json!({"threadId": "thread_9", "turnId": "turn_1"}));
}

// case: gives up rather than hanging when the server never answers the handshake
#[test]
fn gives_up_when_the_server_never_answers_the_handshake() {
    let (mut codex, _told) = stand_in(json!({"initialize": [{"silent": true}]}));
    codex.answer_within = Duration::from_millis(300);
    let begun = Instant::now();
    let refused = codex.start(&turn_in(Mode::Ask)).unwrap_err().to_string();
    assert!(refused.contains("did not answer"), "{refused}");
    assert!(begun.elapsed() < Duration::from_secs(10));
}

// case: turns a record back into the notification it was written from
#[test]
fn turns_a_record_into_the_notification_it_was_written_from() {
    let (mut codex, _told) = turn_with(&[
        ("item/agentMessage/delta", json!({"itemId": "m1", "delta": "Caching"})),
        ("turn/completed", json!({"threadId": "thread_9"})),
    ]);
    let events = to_the_end(&mut codex);
    assert!(of(&events, "message.delta").iter().any(|e| e.payload["text"] == "Caching"));
}

// case: drops a line that is not a notification instead of guessing at it
#[test]
fn drops_a_line_that_is_not_a_notification() {
    let (mut codex, _told) = stand_in(json!({"turn/start": [
        {"result": {"turn": {"id": "turn_1"}}},
        {"raw": "not json\n"},
        {"send": {"jsonrpc": "2.0", "id": 4}},
        {"send": {"jsonrpc": "2.0", "method": "turn/completed", "params": {"threadId": "thread_9"}}},
    ]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    let events = to_the_end(&mut codex);
    // Nothing is guessed from either line: each is said as what it is, and the run goes on to its own end.
    assert_eq!(kinds(&events), ["adapter.diagnostic", "adapter.diagnostic", "run.completed"]);
}

// case: reads a cancelled completion as cancelled and anything else as a lost transport
#[test]
fn a_cancelled_end_is_cancelled_and_any_other_a_lost_transport() {
    let (mut codex, _told) = interruptible();
    codex.stop().unwrap();
    assert_eq!(to_the_end(&mut codex).last().unwrap().kind, "run.cancelled");
    let (mut codex, _told) = stand_in(json!({"turn/start": [{"result": {"turn": {"id": "turn_1"}}}, {"exit": 0}]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    assert_eq!(to_the_end(&mut codex).last().unwrap().kind, "run.failed");
}

fn counted(thread: &str, total: u64, last: u64, cached: u64) -> (&'static str, Value) {
    let reading = |n: u64| json!({"totalTokens": n, "inputTokens": n - 5, "cachedInputTokens": cached, "cacheWriteInputTokens": 0, "outputTokens": 5, "reasoningOutputTokens": 0});
    ("thread/tokenUsage/updated", json!({"threadId": thread, "tokenUsage": {"total": reading(total), "last": reading(last)}}))
}

fn usage_at_the_end(then: &[(&str, Value)]) -> Value {
    let (mut codex, _told) = turn_with(then);
    let events = to_the_end(&mut codex);
    let done = events.last().unwrap();
    assert_eq!(done.kind, "run.completed");
    done.payload["usage"].clone()
}

// case: reports the tokens the thread counted, in the vocabulary every receipt uses
#[test]
fn reports_the_tokens_the_thread_counted() {
    let usage = usage_at_the_end(&[
        ("thread/started", json!({"thread": {"id": "thread_9"}})),
        ("turn/started", json!({"threadId": "thread_9", "turn": {"id": "turn_1"}})),
        counted("thread_9", 18_375, 18_375, 11_008),
        ("turn/completed", json!({"threadId": "thread_9", "turn": {"id": "turn_1"}})),
    ]);
    assert_eq!((&usage["inputTokens"], &usage["outputTokens"], &usage["cacheReadTokens"]), (&json!(18_370), &json!(5), &json!(11_008)));
}

// case: bills a resumed turn for itself, not the whole conversation
#[test]
fn bills_a_resumed_turn_for_itself() {
    let usage = usage_at_the_end(&[
        counted("thread_9", 18_375, 18_375, 0),
        ("turn/started", json!({"threadId": "thread_9", "turn": {"id": "turn_2"}})),
        counted("thread_9", 36_767, 18_392, 0),
        ("turn/completed", json!({"threadId": "thread_9", "turn": {"id": "turn_2"}})),
    ]);
    assert_eq!((&usage["inputTokens"], &usage["outputTokens"]), (&json!(18_392), &json!(0)));
}

// case: is not ended by a sub-agent's turn, and counts what the sub-agent spent
#[test]
fn a_sub_agents_turn_ends_nothing_and_its_spend_is_counted() {
    let (mut codex, _told) = turn_with(&[
        ("thread/started", json!({"thread": {"id": "thread_9"}})),
        ("turn/started", json!({"threadId": "thread_9", "turn": {"id": "p1"}})),
        counted("thread_9", 18_548, 18_548, 0),
        ("item/started", json!({"threadId": "thread_9", "item": {"id": "c1", "type": "collabAgentToolCall", "tool": "spawnAgent", "status": "inProgress"}})),
        ("turn/started", json!({"threadId": "child", "turn": {"id": "k1"}})),
        ("item/agentMessage/delta", json!({"threadId": "child", "itemId": "km", "delta": "PING"})),
        counted("child", 18_379, 18_379, 0),
        ("turn/completed", json!({"threadId": "child", "turn": {"id": "k1"}})),
        ("item/completed", json!({"threadId": "thread_9", "item": {"id": "c1", "type": "collabAgentToolCall", "tool": "spawnAgent", "status": "completed"}})),
        counted("thread_9", 46_273, 27_725, 0),
        ("item/agentMessage/delta", json!({"threadId": "thread_9", "itemId": "pm", "delta": "It said PING."})),
        ("turn/completed", json!({"threadId": "thread_9", "turn": {"id": "p1"}})),
    ]);
    let events = to_the_end(&mut codex);
    assert_eq!(kinds(&events), ["run.started", "tool.started", "tool.completed", "message.delta", "run.completed"]);
    assert_eq!(events[2].payload["name"], "subagent:spawnAgent");
    assert_eq!(events[3].payload["text"], "It said PING.", "the sub-agent's own words are not the run's");
    let done = &events[4].payload;
    assert_eq!(done["runtimeThreadId"], "thread_9");
    assert_eq!(done["usage"]["inputTokens"], 46_268 + 18_374);
}

// case: keeps the transport running through a sub-agent's turn ending
#[test]
fn keeps_running_through_a_sub_agents_turn_ending() {
    let (mut codex, told) = stand_in(json!({
        "turn/start": [
            {"result": {"turn": {"id": "turn_1"}}},
            {"send": {"jsonrpc": "2.0", "method": "turn/started", "params": {"threadId": "thread_9", "turn": {"id": "turn_1"}}}},
            {"send": {"jsonrpc": "2.0", "method": "turn/started", "params": {"threadId": "sub_1", "turn": {"id": "sub_turn"}}}},
            {"send": {"jsonrpc": "2.0", "method": "turn/completed", "params": {"threadId": "sub_1", "turn": {"id": "sub_turn"}}}},
        ],
        "turn/steer": [
            {"result": {"turnId": "turn_1"}},
            {"send": {"jsonrpc": "2.0", "method": "turn/completed", "params": {"threadId": "thread_9", "turn": {"id": "turn_1"}}}},
        ],
    }));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    let before = events_until_end(&mut codex, Duration::from_millis(800));
    assert_eq!(kinds(&before), ["run.started"], "the sub-agent's end is not the run's");
    assert!(alive(codex.child.id()));
    // The steer still aims at the parent's turn.
    assert!(codex.steer("heads up"));
    let steer = told.params_of("turn/steer").unwrap();
    assert_eq!((&steer["threadId"], &steer["expectedTurnId"]), (&json!("thread_9"), &json!("turn_1")));
    assert_eq!(to_the_end(&mut codex).last().unwrap().kind, "run.completed");
}

// case: says nothing about cost when the thread never counted, rather than claiming zero
#[test]
fn says_nothing_about_cost_when_nothing_was_counted() {
    assert_eq!(usage_at_the_end(&[("turn/completed", json!({"threadId": "thread_9"}))]), Value::Null);
}

// case: stops the run and says why, instead of dropping records quietly
#[test]
fn stops_the_run_and_says_why_when_the_queue_backs_up() {
    let (mut codex, _told) = stand_in(json!({"turn/start": [
        {"result": {"turn": {"id": "turn_1"}}},
        {"raw": format!("{}\n", json!({"jsonrpc": "2.0", "method": "item/agentMessage/delta", "params": {"threadId": "thread_9", "turnId": "turn_1", "itemId": "m", "delta": "x"}})).repeat(MAX_QUEUED_RECORDS + 1)},
    ]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while codex.control.failure().is_none() {
        assert!(Instant::now() < deadline, "the flood never backed up");
        std::thread::sleep(Duration::from_millis(10));
    }
    let end = to_the_end(&mut codex).pop().unwrap();
    assert_eq!(end.kind, "run.failed");
    assert!(end.payload["message"].as_str().unwrap().contains("faster than Locust could record it"));
    assert_eq!(end.payload["process"]["outputLimitExceeded"], true);
    let pid = codex.child.id();
    let deadline = Instant::now() + Duration::from_secs(10);
    while alive(pid) && codex.child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "the server was left running");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn typed(command: &str, prompt: &str, plan: Value) -> (Codex, Told) {
    let (codex, told) = stand_in(plan);
    let mut codex = codex.with_command(Some(command.into()));
    codex
        .start(&Turn {
            prompt: prompt.into(),
            model: Some("gpt-6-luna".into()),
            effort: Some("low".into()),
            resume: Some("thread_9".into()),
            ..turn_in(Mode::Ask)
        })
        .unwrap();
    (codex, told)
}

// case: reviews the uncommitted changes as its terminal does, on the thread's own route, and starts no ordinary turn
#[test]
fn reviews_the_uncommitted_changes_on_the_threads_own_route() {
    let (_codex, told) = typed("review", "  ", json!({}));
    assert_eq!(told.params_of("review/start").unwrap(), json!({"threadId": "thread_9", "target": {"type": "uncommittedChanges"}, "delivery": "inline"}));
    let resume = told.params_of("thread/resume").unwrap();
    assert_eq!((&resume["threadId"], &resume["model"], &resume["config"]["model_reasoning_effort"]), (&json!("thread_9"), &json!("gpt-6-luna"), &json!("low")));
    assert_eq!(told.id_of("turn/start"), None);
}

// case: reviews what the person asked for, when they said
#[test]
fn reviews_what_the_person_asked_for() {
    let (_codex, told) = typed("review", "only the error handling", json!({}));
    assert_eq!(told.params_of("review/start").unwrap()["target"], json!({"type": "custom", "instructions": "only the error handling"}));
}

// case: compacts the thread, and the run ends when that turn does
#[test]
fn compacts_the_thread_and_ends_when_that_turn_does() {
    let (mut codex, told) = typed("compact", "", json!({"thread/compact/start": [
        {"result": {}},
        {"send": {"jsonrpc": "2.0", "method": "turn/started", "params": {"threadId": "thread_9", "turn": {"id": "turn_c"}}}},
        {"send": {"jsonrpc": "2.0", "method": "turn/completed", "params": {"threadId": "thread_9", "turn": {"id": "turn_c", "status": "completed"}}}},
    ]}));
    assert_eq!(told.params_of("thread/compact/start").unwrap(), json!({"threadId": "thread_9"}));
    assert_eq!(told.id_of("turn/start"), None);
    let end = to_the_end(&mut codex).pop().unwrap();
    assert_eq!(end.kind, "run.completed");
    assert_eq!(end.payload["process"]["exitCode"], 0);
}
