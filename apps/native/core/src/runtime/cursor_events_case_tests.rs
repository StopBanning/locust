//! Electron's `packages/runtime-adapters/test/cursor-events.test.ts`, case by case, through the caller: each run is a
//! Cursor turn as cursor.rs runs one (`Cursor::spawn`, `start` writes the prompt and closes stdin, `next_event` to the
//! end), whose `cursor-agent` is a stand-in that reads the prompt, prints Electron's own captures
//! (`test/fixtures/cursor/`) or the case's records, and exits as the case says; a run the host cancelled carries the
//! host's cancel flag. The helpers Electron tests on their own (isCursorMessageFragment, cursorToolKind,
//! summarizeCursorInit, toolOutcome, toolPatchFrom) are cursor_events' own, which the normalizer reads each record with.
use super::*;
use crate::runtime::cursor_events::{is_message_fragment, summarize_init, tool_kind, tool_outcome, tool_patch_from};
use serde_json::json;
use std::path::PathBuf;

const STAND_IN: &str = r#"
import os, sys
sys.stdin.read()
for line in open(os.environ['LOCUST_CURSOR_LINES'], encoding='utf-8').read().split('\n'):
    if line:
        sys.stdout.write(line + '\n'); sys.stdout.flush()
sys.exit(int(os.environ['LOCUST_CURSOR_EXIT']))
"#;

const SESSION: &str = "5ea83b16-7669-44d8-aaf3-97a1b41d581b";

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn fixture(name: &str) -> Vec<String> {
    std::fs::read_to_string(format!("{}/../../../packages/runtime-adapters/test/fixtures/cursor/{name}", env!("CARGO_MANIFEST_DIR")))
        .unwrap()
        .split('\n')
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A Cursor turn whose CLI prints `lines` and exits with `exit`; cancelled by the host when `cancelled`.
pub(super) fn turn(lines: &[String], exit: i32, cancelled: bool) -> Vec<Event> {
    let temp = Temp(std::env::temp_dir().join(format!("locust-cursor-case-{}", crate::invariants::unique_nanos())));
    std::fs::create_dir_all(&temp.0).unwrap();
    std::fs::write(temp.0.join("cursor.py"), STAND_IN).unwrap();
    std::fs::write(temp.0.join("lines.jsonl"), lines.join("\n")).unwrap();
    let mut command = Command::new("python3");
    command
        .arg(temp.0.join("cursor.py"))
        .env("LOCUST_CURSOR_LINES", temp.0.join("lines.jsonl"))
        .env("LOCUST_CURSOR_EXIT", exit.to_string())
        .current_dir(&temp.0);
    let mut rt = Cursor::spawn(&mut command).unwrap().with_cancel(Arc::new(AtomicBool::new(cancelled)));
    rt.start(&Turn {
        prompt: "Read the README and summarize the project".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "mission_1".into(),
        run_id: "run_1".into(),
    })
    .unwrap();
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "the Cursor turn never ended");
        if let Some(e) = rt.next_event().unwrap() {
            let end = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            events.push(e);
            if end {
                return events;
            }
        }
    }
}

pub(super) fn run(lines: &[String]) -> Vec<Event> {
    turn(lines, 0, false)
}

pub(super) fn lines_of(records: &[Value]) -> Vec<String> {
    records.iter().map(Value::to_string).collect()
}

fn init() -> Value {
    json!({"type": "system", "subtype": "init", "session_id": "s1", "model": "Auto"})
}

/// The case's records after Cursor's init line.
fn after_init(records: &[Value]) -> Vec<Event> {
    let mut all = vec![init()];
    all.extend_from_slice(records);
    run(&lines_of(&all))
}

fn of<'a>(events: &'a [Event], kind: &str) -> Vec<&'a Event> {
    events.iter().filter(|e| e.kind == kind).collect()
}

fn tools(events: &[Event]) -> Vec<&Event> {
    events.iter().filter(|e| e.kind.starts_with("tool.")).collect()
}

/// Message text rebuilt the way the checkpoint summary does: append or replace per item, in order of first appearance.
pub(super) fn messages(events: &[Event]) -> Vec<String> {
    let mut built: Vec<(String, String)> = vec![];
    for e in events.iter().filter(|e| e.kind == "message.delta") {
        let id = e.payload["itemId"].as_str().unwrap().to_owned();
        let text = e.payload["text"].as_str().unwrap();
        let replace = e.payload["operation"] == "replace";
        match built.iter_mut().find(|(i, _)| *i == id) {
            Some((_, t)) if replace => *t = text.into(),
            Some((_, t)) => t.push_str(text),
            None => built.push((id, text.into())),
        }
    }
    built.into_iter().map(|(_, t)| t).collect()
}

fn finals(events: &[Event]) -> Vec<&Event> {
    events.iter().filter(|e| e.kind == "message.delta" && e.payload["final"] == true).collect()
}

fn said(events: &[Event]) -> Vec<String> {
    of(events, "adapter.diagnostic").iter().map(|e| e.payload["message"].as_str().unwrap().to_owned()).collect()
}

fn assistant(text: &str, stamp: Option<u64>, marker: Option<&str>) -> Value {
    let mut record = json!({"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": text}]}});
    if let Some(stamp) = stamp {
        record["timestamp_ms"] = json!(stamp);
    }
    if let Some(marker) = marker {
        record["model_call_id"] = json!(marker);
    }
    record
}

// --- Cursor's final result carries per-run tokens

// case: keeps input, output, cache read and cache write counts from the result line
#[test]
fn keeps_every_token_count_from_the_result_line() {
    let events = run(&lines_of(&[json!({"type": "result", "subtype": "success", "is_error": false,
        "usage": {"inputTokens": 120, "outputTokens": 45, "cacheReadTokens": 3000, "cacheWriteTokens": 240}})]));
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.completed");
    for (key, n) in [("inputTokens", 120), ("outputTokens", 45), ("cacheReadTokens", 3000), ("cacheWriteTokens", 240)] {
        assert_eq!(last.payload["usage"][key], n, "{key}");
    }
}

// case: does not invent token counts when result usage is omitted
#[test]
fn invents_no_token_counts_without_result_usage() {
    let events = run(&lines_of(&[json!({"type": "result", "subtype": "success", "is_error": false})]));
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.completed");
    assert!(last.payload.get("usage").is_none(), "{}", last.payload);
}

// case: preserves explicit zero counts and drops nonnumeric values
#[test]
fn keeps_zero_counts_and_drops_a_count_that_is_not_a_number() {
    let events = run(&lines_of(&[json!({"type": "result", "subtype": "success", "usage": {
        "inputTokens": 0, "outputTokens": 0, "cacheReadTokens": 0, "cacheWriteTokens": 0, "extra": "not a token count"}})]));
    let usage = &events.last().unwrap().payload["usage"];
    for key in ["inputTokens", "outputTokens", "cacheReadTokens", "cacheWriteTokens"] {
        assert_eq!(usage[key], 0, "{key}");
    }
    assert!(usage.get("extra").is_none(), "{usage}");
}

// --- a read-only Cursor run with partial output, as captured

// case: starts the run with the session id and only the allow-listed init fields
#[test]
fn starts_with_the_session_id_and_only_five_init_fields() {
    let events = run(&fixture("read-only-partial.jsonl"));
    assert_eq!(events[0].kind, "run.started");
    assert_eq!(events.last().unwrap().thread.as_deref(), Some(SESSION));
    let evidence = &events[0].payload["evidence"];
    assert_eq!(evidence["redacted"], true);
    let mut keys: Vec<&str> = evidence["raw"].as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["apiKeySource", "cwd", "model", "permissionMode", "session_id"]);
}

// case: rebuilds each message from its fragments and lets the complete message replace, not double, them
#[test]
fn rebuilds_messages_from_fragments_and_replaces_rather_than_doubles() {
    assert_eq!(
        messages(&run(&fixture("read-only-partial.jsonl"))),
        [
            "I'll read the README and summarize the project in one sentence.",
            "Pebble is a tiny CLI timer whose single command, `pebble start`, counts down 25 minutes and rings a bell.",
        ]
    );
}

// case: keeps the two messages apart: the answer does not overwrite what was said before the tool ran
#[test]
fn keeps_the_preamble_and_the_answer_as_two_messages() {
    let events = run(&fixture("read-only-partial.jsonl"));
    let ids: Vec<&Value> = finals(&events).iter().map(|e| &e.payload["itemId"]).collect();
    assert_eq!(ids, [&json!("msg_0"), &json!("msg_1")]);
}

// case: records the read as one tool that started and completed under the clean call id
#[test]
fn records_the_read_as_one_tool_under_a_clean_id() {
    let events = run(&fixture("read-only-partial.jsonl"));
    let rows = tools(&events);
    assert_eq!(rows.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(), ["tool.started", "tool.completed"]);
    let ids: std::collections::HashSet<&str> = rows.iter().map(|e| e.payload["itemId"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 1);
    assert!(!ids.iter().next().unwrap().contains('\n'));
    assert_eq!(rows[0].payload["toolKind"], "read");
    assert!(rows[0].payload["command"].as_str().unwrap().contains("README.md"));
}

// case: shows reasoning as a step and KEEPS its text
#[test]
fn shows_reasoning_as_a_step_and_keeps_what_it_said() {
    let events = run(&fixture("read-only-partial.jsonl"));
    let steps: Vec<&Event> = events.iter().filter(|e| e.kind == "step.started" || e.kind == "step.completed").collect();
    assert!(steps.iter().all(|e| e.payload["stepKind"] == "reasoning"));
    assert_eq!(steps.len(), 4);
    let ledger = serde_json::to_string(&events.iter().map(|e| &e.payload).collect::<Vec<_>>()).unwrap();
    assert!(ledger.contains("Reading README.md to"));
}

// case: carries the reasoning on the step that closes, not on every fragment
#[test]
fn carries_the_reasoning_on_the_closing_step_only() {
    let events = run(&fixture("read-only-partial.jsonl"));
    let completed = of(&events, "step.completed");
    assert!(!completed.is_empty());
    assert!(completed.iter().any(|e| e.payload["message"].as_str().is_some_and(|m| !m.is_empty())));
    assert!(of(&events, "step.started").iter().all(|e| e.payload.get("message").is_none()));
}

// case: completes the run on exit 0 with a result record
#[test]
fn completes_on_exit_zero_after_a_result_record() {
    assert_eq!(run(&fixture("read-only-partial.jsonl")).last().unwrap().kind, "run.completed");
}

// --- a write-mode Cursor run whose shell commands were rejected, as captured

// case: reports a rejected shell command as a tool that failed, never as one that completed
#[test]
fn reports_a_rejected_shell_command_as_failed() {
    let events = run(&fixture("write-mode-shell-rejected.jsonl"));
    let shells: Vec<&Event> = tools(&events).into_iter().filter(|e| e.payload["toolKind"] == "shell").collect();
    let failed: Vec<&&Event> = shells.iter().filter(|e| e.kind == "tool.failed").collect();
    let completed: Vec<&&Event> = shells.iter().filter(|e| e.kind == "tool.completed").collect();
    assert_eq!(failed.len(), 4);
    assert!(failed.iter().all(|e| e.payload["status"] == "rejected"));
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].payload["command"], "ls -la");
}

// case: names the commands it refused to run, which is the fact a rejection exists to record
#[test]
fn names_each_command_it_refused() {
    let events = run(&fixture("write-mode-shell-rejected.jsonl"));
    let refused: Vec<&Value> = of(&events, "tool.failed").iter().map(|e| &e.payload["command"]).collect();
    assert_eq!(refused, [&json!("dir"), &json!("dir"), &json!("cmd.exe /c dir"), &json!("dir")]);
}

// case: reports a command that RAN and FAILED as failed, with its exit code
#[test]
fn a_command_that_ran_and_failed_is_failed_with_its_exit_code() {
    assert_eq!(tool_outcome(&json!({"success": {"command": "npm test", "exitCode": 1}})), (true, Some("exit".into()), Some(1)));
    assert_eq!(tool_outcome(&json!({"success": {"command": "ls", "exitCode": 0}})), (false, None, Some(0)));
}

// case: treats an outcome it has never seen as a failure, not as a success
#[test]
fn an_outcome_never_seen_is_a_failure() {
    for shape in [json!({"aborted": {}}), json!({"timedOut": {}}), json!({"denied": {}}), json!({})] {
        assert!(tool_outcome(&shape).0, "{shape}");
    }
}

// case: records what the run cost, the way the other adapters do
#[test]
fn records_what_the_captured_run_cost() {
    let events = run(&fixture("write-mode-shell-rejected.jsonl"));
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.completed");
    assert_eq!(last.payload["usage"]["inputTokens"], 15012);
    assert_eq!(last.payload["usage"]["outputTokens"], 1125);
}

// case: reports the edit and the glob as completed tools with their targets
#[test]
fn reports_the_edit_and_the_glob_with_their_targets() {
    let events = run(&fixture("write-mode-shell-rejected.jsonl"));
    let completed = of(&events, "tool.completed");
    let kinds: Vec<&str> = completed.iter().map(|e| e.payload["toolKind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"edit") && kinds.contains(&"glob"), "{kinds:?}");
    let edit = completed.iter().find(|e| e.payload["toolKind"] == "edit").unwrap();
    assert!(edit.payload["command"].as_str().unwrap().contains("NOTES.md"));
    let glob = completed.iter().find(|e| e.payload["toolKind"] == "glob").unwrap();
    assert_eq!(glob.payload["command"], "* in C:\\work\\scratch");
}

// case: carries the resolved model name out of init
#[test]
fn carries_the_resolved_model_out_of_init() {
    let events = run(&fixture("write-mode-shell-rejected.jsonl"));
    assert_eq!(events[0].payload["evidence"]["raw"]["model"], "Composer 2.5");
}

// --- a resumed Cursor turn, as captured

// case: keeps the earlier session id and delivers the whole answer as one final message
#[test]
fn a_resumed_turn_keeps_its_session_and_answers_in_one_final_message() {
    let events = run(&fixture("resumed-turn.jsonl"));
    assert_eq!(events.last().unwrap().thread.as_deref(), Some(SESSION));
    assert_eq!(messages(&events), ["Pebble"]);
    let deltas = of(&events, "message.delta");
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].payload["final"], true);
}

// --- fragment or complete message

// case: tells them apart the way the stream does
#[test]
fn tells_a_fragment_from_a_complete_message() {
    assert!(is_message_fragment(&json!({"timestamp_ms": 1, "type": "assistant"})));
    assert!(!is_message_fragment(&json!({"timestamp_ms": 1, "model_call_id": "m", "type": "assistant"})));
    assert!(!is_message_fragment(&json!({"type": "assistant"})));
}

// case: names tool kinds from their wrapper keys
#[test]
fn names_a_tool_kind_from_its_wrapper_key() {
    assert_eq!(tool_kind("readToolCall"), "read");
    assert_eq!(tool_kind("shellToolCall"), "shell");
    assert_eq!(tool_kind("somethingElse"), "somethingElse");
}

// case: summarizes init to five fields whatever else is there
#[test]
fn summarizes_init_to_its_five_fields() {
    assert_eq!(
        summarize_init(&json!({"session_id": "s", "model": "Auto", "cwd": "C:\\w", "permissionMode": "default", "apiKeySource": "login", "extra": "x"})),
        json!({"session_id": "s", "model": "Auto", "cwd": "C:\\w", "permissionMode": "default", "apiKeySource": "login"})
    );
}

// --- how a Cursor run ends

// case: is a failure when the CLI exits clean without ever saying it finished
#[test]
fn a_clean_exit_with_no_result_is_a_failure() {
    let events = after_init(&[]);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert_eq!(last.payload["kind"], "process-failed");
    assert_eq!(last.payload["runtimeTerminal"], "missing");
}

// case: is a failure, in the CLI's words, when the result says so
#[test]
fn a_result_that_says_error_fails_in_the_clis_words() {
    let events = after_init(&[json!({"type": "result", "subtype": "error", "is_error": true, "result": "Model unavailable"})]);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert_eq!(last.payload["kind"], "unknown");
    assert_eq!(last.payload["message"], "Model unavailable");
}

// case: is cancelled when the host cancelled it
#[test]
fn a_run_the_host_cancelled_is_cancelled() {
    let events = turn(&lines_of(&[init()]), 0, true);
    assert_eq!(events.last().unwrap().kind, "run.cancelled");
}

// case: reports a record it cannot parse as a diagnostic and keeps going
#[test]
fn a_record_it_cannot_parse_is_said_and_the_run_goes_on() {
    let mut lines = vec!["{not json".to_owned()];
    lines.extend(lines_of(&[init()]));
    let events = run(&lines);
    assert_eq!(events[0].kind, "adapter.diagnostic");
    assert_eq!(events[0].payload["code"], "cursor.malformed_record");
    assert_eq!(events[1].kind, "run.started", "the run went on after it");
}

// --- a message longer than the ledger's bound

// case: never takes back text the fragments already delivered
#[test]
fn never_takes_back_text_already_delivered() {
    let long = "x".repeat(20_000);
    let events = after_init(&[
        assistant(&long[..10_000], Some(1), None),
        assistant(&long[10_000..], Some(2), None),
        assistant(&long, None, Some("m1")),
    ]);
    assert_eq!(messages(&events)[0].len(), 20_000);
}

// case: still replaces when the complete message is no shorter
#[test]
fn replaces_when_the_complete_message_is_no_shorter() {
    let events = after_init(&[assistant("Peb", Some(2), None), assistant("Pebble is a timer.", None, Some("m1"))]);
    assert_eq!(messages(&events), ["Pebble is a timer."]);
}

// --- a turn that opens

// case: also closes, so nothing is left looking unfinished
#[test]
fn a_turn_step_that_opens_also_closes() {
    let events = run(&lines_of(&[json!({"type": "system", "subtype": "compact"})]));
    let drawn: Vec<&str> = events.iter().filter(|e| !e.kind.starts_with("run.")).map(|e| e.kind.as_str()).collect();
    assert_eq!(drawn, ["step.started", "step.completed"]);
}

// --- records Cursor sends that this adapter does not draw

fn query(term: &str) -> Value {
    json!({"type": "interaction_query", "subtype": "request", "query_type": "webSearchRequestQuery",
        "query": {"id": 0, "webSearchRequestQuery": {"args": {"searchTerm": term, "toolCallId": "call-1"}}},
        "session_id": "s1", "timestamp_ms": 1})
}

// case: says Cursor's reconnect and resume in words, as the records in a real run had them (0.575)
#[test]
fn says_the_reconnect_and_the_resume_in_words() {
    let events = after_init(&[
        json!({"type": "connection", "subtype": "reconnecting", "session_id": "s1", "timestamp_ms": 1, "attempt": 1, "endpoint_url": "https://agentn.global.api5.cursor.sh"}),
        json!({"type": "retry", "subtype": "starting", "session_id": "s1", "timestamp_ms": 2, "attempt": 1, "is_resume": true}),
    ]);
    let words = said(&events);
    assert_eq!(
        words,
        [
            "Cursor lost its connection and is reconnecting (attempt 1).",
            "Cursor is picking the turn back up after the reconnect (attempt 1).",
        ]
    );
    let joined = words.join(" ");
    assert!(!joined.contains("Unhandled") && !joined.contains("cursor.sh"));
}

// case: says nothing at all about interaction_query, because the tool row already says it
#[test]
fn says_nothing_about_interaction_query() {
    let mut response = query("GOOGL");
    response["subtype"] = json!("response");
    let events = after_init(&[query("TSMC capex"), query("NVDA targets"), response]);
    assert!(of(&events, "adapter.diagnostic").is_empty());
}

// case: still draws the tool call itself, which is where a web search belongs
#[test]
fn draws_the_web_search_tool_call_once() {
    let events = after_init(&[
        json!({"type": "tool_call", "subtype": "started", "call_id": "call-1", "tool_call": {"webSearchToolCall": {"args": {"searchTerm": "TSMC capex"}}, "toolCallId": "call-1"}}),
        query("TSMC capex"),
    ]);
    assert_eq!(of(&events, "tool.started").len(), 1);
}

// case: says an unhandled type once a run, however many arrive
#[test]
fn says_an_unhandled_type_once_however_many_arrive() {
    let odd = json!({"type": "something_new", "session_id": "s1", "timestamp_ms": 1});
    let events = after_init(&[odd.clone(), odd.clone(), odd.clone(), odd]);
    let unhandled: Vec<String> = said(&events).into_iter().filter(|m| m.contains("Unhandled")).collect();
    assert_eq!(unhandled.len(), 1);
    assert!(unhandled[0].contains("something_new"));
}

// case: still says each different type it meets
#[test]
fn says_each_different_unhandled_type() {
    let events = after_init(&[
        json!({"type": "alpha", "session_id": "s1", "timestamp_ms": 1}),
        json!({"type": "beta", "session_id": "s1", "timestamp_ms": 1}),
        json!({"type": "alpha", "session_id": "s1", "timestamp_ms": 1}),
    ]);
    assert_eq!(of(&events, "adapter.diagnostic").len(), 2);
}

// --- a Cursor version that marks nothing

const WORDS: [&str; 7] = ["Looking", " up", " NVIDIA", " forward", " earnings", " now", "."];

/// Each word as a fragment, then the complete message carrying no `model_call_id`, then `and_then` as fragments.
fn unmarked(words: &[&str], and_then: &[&str]) -> Vec<Event> {
    let mut records = vec![];
    let mut stamp = 1;
    let whole = words.concat();
    for word in words.iter().copied().chain(std::iter::once(whole.as_str())).chain(and_then.iter().copied()) {
        stamp += 1;
        records.push(assistant(word, Some(stamp), None));
    }
    after_init(&records)
}

// case: does not let the reply say itself twice
#[test]
fn an_unmarked_reply_does_not_say_itself_twice() {
    let whole = WORDS.concat();
    let built = messages(&unmarked(&WORDS, &[]));
    assert_eq!(built[0], whole);
    assert_ne!(built[0], format!("{whole}{whole}"));
}

// case: closes the message, which is what the rest of the host reads
#[test]
fn an_unmarked_reply_still_closes_its_message() {
    let events = unmarked(&WORDS, &[]);
    assert_eq!(of(&events, "message.delta").last().unwrap().payload["final"], true);
}

// case: starts a new message after the complete one, rather than running two together
#[test]
fn starts_a_new_message_after_the_unmarked_complete_one() {
    assert_eq!(messages(&unmarked(&WORDS, &["Partly", ".", " The", " stock"])), [WORDS.concat(), "Partly. The stock".into()]);
}

// case: still trusts the marker where a version sends one
#[test]
fn still_trusts_the_complete_message_marker() {
    let events = after_init(&[assistant("part", Some(1), None), assistant("part and the rest", None, Some("m1"))]);
    assert_eq!(messages(&events)[0], "part and the rest");
}

// --- the change itself

// case: carries the edit's unified diff as its own field, with counts derived from it
#[test]
fn carries_the_edits_diff_with_its_counts() {
    let events = run(&fixture("write-mode-shell-rejected.jsonl"));
    let edit = of(&events, "tool.completed").into_iter().find(|e| e.payload["toolKind"] == "edit").unwrap();
    let patch = &edit.payload["patch"];
    assert!(patch["text"].as_str().unwrap().contains("+hello"));
    assert!(patch["text"].as_str().unwrap().contains("@@ -1,0 +1 @@"));
    assert_eq!((patch["added"].clone(), patch["removed"].clone(), patch["truncated"].clone()), (json!(1), json!(0), json!(false)));
}

// case: counts the whole change before bounding the text, and says when it bounded
#[test]
fn counts_the_whole_change_before_bounding_it() {
    let lines: Vec<String> = (0..3_000).map(|i| format!("+line {i} {}", "x".repeat(40))).collect();
    let unified = format!("--- a/big.ts\n+++ b/big.ts\n@@ -1,0 +1,3000 @@\n{}\n", lines.join("\n"));
    let patch = tool_patch_from(&unified).unwrap();
    assert_eq!(patch["added"], 3_000);
    assert_eq!(patch["removed"], 0);
    assert_eq!(patch["truncated"], true);
    assert!(patch["text"].as_str().unwrap().len() < unified.len());
}

// case: does not count the file headers as changed lines
#[test]
fn file_headers_are_not_changed_lines() {
    let patch = tool_patch_from("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n").unwrap();
    assert_eq!((patch["added"].clone(), patch["removed"].clone()), (json!(1), json!(1)));
}

// case: records no patch for an edit that reported none
#[test]
fn no_patch_for_an_edit_that_reported_none() {
    assert_eq!(tool_patch_from(""), None);
}

// --- a long answer whose fragments outrun the complete message

fn outrun() -> Vec<Event> {
    let fragment = "x".repeat(9_000);
    run(&lines_of(&[
        assistant(&fragment, Some(1), None),
        assistant(&fragment, Some(2), None),
        assistant(&format!("{fragment}{fragment}"), None, Some("c1")),
    ]))
}

// case: keeps the fragments rather than taking text back out of the ledger
#[test]
fn keeps_fragments_that_outrun_the_complete_message() {
    let built = messages(&outrun());
    assert!(!built.is_empty());
    assert!(built.iter().map(String::len).max().unwrap() >= 18_000);
}

// case: MARKS THE MESSAGE FINAL, so the host still reads what the reply said
#[test]
fn marks_an_outrun_message_final() {
    assert!(!finals(&outrun()).is_empty());
}

// case: does not rewrite the text when it closes the item
#[test]
fn closing_an_outrun_message_adds_no_text() {
    for e in finals(&outrun()) {
        if e.payload["operation"] == "append" {
            assert_eq!(e.payload["text"], "");
        }
    }
}

// --- Cursor's plan

// case: becomes a plan that never shrinks, not four tool rows
#[test]
fn the_todo_list_is_a_plan_that_never_shrinks() {
    let events = run(&fixture("todo-plan.jsonl"));
    let plans = of(&events, "plan.updated");
    assert!(plans.len() >= 3, "{}", plans.len());
    assert!(plans.iter().all(|e| e.payload["plan"].as_array().unwrap().len() == 3));
    assert!(!events.iter().any(|e| e.payload["name"].as_str().is_some_and(|n| n.to_lowercase().contains("todo"))));
}

// case: advances the steps and finishes with all of them done
#[test]
fn the_plan_advances_and_finishes_all_done() {
    let events = run(&fixture("todo-plan.jsonl"));
    let plans = of(&events, "plan.updated");
    let statuses: Vec<Vec<String>> = plans
        .iter()
        .map(|e| e.payload["plan"].as_array().unwrap().iter().map(|s| s["status"].as_str().unwrap_or("").to_uppercase()).collect())
        .collect();
    assert!(statuses.last().unwrap().iter().all(|s| s.contains("COMPLETED")));
    assert!(statuses.iter().any(|row| row.iter().any(|s| s.contains("IN_PROGRESS"))));
    let words: Vec<&str> = plans.last().unwrap().payload["plan"].as_array().unwrap().iter().map(|s| s["content"].as_str().unwrap()).collect();
    assert_eq!(words, ["Create a.txt containing alpha", "Create b.txt containing beta", "Create c.txt listing the two file names"]);
}

// case: invents no plan from a todo call that carries nothing
#[test]
fn an_empty_todo_call_makes_no_plan() {
    let events = run(&lines_of(&[
        json!({"type": "system", "subtype": "init", "session_id": "s1"}),
        json!({"type": "tool_call", "subtype": "completed", "call_id": "c1",
            "tool_call": {"updateTodosToolCall": {"args": {"todos": []}, "result": {"success": {}}, "toolCallId": "c1"}}}),
    ]));
    assert!(of(&events, "plan.updated").is_empty());
}

// --- an edit whose result was too large to keep

// case: closes all four, on the id the start opened, and says the result was too large
#[test]
fn closes_four_oversized_edits_on_their_own_ids() {
    let events = run(&fixture("edit-result-too-large.jsonl"));
    let rows: Vec<&Event> = events.iter().filter(|e| matches!(e.kind.as_str(), "tool.started" | "tool.completed" | "tool.failed")).collect();
    assert_eq!(rows.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(), ["tool.started", "tool.completed"].repeat(4));
    let ids: Vec<&str> = rows.iter().map(|e| e.payload["itemId"].as_str().unwrap()).collect();
    assert_eq!(ids.iter().collect::<std::collections::HashSet<_>>().len(), 4);
    assert!(ids.iter().all(|id| !id.contains('\n') && id.contains('|')));
    for pair in ids.chunks(2) {
        assert_eq!(pair[0], pair[1]);
    }
    let closed = of(&events, "tool.completed");
    assert!(closed.iter().all(|e| e.payload["status"] == "result too large to keep"));
    let targets: Vec<&Value> = closed.iter().map(|e| &e.payload["command"]).collect();
    assert_eq!(targets, [&json!("src/app.ts"), &json!("src/view.ts"), &json!("src/main.ts"), &json!("src/notes.ts")]);
    assert!(of(&events, "run.failed").is_empty());
}
