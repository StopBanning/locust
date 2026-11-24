//! Electron's `packages/runtime-adapters/test/antigravity-events.test.ts`, case by case, on the transcript reader the
//! Antigravity mission driver feeds (antigravity_mission.rs: `TranscriptNormalizer::default()` with the conversation id
//! as `thread`, `accept_line` for each line it tails, `finish_with_stderr` at the end), and on the project-file parser
//! antigravity_host.rs reads (`parse_projects`, `normalize_workspace`, `project_id_for`). The captured transcripts are
//! Electron's own (`test/fixtures/antigravity/`). Electron's `finish(completion)` is Rust's `finish(cancelled, exit)`;
//! its `finalized` is `ended`; Rust events carry no `sourceAdapter` (a run's events are its runtime's by the run).
use super::*;
use crate::runtime::antigravity_projects::{normalize_workspace, parse_projects};

use serde_json::{Value, json};

const CONVERSATION: &str = "03a2fcb4-8fd9-468e-a683-6bf3a5acd077";

fn normalizer() -> TranscriptNormalizer {
    let mut n = TranscriptNormalizer::default();
    n.thread = Some(CONVERSATION.into());
    n
}

fn fixture_path(name: &str) -> String {
    format!("{}/../../../packages/runtime-adapters/test/fixtures/antigravity/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn fixture(name: &str) -> Vec<String> {
    std::fs::read_to_string(fixture_path(name)).unwrap().lines().filter(|l| !l.is_empty()).map(str::to_owned).collect()
}

fn feed(n: &mut TranscriptNormalizer, lines: &[String]) -> Vec<Event> {
    lines.iter().flat_map(|l| n.accept_line(l)).collect()
}

fn run(lines: &[String], cancelled: bool, exit: Option<i32>) -> (TranscriptNormalizer, Vec<Event>) {
    let mut n = normalizer();
    let mut events = feed(&mut n, lines);
    events.extend(n.finish(cancelled, exit));
    (n, events)
}

fn kinds(events: &[Event]) -> Vec<&str> {
    events.iter().map(|e| e.kind.as_str()).collect()
}

fn ledger(events: &[Event]) -> String {
    serde_json::to_string(&events.iter().map(|e| (&e.kind, &e.payload, &e.thread)).collect::<Vec<_>>()).unwrap()
}

fn plain() -> (TranscriptNormalizer, Vec<Event>) {
    run(&fixture("transcript-plain-reply.jsonl"), false, None)
}

fn followup() -> (TranscriptNormalizer, Vec<Event>) {
    run(&fixture("transcript-write-then-followup.jsonl"), false, None)
}

// an Antigravity conversation that only answered, as captured

#[test]
fn says_nothing_about_the_prompt_it_was_handed_or_the_checkpoint_it_was_handed_back() {
    // case: says nothing about the prompt it was handed or the checkpoint it was handed back
    assert_eq!(kinds(&plain().1), ["message.delta", "run.completed"]);
}

#[test]
fn never_lets_antigravitys_checkpoint_summary_into_the_ledger() {
    // case: never lets Antigravity's checkpoint summary into the ledger
    let ledger = ledger(&plain().1);
    for never in ["CHECKPOINT 0", "Conversation Logs", "Single Word Response Test", "transcript.jsonl"] {
        assert!(!ledger.contains(never), "{never}");
    }
}

#[test]
fn delivers_the_planners_answer_whole_as_its_own_final_message() {
    // case: delivers the planner's answer whole, as its own final message
    let p = &plain().1[0].payload;
    assert_eq!((&p["itemId"], &p["operation"], &p["text"], &p["final"]), (&json!("msg_2"), &json!("replace"), &json!("OK"), &json!(true)));
}

#[test]
fn reports_a_final_planner_answer_with_no_tool_calls_as_the_turn_being_over() {
    // case: reports a final planner answer with no tool calls as the turn being over
    assert!(plain().0.latest_final);
}

#[test]
fn carries_the_conversation_id_the_host_passed_in_which_the_transcript_never_prints() {
    // case: carries the conversation id the host passed in, which the transcript never prints
    let (n, events) = plain();
    assert_eq!(n.thread.as_deref(), Some(CONVERSATION));
    assert!(events.iter().all(|e| e.thread.as_deref() == Some(CONVERSATION)));
}

// an Antigravity conversation that wrote a file and was then continued, as captured

#[test]
fn turns_seven_transcript_lines_into_the_four_things_that_actually_happened() {
    // case: turns seven transcript lines into the four things that actually happened
    assert_eq!(
        kinds(&followup().1),
        ["step.started", "step.completed", "tool.started", "tool.completed", "message.delta", "message.delta", "run.completed"]
    );
}

#[test]
fn opens_and_closes_the_reasoning_step_in_one_go_so_nothing_is_left_unfinished() {
    // case: opens and closes the reasoning step in one go, so nothing is left unfinished
    let events = followup().1;
    assert_eq!(events[0].payload["stepKind"], "reasoning");
    assert_eq!(events[1].payload["stepKind"], "reasoning");
}

#[test]
fn never_stores_the_models_reasoning_and_does_not_even_record_that_a_thinking_field_was_there() {
    // case: never stores the model's reasoning, and does not even record that a thinking field was there
    let events = followup().1;
    let evidence = &events[0].payload["evidence"];
    assert!(evidence["raw"].get("thinking").is_none(), "{evidence}");
    assert_eq!(evidence["redacted"], true);
    let ledger = ledger(&events);
    assert!(!ledger.contains("File Creation Verification"));
    assert!(!ledger.contains("easily verifiable operation"));
}

#[test]
fn names_the_write_call_from_its_position_in_the_file_because_the_call_carries_no_id() {
    // case: names the write call from its position in the file, because the call carries no id
    let p = &followup().1[2].payload;
    assert_eq!(p["itemId"], "tool_2_0");
    assert_eq!(p["name"], "write_to_file");
    assert_eq!(p["toolKind"], "write_to_file");
    assert_eq!(p["command"], "c:/work/pebble/hello.txt");
}

#[test]
fn records_an_overwrite_as_a_path_and_no_diff_because_there_is_no_before_text_to_diff_against() {
    // case: records an overwrite as a path and no diff, because there is no before-text to diff against
    assert!(followup().1[2].payload.get("patch").is_none());
}

#[test]
fn attaches_the_tools_result_to_the_call_it_came_from_which_the_result_line_does_not_name() {
    // case: attaches the tool's result to the call it came from, which the result line does not name
    let p = &followup().1[3].payload;
    assert_eq!(p["itemId"], "tool_2_0");
    assert!(p["output"].as_str().unwrap().contains("Created file file:///c:/work/pebble/hello.txt"), "{p}");
}

#[test]
fn recognises_the_follow_up_send_message_line_and_says_nothing_about_it() {
    // case: recognises the follow-up `send-message` line and says nothing about it
    let events = followup().1;
    assert!(!events.iter().any(|e| e.kind == "adapter.diagnostic"));
    assert!(!ledger(&events).contains("MESSAGE_PRIORITY_HIGH"));
    let deltas: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "message.delta")
        .map(|e| {
            assert!(e.payload["evidence"].is_object());
            (e.payload["itemId"].clone(), e.payload["operation"].clone(), e.payload["text"].clone(), e.payload["final"].clone())
        })
        .collect();
    assert_eq!(
        deltas,
        [(json!("msg_4"), json!("replace"), json!("DONE"), json!(true)), (json!("msg_6"), json!("replace"), json!("DONE"), json!(true))]
    );
}

#[test]
fn reports_the_run_complete_when_the_last_line_was_a_planner_answer_with_no_tool_calls() {
    // case: reports the run complete when the last line was a planner answer with no tool calls
    let (n, events) = followup();
    assert!(n.latest_final);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.completed");
    assert_eq!(last.payload["runtimeThreadId"], CONVERSATION);
}

// re-reading a transcript the host has already read

#[test]
fn emits_each_step_exactly_once_however_many_times_the_file_is_polled() {
    // case: emits each step exactly once, however many times the file is polled
    let lines = fixture("transcript-write-then-followup.jsonl");
    let mut n = normalizer();
    assert_eq!(feed(&mut n, &lines).len(), 6);
    assert!(feed(&mut n, &lines).is_empty());
    assert!(feed(&mut n, &lines).is_empty());
    assert!(n.latest_final);
}

// a planner answer that carries the model's reasoning beside it

#[test]
fn is_the_end_of_the_turn_and_the_reasoning_is_still_its_own_step() {
    // case: is the end of the turn, and the reasoning is still its own step
    let mut n = normalizer();
    let events = n.accept_line(
        &json!({"step_index":7,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-09-06T03:02:28Z","thinking":"**Defining the answer**","content":"Locust is a desktop app."}).to_string(),
    );
    assert_eq!(kinds(&events), ["step.started", "step.completed", "message.delta"]);
    assert_eq!(events[2].payload["text"], "Locust is a desktop app.");
    assert!(n.latest_final);
}

#[test]
fn does_not_end_the_turn_on_reasoning_with_no_answer() {
    // case: does not end the turn on reasoning with no answer
    let mut n = normalizer();
    let events = n.accept_line(
        &json!({"step_index":7,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-09-06T03:02:28Z","thinking":"**Still working**"}).to_string(),
    );
    assert_eq!(kinds(&events), ["step.started", "step.completed"]);
    assert!(!n.latest_final);
}

// a step the transcript has not finished writing

fn running() -> String {
    json!({"step_index":2,"source":"MODEL","type":"PLANNER_RESPONSE","status":"RUNNING","created_at":"2026-09-03T08:39:47Z","content":"partial"}).to_string()
}

#[test]
fn holds_it_nothing_is_emitted_and_nothing_claims_the_turn_is_over() {
    // case: holds it: nothing is emitted, and nothing claims the turn is over
    let mut n = normalizer();
    assert!(n.accept_line(&running()).is_empty());
    assert!(!n.latest_final);
}

#[test]
fn normalizes_it_once_it_settles_rather_than_treating_it_as_already_seen() {
    // case: normalizes it once it settles, rather than treating it as already seen
    let mut n = normalizer();
    n.accept_line(&running());
    let events = n.accept_line(
        &json!({"step_index":2,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-09-03T08:39:50Z","content":"DONE"}).to_string(),
    );
    assert_eq!(kinds(&events), ["message.delta"]);
    assert_eq!(events[0].payload["text"], "DONE");
    assert!(n.latest_final);
}

// a write of a file that is not being overwritten

fn created() -> String {
    json!({
        "step_index": 2, "source": "MODEL", "type": "PLANNER_RESPONSE", "status": "DONE", "created_at": "2026-09-03T08:39:47Z",
        "thinking": "SECRET REASONING",
        "tool_calls": [{"name": "write_to_file", "args": {
            "CodeContent": json!("alpha\nbeta\n").to_string(),
            "TargetFile": json!("c:/work/pebble/notes.txt").to_string(),
            "Description": json!("Create notes.txt").to_string(),
        }}],
    })
    .to_string()
}

#[test]
fn reconstructs_the_added_file_from_the_content_the_call_carried() {
    // case: reconstructs the added file from the content the call carried
    let events = normalizer().accept_line(&created());
    let patch = &events[2].payload["patch"];
    assert_eq!(patch["text"], "--- /dev/null\n+++ b/c:/work/pebble/notes.txt\n@@ -0,0 +1,2 @@\n+alpha\n+beta\n");
    assert_eq!(patch["added"], 2);
    assert_eq!(patch["removed"], 0);
}

#[test]
fn still_keeps_the_reasoning_out_of_the_patchs_own_event() {
    // case: still keeps the reasoning out of the patch's own event
    let events = normalizer().accept_line(&created());
    assert!(!ledger(&events).contains("SECRET REASONING"));
}

// a tool result with nothing open to attach it to

#[test]
fn says_so_rather_than_inventing_a_tool_call_to_hang_it_on() {
    // case: says so, rather than inventing a tool call to hang it on
    let mut n = normalizer();
    let orphan = json!({"step_index":0,"source":"MODEL","type":"GENERIC","status":"DONE","created_at":"2026-09-03T08:39:50Z","content":"Created file file:///c:/work/pebble/hello.txt with requested content."});
    assert!(n.accept_line(&orphan.to_string()).is_empty());
    let events = n.finish(false, None);
    assert_eq!(kinds(&events), ["adapter.diagnostic", "run.failed"]);
    assert_eq!(events[0].payload["code"], "antigravity.unattached_result");
    assert_eq!(events[0].payload["level"], "info");
}

// one planner step that opens two calls

fn at(fields: Value) -> String {
    let mut line = json!({"source":"MODEL","status":"DONE","created_at":"2026-09-25T04:33:59Z"});
    line.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    line.to_string()
}

/// (tool.completed itemId and output pairs, diagnostics, the last event's kind) for these lines, in this order.
fn answered(order: &[&str]) -> (Vec<(Value, Value)>, usize, String) {
    let user = json!({"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","created_at":"2026-09-25T04:33:55Z","content":"read both"}).to_string();
    let planner = at(json!({"step_index":1,"type":"PLANNER_RESPONSE","thinking":"both at once","tool_calls":[
        {"name":"view_file","args":{"AbsolutePath": json!("c:/work/agy-alpha.txt").to_string()}},
        {"name":"view_file","args":{"AbsolutePath": json!("c:/work/agy-bravo.txt").to_string()}},
    ]}));
    let alpha = at(json!({"step_index":2,"type":"GENERIC","content":"ALPHA-MARKER-5170"}));
    let bravo = at(json!({"step_index":3,"type":"GENERIC","content":"BRAVO-MARKER-8823"}));
    let answer = at(json!({"step_index":4,"type":"PLANNER_RESPONSE","content":"Both read."}));
    let lines: Vec<String> = order
        .iter()
        .map(|w| match *w {
            "user" => user.clone(),
            "planner" => planner.clone(),
            "alpha" => alpha.clone(),
            "bravo" => bravo.clone(),
            _ => answer.clone(),
        })
        .collect();
    let (_, events) = run(&lines, false, None);
    (
        events.iter().filter(|e| e.kind == "tool.completed").map(|e| (e.payload["itemId"].clone(), e.payload["output"].clone())).collect(),
        events.iter().filter(|e| e.kind == "adapter.diagnostic").count(),
        events.last().unwrap().kind.clone(),
    )
}

fn both_read() -> (Vec<(Value, Value)>, usize, String) {
    (
        vec![(json!("tool_1_0"), json!("ALPHA-MARKER-5170")), (json!("tool_1_1"), json!("BRAVO-MARKER-8823"))],
        0,
        "run.completed".into(),
    )
}

#[test]
fn gives_each_call_its_own_result_in_the_order_the_file_really_wrote_them() {
    // case: gives each call its own result in the order the file really wrote them
    assert_eq!(answered(&["user", "alpha", "planner", "bravo", "answer"]), both_read());
}

#[test]
fn and_in_step_order_where_last_opened_swapped_them() {
    // case: and in step order, where last-opened swapped them
    assert_eq!(answered(&["user", "planner", "alpha", "bravo", "answer"]), both_read());
}

// how an Antigravity run ends

#[test]
fn fails_with_a_reason_a_person_can_act_on_when_no_final_answer_ever_arrived() {
    // case: fails with a reason a person can act on when no final answer ever arrived
    let lines = fixture("transcript-write-then-followup.jsonl");
    let (_, events) = run(&lines[..4], false, None);
    let failure = events.last().unwrap();
    assert_eq!(failure.kind, "run.failed");
    assert_eq!(failure.payload["runtimeTerminal"], "missing");
    assert!(failure.payload["message"].as_str().unwrap().contains("stopped waiting"));
}

#[test]
fn reports_a_cancelled_run_as_cancelled_not_as_a_run_that_failed_to_answer() {
    // case: reports a cancelled run as cancelled, not as a run that failed to answer
    let lines = fixture("transcript-write-then-followup.jsonl");
    assert_eq!(run(&lines[..4], true, None).1.last().unwrap().kind, "run.cancelled");
}

#[test]
fn lets_a_non_zero_exit_from_the_hosts_own_driver_outrank_a_finished_transcript() {
    // case: lets a non-zero exit from the host's own driver outrank a finished transcript
    let (_, events) = run(&fixture("transcript-write-then-followup.jsonl"), false, Some(1));
    let failure = events.last().unwrap();
    assert_eq!(failure.kind, "run.failed");
    assert_eq!(failure.payload["runtimeTerminal"], "completed");
}

#[test]
fn ends_once_a_second_finish_and_anything_accepted_after_it_produce_nothing() {
    // case: ends once: a second finish, and anything accepted after it, produce nothing
    let lines = fixture("transcript-write-then-followup.jsonl");
    let mut n = normalizer();
    feed(&mut n, &lines);
    assert_eq!(n.finish(false, None).len(), 1);
    assert!(n.ended);
    assert!(n.finish(false, None).is_empty());
    assert!(n.accept_line(&lines[0]).is_empty());
}

// a transcript line this build cannot read

#[test]
fn keeps_the_line_as_a_diagnostic_rather_than_dropping_it() {
    // case: keeps the line as a diagnostic rather than dropping it
    let events = normalizer().accept_line("{not json");
    assert_eq!(kinds(&events), ["adapter.diagnostic"]);
    assert_eq!(events[0].payload["code"], "antigravity.malformed_record");
}

#[test]
fn treats_a_line_with_no_step_index_as_malformed_since_there_is_nothing_to_deduplicate_on() {
    // case: treats a line with no step_index as malformed, since there is nothing to deduplicate on
    let events =
        normalizer().accept_line(&json!({"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","content":"OK"}).to_string());
    assert_eq!(events[0].payload["code"], "antigravity.malformed_record");
}

#[test]
fn preserves_a_step_type_this_build_has_never_seen() {
    // case: preserves a step type this build has never seen
    let events = normalizer().accept_line(&json!({"step_index":9,"source":"MODEL","type":"BRAINSTORM","status":"DONE"}).to_string());
    assert_eq!(events[0].payload["code"], "antigravity.unknown_step");
}

// reading a tool call's arguments

fn args() -> Value {
    json!({
        "CodeContent": "\"hello from antigravity\\n\"",
        "Overwrite": "true",
        "TargetFile": "\"c:/work/pebble/hello.txt\"",
        "toolSummary": "\"Create hello.txt\"",
    })
}

#[test]
fn decodes_them_because_every_value_is_a_json_encoded_string_and_not_the_value() {
    // case: decodes them, because every value is a JSON-encoded string and not the value
    let args = args();
    assert_eq!(tool_arg(&args, "Overwrite"), Some(json!(true)));
    assert_eq!(tool_arg(&args, "TargetFile"), Some(json!("c:/work/pebble/hello.txt")));
    assert_eq!(tool_arg(&args, "CodeContent"), Some(json!("hello from antigravity\n")));
    assert_eq!(tool_arg(&args, "Missing"), None);
}

#[test]
fn returns_a_value_that_is_not_json_as_it_arrived_rather_than_discarding_it() {
    // case: returns a value that is not JSON as it arrived, rather than discarding it
    assert_eq!(tool_arg(&json!({"TargetFile": "c:/plain/path.txt"}), "TargetFile"), Some(json!("c:/plain/path.txt")));
}

#[test]
fn summarizes_a_call_on_one_line_falling_back_to_the_models_own_phrasing() {
    // case: summarizes a call on one line, falling back to the model's own phrasing
    assert_eq!(tool_command(&args()).as_deref(), Some("c:/work/pebble/hello.txt"));
    assert_eq!(tool_command(&json!({"toolSummary": "\"Create\\nhello.txt\""})).as_deref(), Some("Create hello.txt"));
    assert_eq!(tool_command(&json!({})), None);
}

#[test]
fn builds_no_patch_for_an_overwrite_whatever_content_the_call_carried() {
    // case: builds no patch for an overwrite, whatever content the call carried
    assert_eq!(write_patch(&args()), None);
}

// the project id `new-conversation` refuses to start without
//
// Electron's `summaries.pb` is a real IDE's file, left out of the public copy of the repository; Electron skips these
// four where it is missing (`describe.skipIf`), and so do these.

fn summaries() -> Option<std::collections::HashMap<String, String>> {
    std::fs::read(fixture_path("summaries.pb")).ok().map(|bytes| parse_projects(&bytes))
}

fn host_with(projects: std::collections::HashMap<String, String>) -> crate::antigravity_host::AntigravityHost {
    crate::antigravity_host::AntigravityHost {
        executable_path: String::new(),
        version: None,
        address: String::new(),
        csrf_token: String::new(),
        projects,
        ports: vec![],
    }
}

#[test]
fn finds_the_workspace_the_ide_had_open_and_the_project_it_belongs_to() {
    // case: finds the workspace the IDE had open and the project it belongs to
    let Some(projects) = summaries() else { return eprintln!("skipped: no summaries.pb, as Electron skips it") };
    assert_eq!(
        projects.into_iter().collect::<Vec<_>>(),
        [("c:/Users/<home>/Documents/antigravtest".to_owned(), "daf0f8ec-bb8e-49e8-a445-954eb0a62d0f".to_owned())]
    );
}

#[test]
fn collapses_both_encodings_of_the_same_workspace_onto_one_key() {
    // case: collapses both encodings of the same workspace onto one key
    let Some(projects) = summaries() else { return eprintln!("skipped: no summaries.pb, as Electron skips it") };
    assert_eq!(projects.len(), 1);
}

#[test]
fn answers_for_a_path_in_the_form_a_host_would_actually_hold_it() {
    // case: answers for a path in the form a host would actually hold it
    let Some(projects) = summaries() else { return eprintln!("skipped: no summaries.pb, as Electron skips it") };
    let host = host_with(projects);
    let id = Some("daf0f8ec-bb8e-49e8-a445-954eb0a62d0f".to_owned());
    assert_eq!(crate::antigravity_host::project_id_for(&host, "C:\\Users\\<home>\\Documents\\antigravtest\\"), id);
    assert_eq!(crate::antigravity_host::project_id_for(&host, "c:/Users/<home>/Documents/antigravtest"), id);
}

#[test]
fn has_no_answer_for_a_workspace_the_file_never_mentioned() {
    // case: has no answer for a workspace the file never mentioned
    let Some(projects) = summaries() else { return eprintln!("skipped: no summaries.pb, as Electron skips it") };
    assert_eq!(crate::antigravity_host::project_id_for(&host_with(projects), "c:/work/pebble"), None);
}

// the project file's parser, without the file

#[test]
fn returns_nothing_for_bytes_that_are_not_this_file_rather_than_a_guess() {
    // case: returns nothing for bytes that are not this file, rather than a guess
    assert!(parse_projects(&[0, 1, 2, 3]).is_empty());
}

#[test]
fn normalizes_a_workspace_to_forward_slashes_a_lower_case_drive_and_no_trailing_slash() {
    // case: normalizes a workspace to forward slashes, a lower-case drive and no trailing slash
    assert_eq!(normalize_workspace("file:///C%3A%5Cwork%5Cpebble%5C"), "c:/work/pebble");
    assert_eq!(normalize_workspace("D:\\work\\pebble"), "d:/work/pebble");
    assert_eq!(normalize_workspace("file:///home/x/proj"), "/home/x/proj");
    assert_eq!(normalize_workspace("/home/x/proj"), "/home/x/proj");
    assert_eq!(normalize_workspace("file:///C:/work/pebble"), "c:/work/pebble");
}

// the row an ask_question draws

fn ask_started() -> Value {
    let ask = json!({
        "step_index": 4, "source": "MODEL", "type": "PLANNER_RESPONSE", "status": "DONE", "created_at": "2026-09-07T19:36:08Z",
        "tool_calls": [{"name": "ask_question", "args": {
            "questions": "[{\"is_multi_select\":false,\"options\":[\"(Recommended) Run performance benchmark tests\",\"Inspect codebase architecture\",\"Execute automated unit tests\",\"Review system environment settings\"],\"question\":\"Which test option would you like to select?\"}]",
            "toolAction": "\"Prompting user with options\"",
            "toolSummary": "\"Interactive option selection\"",
        }}],
    });
    normalizer().accept_line(&ask.to_string()).into_iter().find(|e| e.kind == "tool.started").unwrap().payload
}

#[test]
fn says_what_is_being_asked_and_every_option() {
    // case: says what is being asked, and every option
    let command = ask_started()["command"].as_str().unwrap_or_default().to_owned();
    assert!(command.contains("Which test option would you like to select?"));
    assert!(command.contains("Run performance benchmark tests"));
    assert!(command.contains("Review system environment settings"));
}

#[test]
fn carries_the_question_structured_for_the_card_that_answers_it() {
    // case: carries the question, structured, for the card that answers it
    let started = ask_started();
    assert_eq!(
        started["question"],
        json!({
            "question": "Which test option would you like to select?",
            "options": ["(Recommended) Run performance benchmark tests", "Inspect codebase architecture", "Execute automated unit tests", "Review system environment settings"],
            "multiSelect": false,
            "askedAtStep": 4,
        })
    );
    assert!(!started["command"].as_str().unwrap().contains("answer in Antigravity"));
}

#[test]
fn leaves_every_other_tools_row_exactly_as_it_was() {
    // case: leaves every other tool's row exactly as it was
    let write = json!({"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-09-07T19:36:08Z","tool_calls":[{"name":"write_to_file","args":{"TargetFile":"\"c:/x/notes.md\""}}]});
    let started = normalizer().accept_line(&write.to_string()).into_iter().find(|e| e.kind == "tool.started").unwrap().payload;
    assert!(started.get("question").is_none());
    assert!(started["command"].as_str().unwrap_or_default().contains("notes.md"));
}

// a long planner answer

#[test]
fn keeps_the_end_where_the_blocks_the_host_acts_on_are() {
    // case: keeps the end, where the blocks the host acts on are
    let share = "<locust-share to=\"Booty\">The build is green.</locust-share>";
    let content = format!("{}{share}", "Findings. ".repeat(2_000));
    let events = normalizer().accept_line(
        &json!({"step_index":9,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-09-06T03:02:28Z","content":content}).to_string(),
    );
    let text = events.iter().find(|e| e.kind == "message.delta").unwrap().payload["text"].as_str().unwrap().to_owned();
    assert!(text.starts_with("Findings."));
    assert!(text.ends_with(share));
}
