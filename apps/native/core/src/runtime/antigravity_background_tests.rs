//! Electron's `packages/runtime-adapters/test/antigravity-background.test.ts` (0.487, the 2026-10-10 sweep, 0.537),
//! case by case, through the caller: the Antigravity mission service feeds each transcript line to
//! `TranscriptNormalizer::accept_line` and holds the turn open while `pending_background` is above nought, ending it
//! on `latest_final` (antigravity_mission.rs). Electron's free readers are `background_task`, `task_ending`,
//! `tool_command` and `tool_title` here. The shapes are Colin's transcripts, 2026-09-30, anonymised.
use super::*;

const CONV: &str = "11111111-2222-3333-4444-555555555555";
/// The word before a task's number, kept apart from it.
const TASK: &str = "task";

fn at(fields: Value) -> String {
    let mut line = json!({"source": "MODEL", "status": "DONE", "created_at": "2026-09-21T20:22:22Z"});
    for (k, v) in fields.as_object().unwrap() {
        line[k] = v.clone();
    }
    line.to_string()
}

fn user() -> String {
    json!({"step_index": 0, "source": "USER_EXPLICIT", "type": "USER_INPUT", "status": "DONE", "created_at": "2026-09-21T20:22:00Z", "content": "run the probe"}).to_string()
}

fn encoded(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

fn planner() -> String {
    at(json!({"step_index": 9, "type": "PLANNER_RESPONSE", "tool_calls": [{
        "name": "run_command",
        "args": {"CommandLine": encoded("node _tools/probe.mjs --dev"), "toolAction": encoded("Running the picker probe"), "WaitMsBeforeAsync": "500"},
    }]}))
}

fn running() -> String {
    at(json!({"step_index": 10, "type": "GENERIC", "status": "RUNNING", "content": format!(
        "Created At: 2026-09-21T16:22:22-04:00\nTool is running as a background task with task id: {CONV}/{TASK}-10\nTask Description: node _tools/probe.mjs --dev\nTask logs are available at: file:///c:/logs/{TASK}-10.log"
    )}))
}

fn waiting() -> String {
    at(json!({"step_index": 11, "type": "PLANNER_RESPONSE", "content": "Waiting for task completion."}))
}

fn notice(step: i64, body: &str) -> String {
    json!({"step_index": step, "source": "SYSTEM", "type": "SYSTEM_MESSAGE", "status": "DONE", "created_at": "2026-09-21T20:23:15Z", "content": format!(
        "The following is a <SYSTEM_MESSAGE> not actually sent by the user.\n\n<SYSTEM_MESSAGE>\n[Message] timestamp=2026-09-21T20:23:13Z sender={CONV}/{TASK}-10 priority=MESSAGE_PRIORITY_HIGH content={body}"
    )})
    .to_string()
}

fn finished_at(step: i64) -> String {
    notice(step, &format!("Task id \"{CONV}/{TASK}-10\" finished with result:\n\nThe command exited with code 0.\nOutput:\nprobing the DEV build"))
}

fn after() -> String {
    at(json!({"step_index": 13, "type": "PLANNER_RESPONSE", "content": "The probe passed."}))
}

fn schedule(step: i64) -> String {
    at(json!({"step_index": step, "type": "PLANNER_RESPONSE", "tool_calls": [{
        "name": "schedule",
        "args": {"TimerCondition": encoded(&format!("{CONV}/{TASK}-10")), "Prompt": encoded(&format!("Check if {TASK}-10 finished"))},
    }]}))
}

fn timer_running(step: i64) -> String {
    at(json!({"step_index": step, "type": "GENERIC", "status": "RUNNING", "content": format!(
        "Created At: 2026-09-21T16:23:30-04:00\nTool is running as a background task with task id: {CONV}/{TASK}-{step}\nTask Description: timer"
    )}))
}

/// Electron's `feed`: every event, and after each line how many tasks are out and whether the latest answer is final.
fn feed(lines: &[String]) -> (Vec<Event>, Vec<(usize, bool)>) {
    let mut reader = TranscriptNormalizer { thread: Some(CONV.into()), ..Default::default() };
    let mut events = Vec::new();
    let mut states = Vec::new();
    for line in lines {
        events.extend(reader.accept_line(line));
        states.push((reader.pending_background(), reader.latest_final));
    }
    (events, states)
}

#[test]
fn set_on_a_task_that_already_ended_holds_nothing_open() {
    let (_, states) = feed(&[user(), planner(), running(), finished_at(12), schedule(14), timer_running(15)]);
    assert_eq!(states.last().unwrap().0, 0);
}

#[test]
fn set_while_the_task_runs_settles_when_the_task_ends() {
    let (_, states) = feed(&[user(), planner(), running(), schedule(11), timer_running(12), finished_at(13)]);
    assert_eq!(states[4].0, 2);
    assert_eq!(states.last().unwrap().0, 0);
}

#[test]
fn closes_the_call_as_backgrounded_and_opens_a_background_step_not_did_not_report() {
    let (events, _) = feed(&[user(), planner(), running()]);
    let done = events.iter().find(|e| e.kind == "tool.completed").expect("the call closes");
    assert_eq!(done.payload["background"], true);
    assert_eq!(done.payload["command"], "node _tools/probe.mjs --dev");
    assert_eq!(done.payload["title"], "Running the picker probe");
    let step = events.iter().find(|e| e.kind == "step.started").expect("a background step");
    assert_eq!(step.payload["itemType"], "background");
    assert_eq!(step.payload["status"], "running");
    assert!(!serde_json::to_string(&events.iter().map(|e| &e.payload).collect::<Vec<_>>()).unwrap().contains("did not report"));
}

#[test]
fn holds_the_turn_open_while_it_runs_and_settles_its_row_from_the_tasks_own_notice() {
    let (events, states) = feed(&[user(), planner(), running(), waiting(), finished_at(12), after()]);
    assert_eq!(states[3], (1, true));
    assert_eq!(states[4], (0, false));
    assert_eq!(states[5], (0, true));
    let settled = events.iter().find(|e| e.kind == "step.completed").expect("the row settles");
    assert_eq!(settled.payload["itemType"], "background");
    assert_eq!(settled.payload["status"], "completed");
    assert_eq!(settled.payload["message"], "exited with code 0");
}

#[test]
fn reads_a_non_zero_exit_as_failed_and_a_cancel_as_stopped() {
    let failed = notice(12, &format!("Task id \"{CONV}/{TASK}-10\" finished with result:\n\nThe command exited with code 1."));
    let (events, _) = feed(&[user(), planner(), running(), failed]);
    assert_eq!(events.last().unwrap().kind, "step.failed");
    let cancelled = notice(12, &format!("Task id \"{CONV}/{TASK}-10\" was canceled with result:\nTool execution was canceled"));
    let (events, _) = feed(&[user(), planner(), running(), cancelled]);
    assert_eq!(events.last().unwrap().payload["status"], "stopped");
}

#[test]
fn ignores_a_notice_for_a_task_it_never_saw_start() {
    let (events, states) = feed(&[user(), finished_at(12)]);
    assert!(events.iter().all(|e| e.kind != "step.completed"));
    assert_eq!(states.last().unwrap().0, 0);
}

#[test]
fn finds_the_task_and_what_it_is_doing() {
    let found = background_task(Some(&format!(
        "Tool is running as a background task with task id: {CONV}/{TASK}-757\nTask Description: node _tools/drive-room.mjs"
    )));
    assert_eq!(found, Some((format!("{CONV}/{TASK}-757"), Some("node _tools/drive-room.mjs".to_owned()))));
    assert_eq!(background_task(Some("plain output")), None);
    assert_eq!(task_ending(Some("an unrelated system message")), None);
}

#[test]
fn names_a_command_row_by_the_command_with_the_models_phrase_as_its_title() {
    let args = json!({"CommandLine": encoded("git log -1"), "toolAction": encoded("Checking recent commits")});
    assert_eq!(tool_command(&args).as_deref(), Some("git log -1"));
    assert_eq!(tool_title(&args, Some("git log -1")).as_deref(), Some("Checking recent commits"));
    assert_eq!(tool_title(&json!({"toolAction": encoded("git log -1")}), Some("git log -1")), None);
    // The row the transcript reader opens for that call.
    let line = at(json!({"step_index": 3, "type": "PLANNER_RESPONSE", "tool_calls": [{"name": "run_command", "args": args}]}));
    let (events, _) = feed(&[user(), line]);
    let row = events.iter().find(|e| e.kind == "tool.started").expect("a command row");
    assert_eq!(row.payload["command"], "git log -1");
    assert_eq!(row.payload["title"], "Checking recent commits");
}

#[test]
fn is_an_ordinary_finished_read_nothing_waits_on_it_and_the_answer_ends_the_turn() {
    let reads = at(json!({"step_index": 20, "type": "PLANNER_RESPONSE", "tool_calls": [{"name": "view_file", "args": {"AbsolutePath": encoded("C:/brain/other/transcript.jsonl")}}]}));
    let file_text = at(json!({"step_index": 21, "type": "GENERIC", "content": format!(
        "File Path: C:/brain/other/transcript.jsonl\n{{\"content\":\"Tool is running as a background task with task id: {CONV}/{TASK}-7\nTask Description: npm test\"}}"
    )}));
    let answer = at(json!({"step_index": 22, "type": "PLANNER_RESPONSE", "content": "The review is done."}));
    let (events, states) = feed(&[user(), reads, file_text, answer]);
    assert!(!events.iter().any(|e| e.kind == "step.started" && e.payload["itemType"] == "background"));
    let done = events.iter().find(|e| e.kind == "tool.completed").expect("the read closes");
    assert!(done.payload.get("background").is_none(), "{}", done.payload);
    assert_eq!(*states.last().unwrap(), (0, true));
}
