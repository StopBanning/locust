//! Electron's `packages/runtime-adapters/test/copilot-events.test.ts`, case by case, through the caller: each run is a
//! Copilot turn as copilot.rs runs one outside Approve each (`PrintCopilot::spawn`, `start`, `next_event` to the end),
//! whose `copilot` is a stand-in that prints Electron's own captures (`test/fixtures/copilot/`) or the case's records,
//! writes the case's stderr, and exits as the case says. The host's session id is the turn's (copilot.rs `start`).
//! The helpers Electron tests on their own (copilotFailureFrom, copilotToolCommand, scrubCopilotRecord,
//! summarizeCopilotAutoMode, copilotUsage) are copilot_events' own, which the normalizer reads each record with.
use super::*;
use crate::runtime::copilot_events::{failure_from, scrub_copilot_record, summarize_auto_mode, tool_command, usage};
use serde_json::{Value, json};
use std::path::PathBuf;

const HOST_SESSION: &str = "11111111-2222-3333-4444-555555555555";

const STAND_IN: &str = r#"
import os, sys
for line in open(os.environ['LOCUST_COPILOT_LINES'], encoding='utf-8').read().split('\n'):
    if line:
        sys.stdout.write(line + '\n'); sys.stdout.flush()
sys.stderr.write(open(os.environ['LOCUST_COPILOT_STDERR'], encoding='utf-8').read()); sys.stderr.flush()
sys.exit(int(os.environ['LOCUST_COPILOT_EXIT']))
"#;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture_text(name: &str) -> String {
    std::fs::read_to_string(format!("{}/../../../packages/runtime-adapters/test/fixtures/copilot/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn fixture(name: &str) -> Vec<String> {
    fixture_text(name).split('\n').filter(|l| !l.is_empty()).map(str::to_owned).collect()
}

/// A Copilot turn whose CLI prints `lines`, writes `stderr` and exits with `exit`; stopped by the host when `cancelled`.
fn turn(lines: &[String], exit: i32, stderr: &str, cancelled: bool) -> Vec<Event> {
    let temp = Temp(std::env::temp_dir().join(format!("locust-copilot-case-{}", crate::invariants::unique_nanos())));
    std::fs::create_dir_all(&temp.0).unwrap();
    std::fs::write(temp.0.join("copilot.py"), STAND_IN).unwrap();
    std::fs::write(temp.0.join("lines.jsonl"), lines.join("\n")).unwrap();
    std::fs::write(temp.0.join("stderr.txt"), stderr).unwrap();
    let mut command = Command::new("python3");
    command
        .arg(temp.0.join("copilot.py"))
        .env("LOCUST_COPILOT_LINES", temp.0.join("lines.jsonl"))
        .env("LOCUST_COPILOT_STDERR", temp.0.join("stderr.txt"))
        .env("LOCUST_COPILOT_EXIT", exit.to_string())
        .current_dir(&temp.0);
    let mut rt = PrintCopilot::spawn(&mut command).unwrap().with_cancel(Arc::new(AtomicBool::new(cancelled)));
    rt.start(&Turn {
        prompt: "Say OK".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: HOST_SESSION.into(),
        run_id: "run_1".into(),
    })
    .unwrap();
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "the Copilot turn never ended");
        if let Some(e) = rt.next_event().unwrap() {
            let end = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            events.push(e);
            if end {
                return events;
            }
        }
    }
}

fn run(lines: &[String]) -> Vec<Event> {
    turn(lines, 0, "", false)
}

fn kinds(events: &[Event]) -> Vec<&str> {
    events.iter().map(|e| e.kind.as_str()).collect()
}

fn ledger(events: &[Event]) -> String {
    serde_json::to_string(&events.iter().map(|e| (&e.kind, &e.payload)).collect::<Vec<_>>()).unwrap()
}

/// Message text rebuilt the way the checkpoint summary does, by item, in order.
fn messages(events: &[Event]) -> Vec<(String, String)> {
    let mut buffers: Vec<(String, String)> = vec![];
    for e in events.iter().filter(|e| e.kind == "message.delta") {
        let id = e.payload["itemId"].as_str().unwrap().to_owned();
        let piece = e.payload["text"].as_str().unwrap();
        let replace = e.payload["operation"] == "replace";
        match buffers.iter_mut().find(|(k, _)| *k == id) {
            Some((_, text)) if replace => *text = piece.to_owned(),
            Some((_, text)) => text.push_str(piece),
            None => buffers.push((id, piece.to_owned())),
        }
    }
    buffers
}

fn said(events: &[Event]) -> Vec<String> {
    messages(events).into_iter().map(|(_, t)| t).collect()
}

fn of<'a>(events: &'a [Event], kind: &str) -> Vec<&'a Event> {
    events.iter().filter(|e| e.kind == kind).collect()
}

fn tools(events: &[Event]) -> Vec<&Event> {
    events.iter().filter(|e| e.kind.starts_with("tool.")).collect()
}

fn line(v: Value) -> String {
    v.to_string()
}

fn auto() -> String {
    line(json!({"type":"session.auto_mode_resolved","data":{"chosenModel":"m","availableModels":["m"]}}))
}

fn start() -> String {
    line(json!({"type":"tool.execution_start","data":{"toolCallId":"t1","toolName":"powershell","arguments":{"command":"ls"}}}))
}

fn result() -> String {
    line(json!({"type":"result","sessionId":"s1","exitCode":0,"usage":{}}))
}

// a plain Copilot reply, as captured

fn plain() -> Vec<Event> {
    run(&fixture("plain-reply.jsonl"))
}

#[test]
fn opens_the_run_on_the_record_that_names_the_model_it_actually_got() {
    // case: opens the run on the record that names the model it actually got
    let events = plain();
    assert_eq!(events[0].kind, "run.started");
    let evidence = &events[0].payload["evidence"];
    assert_eq!(evidence["redacted"], true);
    assert_eq!(evidence["raw"], json!({"chosenModel":"gpt-5.6-luna","routingMethod":"auto_v2","availableModels":["gpt-5.6-luna"],"fallback":false}));
}

#[test]
fn prefers_the_session_id_the_cli_printed_over_the_one_the_host_passed_in() {
    // case: prefers the session id the CLI printed over the one the host passed in
    let events = plain();
    assert_eq!(events[0].thread.as_deref(), Some(HOST_SESSION));
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.completed");
    assert_eq!(last.thread.as_deref(), Some("5f4fae14-95d8-43b5-8e17-d4108d6cbb17"));
    assert_eq!(last.payload["runtimeThreadId"], "5f4fae14-95d8-43b5-8e17-d4108d6cbb17");
}

#[test]
fn opens_and_closes_the_turn_and_lets_the_complete_message_replace_its_fragments() {
    // case: opens and closes the turn, and lets the complete message replace its fragments
    let events = plain();
    assert_eq!(kinds(&events), ["run.started", "step.started", "message.delta", "message.delta", "step.completed", "run.completed"]);
    assert_eq!(said(&events), ["OK"]);
}

#[test]
fn records_what_the_run_cost_from_the_checkpoint_and_nothing_else_in_that_record() {
    // case: records what the run cost from the checkpoint, and nothing else in that record
    assert_eq!(plain().last().unwrap().payload["usage"], json!({"premiumRequests":1,"nanoAiu":383_710_000}));
}

#[test]
fn never_lets_the_clis_system_prompt_into_the_ledger() {
    // case: never lets the CLI's system prompt into the ledger
    let serialized = ledger(&plain());
    assert!(!serialized.contains("You are the GitHub Copilot CLI"));
    assert!(!serialized.contains("messages_snapshot"));
}

#[test]
fn never_carries_the_models_opaque_reasoning_or_the_accounts_request_handles() {
    // case: never carries the model's opaque reasoning or the account's request handles
    let serialized = ledger(&plain());
    assert!(!serialized.contains("[opaque "));
    assert!(serialized.contains("\"apiCallId\":\"[redacted]\""), "{serialized}");
}

// a Copilot run that wrote a file, as captured

fn wrote() -> Vec<Event> {
    run(&fixture("write-apply-patch.jsonl"))
}

#[test]
fn reports_apply_patch_as_one_tool_that_started_and_completed_with_the_patch_it_applied() {
    // case: reports apply_patch as one tool that started and completed, with the patch it applied
    let events = wrote();
    let tools = tools(&events);
    assert_eq!(tools.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(), ["tool.started", "tool.completed"]);
    assert_eq!(tools[0].payload["toolKind"], "apply_patch");
    assert!(tools[0].payload["command"].as_str().unwrap().contains("*** Add File: notes.txt"));
    let patch = &tools[1].payload["patch"];
    assert!(patch["text"].as_str().unwrap().contains("diff --git a/C:/work/pebble/cp-write/notes.txt"));
    assert!(patch["text"].as_str().unwrap().contains("+hello from copilot."));
    assert_eq!((&patch["added"], &patch["removed"]), (&json!(2), &json!(0)));
}

#[test]
fn writes_no_message_for_the_turn_that_only_asked_for_a_tool() {
    // case: writes no message for the turn that only asked for a tool
    assert_eq!(said(&wrote()), ["DONE"]);
}

#[test]
fn keeps_the_two_turns_apart_each_opened_and_closed() {
    // case: keeps the two turns apart, each opened and closed
    let events = wrote();
    let steps: Vec<&Value> =
        events.iter().filter(|e| e.kind == "step.started" || e.kind == "step.completed").map(|e| &e.payload["itemId"]).collect();
    assert_eq!(steps, [&json!("0"), &json!("0"), &json!("1"), &json!("1")]);
}

#[test]
fn never_lets_the_operators_own_local_skills_into_the_ledger() {
    // case: never lets the operator's own local skills into the ledger
    let serialized = ledger(&wrote());
    assert!(!serialized.contains("skills_loaded"));
    assert!(!serialized.contains("SKILL.md"));
}

// a read-only Copilot run whose tools were denied, as captured

fn denied() -> Vec<Event> {
    run(&fixture("read-only-tools-denied.jsonl"))
}

#[test]
fn reports_both_refusals_as_tools_that_failed_never_as_tools_that_completed() {
    // case: reports both refusals as tools that failed, never as tools that completed
    let events = denied();
    assert_eq!(
        tools(&events).iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        ["tool.started", "tool.failed", "tool.started", "tool.failed"]
    );
    let failed = of(&events, "tool.failed");
    assert_eq!(failed.iter().map(|e| e.payload["toolKind"].as_str().unwrap()).collect::<Vec<_>>(), ["apply_patch", "powershell"]);
    assert!(failed.iter().all(|e| e.payload["status"] == "denied"));
}

#[test]
fn names_what_each_refused_tool_wanted_to_do_in_the_two_shapes_the_cli_uses() {
    // case: names what each refused tool wanted to do, in the two shapes the CLI uses
    let events = denied();
    let refused: Vec<&str> = of(&events, "tool.failed").iter().map(|e| e.payload["command"].as_str().unwrap()).collect();
    assert!(refused[0].contains("*** Add File: blocked.txt"));
    assert_eq!(refused[1], "Set-Content -Path '.\\blocked.txt' -Value 'hi' -NoNewline");
}

#[test]
fn carries_the_runs_own_account_of_what_it_could_not_do() {
    // case: carries the run's own account of what it could not do
    let events = denied();
    assert_eq!(said(&events), ["Unable to create `blocked.txt` because file-writing tools are blocked."]);
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

// a Copilot session set and then resumed, as captured

#[test]
fn reports_the_same_thread_id_on_both_turns_which_is_what_resume_takes_back() {
    // case: reports the same thread id on both turns, which is what --resume takes back
    let first = run(&fixture("session-first-turn.jsonl"));
    let second = run(&fixture("session-resumed-turn.jsonl"));
    assert_eq!(first.last().unwrap().thread.as_deref(), Some("9a4575f4-30d4-4d2e-99b3-07d9da08fae5"));
    assert_eq!(second.last().unwrap().thread, first.last().unwrap().thread);
}

#[test]
fn shows_the_second_turn_recalling_what_the_first_was_told() {
    // case: shows the second turn recalling what the first was told
    assert_eq!(said(&run(&fixture("session-first-turn.jsonl"))), ["STORED"]);
    assert_eq!(said(&run(&fixture("session-resumed-turn.jsonl"))), ["marmalade-4471"]);
}

#[test]
fn shows_that_thinking_happened_without_writing_down_what_was_thought() {
    // case: shows that thinking happened without writing down what was thought
    let second = run(&fixture("session-resumed-turn.jsonl"));
    let reasoning: Vec<&str> = second.iter().filter(|e| e.payload["stepKind"] == "reasoning").map(|e| e.kind.as_str()).collect();
    assert_eq!(reasoning, ["step.started", "step.completed"]);
    let serialized = ledger(&second);
    assert!(!serialized.contains("[opaque "));
    assert!(serialized.contains("\"reasoningId\":\"[redacted]\""), "{serialized}");
}

// a Copilot run an organisation policy refused, as captured

fn policy() -> Vec<Event> {
    turn(&fixture("policy-denied.jsonl"), 1, &fixture_text("stderr-policy-denied.txt"), false)
}

#[test]
fn fails_with_a_reason_a_person_can_act_on_not_just_the_process_exited_1() {
    // case: fails with a reason a person can act on, not just 'the process exited 1'
    let events = policy();
    let failed = events.last().unwrap();
    assert_eq!(failed.kind, "run.failed");
    assert_eq!(failed.payload["kind"], "authentication-failed");
    let message = failed.payload["message"].as_str().unwrap();
    assert!(message.contains("Copilot plan required"));
    assert!(message.contains("https://github.com/settings/copilot"));
}

#[test]
fn says_nothing_about_the_run_beyond_the_session_warning_it_printed() {
    // case: says nothing about the run beyond the session warning it printed
    let events = policy();
    assert_eq!(kinds(&events), ["adapter.diagnostic", "run.failed"]);
    assert!(events[0].payload["message"].as_str().unwrap().contains("Third-party MCP servers are disabled"));
}

// a Copilot run given a model the account cannot use

#[test]
fn fails_in_the_clis_own_words_which_name_the_model_it_was_handed() {
    // case: fails in the CLI's own words, which name the model it was handed
    let events = turn(&[], 1, &fixture_text("stderr-unknown-model.txt"), false);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "run.failed");
    assert_eq!(events[0].payload["kind"], "unknown");
    assert_eq!(events[0].payload["message"], "Model \"definitely-not-a-model\" from --model flag is not available.");
}

#[test]
fn reads_both_refusals_off_stderr_and_nothing_else_off_it() {
    // case: reads both refusals off stderr and nothing else off it
    assert_eq!(failure_from(&fixture_text("stderr-policy-denied.txt")).unwrap().0, "authentication-failed");
    assert_eq!(failure_from(&fixture_text("stderr-unknown-model.txt")).unwrap().0, "unknown");
    assert_eq!(failure_from("Error: something else entirely"), None);
    assert_eq!(failure_from(""), None);
}

// what a completed Copilot tool call means

#[test]
fn keeps_a_long_failure_message_within_what_the_ledger_keeps_2026_10_10_sweep() {
    // case: keeps a long failure message within what the ledger keeps (2026-10-10 sweep)
    let events = run(&[
        auto(),
        start(),
        line(json!({"type":"tool.execution_complete","data":{"toolCallId":"t1","success":false,"error":{"message":"x".repeat(600)}}})),
    ]);
    let failed = of(&events, "tool.failed");
    assert_eq!(failed[0].payload["status"].as_str().unwrap().chars().count(), 512);
}

#[test]
fn is_a_failure_unless_the_cli_said_success_in_so_many_words() {
    // case: is a failure unless the CLI said success in so many words
    for data in [
        json!({"toolCallId":"t1","success":false,"error":{"message":"boom","code":"failed"}}),
        json!({"toolCallId":"t1"}),
        json!({"toolCallId":"t1","success":"true"}),
    ] {
        let events = run(&[auto(), start(), line(json!({"type":"tool.execution_complete","data":data}))]);
        assert!(events.iter().any(|e| e.kind == "tool.failed"), "{data}");
        assert!(!events.iter().any(|e| e.kind == "tool.completed"), "{data}");
    }
}

#[test]
fn records_no_patch_for_a_tool_whose_detail_was_prose_rather_than_a_diff() {
    // case: records no patch for a tool whose detail was prose rather than a diff
    let events = run(&[
        auto(),
        start(),
        line(json!({"type":"tool.execution_complete","data":{"toolCallId":"t1","success":true,"result":{"content":"ok","detailedContent":"Listed 3 files."}}})),
    ]);
    assert!(of(&events, "tool.completed")[0].payload.get("patch").is_none());
}

#[test]
fn names_a_tool_call_whichever_way_its_arguments_were_spelled() {
    // case: names a tool call whichever way its arguments were spelled
    assert_eq!(tool_command(&json!("*** Begin Patch\n")).as_deref(), Some("*** Begin Patch\n"));
    assert_eq!(tool_command(&json!({"command":"ls","description":"list"})).as_deref(), Some("ls"));
    assert_eq!(tool_command(&json!({"description":"list"})).as_deref(), Some("list"));
    assert_eq!(tool_command(&Value::Null), None);
}

// what this adapter refuses to write down

#[test]
fn strips_the_opaque_blobs_wherever_in_a_record_they_sit() {
    // case: strips the opaque blobs wherever in a record they sit
    let scrubbed = scrub_copilot_record(
        &json!({"type":"assistant.message","data":{"content":"hi","reasoningOpaque":"AAAA","nested":[{"encryptedContent":"BBBB","keep":1}]}}),
        0,
    );
    assert_eq!(scrubbed["data"]["content"], "hi");
    assert_eq!(scrubbed["data"]["reasoningOpaque"], "[redacted]");
    assert_eq!(scrubbed["data"]["nested"][0]["encryptedContent"], "[redacted]");
    assert_eq!(scrubbed["data"]["nested"][0]["keep"], 1);
}

#[test]
fn keeps_only_the_four_auto_mode_fields_that_say_which_model_ran() {
    // case: keeps only the four auto-mode fields that say which model ran
    assert_eq!(
        summarize_auto_mode(&json!({"chosenModel":"m","routingMethod":"auto_v2","availableModels":["m"],"fallback":false,"categoryScores":{"debugging":0.6},"endToEndLatencyMs":145.9})),
        json!({"chosenModel":"m","routingMethod":"auto_v2","availableModels":["m"],"fallback":false})
    );
    assert_eq!(summarize_auto_mode(&json!("not an object")), json!({}));
}

#[test]
fn keeps_only_the_two_totals_from_a_checkpoint_that_is_mostly_cache_bookkeeping() {
    // case: keeps only the two totals from a checkpoint that is mostly cache bookkeeping
    assert_eq!(
        usage(&json!({"totalPremiumRequests":2,"totalNanoAiu":5,"promptCacheBreakState":[{"big":"x"}]})),
        Some(json!({"premiumRequests":2,"nanoAiu":5}))
    );
    assert_eq!(usage(&json!({"promptCacheBreakState":[]})), None);
}

// how a Copilot run ends

#[test]
fn is_a_failure_when_the_cli_exits_clean_without_ever_printing_its_result_record() {
    // case: is a failure when the CLI exits clean without ever printing its result record
    let events = run(&[auto()]);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert_eq!((&last.payload["kind"], &last.payload["runtimeTerminal"]), (&json!("process-failed"), &json!("missing")));
}

#[test]
fn is_a_failure_when_the_result_record_itself_reports_a_non_zero_exit() {
    // case: is a failure when the result record itself reports a non-zero exit
    let events = run(&[auto(), line(json!({"type":"result","sessionId":"s1","exitCode":2}))]);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert_eq!(last.payload["runtimeTerminal"], "failed");
}

#[test]
fn is_cancelled_when_the_host_cancelled_it() {
    // case: is cancelled when the host cancelled it
    assert_eq!(turn(&[auto(), result()], 0, "", true).last().unwrap().kind, "run.cancelled");
}

#[test]
fn still_resumes_from_the_hosts_id_when_the_cli_never_printed_one() {
    // case: still resumes from the host's id when the CLI never printed one
    let events = run(&[auto()]);
    assert_eq!(events.last().unwrap().thread.as_deref(), Some(HOST_SESSION));
    assert_eq!(events.last().unwrap().payload["runtimeThreadId"], HOST_SESSION);
}

#[test]
fn reports_a_record_it_cannot_parse_as_a_diagnostic_and_keeps_going() {
    // case: reports a record it cannot parse as a diagnostic and keeps going
    let events = run(&["{not json".to_owned(), auto(), result()]);
    assert_eq!(events[0].kind, "adapter.diagnostic");
    assert_eq!(events[0].payload["code"], "copilot.malformed_record");
    assert_eq!(kinds(&events)[1..], ["run.started", "run.completed"]);
}

#[test]
fn says_so_when_it_meets_a_record_type_this_build_has_never_seen() {
    // case: says so when it meets a record type this build has never seen
    let events = run(&[auto(), line(json!({"type":"assistant.brand_new","data":{}})), result()]);
    assert!(events.iter().any(|e| e.kind == "adapter.diagnostic" && e.payload["code"] == "copilot.unknown_event"));
}

// a view is not a change

#[test]
fn attaches_a_patch_to_the_edit_alone_the_views_diff_shaped_listing_carries_none() {
    // case: attaches a patch to the edit alone; the view's diff-shaped listing carries none
    let events = run(&fixture("write-mode-view-then-edit.jsonl"));
    let completed = of(&events, "tool.completed");
    let with_patch: Vec<&&Event> = completed.iter().filter(|e| e.payload.get("patch").is_some()).collect();
    assert_eq!(with_patch.iter().map(|e| e.payload["toolKind"].as_str().unwrap()).collect::<Vec<_>>(), ["apply_patch"]);
    let patch = &with_patch[0].payload["patch"];
    assert_eq!((&patch["added"], &patch["removed"]), (&json!(1), &json!(1)));
    assert!(completed.iter().any(|e| e.payload["toolKind"] == "view"));
}

// Copilot CLI 1.0.83's stream, measured 2026-09-06: each record's events are read off the whole turn, in order.

#[test]
fn opens_one_reasoning_step_on_the_first_delta_closes_it_on_the_block_and_says_nothing_per_delta() {
    // case: opens one reasoning step on the first delta, closes it on the block, and says nothing per delta
    let events = run(&[
        line(json!({"type":"assistant.reasoning_delta","data":{"reasoningId":"r1","deltaContent":"**Reading"}})),
        line(json!({"type":"assistant.reasoning_delta","data":{"reasoningId":"r1","deltaContent":" README"}})),
        line(json!({"type":"assistant.reasoning_delta","data":{"reasoningId":"r1","deltaContent":" file**"}})),
        line(json!({"type":"assistant.reasoning","data":{"reasoningId":"r1","content":"**Reading README file**"}})),
    ]);
    let before_end = &events[..events.len() - 1];
    assert_eq!(kinds(before_end), ["step.started", "step.completed"]);
    assert!(!ledger(before_end).contains("r1"));
}

#[test]
fn stays_quiet_on_the_streams_bookkeeping_and_says_an_unknown_type_once() {
    // case: stays quiet on the stream's bookkeeping and says an unknown type once
    let mut lines: Vec<String> = ["assistant.tool_call_delta", "model.call_start", "model.call_finished", "assistant.message_start", "assistant.idle"]
        .iter()
        .map(|t| line(json!({"type":t,"data":{}})))
        .collect();
    lines.push(line(json!({"type":"assistant.something_new","data":{}})));
    lines.push(line(json!({"type":"assistant.something_new","data":{}})));
    let events = run(&lines);
    let before_end = &events[..events.len() - 1];
    assert_eq!(kinds(before_end), ["adapter.diagnostic"]);
    assert_eq!(before_end[0].payload["code"], "copilot.unknown_event");
}

#[test]
fn says_nothing_of_the_tools_locust_itself_turned_off_and_still_says_any_other_notice_once_0378() {
    // case: says nothing of the tools Locust itself turned off, and still says any other notice once (0.378)
    let quiet = run(&[line(
        json!({"type":"session.info","data":{"infoType":"configuration","message":"Disabled tools: session_store_sql"},"ephemeral":true}),
    )]);
    assert_eq!(kinds(&quiet), ["run.failed"]);
    let other = run(&[line(json!({"type":"session.info","data":{"infoType":"model","message":"Switched to another model"}}))]);
    let before_end = &other[..other.len() - 1];
    assert_eq!(kinds(before_end), ["adapter.diagnostic"]);
    assert_eq!(before_end[0].payload["code"], "copilot.unknown_event");
}

#[test]
fn names_what_view_glob_and_grep_acted_on() {
    // case: names what view, glob and grep acted on
    let events = run(&[
        line(json!({"type":"tool.execution_start","data":{"toolCallId":"c1","toolName":"view","arguments":{"path":"README.md"}}})),
        line(json!({"type":"tool.execution_start","data":{"toolCallId":"c2","toolName":"glob","arguments":{"pattern":"**/*.md"}}})),
    ]);
    let started = of(&events, "tool.started");
    assert_eq!(started[0].payload["command"], "README.md");
    assert_eq!(started[1].payload["command"], "**/*.md");
}

// a reply whose deltas carried a key

#[test]
fn is_still_closed_by_its_complete_message_and_the_key_stays_redacted() {
    // case: is still closed by its complete message, and the key stays redacted
    let key = format!("sk-{}", "a".repeat(40));
    let events = run(&[
        line(json!({"type":"assistant.message_delta","data":{"messageId":"m1","deltaContent":format!("The key is {key}. ")}})),
        line(json!({"type":"assistant.message_delta","data":{"messageId":"m1","deltaContent":"Done."}})),
        line(json!({"type":"assistant.message","data":{"messageId":"m1","content":format!("The key is {key}. Done.")}})),
    ]);
    let deltas = of(&events, "message.delta");
    assert!(deltas.iter().any(|d| d.payload["final"] == true));
    assert!(deltas.iter().all(|d| !d.payload["text"].as_str().unwrap().contains(&key)));
    assert_eq!(messages(&events).into_iter().find(|(id, _)| id == "msg_m1").map(|(_, t)| t).as_deref(), Some("The key is [redacted]. Done."));
}
