//! Electron's `packages/runtime-adapters/test/acp-events.test.ts`, case by case, through the caller: each turn is a
//! real `Acp` run (spawn, start, next_event) against an agent on the pipes -- Electron's own measured transcripts played
//! back (`fixtures/acp/`), or a scripted agent saying what Electron's records say -- so `AcpNormalizer` is fed exactly
//! as acp.rs feeds it, refusals and prompt ends included.
use super::agent_support::*;
use super::*;
use crate::approval::Mode;

fn opencode_rejected() -> Vec<Event> {
    whole_turn(&Agent::replaying("opencode-1.18.27-bash-rejected.jsonl"), opencode(), Mode::Ask, None)
}

/// A scripted agent that answers its first prompt by saying `updates` and ending on `result`.
fn saying(updates: Value, result: Value) -> Agent {
    Agent::new(&format!(
        "UPDATES = json.loads({u:?})\nRESULT = json.loads({r:?})\nfor m in messages():\n    if usual(m): continue\n    if m.get('method') == 'session/prompt':\n        for u in UPDATES: update(u)\n        answer(m, RESULT)\n",
        u = updates.to_string(),
        r = result.to_string()
    ))
}

fn turn_saying(updates: Value) -> Vec<Event> {
    whole_turn(&saying(updates, json!({"stopReason":"end_turn"})), copilot(), Mode::Ask, None)
}

fn chunk(text: &str, message_id: Option<&str>) -> Value {
    let mut u = json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":text}});
    if let Some(id) = message_id {
        u["messageId"] = json!(id);
    }
    u
}

fn shell(status: &str) -> Value {
    json!({"sessionUpdate": if status == "pending" {"tool_call"} else {"tool_call_update"},"toolCallId":"t1","kind":"execute","status":status,"rawInput":{"command":"npm test"}})
}

// ---- an ACP turn, from OpenCode's own stream

#[test]
fn starts_thinks_asks_to_run_a_command_is_refused_and_ends_as_the_products_events() {
    // case: starts, thinks, asks to run a command, is refused, and ends -- as the product's events
    let events = opencode_rejected();
    let shown: Vec<_> = product(&events).iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(shown, ["run.started", "step.started", "step.completed", "tool.started", "tool.failed", "run.completed"]);
    // Rust events carry no source adapter (the ledger writer stamps the runtime); every one names the session.
    assert!(events.iter().all(|e| e.thread.as_deref() == Some("ses_f1fddcf4bffeVoJmoJ2fz70R86")));
    assert_eq!(events.last().unwrap().payload["runtimeThreadId"], "ses_f1fddcf4bffeVoJmoJ2fz70R86");
}

#[test]
fn records_the_refused_call_as_declined_with_its_command_not_as_a_failure() {
    // case: records the refused call as declined, with its command -- not as a failure
    let events = opencode_rejected();
    let refused = first(&events, "tool.failed");
    assert_eq!(refused["toolKind"], "command_execution");
    assert_eq!(refused["name"], "bash");
    assert_eq!(refused["command"], "echo ACP-PROBE");
    assert_eq!(refused["status"], "declined");
    // The first report named only the folder: the row waits for the command.
    assert!(first(&events, "tool.started").get("command").is_none());
}

#[test]
fn keeps_what_was_thought_on_the_thinking_step_as_every_runtimes_is_and_nothing_of_the_machines_own_commands() {
    // case: keeps what was thought on the thinking step, as every runtime's is -- and nothing of the machine's own commands
    let events = opencode_rejected();
    let all = serde_json::to_string(&events.iter().map(|e| &e.payload).collect::<Vec<_>>()).unwrap();
    assert!(!all.contains("example-command"));
    let path = format!("{}/../../../packages/runtime-adapters/test/fixtures/acp/opencode-1.18.27-bash-rejected.jsonl", env!("CARGO_MANIFEST_DIR"));
    let thought: String = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|l| l["message"]["params"]["update"]["sessionUpdate"] == "agent_thought_chunk")
        .map(|l| l["message"]["params"]["update"]["content"]["text"].as_str().unwrap_or("").to_owned())
        .collect();
    let thought = thought.trim();
    assert!(thought.len() > 20);
    let step = first(&events, "step.completed");
    assert_eq!(step["stepKind"], "reasoning");
    assert_eq!(step["message"], thought);
    // On the step, and only there: no record's evidence carries it too.
    assert!(events.iter().all(|e| !e.payload["evidence"].to_string().contains(&thought[..20])));
}

#[test]
fn ends_with_the_turns_usage() {
    // case: ends with the turn's usage
    let events = opencode_rejected();
    let usage = &events.last().unwrap().payload["usage"];
    assert_eq!(usage["inputTokens"].as_f64(), Some(11569.0));
    assert_eq!(usage["outputTokens"].as_f64(), Some(28.0));
}

// ---- an ACP turn, from Copilot's own stream

#[test]
fn names_the_call_by_what_the_agent_said_it_was_for_and_the_command_it_asked_to_run() {
    // case: names the call by what the agent said it was for, and the command it asked to run
    let events = whole_turn(&Agent::replaying("copilot-1.0.88-bash-rejected.jsonl"), copilot(), Mode::Ask, None);
    let started = first(&events, "tool.started");
    assert_eq!(started["name"], "bash");
    assert_eq!(started["command"], "echo ACP-PROBE");
    assert_eq!(started["title"], "Run requested probe");
    assert_eq!(first(&events, "tool.failed")["status"], "declined");
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

// ---- a Copilot session loaded over ACP, a command approved, and the answer (measured 2026-09-26)

const LOADED: &str = "3abe5d9d-b422-4bf5-8c95-97d605d3f819";

fn loaded_and_approved() -> Vec<Event> {
    // Auto: the mode's own answer to every request is yes, as the person gave it in the measured run.
    whole_turn(&Agent::replaying("copilot-1.0.88-load-approved.jsonl"), copilot(), Mode::Auto, Some(LOADED))
}

#[test]
fn keeps_none_of_the_conversation_the_load_replayed_only_the_turn_it_was_sent() {
    // case: keeps none of the conversation the load replayed -- only the turn it was sent
    let events = loaded_and_approved();
    // The replay carried the first turn's "Remember the word PLUM-3" and its "OK".
    assert_eq!(said(&events), ["ACP-OK-7 PLUM-3"]);
    assert!(!shown(&events).contains("Remember the word"));
}

#[test]
fn runs_the_approved_command_and_keeps_what_it_printed() {
    // case: runs the approved command, and keeps what it printed
    let events = loaded_and_approved();
    let shown: Vec<_> = product(&events).iter().map(|e| e.kind.as_str()).filter(|k| *k != "message.delta").collect();
    assert_eq!(shown, ["run.started", "tool.started", "tool.completed", "run.completed"]);
    let done = first(&events, "tool.completed");
    assert_eq!(done["toolKind"], "command_execution");
    assert_eq!(done["name"], "bash");
    assert_eq!(done["command"], "echo ACP-OK-7");
    assert_eq!(done["title"], "Print the required value from the shell");
    assert_eq!(done["status"], "completed");
    assert!(done["output"].as_str().unwrap().contains("ACP-OK-7"));
}

#[test]
fn names_the_session_it_loaded_and_what_the_turn_cost() {
    // case: names the session it loaded, and what the turn cost
    let events = loaded_and_approved();
    let end = events.last().unwrap();
    assert_eq!(end.kind, "run.completed");
    assert_eq!(end.payload["runtimeThreadId"], LOADED);
    assert_eq!(end.payload["usage"]["inputTokens"].as_f64(), Some(22408.0));
    assert_eq!(end.payload["usage"]["outputTokens"].as_f64(), Some(57.0));
}

// ---- what the agent says

#[test]
fn sent_whole_rather_than_token_by_token_is_bounded_like_every_other_message_and_keeps_both_ends() {
    // case: sent whole rather than token by token, is bounded like every other message -- and keeps both ends (review 2026-10-15, C1)
    let long = format!("START {} END", "w".repeat(200_000));
    let events = turn_saying(json!([chunk(&long, Some("m1")), chunk(" more", Some("m1"))]));
    let deltas: Vec<_> = events.iter().filter(|e| e.kind == "message.delta").map(|e| &e.payload).collect();
    assert!(deltas.iter().all(|d| d["text"].as_str().unwrap().encode_utf16().count() <= 131_072));
    let whole = &said(&events)[0];
    assert!(whole.starts_with("START "));
    assert!(whole.ends_with("END more"));
    // The control: an ordinary chunk passes through as it came.
    assert_eq!(deltas[1]["operation"], "append");
    assert_eq!(deltas[1]["text"], " more");
}

#[test]
fn streams_as_it_is_written_and_is_marked_whole_and_final_when_the_turn_ends() {
    // case: streams as it is written, and is marked whole and final when the turn ends
    let events = turn_saying(json!([chunk("Done: ", Some("m1")), chunk("port is 3001.", Some("m1"))]));
    let deltas: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "message.delta")
        .map(|e| (e.payload["itemId"].clone(), e.payload["operation"].clone(), e.payload["text"].clone(), e.payload["final"].clone()))
        .collect();
    assert_eq!(
        deltas,
        [
            (json!("m1"), json!("append"), json!("Done: "), json!(false)),
            (json!("m1"), json!("append"), json!("port is 3001."), json!(false)),
            (json!("m1"), json!("replace"), json!("Done: port is 3001."), json!(true)),
        ]
    );
}

#[test]
fn what_is_said_before_a_tool_call_and_after_it_are_two_messages_each_whole_copilot_names_neither() {
    // case: what is said before a tool call and after it are two messages, each whole -- Copilot names neither
    let events = turn_saying(json!([chunk("I'll run the tests.", None), shell("pending"), shell("completed"), chunk("All 12 pass.", None)]));
    assert_eq!(said(&events), ["I'll run the tests.", "All 12 pass."]);
    let finals: std::collections::HashSet<_> = events
        .iter()
        .filter(|e| e.kind == "message.delta" && e.payload["final"] == true)
        .map(|e| e.payload["itemId"].to_string())
        .collect();
    assert_eq!(finals.len(), 2);
    // In the order it happened: the first message is whole before the call starts.
    let started = events.iter().position(|e| e.kind == "tool.started").unwrap();
    let whole = events.iter().position(|e| e.payload["final"] == true).unwrap();
    assert!(started > whole);
}

#[test]
fn an_agent_that_keeps_one_message_id_across_a_tool_call_gets_one_item_per_stretch_so_neither_replaces_the_other() {
    // case: an agent that keeps one message id across a tool call gets one item per stretch, so neither replaces the other
    let agent = saying(
        json!([chunk("Checking.", Some("m1")), shell("pending"), shell("completed"), chunk("Fine.", Some("m1"))]),
        json!({"stopReason":"end_turn"}),
    );
    let events = whole_turn(&agent, opencode(), Mode::Ask, None);
    let finals: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "message.delta" && e.payload["final"] == true)
        .map(|e| (e.payload["itemId"].as_str().unwrap(), e.payload["text"].as_str().unwrap()))
        .collect();
    assert_eq!(finals, [("m1", "Checking."), ("m1:2", "Fine.")]);
}

#[test]
fn the_persons_own_words_replayed_are_never_taken_for_the_agents() {
    // case: the person's own words, replayed, are never taken for the agent's
    let events = turn_saying(json!([
        {"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"Remember PLUM-3."}},
        chunk("OK.", None)
    ]));
    assert_eq!(said(&events), ["OK."]);
    assert!(product(&events).iter().all(|e| e.kind != "adapter.diagnostic"));
}

#[test]
fn an_edit_carries_its_change_as_a_patch_worked_out_from_before_and_after() {
    // case: an edit carries its change as a patch, worked out from before and after
    let events = turn_saying(json!([
        {"sessionUpdate":"tool_call","toolCallId":"t1","kind":"edit","title":"Edit app.js","status":"pending","locations":[{"path":"app.js"}]},
        {"sessionUpdate":"tool_call_update","toolCallId":"t1","status":"completed","content":[{"type":"diff","path":"app.js","oldText":"const port = 3000\nlisten(port)\n","newText":"const port = 3001\nlisten(port)\n"}]}
    ]));
    let done = first(&events, "tool.completed");
    assert_eq!(done["toolKind"], "file_change");
    assert_eq!(done["name"], "edit");
    assert_eq!(done["command"], "app.js");
    assert_eq!(done["status"], "completed");
    assert_eq!(done["patch"]["added"], 1);
    assert_eq!(done["patch"]["removed"], 1);
    assert_eq!(done["patch"]["truncated"], false);
}

#[test]
fn a_plan_goes_to_the_plan_card_as_the_agent_wrote_it() {
    // case: a plan goes to the plan card as the agent wrote it
    let events = turn_saying(json!([{"sessionUpdate":"plan","entries":[
        {"content":"Read app.js","priority":"high","status":"completed"},
        {"content":"Change the port","priority":"high","status":"in_progress"}
    ]}]));
    assert_eq!(
        first(&events, "plan.updated")["plan"],
        json!([{"content":"Read app.js","status":"completed"},{"content":"Change the port","status":"in_progress"}])
    );
}

// ---- how a turn ends

/// The last event of a turn whose agent ends each prompt on the next of `results` (a steer queued for each further
/// one, as a denial's reason is sent on).
fn ended_on(results: Value) -> Event {
    let n = results.as_array().unwrap().len();
    let agent = Agent::new(&format!(
        "RESULTS = json.loads({r:?})\nfor m in messages():\n    if usual(m): continue\n    if m.get('method') == 'session/prompt':\n        r = RESULTS.pop(0)\n        if r is None: sys.exit(0)\n        answer(m, r)\n",
        r = results.to_string()
    ));
    let mut acp = agent.spawn(copilot());
    acp.start(&turn(&agent.dir, Mode::Ask, None, None)).unwrap();
    for i in 1..n {
        assert!(acp.steer(&format!("and then {i}")));
    }
    events_until_end(&mut acp, |_, _| {}).pop().unwrap()
}

/// The end of a turn whose session the agent refused with `why`, as a CLI that is not signed in does.
fn refused_with(why: &str) -> Event {
    let agent = Agent::new(&format!(
        "for m in messages():\n    if m.get('method') == 'initialize': answer(m, dict(protocolVersion=1, agentCapabilities=dict(loadSession=True)))\n    elif m.get('method') == 'session/new': say(dict(id=m['id'], error=dict(code=-32000, message={why:?})))\n"
    ));
    whole_turn(&agent, copilot(), Mode::Ask, None).pop().unwrap()
}

#[test]
fn cancelled_by_the_person_or_by_the_agents_own_answer() {
    // case: cancelled, by the person or by the agent's own answer
    assert_eq!(ended_on(json!([{"stopReason":"cancelled"}])).kind, "run.cancelled");
    let agent = Agent::new("for m in messages():\n    if usual(m): continue\n");
    let mut acp = agent.spawn(copilot()).with_cancel_grace(Duration::from_millis(200));
    acp.start(&turn(&agent.dir, Mode::Ask, None, None)).unwrap();
    acp.stop().unwrap();
    assert_eq!(events_until_end(&mut acp, |_, _| {}).last().unwrap().kind, "run.cancelled");
}

#[test]
fn failed_when_the_agent_ended_before_its_turn_did_or_refused_it_and_a_cli_that_is_signed_out_says_so() {
    // case: failed when the agent ended before its turn did, or refused it -- and a CLI that is signed out says so
    let signed_out = refused_with("copilot: not signed in");
    assert_eq!(signed_out.kind, "run.failed");
    assert_eq!(signed_out.payload["kind"], "authentication-failed");
    assert_eq!(signed_out.payload["message"], "copilot: not signed in");
    assert_eq!(signed_out.payload["runtimeTerminal"], "missing");
    assert_eq!(refused_with("Authentication required").payload["kind"], "authentication-failed");
    let exited = Agent::new("for m in messages():\n    if usual(m): continue\n    if m.get('method') == 'session/prompt': sys.exit(0)\n");
    assert_eq!(whole_turn(&exited, copilot(), Mode::Ask, None).last().unwrap().payload["kind"], "process-failed");
    let refused = ended_on(json!([{"stopReason":"refusal"}]));
    assert_eq!(refused.kind, "run.failed");
    assert_eq!(refused.payload["kind"], "safety-blocked");
}

#[test]
fn cut_off_at_a_limit_says_so_beside_the_completion() {
    // case: cut off at a limit says so, beside the completion (2026-10-10 sweep)
    let events = whole_turn(&saying(json!([]), json!({"stopReason":"max_tokens"})), copilot(), Mode::Ask, None);
    let warned: Vec<_> = product(&events).into_iter().filter(|e| e.kind == "adapter.diagnostic").collect();
    assert_eq!(warned.len(), 1);
    assert_eq!(warned[0].payload["level"], "warning");
    assert_eq!(warned[0].payload["code"], "acp.stopped_at_limit");
    assert_eq!(events.last().unwrap().kind, "run.completed");
    let control = whole_turn(&saying(json!([]), json!({"stopReason":"end_turn"})), copilot(), Mode::Ask, None);
    assert!(product(&control).iter().all(|e| e.kind != "adapter.diagnostic"));
}

#[test]
fn a_run_lost_during_a_later_prompt_did_not_finish_though_an_earlier_one_did() {
    // case: a run lost during a later prompt did not finish, though an earlier one did
    let last = ended_on(json!([{"stopReason":"end_turn"}, null]));
    assert_eq!(last.kind, "run.failed");
    assert_eq!(last.payload["kind"], "process-failed");
}

#[test]
fn a_second_prompt_a_denials_reason_sent_on_is_paid_for_as_well_as_the_first() {
    // case: a second prompt -- a denial's reason, sent on -- is paid for as well as the first
    let last = ended_on(json!([
        {"stopReason":"end_turn","usage":{"inputTokens":100,"outputTokens":10}},
        {"stopReason":"end_turn","usage":{"inputTokens":150,"outputTokens":20}}
    ]));
    assert_eq!(last.kind, "run.completed");
    let usage = last.payload["usage"].as_object().unwrap();
    assert_eq!(usage.len(), 2);
    assert_eq!(usage["inputTokens"].as_f64(), Some(250.0));
    assert_eq!(usage["outputTokens"].as_f64(), Some(30.0));
}
