// Carries, case by case where Rust has the behaviour, three of packages/mission-store's record files (Electron
// 4ce2f37a): a-check-result-is-kept-with-its-turn, a-card-answered-is-kept-with-its-turn and
// a-helpers-calls-are-kept-as-its-own. Written by Rust's writers (`ledger_write`), read back as Electron's
// `getMission` reads them (`mission_export::recover_mission_from_path`) and, for what a conversation draws, by the
// window's own reader (`ledger::read_transcript`).
use crate::{
    ledger_write::{LedgerWriter, append_edit_check, append_peer_links, bounded_edit_check},
    mission_export::recover_mission_from_path,
    store::Store,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn temp(tag: &str) -> (Temp, Store) {
    let root = std::env::temp_dir().join(format!("locust-{tag}-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    (Temp(root), store)
}
/// A mission written by the real writer, at the version it writes (27).
fn written(store: &Store, runtime: &str, prompt: &str) -> (LedgerWriter, PathBuf) {
    let writer = LedgerWriter::create_runtime(store, "mission_1", prompt, "account-default", runtime, runtime, &[], None).unwrap();
    let path = writer.path.clone();
    (writer, path)
}
/// A mission whose first record says an older version, written by hand as Electron's tests do.
fn older(store: &Store, schema: u64, records: &[Value]) -> PathBuf {
    let dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mission_1.jsonl");
    let mut lines = vec![json!({ "schemaVersion": schema, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": NOW,
        "metadata": { "missionId": "mission_1", "runId": "run_1", "prompt": "Change notes.txt.", "runtime": "codex", "model": "account-default",
            "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0", "workspaceId": "ws_test",
            "sandbox": "workspace-write", "executionPolicyVersion": 1, "createdAt": NOW } }).to_string()];
    lines.extend(records.iter().map(Value::to_string));
    std::fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();
    path
}
fn recovered(path: &Path) -> Value {
    recover_mission_from_path(path).expect("the mission reads back")
}

const NOW: &str = "2026-09-25T17:00:00.000Z";
fn failed() -> Value {
    json!({ "command": "node check.js", "outcome": "failed", "newLines": ["FAIL: notes.txt says BROKEN"], "unchanged": false, "first": true, "occurredAt": NOW })
}

// --- a-check-result-is-kept-with-its-turn ---

// case: comes back from the file exactly as written
#[test]
fn a_check_result_comes_back_from_the_file_exactly_as_written() {
    let (_t, store) = temp("check");
    let (writer, path) = written(&store, "claude", "Change notes.txt.");
    drop(writer);
    append_edit_check(&path, &failed()).unwrap();
    let mission = recovered(&path);
    assert_eq!(mission["issues"], json!([]));
    assert_eq!(mission["editChecks"], json!([failed()]));
    assert_eq!(mission["ledgerSequence"], 2);
    // What the conversation draws reads it too.
    assert_eq!(crate::ledger::read_transcript(&path).edit_check, Some(failed()));
}

// case: keeps the file walkable after it: a later record still reads
#[test]
fn a_check_result_keeps_the_file_walkable_after_it() {
    let (_t, store) = temp("check");
    let (writer, path) = written(&store, "claude", "Change notes.txt.");
    drop(writer);
    append_edit_check(&path, &failed()).unwrap();
    append_peer_links(&path, &[("posted", "wm_1", "tm_atlas", NOW)]).unwrap();
    let mission = recovered(&path);
    assert_eq!(mission["issues"], json!([]));
    assert_eq!(mission["editChecks"].as_array().unwrap().len(), 1);
    assert_eq!(mission["peerLinks"].as_array().unwrap().len(), 1);
}

// case: is refused on a mission written before version 17, which its own readers would stop at
#[test]
fn a_check_result_is_refused_on_a_mission_written_before_version_17() {
    let (_t, store) = temp("check");
    let path = older(&store, 16, &[]);
    let error = append_edit_check(&path, &failed()).unwrap_err();
    assert!(error.to_string().contains("cannot hold check results"), "{error}");
}

// The other half of "is read as damage in a file written before version 17": what the conversation draws holds no
// check from such a file. Rust's `recover_mission_from_path` does not refuse it (see the tsv row).
#[test]
fn a_check_result_in_a_file_written_before_version_17_is_not_drawn() {
    let (_t, store) = temp("check");
    let path = older(&store, 16, &[json!({ "schemaVersion": 16, "recordType": "mission.edit_check", "ledgerSequence": 2, "occurredAt": NOW, "check": failed() })]);
    assert_eq!(crate::ledger::read_transcript(&path).edit_check, None);
}

// case: refuses an outcome it does not know, and an unbounded list of lines
#[test]
fn a_check_result_with_an_outcome_nobody_defined_or_unbounded_lines_is_refused() {
    let (_t, store) = temp("check");
    let (writer, path) = written(&store, "claude", "Change notes.txt.");
    drop(writer);
    let mut maybe = failed();
    maybe["outcome"] = json!("maybe");
    assert!(append_edit_check(&path, &maybe).is_err());
    let mut many = failed();
    many["newLines"] = json!(vec!["x"; 61]);
    assert!(append_edit_check(&path, &many).is_err());
    // Neither reached the file.
    assert_eq!(recovered(&path)["editChecks"], json!([]));
}

#[test]
fn a_big_check_result_is_bounded_to_what_the_reader_accepts() {
    let (_t, store) = temp("check");
    let (writer, path) = written(&store, "claude", "Change notes.txt.");
    drop(writer);
    let mut huge = failed();
    huge["newLines"] = json!((0..400).map(|i| format!("line {i} {}", "y".repeat(900))).collect::<Vec<_>>());
    append_edit_check(&path, &bounded_edit_check(&huge)).unwrap();
    let mission = recovered(&path);
    assert_eq!(mission["issues"], json!([]));
    let kept = mission["editChecks"][0]["newLines"].as_array().unwrap();
    assert_eq!(kept.len(), 60);
    assert_eq!(kept[0].as_str().unwrap().len(), 500);
    assert!(std::fs::read_to_string(&path).unwrap().len() < 60_000);
}

// case: is read as damage in a file written before version 17
#[test]
fn a_check_result_in_a_file_written_before_17_is_read_as_damage() {
    let (_t, store) = temp("check");
    let path = older(&store, 16, &[json!({ "schemaVersion": 16, "recordType": "mission.edit_check", "ledgerSequence": 2, "occurredAt": NOW, "check": failed() })]);
    let mission = recovered(&path);
    assert_eq!(mission["editChecks"], json!([]));
    assert_eq!(mission["issues"], json!(["invalid-record"]));
    // The same record at 17 is a check.
    let (_t17, store17) = temp("check");
    let path17 = older(&store17, 17, &[json!({ "schemaVersion": 17, "recordType": "mission.edit_check", "ledgerSequence": 2, "occurredAt": NOW, "check": failed() })]);
    assert_eq!(recovered(&path17)["editChecks"], json!([failed()]));
}

// case: bounds a big result to what the reader accepts, so the host never writes one it would refuse
#[test]
fn the_hosts_check_record_is_bounded_to_what_the_reader_accepts() {
    // What native.rs writes after a turn: `edit_check_record_json` of the check's result, appended to the mission.
    let (_t, store) = temp("check");
    let (writer, path) = written(&store, "claude", "Change notes.txt.");
    drop(writer);
    let result = crate::edit_check::EditCheckResult {
        command: "node check.js".into(),
        outcome: crate::edit_check::EditCheckOutcome::Failed,
        new_lines: (0..400).map(|i| format!("line {i} {}", "y".repeat(900))).collect(),
        unchanged: false,
        first: true,
        lines: None,
        before: None,
    };
    append_edit_check(&path, &crate::edit_check::edit_check_record_json(&result, NOW)).unwrap();
    let mission = recovered(&path);
    assert_eq!(mission["issues"], json!([]));
    let kept = mission["editChecks"][0]["newLines"].as_array().unwrap();
    assert_eq!(kept.len(), 60);
    assert_eq!(kept[0].as_str().unwrap().len(), 500);
    assert!(std::fs::read_to_string(&path).unwrap().len() < 60_000);
}

// --- a-card-answered-is-kept-with-its-turn ---

fn allowed() -> Value {
    json!({ "approvalId": "ap_1", "kind": "command", "asked": "Run a command\nnpm test", "answer": "allowed", "by": "card",
        "askedAt": "2026-10-04T00:59:50.000Z", "occurredAt": "2026-10-04T01:00:00.000Z" })
}
fn denied_by_rule() -> Value {
    json!({ "approvalId": "ap_2", "kind": "command", "asked": "Run a command\ngit push origin main", "answer": "denied", "by": "saved-rule",
        "words": "never run git push for Wren in this folder.", "askedAt": "2026-10-04T01:00:10.000Z", "occurredAt": "2026-10-04T01:00:11.000Z" })
}
fn by_earlier_always() -> Value {
    json!({ "approvalId": "ap_3", "kind": "command", "asked": "Run a command\necho two", "answer": "allowed", "by": "earlier-always",
        "words": "The Always given earlier in this run allows every command it runs.", "askedAt": "2026-10-04T01:00:20.000Z", "occurredAt": "2026-10-04T01:00:20.000Z" })
}

// case: comes back from the file exactly as written, in order, with the schema version that holds it
#[test]
fn a_card_answered_comes_back_from_the_file_exactly_as_written_in_order() {
    let (_t, store) = temp("approval");
    let (mut writer, path) = written(&store, "codex", "Tidy the build folder.");
    writer.append_approval(&allowed()).unwrap();
    writer.append_approval(&denied_by_rule()).unwrap();
    drop(writer);
    let mission = recovered(&path);
    assert_eq!(mission["approvals"], json!([allowed(), denied_by_rule()]));
    assert_eq!(mission["schemaVersion"], 27);
    assert_eq!(mission["issues"], json!([]));
}

// case: sits between a run's events without disturbing their numbering
#[test]
fn a_card_answered_sits_between_a_runs_events_without_disturbing_their_numbering() {
    let (_t, store) = temp("approval");
    let (mut writer, path) = written(&store, "codex", "Tidy the build folder.");
    let said = || json!({ "itemId": "answer", "operation": "append", "text": "hi", "final": false, "evidence": { "redacted": true } });
    writer.append("message.delta", said(), None).unwrap();
    writer.append_approval(&allowed()).unwrap();
    writer.append("message.delta", said(), None).unwrap();
    drop(writer);
    let mission = recovered(&path);
    let sequences: Vec<_> = mission["events"].as_array().unwrap().iter().map(|e| e["sequence"].clone()).collect();
    assert_eq!(sequences, [json!(1), json!(2)]);
    assert_eq!(mission["approvals"].as_array().unwrap().len(), 1);
    assert_eq!(mission["issues"], json!([]));
}

// case: refuses an answer or an answerer it does not know
#[test]
fn a_card_answer_or_answerer_nobody_defined_is_refused() {
    let (_t, store) = temp("approval");
    let (mut writer, path) = written(&store, "codex", "Tidy the build folder.");
    let mut maybe = allowed();
    maybe["answer"] = json!("maybe");
    assert_eq!(writer.append_approval(&maybe).unwrap_err().to_string(), "Approval answer is invalid");
    let mut teammate = allowed();
    teammate["by"] = json!("the teammate");
    assert_eq!(writer.append_approval(&teammate).unwrap_err().to_string(), "Approval answerer is invalid");
    drop(writer);
    assert_eq!(recovered(&path)["approvals"], json!([]));
}

#[test]
fn an_answer_by_an_earlier_always_is_kept_beside_the_card_that_gave_it() {
    let (_t, store) = temp("approval");
    let (mut writer, path) = written(&store, "codex", "Tidy the build folder.");
    let mut always = allowed();
    always["answer"] = json!("allowed-always");
    writer.append_approval(&always).unwrap();
    writer.append_approval(&by_earlier_always()).unwrap();
    drop(writer);
    let mission = recovered(&path);
    let by: Vec<_> = mission["approvals"].as_array().unwrap().iter().map(|a| a["by"].clone()).collect();
    assert_eq!(by, [json!("card"), json!("earlier-always")]);
    assert_eq!(mission["issues"], json!([]));
}

// --- a-helpers-calls-are-kept-as-its-own ---

fn tool(sequence: u64, kind: &str, item: &str, name: &str, extra: Value) -> Value {
    let mut payload = json!({ "itemId": item, "toolKind": "tool_use", "name": name,
        "phase": if kind == "tool.started" { "started" } else { "completed" }, "evidence": { "redacted": true } });
    for (k, v) in extra.as_object().unwrap() {
        payload[k] = v.clone();
    }
    json!({ "id": format!("run_1:claude:{sequence}"), "runId": "run_1", "missionId": "mission_1", "sequence": sequence, "occurredAt": NOW,
        "sourceAdapter": "claude", "type": kind, "payload": payload })
}
fn children_of(events: &Value) -> Vec<String> {
    events.as_array().unwrap().iter().filter_map(|e| {
        let parent = e["payload"]["parentItemId"].as_str()?;
        Some(format!("{} {} under {parent}", e["type"].as_str().unwrap(), e["payload"]["itemId"].as_str().unwrap()))
    }).collect()
}

// case: reads a ledger written before the bump whole, with no child rows and no error
#[test]
fn a_ledger_written_before_v22_reads_whole_with_no_child_rows() {
    let (_t, store) = temp("helper");
    let helper_only = [
        tool(1, "tool.started", "toolu_agent", "Agent", json!({ "command": "Count the files" })),
        tool(2, "tool.completed", "toolu_agent", "Agent", json!({ "command": "Count the files", "status": "Explore", "output": "There are 7 files." })),
    ];
    let records: Vec<Value> = helper_only.iter().enumerate().map(|(i, event)|
        json!({ "schemaVersion": 21, "recordType": "mission.event", "ledgerSequence": i + 2, "occurredAt": NOW, "event": event })).collect();
    let path = older(&store, 21, &records);
    let mission = recovered(&path);
    assert_eq!(mission["issues"], json!([]));
    assert_eq!(mission["events"].as_array().unwrap().len(), 2);
    assert!(children_of(&mission["events"]).is_empty());
}

#[test]
fn a_helpers_calls_are_written_at_27_and_read_back_under_the_helpers_row() {
    let (_t, store) = temp("helper");
    let (mut writer, path) = written(&store, "claude", "Use a helper to count the files here.");
    for event in [
        tool(1, "tool.started", "toolu_agent", "Agent", json!({ "command": "Count the files" })),
        tool(2, "tool.started", "toolu_glob", "Glob", json!({ "command": "**/*", "parentItemId": "toolu_agent" })),
        tool(3, "tool.completed", "toolu_glob", "Glob", json!({ "command": "**/*", "parentItemId": "toolu_agent", "output": "a.txt\nb.txt" })),
        tool(4, "tool.completed", "toolu_agent", "Agent", json!({ "command": "Count the files", "status": "Explore", "output": "There are 7 files." })),
    ] {
        writer.append(event["type"].as_str().unwrap(), event["payload"].clone(), None).unwrap();
    }
    drop(writer);
    let mission = recovered(&path);
    assert_eq!(mission["schemaVersion"], 27);
    assert_eq!(mission["issues"], json!([]));
    assert_eq!(children_of(&mission["events"]), ["tool.started toolu_glob under toolu_agent", "tool.completed toolu_glob under toolu_agent"]);
}

// Rust's own: the rest of mission-store's validateEditCheck and validateApproval, which no Electron case reaches.
#[test]
fn a_check_result_the_reader_would_refuse_in_any_field_is_never_written() {
    let (_t, store) = temp("check");
    let (writer, path) = written(&store, "claude", "Change notes.txt.");
    drop(writer);
    let refused = |field: &str, value: Value| {
        let mut check = failed();
        check[field] = value;
        crate::ledger_write::validate_edit_check(&check).unwrap_err().to_string()
    };
    assert_eq!(refused("command", json!("   ")), "Edit check command is invalid");
    assert_eq!(refused("command", json!("c".repeat(501))), "Edit check command is invalid");
    assert_eq!(refused("newLines", json!(["y".repeat(501)])), "Edit check line is invalid");
    assert_eq!(refused("newLines", json!([7])), "Edit check line is invalid");
    assert_eq!(refused("unchanged", json!("no")), "Edit check flags are invalid");
    assert_eq!(refused("first", Value::Null), "Edit check flags are invalid");
    assert_eq!(refused("occurredAt", json!("yesterday")), "Edit check timestamp is invalid");
    let mut fits = failed();
    fits["newLines"] = json!(vec!["y".repeat(500); 60]);
    append_edit_check(&path, &fits).unwrap();
}

#[test]
fn a_card_answer_the_reader_would_refuse_in_any_field_is_never_written() {
    let refused = |field: &str, value: Value| {
        let mut approval = allowed();
        approval[field] = value;
        crate::ledger_write::validate_approval(&approval).unwrap_err().to_string()
    };
    assert_eq!(refused("approvalId", json!("a".repeat(201))), "Approval id is invalid");
    assert_eq!(refused("kind", json!("anything")), "Approval kind is invalid");
    assert_eq!(refused("asked", json!("q".repeat(2_001))), "Approval question is invalid");
    assert_eq!(refused("words", json!("w".repeat(1_001))), "Approval words are invalid");
    assert_eq!(refused("words", Value::Null), "Approval words are invalid");
    assert_eq!(refused("askedAt", json!("soon")), "Approval asked timestamp is invalid");
    assert_eq!(refused("occurredAt", json!("")), "Approval timestamp is invalid");
    // Colin, 17 Nov: kept as Electron. A refusal by the hard command guard is no card answer (Electron records none for
    // its guard), so the answerer `command-guard` that approval.rs gives it is refused and leaves no line in the record.
    assert_eq!(refused("by", json!("command-guard")), "Approval answerer is invalid");
    for kind in ["command", "file-change", "question", "connector"] {
        let mut approval = denied_by_rule();
        approval["kind"] = json!(kind);
        approval["words"] = json!("w".repeat(1_000));
        crate::ledger_write::validate_approval(&approval).unwrap();
    }
}

// Rust's own: boundedEditCheck cuts the command to the 500 the reader takes, as it cuts each line.
#[test]
fn a_check_results_long_command_is_cut_to_what_the_reader_accepts() {
    let mut long = failed();
    long["command"] = json!("c".repeat(600));
    let bounded = bounded_edit_check(&long);
    assert_eq!(bounded["command"].as_str().unwrap().len(), 500);
    crate::ledger_write::validate_edit_check(&bounded).unwrap();
}

#[path = "mission_card_schema_tests.rs"]
mod mission_card_schema_tests;
