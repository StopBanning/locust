//! `what-was-said-while-working-stays-where-it-was-said.test.ts` (9 cases): a turn is drawn in the order it happened --
//! each thing said, then the steps taken before the next thing said as one line -- while it runs and after, with nothing
//! moving when it ends. The events go through the ledger's own reader (`Transcript::apply`) and the thread's pieces
//! (`steps_fold::thread_pieces`), as the window draws them.
use crate::ledger::{Item, Transcript};
use crate::steps_fold::{thread_pieces, Piece};
use serde_json::{json, Value};

struct Events {
    sequence: u64,
    list: Vec<Value>,
}

impl Events {
    fn new() -> Self {
        Events { sequence: 0, list: Vec::new() }
    }
    fn event(&mut self, kind: &str, payload: Value) -> &mut Self {
        self.sequence += 1;
        let mut payload = payload;
        payload["evidence"] = json!({ "redacted": false });
        self.list.push(json!({ "event": {
            "id": format!("evt_{}", self.sequence), "runId": "run_1", "missionId": "mission_1", "sequence": self.sequence,
            "occurredAt": "2026-09-23T12:00:00.000Z", "sourceAdapter": "opencode", "type": kind, "payload": payload,
        } }));
        self
    }
    fn started(&mut self) -> &mut Self {
        self.event("run.started", json!({ "runtimeThreadId": "thread-1" }))
    }
    fn said_as(&mut self, id: &str, text: &str, operation: &str) -> &mut Self {
        self.event("message.delta", json!({ "itemId": id, "operation": operation, "text": text, "final": true }))
    }
    fn said(&mut self, id: &str, text: &str) -> &mut Self {
        self.said_as(id, text, "append")
    }
    fn tool_start(&mut self, id: &str, name: &str, command: &str) -> &mut Self {
        self.event("tool.started", json!({ "itemId": id, "toolKind": "command_execution", "name": name, "phase": "started", "command": command }))
    }
    fn tool_done(&mut self, id: &str, name: &str, command: &str) -> &mut Self {
        self.event("tool.completed", json!({ "itemId": id, "toolKind": "command_execution", "name": name, "phase": "completed", "exitCode": 0, "command": command }))
    }
    fn completed(&mut self) -> &mut Self {
        self.event("run.completed", json!({ "process": { "exitCode": 0, "signal": null, "stderr": "", "stderrTruncated": false, "recordCount": 3 } }))
    }
    fn items(&self) -> Vec<Item> {
        let mut transcript = Transcript::default();
        transcript.apply(self.list.clone());
        transcript.items
    }
}

/// Yurt's walkthrough, as the runtime sent it.
fn walkthrough(end: bool) -> Events {
    let mut e = Events::new();
    e.started()
        .said("m1", "Creating your HELLO file and lining up verification.")
        .tool_start("t1", "write", "hello.txt")
        .tool_done("t1", "write", "hello.txt")
        .said("m2", "File write is underway \u{2014} then I'll print it back to confirm.")
        .tool_start("t2", "shell", "cat hello.txt")
        .tool_done("t2", "shell", "cat hello.txt")
        .said("m3", "DONE");
    if end {
        e.completed();
    }
    e
}

/// The turn as a reader meets it: words, steps lines by their rows' names, and the foot once the turn has ended.
fn shape(items: &[Item], running: bool) -> Vec<String> {
    thread_pieces(items, running)
        .into_iter()
        .filter_map(|piece| match piece {
            Piece::Item(i) => match &items[i] {
                Item::Assistant { text, .. } => Some(format!("said: {text}")),
                Item::Trace(_) if !running => Some("foot".into()),
                _ => None,
            },
            Piece::Steps { group, .. } => Some(format!(
                "steps: {}",
                group.items.iter().map(|&i| match &items[i] { Item::Tool { summary, .. } => summary.clone(), _ => String::new() }).collect::<Vec<_>>().join(", ")
            )),
        })
        .collect()
}

// case: reads in the order it happened: each thing said, then the steps before the next
#[test]
fn reads_in_the_order_it_happened_each_thing_said_then_the_steps_before_the_next() {
    assert_eq!(
        shape(&walkthrough(true).items(), false),
        [
            "said: Creating your HELLO file and lining up verification.",
            "steps: hello.txt",
            "said: File write is underway \u{2014} then I'll print it back to confirm.",
            "steps: cat hello.txt",
            "said: DONE",
            "foot",
        ]
    );
}

// case: places a message where it BEGAN, so a full record restated after the next step leaves it before it (gap 9)
#[test]
fn places_a_message_where_it_began_so_a_full_record_restated_after_the_next_step_leaves_it_before_it() {
    let mut e = Events::new();
    e.event("run.started", json!({}))
        .said_as("m1", "Checking the ", "append")
        .tool_start("t1", "shell", "ls")
        .said_as("m1", "Checking the folder first.", "replace")
        .tool_done("t1", "shell", "ls")
        .said("m2", "It is empty.")
        .completed();
    assert_eq!(shape(&e.items(), false), ["said: Checking the folder first.", "steps: ls", "said: It is empty.", "foot"]);
}

// case: draws a stopped turn’s last words where they were said, never below its steps as an answer
#[test]
fn draws_a_stopped_turns_last_words_where_they_were_said_never_below_its_steps_as_an_answer() {
    for end in ["run.failed", "run.cancelled", ""] {
        let mut e = Events::new();
        e.event("run.started", json!({}))
            .said("m1", "Writing the file now.")
            .tool_start("t1", "write", "a.txt")
            .tool_done("t1", "write", "a.txt")
            .said("m2", "Let me check it was written.")
            .tool_start("t2", "read", "a.txt");
        if !end.is_empty() {
            e.event(end, json!({ "reason": "x" }));
        }
        let drawn = shape(&e.items(), false);
        assert_eq!(drawn[drawn.len() - 2], "steps: a.txt", "{end}: {drawn:?}");
        let said = drawn.iter().position(|d| d == "said: Let me check it was written.").unwrap();
        let last_steps = drawn.iter().rposition(|d| d == "steps: a.txt").unwrap();
        assert!(said < last_steps, "{end}: {drawn:?}");
    }
}

// case: puts a row the host added after the run ended in no group: it is in the turn’s files at the foot
#[test]
fn puts_a_row_the_host_added_after_the_run_ended_in_no_group_it_is_in_the_turns_files_at_the_foot() {
    let mut e = Events::new();
    e.event("run.started", json!({})).said("m1", "All written.").completed().tool_start("obs", "edit", "a.txt");
    let items = e.items();
    assert_eq!(shape(&items, false), ["said: All written.", "foot"]);
    // The foot's rows are every row of the turn, the host's included.
    let foot_rows: Vec<&str> = items.iter().filter_map(|i| match i { Item::Tool { summary, .. } => Some(summary.as_str()), _ => None }).collect();
    assert_eq!(foot_rows, ["a.txt"]);
    assert!(items.iter().any(|i| matches!(i, Item::Trace(_))));
}

// case: does not split the steps on a message with nothing on screen
#[test]
fn does_not_split_the_steps_on_a_message_with_nothing_on_screen() {
    let mut e = Events::new();
    e.event("run.started", json!({}))
        .tool_start("t1", "shell", "ls")
        .tool_done("t1", "shell", "ls")
        .said("m0", "   ")
        .tool_start("t2", "shell", "pwd")
        .tool_done("t2", "shell", "pwd")
        .said("m1", "Done.")
        .completed();
    assert_eq!(shape(&e.items(), false), ["steps: ls, pwd", "said: Done.", "foot"]);
}

// case: counts nothing it said as a tool call
#[test]
fn counts_nothing_it_said_as_a_tool_call() {
    let items = walkthrough(true).items();
    let Some(Item::Trace(trace)) = items.iter().find(|i| matches!(i, Item::Trace(_))) else { panic!("no foot") };
    let line = trace.text();
    assert!(!line.contains("tool call"), "{line}");
    assert!(!line.contains("4 tool calls") && !line.contains("3 tool calls"), "{line}");
    // What it ran is named as what it ran; the words it said are not in the count.
    assert!(line.contains("ran cat hello.txt"), "{line}");
}

// case: leaves a turn that only talked exactly as it was
#[test]
fn leaves_a_turn_that_only_talked_exactly_as_it_was() {
    let mut e = Events::new();
    e.event("run.started", json!({})).said("m1", "Hello.").said("m2", "Goodbye.").completed();
    assert_eq!(shape(&e.items(), false), ["said: Hello.", "said: Goodbye."]);
}

// case: draws the same order while it runs, so nothing moves when it ends
#[test]
fn draws_the_same_order_while_it_runs_so_nothing_moves_when_it_ends() {
    let live = shape(&walkthrough(false).items(), true);
    let mut ended = shape(&walkthrough(true).items(), false);
    ended.pop();
    assert_eq!(live, ended);
}

// case: leaves a step still going to the live line, and puts it in its group once it ends
#[test]
fn leaves_a_step_still_going_to_the_live_line_and_puts_it_in_its_group_once_it_ends() {
    let mut e = Events::new();
    e.event("run.started", json!({})).said("m1", "Looking.").tool_start("t1", "shell", "ls").tool_done("t1", "shell", "ls").tool_start("t2", "shell", "npm test");
    let before = e.items();
    assert_eq!(shape(&before, true), ["said: Looking.", "steps: ls"]);
    let after_events = { e.tool_done("t2", "shell", "npm test"); e.items() };
    assert_eq!(shape(&after_events, true), ["said: Looking.", "steps: ls, npm test"]);
    // The same group, grown: it keeps its place (the run it began), so an opened group stays open.
    let place = |items: &[Item]| thread_pieces(items, true).into_iter().find_map(|p| match p { Piece::Steps { run, group } => Some((run, group.items[0])), _ => None });
    assert_eq!(place(&before), place(&after_events));
    assert!(place(&before).is_some());
}
