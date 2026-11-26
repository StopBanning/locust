//! Electron's `packages/runtime-adapters/test/opencode-events.test.ts`, case by case. Electron drives its normalizer
//! (`createOpenCodeEventNormalizer`) over OpenCode's JSON records and ends it with the process's completion; the native
//! app reads OpenCode through its server, and opencode_serve.rs `push` hands each record it builds (`step_start`,
//! `text`, `tool_use`, `step_finish`, `error`) to this same `Normalizer::accept` with the session stamped on, then
//! `finish` with how the run ended. So each case runs Electron's own captures (`test/fixtures/opencode/`) or its own
//! records through `accept` and `finish`, as the server driver does. Electron's helpers `addedFilePatch`,
//! `openCodeToolOutcome` and `openCodeToolTarget` are inline in `accept` here, so their cases go through a tool record.
use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn fixture(name: &str) -> Vec<Value> {
    std::fs::read_to_string(format!(
        "{}/../../../packages/runtime-adapters/test/fixtures/opencode/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
    .split('\n')
    .filter(|l| !l.is_empty())
    .map(|l| serde_json::from_str(l).unwrap())
    .collect()
}

/// How the run ended, as `finish` is told it (Electron's `RuntimeProcessCompletion`: exit 0, not cancelled).
#[derive(Clone)]
struct End {
    cancelled: bool,
    code: Option<i32>,
    stderr: &'static str,
    output_limit: bool,
}
const CLEAN: End = End { cancelled: false, code: Some(0), stderr: "", output_limit: false };
fn exit(code: i32) -> End {
    End { code: Some(code), ..CLEAN }
}

fn run_ending(records: &[Value], end: End) -> (Normalizer, Vec<Event>) {
    let mut normal = Normalizer::new(None);
    let mut events: Vec<Event> = records.iter().flat_map(|r| normal.accept(r)).collect();
    events.extend(normal.finish(end.cancelled, end.code, end.stderr, end.output_limit));
    (normal, events)
}
fn run(records: &[Value]) -> Vec<Event> {
    run_ending(records, CLEAN).1
}

/// Message text rebuilt the way the checkpoint summary does, in the order each item first appeared.
fn messages(events: &[Event]) -> Vec<String> {
    let mut order = vec![];
    let mut buffers: BTreeMap<String, String> = BTreeMap::new();
    for e in events.iter().filter(|e| e.kind == "message.delta") {
        let id = e.payload["itemId"].as_str().unwrap().to_owned();
        let text = e.payload["text"].as_str().unwrap();
        if !buffers.contains_key(&id) {
            order.push(id.clone());
        }
        let held = buffers.entry(id).or_default();
        if e.payload["operation"] == "replace" {
            *held = text.into();
        } else {
            held.push_str(text);
        }
    }
    order.into_iter().map(|id| buffers.remove(&id).unwrap()).collect()
}

fn kinds<'a>(events: &'a [Event], prefix: &str) -> Vec<&'a str> {
    events.iter().filter(|e| e.kind.starts_with(prefix)).map(|e| e.kind.as_str()).collect()
}

fn record(kind: &str, part: Value) -> Value {
    json!({"type":kind,"sessionID":"ses_1","part":part})
}
fn started() -> Value {
    record("step_start", json!({"type":"step-start"}))
}
fn stopped(reason: &str) -> Value {
    record("step_finish", json!({"type":"step-finish","reason":reason,"tokens":{"input":10,"output":2}}))
}
fn tool(name: &str, state: Value) -> Value {
    record("tool_use", json!({"type":"tool","tool":name,"callID":"call_1","state":state}))
}
fn completed_tool(events: &[Event]) -> Value {
    events.iter().find(|e| e.kind == "tool.completed" || e.kind == "tool.failed").unwrap().payload.clone()
}
fn failed_message(events: &[Event]) -> String {
    events.iter().find(|e| e.kind == "run.failed").unwrap().payload["message"].as_str().unwrap().to_owned()
}
fn diagnostics(events: &[Event], code: &str) -> usize {
    events.iter().filter(|e| e.kind == "adapter.diagnostic" && e.payload["code"] == code).count()
}

// a write-mode OpenCode run that read and then wrote, as captured

#[test]
fn takes_the_session_id_from_the_first_record_not_from_a_header_record_it_never_sends() {
    let (normal, events) = run_ending(&fixture("write-mode-read-and-write.jsonl"), CLEAN);
    assert_eq!(normal.base.thread.as_deref(), Some("ses_f99eea38bffe0EoUlZ5GohEmmF"));
    assert_eq!(events[0].kind, "step.started");
    assert!(events.iter().all(|e| e.thread.as_deref() == Some("ses_f99eea38bffe0EoUlZ5GohEmmF")));
}

#[test]
fn opens_and_closes_one_turn_per_step_so_nothing_is_left_looking_unfinished() {
    let events = run(&fixture("write-mode-read-and-write.jsonl"));
    let steps: Vec<_> = events.iter().filter(|e| e.kind == "step.started" || e.kind == "step.completed").collect();
    assert_eq!(
        steps.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        ["step.started", "step.completed", "step.started", "step.completed", "step.started", "step.completed"]
    );
    assert!(steps.iter().all(|e| e.payload["stepKind"] == "turn"));
}

#[test]
fn delivers_each_text_part_whole_as_its_own_final_message() {
    let events = run(&fixture("write-mode-read-and-write.jsonl"));
    assert_eq!(messages(&events), ["Creating your notes file.", "File location confirmed — writing it now.", "DONE"]);
    let deltas: Vec<_> = events.iter().filter(|e| e.kind == "message.delta").collect();
    assert!(deltas.iter().all(|e| e.payload["final"] == true));
    assert!(deltas.iter().all(|e| e.payload["operation"] == "replace"));
}

#[test]
fn reports_each_tool_call_as_a_started_and_a_completed_event_under_its_call_id() {
    let events = run(&fixture("write-mode-read-and-write.jsonl"));
    let tools: Vec<_> = events.iter().filter(|e| e.kind.starts_with("tool.")).collect();
    assert_eq!(
        tools.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        ["tool.started", "tool.completed", "tool.started", "tool.completed"]
    );
    assert_eq!(
        tools.iter().map(|e| e.payload["toolKind"].as_str().unwrap()).collect::<Vec<_>>(),
        ["read", "read", "write", "write"]
    );
    assert_eq!(tools[0].payload["itemId"], "call_01a066117b107a81bd58d10b2637c61b");
    assert_eq!(tools[0].payload["command"], "C:\\work\\pebble");
}

#[test]
fn carries_the_created_files_change_built_from_the_content_it_wrote() {
    let events = run(&fixture("write-mode-read-and-write.jsonl"));
    let write = events.iter().find(|e| e.kind == "tool.completed" && e.payload["toolKind"] == "write").unwrap();
    let patch = &write.payload["patch"];
    let text = patch["text"].as_str().unwrap();
    assert!(text.contains("--- /dev/null"));
    assert!(text.contains("+++ b/C:\\work\\pebble\\notes.txt"));
    assert!(text.contains("+hello from opencode"));
    assert_eq!((patch["added"].as_u64(), patch["removed"].as_u64()), (Some(1), Some(0)));
}

#[test]
fn adds_up_what_every_step_cost_rather_than_reporting_the_last_step_as_the_total() {
    // 8673 + 1041 + 389 in, 113 + 136 + 11 out plus 853 reasoning, 18,835 read from cache; an explicit 0 cost is left off.
    let events = run(&fixture("write-mode-read-and-write.jsonl"));
    let done = events.last().unwrap();
    assert_eq!(done.kind, "run.completed");
    assert_eq!(done.payload["usage"], json!({"inputTokens":10_103,"outputTokens":1_113,"cacheReadTokens":18_835}));
}

#[test]
fn carries_a_real_cost_when_a_model_has_one() {
    let events = run(&[
        started(),
        record(
            "step_finish",
            json!({"type":"step-finish","reason":"stop","cost":0.0123,"tokens":{"input":900,"output":40,"reasoning":10,"cache":{"read":100,"write":50}}}),
        ),
    ]);
    assert_eq!(
        events.last().unwrap().payload["usage"],
        json!({"inputTokens":900,"outputTokens":50,"cacheReadTokens":100,"cacheWriteTokens":50,"usd":0.0123})
    );
}

// a resumed OpenCode turn, as captured

#[test]
fn keeps_the_earlier_session_id_which_is_what_s_takes_back() {
    let (normal, events) = run_ending(&fixture("resumed-turn.jsonl"), CLEAN);
    assert_eq!(normal.base.thread.as_deref(), Some("ses_f99eea38bffe0EoUlZ5GohEmmF"));
    assert_eq!(messages(&events), ["DONE"]);
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

// a read-only OpenCode run, as captured: the permission config denies edit/write/bash/patch

#[test]
fn shows_a_run_that_could_only_read_and_no_write_tool_anywhere_in_it() {
    let events = run(&fixture("read-only-no-write-tool.jsonl"));
    let tools: Vec<_> = events
        .iter()
        .filter(|e| e.kind.starts_with("tool."))
        .map(|e| e.payload["toolKind"].as_str().unwrap())
        .collect();
    assert_eq!(tools, ["read", "read"]);
    assert!(!events.iter().any(|e| e.kind == "tool.failed"));
}

#[test]
fn carries_the_runtimes_own_account_of_what_it_was_not_given() {
    let said = messages(&run(&fixture("read-only-no-write-tool.jsonl"))).join("\n");
    assert!(said.contains("no write tool is available"));
    assert!(said.contains("`blocked.txt` was not created"));
}

// an OpenCode plan-agent run, as captured

#[test]
fn is_a_run_that_only_talked_which_is_why_plan_mode_is_not_the_read_only_mechanism() {
    let events = run(&fixture("plan-agent-text-only.jsonl"));
    assert!(kinds(&events, "tool.").is_empty());
    assert!(messages(&events)[0].contains("In PLAN MODE"));
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

// a change OpenCode reported (Electron's addedFilePatch, built inside `accept` for a write)

fn written(content: &str, exists: Value) -> Option<Value> {
    let mut meta = json!({});
    if !exists.is_null() {
        meta["exists"] = exists;
    }
    let events = run(&[tool(
        "write",
        json!({"status":"completed","input":{"filePath":"a.txt","content":content},"metadata":meta,"output":""}),
    )]);
    let patch = completed_tool(&events)["patch"].clone();
    (!patch.is_null()).then_some(patch)
}

#[test]
fn becomes_a_patch_only_when_the_runtime_said_the_file_did_not_exist() {
    let made = written("one\ntwo\n", json!(false)).unwrap();
    assert_eq!((made["added"].as_u64(), made["removed"].as_u64()), (Some(2), Some(0)));
    // An overwrite reports no before-text, so there is no diff to be had.
    assert!(written("one\n", json!(true)).is_none());
    assert!(written("one\n", Value::Null).is_none());
}

#[test]
fn counts_a_trailing_newline_as_the_end_of_the_last_line_not_as_another_one() {
    assert_eq!(written("only\n", json!(false)).unwrap()["added"], 1);
    assert_eq!(written("only", json!(false)).unwrap()["added"], 1);
}

// what a tool call's status means (Electron's openCodeToolOutcome and openCodeToolTarget, inside `accept`)

fn outcome(state: Value) -> Value {
    completed_tool(&run(&[tool("bash", state)]))["status"].clone()
}
fn failed(state: Value) -> bool {
    run(&[tool("bash", state)]).iter().any(|e| e.kind == "tool.failed")
}

#[test]
fn treats_anything_but_completed_as_a_failure_including_nothing_at_all() {
    const REJECTED: &str = "The user rejected permission to use this specific tool call";
    assert!(!failed(json!({"status":"completed"})));
    assert!(outcome(json!({"status":"completed"})).is_null());
    assert!(failed(json!({"status":"error"})));
    assert_eq!(outcome(json!({"status":"error"})), "error");
    // A declined or mode-refused call never ran, and says so.
    assert_eq!(outcome(json!({"status":"error","error":format!("{REJECTED}.")})), "refused");
    assert_eq!(
        outcome(json!({"status":"error","error":format!("{REJECTED} with the following feedback: The person declined this in Locust.")})),
        "declined"
    );
    assert_eq!(
        outcome(json!({"status":"error","error":format!("{REJECTED} with the following feedback: The person declined this, and said: Keep it as hi, please.")})),
        "declined"
    );
    assert_eq!(
        outcome(json!({"status":"error","error":format!("{REJECTED} with the following feedback: {}", super::super::opencode_serve::OPENCODE_REFUSED_UNASKED)})),
        "refused"
    );
    assert_eq!(
        outcome(json!({"status":"error","error":format!("{REJECTED} with the following feedback: Locust refused this without asking anyone: this teammate is in Ask mode, which reads and changes nothing, so nothing was run.")})),
        "refused"
    );
    for shape in [json!({"status":"pending"}), json!({"status":""}), json!({})] {
        assert!(failed(shape));
    }
}

#[test]
fn names_what_a_tool_acted_on_whichever_field_the_tool_used() {
    let target = |input: Value, metadata: Value| {
        completed_tool(&run(&[tool("read", json!({"status":"completed","input":input,"metadata":metadata}))]))["command"].clone()
    };
    assert_eq!(target(json!({"filePath":"a.txt"}), json!({})), "a.txt");
    assert_eq!(target(json!({"command":"ls"}), json!({})), "ls");
    assert_eq!(target(json!({}), json!({"filepath":"b.txt"})), "b.txt");
    assert!(target(json!({}), json!({})).is_null());
}

// a stop at a tool the mode refuses (0.543)

#[test]
fn says_nothing_was_changed_only_when_nothing_that_could_change_anything_had_finished() {
    let refused = End { code: Some(1), stderr: "permission requested: bash (npm test); auto-rejecting", ..CLEAN };
    let wrote = tool("write", json!({"status":"completed","input":{"filePath":"notes.md","content":"hi"},"output":""}));
    assert_eq!(
        failed_message(&run_ending(&[started()], refused.clone()).1),
        "The mode this run is in does not allow bash, so OpenCode stopped when it tried to use it. Nothing was changed."
    );
    assert_eq!(
        failed_message(&run_ending(&[started(), wrote], refused).1),
        "The mode this run is in does not allow bash, so OpenCode stopped when it tried to use it. What it did before that still stands: look at the folder."
    );
}

// a failed OpenCode tool call

#[test]
fn is_reported_as_failed_never_as_a_tool_that_completed() {
    let events = run(&[started(), tool("bash", json!({"status":"error","input":{"command":"npm test"},"output":"denied"}))]);
    assert_eq!(kinds(&events, "tool."), ["tool.started", "tool.failed"]);
    let p = completed_tool(&events);
    assert_eq!(p["status"], "error");
    assert_eq!(p["command"], "npm test");
}

// how an OpenCode run ends

#[test]
fn is_a_failure_when_the_stream_stops_mid_tool_call_however_cleanly_the_process_exited() {
    let events = run(&[started(), stopped("tool-calls")]);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert_eq!(last.payload["kind"], "process-failed");
    assert_eq!(last.payload["runtimeTerminal"], "missing");
}

#[test]
fn is_a_failure_when_the_process_exited_non_zero_even_after_a_step_said_stop() {
    let events = run_ending(&[started(), stopped("stop")], exit(1)).1;
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert_eq!(last.payload["runtimeTerminal"], "completed");
}

#[test]
fn is_cancelled_when_the_host_cancelled_it() {
    let events = run_ending(&[started(), stopped("stop")], End { cancelled: true, ..CLEAN }).1;
    assert_eq!(events.last().unwrap().kind, "run.cancelled");
}

#[test]
fn says_so_when_it_meets_a_record_type_this_build_has_never_seen() {
    let events = run(&[started(), json!({"type":"brand_new","sessionID":"ses_1"}), stopped("stop")]);
    assert_eq!(diagnostics(&events, "opencode.unknown_event"), 1);
}

// an edit's own diff

#[test]
fn attaches_the_unified_diff_opencode_reported_for_an_edit_with_counts_from_it() {
    let events = run(&fixture("write-mode-edit-with-diff.jsonl"));
    let edit = events.iter().find(|e| e.kind == "tool.completed" && e.payload["toolKind"] == "edit").unwrap();
    let patch = &edit.payload["patch"];
    assert_eq!((patch["added"].as_u64(), patch["removed"].as_u64()), (Some(1), Some(1)));
    let text = patch["text"].as_str().unwrap();
    assert!(text.contains("-export const status = \"draft\";"));
    assert!(text.contains("+export const status = \"final\";"));
    assert!(!text.contains("<home>"));
}

// a run the host stopped for output volume

#[test]
fn says_so_instead_of_blaming_a_missing_stop_step() {
    let message = failed_message(&run_ending(&[], End { code: None, output_limit: true, ..CLEAN }).1);
    assert!(message.contains("faster than Locust could record it"));
    assert!(!message.contains("without a step that reported it had stopped"));
}

#[test]
fn still_puts_the_confined_workspace_refusal_first_which_says_more() {
    // Natively the refusal reaches `finish` as the server's own words on stderr, so the run here names the path.
    let both = End { code: None, stderr: "permission requested: external_directory (/etc/passwd)", output_limit: true, ..CLEAN };
    let message = failed_message(&run_ending(&[], both).1);
    assert!(message.contains("outside the folder"), "{message}");
    assert!(!message.contains("faster than Locust could record it"));
}

#[test]
fn leaves_an_ordinary_non_zero_exit_saying_what_it_always_said() {
    assert!(failed_message(&run_ending(&[], exit(2)).1).contains("without a step that reported it had stopped"));
}

// OpenCode's plan: `todowrite` is `plan.updated`, as Codex's todo_list is

const STEPS: [&str; 3] = ["read README", "write notes", "verify"];
const SEQUENCE: [[&str; 3]; 4] = [
    ["in_progress", "pending", "pending"],
    ["completed", "in_progress", "pending"],
    ["completed", "completed", "in_progress"],
    ["completed", "completed", "completed"],
];
fn todo_line(todos: Value, call: &str) -> Value {
    json!({"type":"tool_use","sessionID":"ses_1","part":{"tool":"todowrite","callID":call,"state":{"status":"completed","input":{"todos":todos}}}})
}
fn sequence() -> Vec<Value> {
    SEQUENCE
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let todos: Vec<_> =
                STEPS.iter().enumerate().map(|(s, content)| json!({"id":s.to_string(),"content":content,"status":row[s]})).collect();
            todo_line(json!(todos), &format!("call_{i}"))
        })
        .collect()
}
fn field(plan: &Event, key: &str) -> Vec<String> {
    plan.payload["plan"].as_array().unwrap().iter().map(|s| s[key].as_str().unwrap().to_owned()).collect()
}

#[test]
fn becomes_a_plan_four_times_not_four_tool_rows() {
    let events = run(&sequence());
    assert_eq!(events.iter().filter(|e| e.kind == "plan.updated").count(), 4);
    assert!(kinds(&events, "tool.").is_empty());
}

#[test]
fn carries_the_steps_and_their_advancing_state() {
    let events = run(&sequence());
    let plans: Vec<_> = events.iter().filter(|e| e.kind == "plan.updated").collect();
    assert_eq!(field(plans[0], "content"), STEPS);
    assert_eq!(field(plans[0], "status"), ["in_progress", "pending", "pending"]);
    assert_eq!(field(plans.last().unwrap(), "status"), ["completed", "completed", "completed"]);
}

#[test]
fn invents_no_plan_when_a_todo_call_carries_no_todos() {
    let events = run(&[todo_line(json!([]), "call_0")]);
    assert_eq!(events.iter().filter(|e| e.kind == "plan.updated").count(), 0);
    assert_eq!(events.iter().filter(|e| e.kind == "tool.completed").count(), 1);
}

#[test]
fn leaves_every_other_tool_alone() {
    let events = run(&[json!({"type":"tool_use","sessionID":"ses_1","part":{"tool":"bash","callID":"call_0","state":{"status":"completed","input":{"command":"ls"}}}})]);
    assert_eq!(events.iter().filter(|e| e.kind == "plan.updated").count(), 0);
    assert_eq!(events.iter().filter(|e| e.kind == "tool.completed").count(), 1);
}

#[test]
fn reads_the_plan_out_of_a_captured_live_run_advancing_step_by_step() {
    let events = run(&fixture("todo-plan.jsonl"));
    let plans: Vec<_> = events.iter().filter(|e| e.kind == "plan.updated").collect();
    assert_eq!(plans.len(), 5);
    assert!(!events.iter().any(|e| e.payload["name"].as_str().is_some_and(|n| n.to_lowercase().contains("todo"))));
    let statuses: Vec<_> = plans.iter().map(|p| field(p, "status")).collect();
    assert_eq!(statuses[0], ["pending", "pending", "pending"]);
    assert_eq!(statuses.last().unwrap(), &["completed", "completed", "completed"]);
    assert!(statuses.iter().any(|row| row.iter().any(|s| s == "in_progress")));
    let first = plans[0].payload["plan"].as_array().unwrap();
    assert_eq!(first.len(), 3);
    assert!(first.iter().all(|step| step["content"].as_str().is_some_and(|c| !c.is_empty())));
}

// an OpenCode compaction (A6.9), shaped as OpenCode writes it

const NOTE: &str = "Continue if you have next steps, or stop and ask for clarification if you are unsure how to proceed.";
fn said(text: &str) -> Value {
    record("text", json!({"type":"text","text":text,"time":{"start":1,"end":2}}))
}
fn read_tool() -> Value {
    record(
        "tool_use",
        json!({"type":"tool","tool":"read","callID":"call_1","state":{"status":"completed","input":{"filePath":"a.txt"},"output":"hi"}}),
    )
}
fn note() -> Value {
    record(
        "text",
        json!({"type":"text","synthetic":true,"metadata":{"compaction_continue":true},"text":NOTE,"time":{"start":3,"end":3}}),
    )
}
fn compacted() -> Vec<Value> {
    let summary = "## Objective\nSay what a.txt holds.\n\n## Work State\nRead it; <locust-share to=\"Booty\">old news</locust-share>";
    vec![
        started(), said("Reading the file first."), read_tool(), stopped("tool-calls"),
        started(), said(summary), stopped("stop"),
        note(),
        started(), said("a.txt says hi."), stopped("stop"),
    ]
}

#[test]
fn never_gives_opencodes_note_to_the_model_as_the_teammates_words() {
    assert!(!messages(&run(&compacted())).iter().any(|t| t.contains("Continue if you have next steps")));
    // Nor any other text OpenCode marks as its own.
    let other = record("text", json!({"type":"text","synthetic":true,"text":"Summarize the task tool output above.","time":{"start":1,"end":2}}));
    let quiet = run(&[started(), other, said("Done."), stopped("stop")]);
    assert_eq!(messages(&quiet), ["Done."]);
    assert_eq!(diagnostics(&quiet, "opencode.context_compacted"), 0);
}

#[test]
fn takes_the_summary_back_so_it_is_neither_drawn_nor_the_reply_and_says_what_happened() {
    let events = run(&compacted());
    assert_eq!(messages(&events), ["Reading the file first.", "", "a.txt says hi."]);
    let notices: Vec<_> =
        events.iter().filter(|e| e.kind == "adapter.diagnostic" && e.payload["code"] == "opencode.context_compacted").collect();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].payload["level"], "info");
    assert_eq!(notices[0].payload["message"], COMPACTED);
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn does_not_count_the_summarys_stop_as_the_runs() {
    let events = run(&compacted()[..8]);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert_eq!(last.payload["runtimeTerminal"], "missing");
}

#[test]
fn takes_nothing_back_from_a_step_that_called_a_tool_or_across_a_step_that_began() {
    let tooled = run(&[started(), said("Reading the file first."), read_tool(), stopped("stop"), note(), started(), said("Done."), stopped("stop")]);
    assert_eq!(messages(&tooled), ["Reading the file first.", "Done."]);
    let late = run(&[started(), said("An answer."), stopped("stop"), started(), note(), said("Done."), stopped("stop")]);
    assert_eq!(messages(&late), ["An answer.", "Done."]);
    assert_eq!(diagnostics(&late, "opencode.context_compacted"), 1);
}

// an OpenCode compaction, as captured (opencode 1.18.27, context cut to 24,000 tokens)

#[test]
fn keeps_opencodes_summary_and_note_out_of_the_teammates_words_and_ends_on_the_real_answer() {
    let events = run(&fixture("compaction-small-context.jsonl"));
    let said: Vec<_> = messages(&events).into_iter().filter(|t| !t.is_empty()).collect();
    assert!(!said.iter().any(|t| t.contains("Continue if you have next steps")));
    assert!(!said.iter().any(|t| t.contains("## Objective")));
    assert!(said.last().unwrap().starts_with("Here are the last lines of all six files"));
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn says_once_that_the_conversation_was_summarized() {
    assert_eq!(diagnostics(&run(&fixture("compaction-small-context.jsonl")), "opencode.context_compacted"), 1);
}

// an OpenCode run the provider refused as too long, which recovered (A6.10, captured)

fn codes(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e.kind == "adapter.diagnostic")
        .map(|e| format!("{} {}", e.payload["level"].as_str().unwrap(), e.payload["code"].as_str().unwrap()))
        .collect()
}

#[test]
fn ends_completed_on_the_answer_though_the_process_exits_1() {
    let events = run_ending(&fixture("context-overflow-recovered.jsonl"), exit(1)).1;
    let said: Vec<_> = messages(&events).into_iter().filter(|t| !t.is_empty()).collect();
    assert_eq!(said, ["RECOVERED-ANSWER"]);
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert_eq!(codes(&events), ["info opencode.context_overflow", "info opencode.context_compacted"]);
}

#[test]
fn still_fails_when_the_retry_never_reached_its_own_stop() {
    let events = run_ending(&fixture("context-overflow-recovered.jsonl")[..5], exit(1)).1;
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert_eq!(last.payload["runtimeTerminal"], "missing");
    assert!(last.payload["message"].as_str().unwrap().contains("maximum context length"));
}

#[test]
fn still_fails_when_a_different_error_follows_the_recovery() {
    let mut lines = fixture("context-overflow-recovered.jsonl");
    lines.push(json!({"type":"error","sessionID":"ses_1","error":{"name":"APIError","data":{"message":"Upstream went away"}}}));
    let events = run_ending(&lines, exit(1)).1;
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert!(last.payload["message"].as_str().unwrap().contains("Upstream went away"));
}

// an OpenCode tool call's own clock (0.459)

fn timed(time: Value) -> Value {
    let mut state = json!({"status":"completed","input":{"command":"npm test"},"output":"ok"});
    if !time.is_null() {
        state["time"] = time;
    }
    record("tool_use", json!({"type":"tool","tool":"bash","callID":"call_1","state":state}))
}

#[test]
fn rides_on_the_completion_in_milliseconds() {
    let events = run(&[started(), timed(json!({"start":1790312294609u64,"end":1790312298809u64}))]);
    assert_eq!(completed_tool(&events)["durationMs"], 4200);
}

#[test]
fn is_left_out_when_the_part_says_nothing_usable() {
    for time in [Value::Null, json!({"start":5}), json!({"start":9,"end":3}), json!({"start":"a","end":"b"})] {
        let events = run(&[started(), timed(time)]);
        assert!(completed_tool(&events).get("durationMs").is_none());
    }
}
