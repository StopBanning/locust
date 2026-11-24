//! Electron's `packages/runtime-adapters/test/agy-events.test.ts`, case by case, through the caller: each run is a
//! native Antigravity turn (`Antigravity::spawn`, `start`, `next_event` to the end) whose `agy` is a stand-in that reads
//! the prompt line, prints Electron's own capture (`test/fixtures/agy-2026-10-02/`) or the records the case writes, and
//! exits as the capture did. The turn's mode is the sandbox Electron hands its normalizer (antigravity.rs `sandbox_for`).
use crate::runtime::{Event, Runtime, Turn, agy_events::QUOTA_SENTENCE, antigravity::Antigravity};
use crate::approval::Mode;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command, time::Instant};

const STAND_IN: &str = r#"
import os, sys, time
sys.stdin.read()
for line in open(os.environ['LOCUST_AGY_LINES'], encoding='utf-8').read().splitlines():
    if line.strip():
        sys.stdout.write(line + '\n'); sys.stdout.flush()
sys.exit(int(os.environ['LOCUST_AGY_EXIT']))
"#;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn capture(name: &str) -> Vec<String> {
    let path = format!("{}/../../../packages/runtime-adapters/test/fixtures/agy-2026-10-02/{name}.jsonl", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap().trim().lines().map(str::to_owned).collect()
}

/// A native Antigravity turn in `mode` whose agy prints `lines` and exits with `exit`.
pub(super) fn turn_of(lines: &[String], exit: i32, mode: Mode) -> Vec<Event> {
    let temp = Temp(std::env::temp_dir().join(format!("locust-agy-case-{}", crate::invariants::unique_nanos())));
    std::fs::create_dir_all(&temp.0).unwrap();
    std::fs::write(temp.0.join("agy.py"), STAND_IN).unwrap();
    std::fs::write(temp.0.join("lines.jsonl"), lines.join("\n")).unwrap();
    let mut command = Command::new("python3");
    command
        .arg(temp.0.join("agy.py"))
        .env("LOCUST_AGY_LINES", temp.0.join("lines.jsonl"))
        .env("LOCUST_AGY_EXIT", exit.to_string())
        .current_dir(&temp.0);
    let mut rt = Antigravity::spawn(&mut command).unwrap();
    rt.start(&Turn {
        prompt: "Read note.txt".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode,
        approvals: None,
        mission_id: "agy_case".into(),
        run_id: "run_agy_case".into(),
    })
    .unwrap();
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "the agy turn never ended");
        if let Some(e) = rt.next_event().unwrap() {
            let end = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            events.push(e);
            if end {
                return events;
            }
        }
    }
}

fn run(name: &str, exit: i32) -> Vec<Event> {
    turn_of(&capture(name), exit, Mode::Ask)
}

fn types(events: &[&Event]) -> Vec<String> {
    events.iter().map(|e| e.kind.clone()).collect()
}

fn tools(events: &[Event]) -> Vec<&Event> {
    events.iter().filter(|e| e.kind.starts_with("tool.")).collect()
}

fn settled(events: &[Event]) -> Vec<&Event> {
    events.iter().filter(|e| e.kind == "tool.completed" || e.kind == "tool.failed").collect()
}

fn last_delta(events: &[Event]) -> &Value {
    &events.iter().rev().find(|e| e.kind == "message.delta").unwrap().payload
}

fn diagnostic<'a>(events: &'a [Event], code: &str) -> Option<&'a Value> {
    events.iter().find(|e| e.kind == "adapter.diagnostic" && e.payload["code"] == code).map(|e| &e.payload)
}

fn step(index: u32, fields: Value) -> String {
    let mut update = json!({"conversation_id":"c1","step_index":index});
    update.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    json!({"event":"step_update","step_update":update}).to_string()
}

// ---- Antigravity CLI's stream

#[test]
fn reads_a_file_one_tool_row_the_answer_whole_the_conversation_to_carry_on_and_usage() {
    // case: reads a file: one tool row, the answer whole, the conversation to carry on, and usage
    let events = run("reads-a-file", 0);
    assert!(events.iter().all(|e| e.thread.as_deref() == Some("c0da4624-b6cd-4e9e-9979-87b67b7abe60")));
    let tools = tools(&events);
    assert_eq!(types(&tools), ["tool.started", "tool.completed"]);
    assert_eq!(tools[0].payload["name"], "view_file");
    assert_eq!(tools[0].payload["command"], "C:/Users/person/Documents/locust-scratch/agy-probe/note.txt");
    let last = last_delta(&events);
    assert_eq!(last["text"], "hello from a file\nDONE\n");
    assert_eq!(last["final"], true);
    assert_eq!(last["operation"], "replace");
    let done = events.last().unwrap();
    assert_eq!(done.kind, "run.completed");
    let usage = done.payload["usage"].as_object().unwrap();
    assert_eq!(usage.len(), 2);
    assert_eq!(usage["inputTokens"].as_f64(), Some(20836.0));
    assert_eq!(usage["outputTokens"].as_f64(), Some(173.0));
}

#[test]
fn the_answer_arrived_whole_then_the_service_was_unavailable_finished_with_a_line_saying_so() {
    // case: the answer arrived whole, then the service was unavailable: finished, with a line saying so (0.657)
    let events = run("answered-then-unavailable", 1);
    let last = last_delta(&events);
    assert_eq!(last["text"], "hello from a file\nDONE\n");
    assert_eq!(last["final"], true);
    let said = events.iter().find(|e| e.kind == "adapter.diagnostic").unwrap();
    assert_eq!(said.payload["level"], "warning");
    assert_eq!(said.payload["code"], "antigravity.unavailable_after_answer");
    assert!(said.payload["message"].as_str().unwrap().contains("send \"continue\""));
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert!(events.iter().all(|e| e.kind != "run.failed"));
}

#[test]
fn a_sentence_said_before_a_step_is_not_said_again_on_top_of_the_answer() {
    // case: a sentence said before a step is not said again on top of the answer (0.668)
    let lines = [
        r#"{"event":"init","conversation_id":"c1"}"#.to_owned(),
        step(1, json!({"step_type":"agent_response","state":"DONE","text_delta":"I am about to read note.txt."})),
        step(2, json!({"step_type":"tool","state":"DONE","tool_name":"view_file","tool_info":{"parameters":{"AbsolutePath":"C:/w/note.txt"}}})),
        step(3, json!({"step_type":"agent_response","state":"DONE","text_delta":"It says hello. FINISHED"})),
        r#"{"event":"result","result":{"conversation_id":"c1","status":"SUCCESS","response":"I am about to read note.txt.\nIt says hello. FINISHED"}}"#.to_owned(),
    ];
    let events = turn_of(&lines, 0, Mode::Ask);
    let last = last_delta(&events);
    assert_eq!(last["text"], "It says hello. FINISHED");
    assert_eq!(last["final"], true);
    assert_eq!(last["itemId"], "msg_3");
}

#[test]
fn a_quota_used_up_is_the_limit_card_with_the_reset_as_the_instant_it_names() {
    // case: a quota used up is the limit card, with the reset as the instant it names
    let raw = r#"{"event":"result","result":{"conversation_id":"c1","status":"ERROR","response":"","error":"Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 33h54m24s."}}"#;
    let before = chrono::Utc::now();
    let events = turn_of(&[raw.to_owned()], 1, Mode::Ask);
    let after = chrono::Utc::now();
    let limit = &events.iter().find(|e| e.kind == "route.limit_detected").unwrap().payload;
    assert_eq!(limit["kind"], "quota-exhausted");
    // The run's clock is the machine's: the instant is the one 33h54m24s from when the run ended, to the second.
    let message = limit["message"].as_str().unwrap();
    let when = message.strip_prefix("Antigravity's quota is used up · resets ").unwrap();
    assert!(crate::invariants::regex(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$").is_match(when), "{message}");
    let at = chrono::DateTime::parse_from_rfc3339(when).unwrap().with_timezone(&chrono::Utc);
    let span = chrono::Duration::seconds(33 * 3600 + 54 * 60 + 24);
    assert!(at >= (before + span).with_nanosecond(0).unwrap() && at <= after + span, "{message}");
    assert_eq!(events.last().unwrap().kind, "run.failed");
    // Under the card, one sentence that says what to do: not the card's fact again with an upgrade pitch.
    assert_eq!(events.last().unwrap().payload["message"], QUOTA_SENTENCE);
    assert!(!crate::invariants::regex(r"(?i)upgrade|subscription").is_match(QUOTA_SENTENCE));
    assert_eq!(crate::runtime::agy_events::quota_limit("RESOURCE_EXHAUSTED", 0).as_deref(), Some("Antigravity's quota is used up"));
    assert!(
        crate::runtime::agy_events::quota_limit("API error (attempt 1): UNAVAILABLE (code 503): The service is currently unavailable.", 0).is_none()
    );
}
use chrono::Timelike;

#[test]
fn a_503_with_no_answer_is_still_a_failure() {
    // case: a 503 with no answer is still a failure
    let raw = r#"{"event":"result","result":{"conversation_id":"c1","status":"ERROR","response":"","error":"API error (attempt 1): UNAVAILABLE (code 503): The service is currently unavailable."}}"#;
    let events = turn_of(&[raw.to_owned()], 1, Mode::Ask);
    assert_eq!(events.last().unwrap().kind, "run.failed");
    assert!(diagnostic(&events, "antigravity.unavailable_after_answer").is_none());
}

#[test]
fn a_write_in_the_default_mode_is_refused_its_row_says_so_and_the_person_is_told_how_to_allow_it() {
    // case: a write in the default mode is refused, its row says so, and the person is told how to allow it
    let events = run("write-denied", 0);
    assert_eq!(types(&tools(&events)), ["tool.started", "tool.failed"]);
    let said = events.iter().find(|e| e.kind == "adapter.diagnostic").unwrap();
    assert_eq!(said.payload["level"], "warning");
    assert_eq!(
        said.payload["message"],
        "It stopped there. Antigravity was not allowed to change files in this mode. Choose Edit or Auto to let it."
    );
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn in_edit_the_file_is_written_and_the_command_refused_said_as_such() {
    // case: in Edit, the file is written and the command refused, said as such
    let events = turn_of(&capture("edit-then-command-denied"), 0, Mode::AcceptEdits);
    let tools = tools(&events);
    let rows: Vec<_> = tools.iter().map(|e| format!("{}:{}", e.kind, e.payload["name"].as_str().unwrap())).collect();
    assert_eq!(
        rows,
        ["tool.started:write_to_file", "tool.completed:write_to_file", "tool.started:run_command", "tool.failed:run_command"]
    );
    let said = events.iter().find(|e| e.kind == "adapter.diagnostic").unwrap().payload["message"].as_str().unwrap();
    assert!(said.starts_with("It stopped there. Antigravity was not allowed to run a command in this mode"), "{said}");
    // The command never ran: refused, with its reason, not failed (0.541).
    assert_eq!(tools.last().unwrap().payload["status"], "refused");
    assert_eq!(tools.last().unwrap().payload["output"], "Not allowed in this mode.");
}

#[test]
fn a_refusal_agy_reports_as_done_is_refused_once_the_result_names_it() {
    // case: a refusal agy reports as DONE is refused once the result names it (0.550, Sol on 0.546)
    let events = run("ask-command-denied-done", 0);
    let tools = tools(&events);
    assert_eq!(types(&tools), ["tool.started", "tool.failed"]);
    assert_eq!(tools[1].payload["status"], "refused");
    assert_eq!(tools[1].payload["command"], "git status");
    // The step settles before the note that says why.
    let failed = events.iter().position(|e| e.kind == "tool.failed").unwrap();
    let note = events.iter().position(|e| e.kind == "adapter.diagnostic").unwrap();
    assert!(failed < note);
}

#[test]
fn a_write_that_ran_with_nothing_to_show_is_still_done_when_the_result_refused_something_else() {
    // case: a write that ran with nothing to show is still done when the result refused something else
    let events = run("edit-then-command-denied", 0);
    let write = events.iter().find(|e| e.kind != "tool.started" && e.kind.starts_with("tool.") && e.payload["name"] == "write_to_file");
    assert_eq!(write.unwrap().kind, "tool.completed");
}

#[test]
fn a_refusal_agy_explains_before_it_stops_is_still_refused() {
    // case: a refusal agy explains before it stops is still refused (0.551, Boss on 0.550)
    let events = run("ask-command-denied-then-said", 0);
    let settled = settled(&events);
    assert_eq!(types(&settled), ["tool.failed"]);
    assert_eq!(settled[0].payload["status"], "refused");
}

#[test]
fn plan_mode_its_own_plan_file_is_written_the_change_it_went_on_to_make_is_refused() {
    // case: plan mode: its own plan file is written, the change it went on to make is refused
    let events = turn_of(&capture("plan-mode-writes-its-plan"), 0, Mode::Plan);
    let settled = settled(&events);
    let by = |end: &str| *settled.iter().find(|e| e.payload["command"].as_str().is_some_and(|c| c.ends_with(end))).unwrap();
    assert_eq!(by("plan.md").kind, "tool.completed");
    assert_eq!(by("greet.js").kind, "tool.failed");
    assert_eq!(by("greet.js").payload["status"], "refused");
}

#[test]
fn in_edit_an_edit_is_never_held_it_is_done_as_it_ends() {
    // case: in Edit an edit is never held: it is done as it ends
    let events = turn_of(&capture("edit-then-command-denied"), 0, Mode::AcceptEdits);
    let write = events.iter().position(|e| e.kind == "tool.completed" && e.payload["name"] == "write_to_file").unwrap();
    let command = events.iter().position(|e| e.kind == "tool.started" && e.payload["name"] == "run_command").unwrap();
    assert!(write < command);
}

#[test]
fn an_allowed_command_carries_its_command_line_and_its_output() {
    // case: an allowed command carries its command line and its output
    let events = run("command-allowed", 0);
    let done = &events.iter().find(|e| e.kind == "tool.completed").unwrap().payload;
    assert_eq!(done["command"], "node -e \"console.log(40+2)\"");
    assert_eq!(done["output"], "42\n");
}

#[test]
fn an_unknown_model_fails_the_run_with_agys_own_reason_first_line_only() {
    // case: an unknown model fails the run with agy's own reason, first line only
    let events = run("unknown-model", 1);
    let failed = events.last().unwrap();
    assert_eq!(failed.kind, "run.failed");
    assert_eq!(
        failed.payload["message"],
        "Antigravity could not run it: invalid model selection (--model \"no-such-model\" --effort \"\"): model no-such-model is not recognized as a known model or custom model in settings"
    );
}

#[test]
fn never_records_reasoning_text_agy_only_counts_it() {
    // case: never records reasoning text: agy only counts it
    let events = run("reads-a-file", 0);
    let all = serde_json::to_string(&events.iter().map(|e| &e.payload).collect::<Vec<_>>()).unwrap();
    assert!(!crate::invariants::regex(r#""thinking"|reasoning text"#).is_match(&all));
}

// ---- the agy command (antigravity.rs builds it; the turn above is started with it)

#[test]
fn is_print_mode_with_the_prompt_on_stdin_and_each_mode_is_the_measured_flag() {
    // case: is print mode with the prompt on stdin, and each mode is the measured flag
    use crate::runtime::antigravity::print_args;
    let ask = print_args(Mode::Ask, Some("gemini-3.8-flash-low"), None, None, &[], None);
    // Electron passes `--model gemini-3.8-flash-low`; Rust names the effort beside the model, which agy takes too
    // (commands.ts parseAgyModelList, measured 2026-10-02: "it also takes `--model gemini-3.8-flash --effort low`").
    assert_eq!(
        ask,
        ["--output-format", "stream-json", "--input-format", "stream-json", "--model", "gemini-3.8-flash", "--effort", "low", "-p="]
    );
    assert!(print_args(Mode::AcceptEdits, None, None, None, &[], None).contains(&"accept-edits".to_owned()));
    assert!(print_args(Mode::Auto, None, None, None, &[], None).contains(&"--dangerously-skip-permissions".to_owned()));
    assert!(!print_args(Mode::AcceptEdits, None, None, None, &[], None).contains(&"--dangerously-skip-permissions".to_owned()));
    let resumed = print_args(Mode::Ask, None, None, Some("c0da"), &[], None);
    assert!(resumed.windows(2).any(|w| w == ["--conversation", "c0da"]));
}

#[test]
fn allows_everything_only_under_full_access_and_the_flag_stays_banned_for_every_other_runtime() {
    // case: allows everything only under full access, and the flag stays banned for every other runtime
    // Electron's last gate refuses the flag on any other spec; Rust builds each argv whole from the mode, so the check
    // is that no other mode and no other runtime's argv ever carries it.
    use crate::runtime::antigravity::print_args;
    for mode in [Mode::Ask, Mode::Plan, Mode::AcceptEdits, Mode::ApproveEach] {
        assert!(!print_args(mode, None, None, None, &[], None).iter().any(|a| a == "--dangerously-skip-permissions"));
    }
    assert!(print_args(Mode::Auto, None, None, None, &[], None).iter().any(|a| a == "--dangerously-skip-permissions"));
    for mode in [Mode::Ask, Mode::Plan, Mode::AcceptEdits, Mode::ApproveEach, Mode::Auto] {
        let claude = crate::runtime::claude::print_args(&crate::runtime::claude::PrintOptions { mode, bridge: Some("/bridge.json"), ..Default::default() })
            .unwrap();
        assert!(!claude.iter().any(|a| a == "--dangerously-skip-permissions"), "{mode:?}");
    }
}

#[test]
fn sends_the_prompt_in_the_one_input_shape_agy_accepted() {
    // case: sends the prompt in the one input shape agy accepted
    let line: Value = serde_json::from_slice(&crate::runtime::antigravity::user_line("Fix it")).unwrap();
    assert_eq!(line, json!({"event":"user","message":{"role":"user","content":[{"type":"text","text":"Fix it"}]}}));
}

#[test]
fn reads_agy_models_as_printed() {
    // case: reads agy models as printed
    let listed = crate::models::parse_agy_model_list("Fetching available models...\ngemini-3.8-flash-high\tGemini 3.8 Flash (High)\ngemini-3.1-pro-low\tGemini 3.1 Pro (Low)\nclaude-opus-4-6-thinking\tClaude Opus 4.6 (Thinking)\n").unwrap();
    assert_eq!(
        listed["models"],
        json!([
            {"id":"gemini-3.8-flash","displayName":"Gemini 3.8 Flash","efforts":["high"]},
            {"id":"gemini-3.1-pro","displayName":"Gemini 3.1 Pro","efforts":["low"]},
            {"id":"claude-opus-4-6-thinking","displayName":"Claude Opus 4.6 (Thinking)"}
        ])
    );
    // Each effort a row of its own in agy: one model here, efforts in order.
    let flash = crate::models::parse_agy_model_list("gemini-3.8-flash-high\tGemini 3.8 Flash (High)\ngemini-3.8-flash-low\tGemini 3.8 Flash (Low)\ngemini-3.8-flash-medium\tGemini 3.8 Flash (Medium)\n").unwrap();
    assert_eq!(flash["models"], json!([{"id":"gemini-3.8-flash","displayName":"Gemini 3.8 Flash","efforts":["low","medium","high"]}]));
}

#[test]
fn thinking_reads_as_its_time_only_where_thinking_tokens_were_spent() {
    // case: thinking reads as its time, only where thinking tokens were spent (0.542)
    // reads-a-file: step 1 spent 62 thinking tokens over 4.39s before its tool.
    let events = run("reads-a-file", 0);
    let thought: Vec<_> = events.iter().filter(|e| e.kind == "step.completed" && e.payload["stepKind"] == "reasoning").collect();
    assert_eq!(thought.len(), 1);
    assert_eq!(thought[0].payload["durationMs"], 4385);
    assert!(thought[0].payload.get("message").is_none());
    // command-allowed spent none: no line claims it thought.
    let none = run("command-allowed", 0);
    assert!(none.iter().all(|e| !(e.kind.starts_with("step.") && e.payload["stepKind"] == "reasoning")));
}

#[test]
fn a_conversation_the_cli_does_not_have_ends_the_session_and_says_why() {
    // case: a conversation the CLI does not have ends the session and says why (0.541, a conversation the app began)
    let events = run("conversation-not-found", 1);
    let failed = events.last().unwrap();
    assert_eq!(failed.kind, "run.failed");
    assert_eq!(failed.payload["sessionEnded"], true);
    assert_eq!(
        failed.payload["message"],
        "Antigravity CLI does not have this conversation: it began in the Antigravity app. Send it again and it starts fresh, with the conversation so far."
    );
}
