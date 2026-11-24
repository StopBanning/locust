//! Electron's `packages/runtime-adapters/test/muse-events.test.ts`, case by case, through the caller: each run is a Muse
//! Code turn as muse.rs runs one (`Muse::spawn`, `start`, `next_event` to the end), whose `muse` is a stand-in that
//! writes Electron's own capture (`test/fixtures/muse/echo-provider-run.jsonl`) byte for byte, byte-order mark and CRLF
//! included, or the case's records, then exits as the case says; a run the host cancelled carries the host's cancel
//! flag. The two helpers Electron tests on their own (museSessionIdOf, museTaskIsInternal) are muse_events' own.
//! Fixed 24 Nov: a run Locust stopped for sending more than it takes in (a line past 8 MiB, or a full record queue)
//! ended saying no record said the run had finished, never that Locust stopped it.
use super::*;
use crate::runtime::muse_events::{MUSE_BILLING_SENTENCE, MUSE_SANDBOX_SENTENCE, muse_session_id_of, muse_task_is_internal};
use serde_json::json;

const STAND_IN: &str = r#"
import os, sys
sys.stdout.buffer.write(open(os.environ['LOCUST_MUSE_OUTPUT'], 'rb').read()); sys.stdout.flush()
sys.exit(int(os.environ['LOCUST_MUSE_EXIT']))
"#;

const SESSION: &str = "01a0c5fe-68c8-7903-b6db-0d3c9021bfa6";
const CAUSATION: &str = "2dcba620-64bd-4fb8-93b6-c0f0af3d116b";
const TASK: &str = "01a0c5fe-9999-7000-8000-000000000001";

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn capture() -> Vec<u8> {
    std::fs::read(format!("{}/../../../packages/runtime-adapters/test/fixtures/muse/echo-provider-run.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// A Muse turn whose CLI writes `output` exactly and exits with `exit`; cancelled by the host when `cancelled`.
fn turn(output: &[u8], exit: i32, cancelled: bool) -> Vec<Event> {
    let temp = Temp(std::env::temp_dir().join(format!("locust-muse-case-{}", crate::invariants::unique_nanos())));
    std::fs::create_dir_all(&temp.0).unwrap();
    std::fs::write(temp.0.join("muse.py"), STAND_IN).unwrap();
    std::fs::write(temp.0.join("output"), output).unwrap();
    let mut command = Command::new("python3");
    command
        .arg(temp.0.join("muse.py"))
        .env("LOCUST_MUSE_OUTPUT", temp.0.join("output"))
        .env("LOCUST_MUSE_EXIT", exit.to_string())
        .current_dir(&temp.0);
    let mut rt = Muse::spawn(&mut command).unwrap().with_cancel(Arc::new(AtomicBool::new(cancelled)));
    rt.start(&Turn {
        prompt: "Reply with exactly PING".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::AcceptEdits,
        approvals: None,
        mission_id: "mission_1".into(),
        run_id: "run_1".into(),
    })
    .unwrap();
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "the Muse turn never ended");
        if let Some(e) = rt.next_event().unwrap() {
            let end = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            events.push(e);
            if end {
                return events;
            }
        }
    }
}

/// One record of the captured envelope, with a payload of the case's choosing.
fn record(payload_type: &str, payload: Value, sequence: u64) -> Value {
    json!({
        "schema_version": 1,
        "id": format!("018f0000-0000-7000-8000-0000000000{sequence:02}"),
        "stream": {"kind": "session", "id": SESSION},
        "sequence": sequence,
        "recorded_at": 1_780_531_400_000_000u64 + sequence,
        "record_type": "event",
        "durability": "durable",
        "causation_id": CAUSATION,
        "payload_type": payload_type,
        "payload_schema_version": 1,
        "payload": payload,
    })
}

fn lifecycle(kind: &str, extra: Value, sequence: u64) -> Value {
    let mut event = json!({"kind": kind, "task_id": TASK});
    for (k, v) in extra.as_object().unwrap() {
        event[k] = v.clone();
    }
    record(&format!("task.lifecycle.{kind}"), json!({"kind": "task_lifecycle", "task_id": TASK, "event": event}), sequence)
}

fn completed_terminal(sequence: u64) -> Value {
    record("run.terminal.completed", json!({"terminal": "completed", "text": "done", "reason": null}), sequence)
}

fn started_run() -> Value {
    record("run.lifecycle.started", json!({"prompt": "hello"}), 1)
}

fn output_of(records: &[Value]) -> Vec<u8> {
    records.iter().map(|r| format!("{r}\n")).collect::<String>().into_bytes()
}

fn records_then(records: &[Value], exit: i32) -> Vec<Event> {
    turn(&output_of(records), exit, false)
}

fn types_of(events: &[Event]) -> Vec<&str> {
    events.iter().map(|e| e.kind.as_str()).collect()
}

/// The events a case's records drew, without the run's own end.
fn drawn(events: &[Event]) -> Vec<&str> {
    types_of(events).into_iter().filter(|k| !k.starts_with("run.")).collect()
}

fn codes_of(events: &[Event]) -> Vec<&str> {
    events.iter().filter(|e| e.kind == "adapter.diagnostic").map(|e| e.payload["code"].as_str().unwrap()).collect()
}

fn first<'a>(events: &'a [Event], kind: &str) -> &'a Event {
    events.iter().find(|e| e.kind == kind).unwrap_or_else(|| panic!("no {kind} in {:?}", types_of(events)))
}

// --- the Muse Code run that was actually captured

// case: resumes against the session, not the run or the task
#[test]
fn the_captured_run_resumes_against_its_session() {
    let events = turn(&capture(), 0, false);
    let thread = events.last().unwrap().thread.as_deref();
    assert_eq!(thread, Some(SESSION));
    assert_ne!(thread, Some(CAUSATION));
    assert!(events.iter().all(|e| e.thread.as_deref() == Some(SESSION)));
}

// case: reads the first record, byte-order mark and all
#[test]
fn reads_the_first_record_despite_its_byte_order_mark() {
    assert_eq!(&capture()[..3], [0xEF, 0xBB, 0xBF], "the capture starts with its mark");
    let events = turn(&capture(), 0, false);
    assert!(!codes_of(&events).contains(&"muse.malformed_record"));
    assert_eq!(events[0].kind, "step.started", "record one was read");
}

// case: says the answer once
#[test]
fn the_captured_answer_is_said_once() {
    let events = turn(&capture(), 0, false);
    let mut text = String::new();
    for e in events.iter().filter(|e| e.kind == "message.delta" && e.payload["itemId"] == "msg_0") {
        if e.payload["operation"] == "replace" {
            text.clear();
        }
        text.push_str(e.payload["text"].as_str().unwrap());
    }
    assert_eq!(text, "echo: Reply with exactly PING");
    let finals: Vec<&Event> = events.iter().filter(|e| e.kind == "message.delta" && e.payload["final"] == true).collect();
    assert_eq!(finals.len(), 1);
    assert_eq!(finals[0].payload["operation"], "replace");
}

// case: keeps Muse's own scaffolding off the transcript
#[test]
fn keeps_muses_own_tasks_off_the_transcript() {
    let events = turn(&capture(), 0, false);
    assert!(!events.iter().any(|e| e.kind.starts_with("tool.")));
    assert_eq!(types_of(&events), ["step.started", "message.delta", "adapter.diagnostic", "message.delta", "step.completed", "run.completed"]);
}

// case: understands every record it was sent
#[test]
fn understands_every_captured_record() {
    let events = turn(&capture(), 0, false);
    let codes = codes_of(&events);
    assert!(!codes.contains(&"muse.unknown_event") && !codes.contains(&"muse.unknown_task_state"), "{codes:?}");
}

// case: does not fail the run for a task that failed
#[test]
fn a_failed_internal_task_does_not_fail_the_run() {
    let events = turn(&capture(), 0, false);
    let failure = events
        .iter()
        .find(|e| e.kind == "adapter.diagnostic" && e.payload["code"] == "muse.internal_task_failed")
        .expect("the internal task's failure is said");
    assert!(failure.payload["message"].as_str().unwrap().contains("does not support base instructions"));
    assert_eq!(failure.payload["level"], "info");
    assert!(types_of(&events).contains(&"run.completed"));
    assert!(!types_of(&events).contains(&"run.failed"));
}

// --- a Muse task that is not Muse's own machinery

// case: draws one row, from start to finish, naming what it did
#[test]
fn a_tool_task_draws_one_row_naming_what_it_did() {
    let events = records_then(
        &[
            lifecycle("proposed", json!({"task_kind": "tool.shell.command"}), 1),
            lifecycle("accepted", json!({}), 2),
            lifecycle("scheduled", json!({}), 3),
            lifecycle("side_effect_intent", json!({"operation": "shell.execute"}), 4),
            lifecycle("started", json!({}), 5),
            lifecycle("completed", json!({}), 6),
        ],
        0,
    );
    assert_eq!(drawn(&events), ["tool.started", "tool.completed"]);
    for e in events.iter().filter(|e| e.kind.starts_with("tool.")) {
        assert_eq!(e.payload["name"], "shell.command");
        assert_eq!(e.payload["itemId"], TASK);
        assert_eq!(e.payload["command"], "shell.execute");
    }
}

// case: fails its own row and leaves the run alone
#[test]
fn a_failed_tool_task_fails_its_row_not_the_run() {
    let events = records_then(
        &[
            lifecycle("proposed", json!({"task_kind": "tool.file.write"}), 1),
            lifecycle("started", json!({}), 2),
            lifecycle("failed", json!({"reason": "permission denied"}), 3),
            completed_terminal(4),
        ],
        0,
    );
    assert!(types_of(&events).contains(&"run.completed"));
    assert_eq!(first(&events, "tool.failed").payload["status"], "permission denied");
}

// case: says once, in a sentence, that Muse's own sandbox is not set up (2026-10-13, the voxel round)
#[test]
fn says_once_that_muses_sandbox_is_not_set_up() {
    let reason = "environment failure: sandbox enforcement unavailable: windows_elevated setup_required: sandbox users are not ready\nThe execution environment is broken: the command was never started and every later command will fail the same way.";
    let events = records_then(
        &[
            lifecycle("proposed", json!({"task_kind": "tool.powershell"}), 1),
            lifecycle("started", json!({}), 2),
            lifecycle("failed", json!({"reason": reason}), 3),
            lifecycle("started", json!({}), 4),
            lifecycle("failed", json!({"reason": reason}), 5),
            completed_terminal(6),
        ],
        0,
    );
    assert_eq!(types_of(&events).iter().filter(|k| **k == "tool.failed").count(), 2);
    assert!(types_of(&events).contains(&"run.completed"));
    let said: Vec<&Event> = events.iter().filter(|e| e.kind == "adapter.diagnostic" && e.payload["code"] == "muse.sandbox_not_set_up").collect();
    assert_eq!(said.len(), 1);
    assert_eq!(said[0].payload["level"], "warning");
    assert_eq!(said[0].payload["message"], MUSE_SANDBOX_SENTENCE);
    assert!(MUSE_SANDBOX_SENTENCE.contains("muse sandbox windows setup"));
}

// case: says nothing about the sandbox for a command that failed some other way
#[test]
fn says_nothing_of_the_sandbox_for_another_failure() {
    let events = records_then(
        &[lifecycle("proposed", json!({"task_kind": "tool.powershell"}), 1), lifecycle("failed", json!({"reason": "exit code 1"}), 2)],
        0,
    );
    assert!(!codes_of(&events).contains(&"muse.sandbox_not_set_up"));
}

// case: keeps a long failure reason within what the ledger keeps (2026-10-10 sweep)
#[test]
fn a_long_failure_reason_is_kept_to_512() {
    let events = records_then(
        &[
            lifecycle("proposed", json!({"task_kind": "tool.file.write"}), 1),
            lifecycle("started", json!({}), 2),
            lifecycle("failed", json!({"reason": "y".repeat(600)}), 3),
        ],
        0,
    );
    assert_eq!(first(&events, "tool.failed").payload["status"].as_str().unwrap().chars().count(), 512);
}

// --- a Muse run that did not finish

// case: says the stream stopped rather than guessing why
#[test]
fn says_the_stream_stopped_without_guessing_why() {
    let events = records_then(&[started_run()], 0);
    let failed = first(&events, "run.failed");
    assert_eq!(failed.payload["message"], "Muse Code ended without a record saying the run had finished.");
    assert_eq!(failed.payload["runtimeTerminal"], "missing");
}

// case: repeats the runtime's own terminal word and reason
#[test]
fn repeats_muses_own_terminal_word_and_reason() {
    let events = records_then(
        &[started_run(), record("run.terminal.cancelled", json!({"terminal": "cancelled", "text": null, "reason": "interrupted by user"}), 2)],
        0,
    );
    let failed = first(&events, "run.failed");
    assert_eq!(failed.payload["message"], "Muse Code ended cancelled: interrupted by user");
    assert_eq!(failed.payload["runtimeTerminal"], "failed");
}

// case: says a billing refusal is the account's, not the app breaking
#[test]
fn a_billing_refusal_is_said_as_the_accounts() {
    let reason = "API error 402 [request_id=db5edbb9-ecbe-4f80-be1d-108647c7586c]: Billing verification failed. Please check your payment method. (billing_error)";
    let events = records_then(
        &[started_run(), record("run.terminal.failed", json!({"kind": "run_terminal", "terminal": "failed", "text": "", "reason": reason}), 2)],
        1,
    );
    let message = first(&events, "run.failed").payload["message"].as_str().unwrap().to_owned();
    assert_eq!(message, MUSE_BILLING_SENTENCE);
    assert!(!message.contains("request_id"));
}

// case: says so when Locust was the one that stopped it
#[test]
fn says_so_when_locust_stopped_the_run() {
    // A line past the 8 MiB Locust takes in, after the run opened.
    let mut output = output_of(&[started_run()]);
    output.extend(std::iter::repeat_n(b'x', 9 * 1024 * 1024));
    output.push(b'\n');
    let events = turn(&output, 1, false);
    let failed = first(&events, "run.failed");
    assert!(failed.payload["message"].as_str().unwrap().contains("faster than Locust could record it"), "{}", failed.payload);
    assert_eq!(failed.payload["process"]["outputLimitExceeded"], true);
}

// case: reports a cancelled run as cancelled, not failed
#[test]
fn a_run_the_host_cancelled_is_cancelled_not_failed() {
    let events = turn(&output_of(&[started_run()]), 0, true);
    assert!(types_of(&events).contains(&"run.cancelled"));
    assert!(!types_of(&events).contains(&"run.failed"));
}

// --- the two facts the adapter refuses to guess at

// case: takes a thread id from a session stream and from nothing else
#[test]
fn a_thread_id_comes_from_a_session_stream_only() {
    assert_eq!(muse_session_id_of(&json!({"stream": {"kind": "session", "id": "s1"}})).as_deref(), Some("s1"));
    assert_eq!(muse_session_id_of(&json!({"stream": {"kind": "run", "id": "r1"}})), None);
    assert_eq!(muse_session_id_of(&json!({"stream": {"kind": "task", "id": "t1"}})), None);
    assert_eq!(muse_session_id_of(&json!({"id": "s1"})), None);
}

// case: knows which task kinds are Muse talking to itself
#[test]
fn knows_which_task_kinds_are_muses_own() {
    assert!(muse_task_is_internal(Some("model.unknown.response")));
    assert!(muse_task_is_internal(Some("reminder.agent.skill-reminder")));
    assert!(muse_task_is_internal(Some("reminder.agent.verify-reminder")));
    assert!(!muse_task_is_internal(Some("tool.shell.command")));
    assert!(!muse_task_is_internal(None));
}
