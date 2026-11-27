//! Antigravity's rows read like Claude's (`apps/desktop/src/renderer/src/antigravity-rows-read-like-claude.test.ts`).
use super::*;
use locust_core::activity_trace::{activity_trace, outcome};
use locust_core::ledger::{Item, Transcript};
use locust_core::runtime::agy_events::AgyNormalizer;
use locust_core::signal_rail;
use locust_core::thread_words;
use locust_core::tool_fold;
use serde_json::{Value, json};

fn agy_event(seq: u64, kind: &str, payload: Value) -> Value {
    let occurred_at = chrono::DateTime::from_timestamp_millis(
        1_790_942_400_000 + (seq as i64) * 1000,
    )
    .unwrap()
    .to_rfc3339();
    json!({
        "id": format!("run:antigravity:{seq}"),
        "runId": "run",
        "sequence": seq,
        "occurredAt": occurred_at,
        "sourceAdapter": "antigravity",
        "type": kind,
        "payload": payload,
    })
}

fn transcript_of(events: &[Value]) -> Transcript {
    let mut t = Transcript::default();
    t.apply(
        events
            .iter()
            .map(|e| json!({"recordType": "mission.event", "event": e}))
            .collect(),
    );
    t
}

fn base_events() -> Vec<Value> {
    vec![
        agy_event(1, "run.started", json!({})),
        agy_event(
            2,
            "tool.started",
            json!({"itemId": "tool_1", "toolKind": "manage_task", "name": "manage_task", "phase": "started"}),
        ),
        agy_event(
            3,
            "tool.completed",
            json!({"itemId": "tool_1", "toolKind": "manage_task", "name": "manage_task", "phase": "completed"}),
        ),
        agy_event(
            4,
            "tool.started",
            json!({"itemId": "tool_2", "toolKind": "run_command", "name": "run_command", "command": "git status", "phase": "started"}),
        ),
        agy_event(
            5,
            "tool.failed",
            json!({"itemId": "tool_2", "toolKind": "run_command", "name": "run_command", "command": "git status", "phase": "completed", "status": "refused", "output": "Not allowed in this mode."}),
        ),
        agy_event(6, "run.completed", json!({})),
    ]
}

fn draw_item(item: Item, open: &[&str]) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    for key in open {
        s.activity_open.insert((*key).into(), true);
    }
    let mut rt = rune::Runtime::new(
        (s, item),
        |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"),
        (900.0, 400.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements()
}

fn draw_group(items: Vec<Item>, open: &[&str]) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    for key in open {
        s.activity_open.insert((*key).into(), true);
    }
    let mut rt = rune::Runtime::new(
        (s, items),
        |(s, items), _| activity_group(items, &Palette::new(true), s, "conv"),
        (900.0, 400.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements()
}

fn drawn_words(all: &[rune::ElementInfo]) -> String {
    all.iter()
        .filter_map(|e| e.text.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

// case: reads "Thought for Ns" from the time agy gives the step, though the step arrives already over
#[test]
fn reads_thought_for_ns_from_the_time_agy_gives_the_step() {
    // Also verify the normalizer emits durationMs from agy's duration_seconds.
    let mut norm = AgyNormalizer::default();
    let step_line = json!({
        "event": "step_update",
        "step_update": {
            "step_index": 1,
            "step_type": "agent_response",
            "state": "DONE",
            "duration_seconds": 4.385,
            "usage": {"thinking_tokens": 120}
        }
    })
    .to_string();
    let emitted = norm.accept_line(&step_line);
    let completed = emitted
        .iter()
        .find(|e| e.kind == "step.completed")
        .expect("step.completed emitted");
    assert_eq!(completed.payload["stepKind"], "reasoning");
    assert_eq!(completed.payload["durationMs"], 4385);

    // Both arrive at the same timestamp, as agy sends them: its own timing is the length.
    let same_time = "2026-10-02T12:00:02.000Z";
    let thought = vec![
        agy_event(1, "run.started", json!({})),
        json!({"id": "run:antigravity:2", "runId": "run", "sequence": 2, "occurredAt": same_time, "sourceAdapter": "antigravity", "type": "step.started", "payload": {"stepKind": "reasoning", "itemId": "thought_1"}}),
        json!({"id": "run:antigravity:3", "runId": "run", "sequence": 3, "occurredAt": same_time, "sourceAdapter": "antigravity", "type": "step.completed", "payload": {"stepKind": "reasoning", "itemId": "thought_1", "durationMs": 4385}}),
        agy_event(4, "tool.started", json!({"itemId": "tool_2", "toolKind": "view_file", "name": "view_file", "command": "note.txt", "phase": "started"})),
        agy_event(5, "tool.completed", json!({"itemId": "tool_2", "toolKind": "view_file", "name": "view_file", "command": "note.txt", "phase": "completed"})),
        agy_event(6, "run.completed", json!({})),
    ];
    let t = transcript_of(&thought);
    match &t.items[0] {
        Item::Thought {
            text, duration_ms, ..
        } => {
            assert_eq!(*duration_ms, Some(4385));
            assert_eq!(text, "");
        }
        other => panic!("expected Thought, got {other:?}"),
    }
    let said = drawn_words(&draw_item(t.items[0].clone(), &[]));
    assert!(said.contains("Thought for 4s"), "{said}");
}

// case: reads as a wait on that helper, at the length agy timed, with what its list said under it
#[test]
fn reads_as_a_wait_on_that_helper_at_the_length_agy_timed_with_what_its_list_said_under_it() {
    let waited = vec![
        agy_event(1, "run.started", json!({})),
        agy_event(
            2,
            "step.started",
            json!({"stepKind": "reasoning", "itemId": "thought_624"}),
        ),
        agy_event(
            3,
            "step.completed",
            json!({
                "stepKind": "reasoning",
                "itemId": "thought_624",
                "durationMs": 309_291,
                "message": "**Waiting on its helper Soak & Sweep Runner**\nIts list says the helper is waiting for dependents, with no step since 7:18 AM."
            }),
        ),
        agy_event(
            4,
            "adapter.diagnostic",
            json!({
                "level": "warning",
                "code": "antigravity.rereading",
                "message": "Antigravity has read this whole conversation again 3 times this turn, about 230,000 tokens each, with none of it reused: 691,000 tokens of your quota so far. If it is only waiting, press Stop and send \"continue\" later.",
                "terminal": false
            }),
        ),
        agy_event(5, "run.completed", json!({})),
    ];
    let t = transcript_of(&waited);
    let (thought_id, thought_text, thought_ms) = match &t.items[0] {
        Item::Thought {
            id,
            text,
            duration_ms,
        } => (id.clone(), text.clone(), *duration_ms),
        other => panic!("expected Thought, got {other:?}"),
    };
    assert_eq!(thought_ms, Some(309_291));
    assert_eq!(
        thread_words::thought_headline(&thought_text).as_deref(),
        Some("Waiting on its helper Soak & Sweep Runner")
    );
    let row_words = drawn_words(&draw_item(t.items[0].clone(), &[]));
    assert!(
        row_words.contains("Thought for 5m 09s · Waiting on its helper Soak & Sweep Runner"),
        "{row_words}"
    );
    let group_key = format!("conv/group/{thought_id}");
    let open_words = drawn_words(&draw_group(vec![t.items[0].clone()], &[&group_key]));
    assert!(
        open_words.contains("Its list says the helper is waiting for dependents, with no step since 7:18 AM."),
        "{open_words}"
    );
    let trace = activity_trace(&t.items, &waited, outcome(&waited, false));
    assert!(
        trace.notices.iter().any(|n| n.message.contains("press Stop and send")),
        "{:?}",
        trace.notices
    );
}

// case: a check on a background command reads as one, never as manage_task
#[test]
fn a_check_on_a_background_command_reads_as_one_never_as_manage_task() {
    let events = base_events();
    let t = transcript_of(&events);
    assert_eq!(tool_fold::shown_name("manage_task", "", ""), "Checked on a command");
    let group_key = format!("conv/group/{}", activity_id(&t.items[0]));
    let said = drawn_words(&draw_group(t.items.clone(), &[&group_key]));
    assert!(said.contains("Checked on a command"), "{said}");
    assert!(!said.contains("manage_task"), "{said}");
}

// case: a command the mode refused reads refused, never failed or non-zero
#[test]
fn a_command_the_mode_refused_reads_refused_never_failed_or_non_zero() {
    let events = base_events();
    let t = transcript_of(&events);
    let group_key = format!("conv/group/{}", activity_id(&t.items[0]));
    let shell_key = activity_drawer_key("conv", &t.items[1]);
    let els = draw_group(t.items.clone(), &[&group_key, &shell_key]);
    let badge = els
        .iter()
        .find(|e| e.key.as_deref() == Some("conv/tool_2/badge"))
        .expect("run_command draws as a shell row with a verdict badge");
    let said = drawn_words(&els);
    assert!(said.contains("git status"), "{said}");
    assert!(said.contains("refused"), "{said}");
    assert!(said.contains("Not allowed in this mode."), "{said}");
    assert!(!said.contains("failed"), "{said}");
    assert!(!said.contains("non-zero"), "{said}");
    assert_eq!(badge.key.as_deref(), Some("conv/tool_2/badge"));

    let line = activity_trace(&t.items, &events, outcome(&events, false)).text();
    assert!(line.contains("1 refused"), "{line}");
    assert!(!line.contains("non-zero"), "{line}");
    assert!(!line.contains("ran git status"), "{line}");
}

// case: a refused command did not run, and a runtime note is its sentence, not its code
#[test]
fn a_refused_command_did_not_run_and_a_runtime_note_is_its_sentence_not_its_code() {
    let mut with_note = base_events();
    let end = with_note.pop().unwrap();
    with_note.push(agy_event(
        6,
        "adapter.diagnostic",
        json!({
            "level": "warning",
            "code": "antigravity.denied_actions",
            "message": "It stopped there. Antigravity was not allowed to run a command in this mode.",
            "terminal": false
        }),
    ));
    with_note.push(end);

    let names: Vec<String> = signal_rail::build(&with_note, false)
        .into_iter()
        .map(|row| row.name)
        .collect();
    let joined = names.join(" | ");
    assert!(
        names.iter().any(|n| n == "Did not run git status"),
        "{joined}"
    );
    assert!(!joined.contains("Ran git status"), "{joined}");
    assert!(!joined.contains("antigravity.denied_actions"), "{joined}");
    assert!(
        names.iter().any(|n| n.starts_with("It stopped there.")),
        "{joined}"
    );
}
