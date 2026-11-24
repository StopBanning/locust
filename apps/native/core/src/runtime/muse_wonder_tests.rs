//! Electron's `packages/runtime-adapters/test/a-real-muse-wonder-run-reads-whole.test.ts`, through the caller: the Muse
//! runtime (muse.rs) parses each line of `muse exec --json` (a byte-order mark dropped) and hands the record to
//! `MuseNormalizer::accept`, then asks `finish` for the end. The capture is the first paid Muse Code run Locust read:
//! `test/fixtures/muse/wonder-tool-run-1.4.6.jsonl`, Muse Code 1.4.6 on Muse Wonder 1.0, 2026-10-12.
use super::*;

fn captured() -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/runtime-adapters/test/fixtures/muse/wonder-tool-run-1.4.6.jsonl");
    std::fs::read_to_string(path).unwrap().lines().map(|l| l.trim_end_matches('\r').to_owned()).filter(|l| !l.is_empty()).collect()
}

fn run(lines: &[String]) -> Vec<Event> {
    let mut muse = MuseNormalizer::default();
    let mut events = vec![];
    for line in lines {
        let record: Value = serde_json::from_str(line.trim_start_matches('\u{feff}')).unwrap();
        events.extend(muse.accept(&record));
    }
    events.extend(muse.finish(false, Some(0)));
    events
}

/// A task record shaped as the capture's are.
fn task(task_id: &str, event: Value) -> String {
    let kind = event["kind"].as_str().unwrap().to_owned();
    let mut whole = json!({"task_id": task_id});
    for (k, v) in event.as_object().unwrap() {
        whole[k] = v.clone();
    }
    json!({"schema_version":1,"record_type":"event","payload_type":format!("task.lifecycle.{kind}"),
        "payload":{"kind":"task_lifecycle","task_id":task_id,"event":whole}})
    .to_string()
}

fn said(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e.kind == "adapter.diagnostic")
        .map(|e| format!("{}: {}", e.payload["code"].as_str().unwrap_or(""), e.payload["message"].as_str().unwrap_or("")))
        .collect()
}

#[test]
fn reads_whole_its_tool_by_name_its_answer_its_end_and_nothing_left_unread() {
    // case: reads whole: its tool by name, its answer, its end, and nothing left unread
    let events = run(&captured());
    assert_eq!(said(&events), Vec::<String>::new());
    let tools: Vec<&str> = events.iter().filter(|e| e.kind == "tool.started").map(|e| e.payload["name"].as_str().unwrap()).collect();
    assert_eq!(tools, ["search", "search", "search"]);
    assert_eq!(events.iter().filter(|e| e.kind == "tool.completed").count(), 3);
    let last = events.iter().find(|e| e.kind == "message.delta" && e.payload["final"] == true).unwrap();
    assert!(last.payload["text"].as_str().unwrap().starts_with("There are 2 files in this folder"));
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn says_once_that_metas_model_is_being_tried_again_and_nothing_for_its_first_try() {
    // case: says once that Meta's model is being tried again, and nothing for its first try
    let status = |attempt: u64| {
        task("model_1", json!({"kind":"status","message":format!("opening meta model stream attempt {attempt}/10"),
            "details":{"phase":"opening_stream","facets":[{"kind":"external_attempt","system":"meta","operation":"model.response","attempt":attempt,"max_attempts":10}]}}))
    };
    let events = run(&[task("model_1", json!({"kind":"proposed","task_kind":"model.meta.response"})), status(1), status(2), status(2), status(3)]);
    assert_eq!(
        said(&events),
        [
            "muse.provider_busy: Meta's model did not answer at once, and Muse Code is trying again on its own (attempt 2 of 10).",
            "muse.provider_busy: Meta's model did not answer at once, and Muse Code is trying again on its own (attempt 3 of 10).",
        ]
    );
}

#[test]
fn lets_the_sessions_bookkeeping_go_quietly_the_branch_it_saw_in_a_git_folder_among_it() {
    // case: lets the session's bookkeeping go quietly, the branch it saw in a git folder among it
    let branch = json!({"schema_version":1,"record_type":"event","payload_type":"session.workspace_branch.observed",
        "payload":{"kind":"session_workspace_branch_observed","branch":"main"}})
    .to_string();
    assert_eq!(said(&run(&[branch])), Vec::<String>::new());
}

#[test]
fn fails_a_tool_it_would_not_run_on_that_tools_row_and_lets_its_own_reminder_go_quietly() {
    // case: fails a tool it would not run on that tool's row, and lets its own reminder go quietly
    let events = run(&[
        task("tool_1", json!({"kind":"proposed","task_kind":"tool.edit"})),
        task("tool_1", json!({"kind":"rejected","reason":"policy_denied"})),
        task("reminder_1", json!({"kind":"proposed","task_kind":"reminder.agent.skill-reminder"})),
        task("reminder_1", json!({"kind":"rejected","reason":"skip_if_running"})),
    ]);
    let failed: Vec<&Value> = events.iter().filter(|e| e.kind == "tool.failed").map(|e| &e.payload).collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["name"], "edit");
    assert_eq!(failed[0]["status"], "Muse Code did not run it (policy_denied)");
    assert_eq!(said(&events), Vec::<String>::new());
}
