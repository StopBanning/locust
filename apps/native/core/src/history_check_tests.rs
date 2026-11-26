//! Cutover 1, row 4: the history check over made-up folders under the test's temp dir.
use super::*;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

const NOW: &str = "2026-11-25T10:00:00.000Z";

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locust-history-check-{tag}-{}-{}",
        std::process::id(),
        crate::native::uuid_v4().unwrap()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn header(id: &str, prompt: &str) -> Value {
    json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {
        "missionId": id, "runId": "run_1", "prompt": prompt, "runtime": "codex", "model": "account-default",
        "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0",
        "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": NOW}})
}

fn event(id: &str) -> Value {
    json!({"schemaVersion": 27, "recordType": "mission.event", "event": {"id": "event_1", "runId": "run_1",
        "missionId": id, "sequence": 1, "type": "message.delta", "occurredAt": NOW, "sourceAdapter": "cursor",
        "payload": {"itemId": "msg_0", "operation": "append", "text": "Hello.", "final": true,
        "evidence": {"redacted": true}}}})
}

fn lines(records: &[Value]) -> String {
    records.iter().map(|r| format!("{r}\n")).collect()
}

/// A good conversation file: a header with a prompt and one reply.
fn good(dir: &Path, id: &str) {
    fs::write(dir.join(format!("{id}.jsonl")), lines(&[header(id, "Look at this."), event(id)])).unwrap();
}

fn found(done: &HistoryCheck, reason_starts: &str) -> Vec<String> {
    done.findings.iter().filter(|(why, _)| why.starts_with(reason_starts)).flat_map(|(_, v)| v.clone()).collect()
}

#[test]
fn a_whole_conversation_is_read_and_reports_nothing() {
    let dir = temp("whole");
    good(&dir, "mission_a");
    good(&dir, "mission_b");
    fs::write(dir.join(".writable"), "").unwrap();
    fs::write(dir.join("notes.txt"), "not a conversation").unwrap();
    let done = check_history(&dir).unwrap();
    assert_eq!(done, HistoryCheck { read_whole: 2, findings: BTreeMap::new() });
    assert_eq!(done.files(), 2);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_cut_off_last_line_and_a_bad_record_are_damaged_not_refused() {
    let dir = temp("damaged");
    let cut = format!("{}{}", lines(&[header("mission_cut", "Hi.")]), r#"{"schemaVersion":27,"recordType":"mission.ev"#);
    fs::write(dir.join("mission_cut.jsonl"), cut).unwrap();
    let bad = format!("{}this is not json\n", lines(&[header("mission_bad", "Hi.")]));
    fs::write(dir.join("mission_bad.jsonl"), bad).unwrap();
    let done = check_history(&dir).unwrap();
    assert_eq!(done.read_whole, 0);
    assert_eq!(found(&done, "damaged: only the part before the fault was read (truncated-tail)"), ["mission_cut.jsonl"]);
    assert_eq!(found(&done, "damaged: only the part before the fault was read (invalid-record)"), ["mission_bad.jsonl"]);
    assert!(found(&done, "refused").is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_file_with_no_readable_header_or_a_newer_schema_is_refused_with_its_reason() {
    let dir = temp("refused");
    fs::write(dir.join("mission_nohead.jsonl"), "not a header\n").unwrap();
    fs::write(dir.join("mission_empty.jsonl"), "").unwrap();
    let mut newer = header("mission_newer", "Hi.");
    newer["schemaVersion"] = json!(99);
    fs::write(dir.join("mission_newer.jsonl"), lines(&[newer])).unwrap();
    let done = check_history(&dir).unwrap();
    assert_eq!(found(&done, "refused: no conversation could be read (invalid-record)"), ["mission_empty.jsonl", "mission_nohead.jsonl"]);
    assert_eq!(found(&done, "refused: no conversation could be read (unsupported-schema)"), ["mission_newer.jsonl"]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_file_named_like_no_conversation_is_refused_for_its_name() {
    let dir = temp("name");
    fs::write(dir.join("bad name.jsonl"), lines(&[header("mission_x", "Hi."), event("mission_x")])).unwrap();
    let done = check_history(&dir).unwrap();
    assert_eq!(found(&done, "refused: the file name is not a conversation id"), ["bad name.jsonl"]);
    assert_eq!(done.read_whole, 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_conversation_with_only_its_opening_prompt_is_listed_as_read_with_zero_turns() {
    let dir = temp("zero");
    fs::write(dir.join("mission_zero.jsonl"), lines(&[header("mission_zero", "Asked, never answered.")])).unwrap();
    good(&dir, "mission_ok");
    let done = check_history(&dir).unwrap();
    assert_eq!(found(&done, "read with zero turns: only the opening prompt"), ["mission_zero.jsonl"]);
    assert_eq!(done.read_whole, 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_trash_is_checked_and_its_findings_say_so() {
    let dir = temp("trash");
    fs::create_dir_all(dir.join(".trash")).unwrap();
    good(&dir, "mission_a");
    good(&dir.join(".trash"), "mission_t");
    fs::write(dir.join(".trash").join("mission_u.jsonl"), "junk\n").unwrap();
    let done = check_history(&dir).unwrap();
    assert_eq!(done.read_whole, 2);
    assert_eq!(
        found(&done, "in .trash: refused: no conversation could be read (invalid-record)"),
        ["mission_u.jsonl"]
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_that_cannot_be_listed_is_an_error_and_nothing_is_written() {
    let dir = temp("missing");
    let before = as_it_is(&dir);
    let err = check_history(&dir.join("nope")).unwrap_err();
    assert!(err.contains("could not be listed"), "{err}");
    assert_eq!(as_it_is(&dir), before);
    let _ = fs::remove_dir_all(&dir);
}

/// Every entry under `dir` with its bytes and modified time.
fn as_it_is(dir: &Path) -> Vec<(PathBuf, Vec<u8>, std::time::SystemTime)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            let modified = fs::symlink_metadata(&path).unwrap().modified().unwrap();
            if path.is_dir() {
                stack.push(path.clone());
                out.push((path, Vec::new(), modified));
            } else {
                out.push((path.clone(), fs::read(&path).unwrap(), modified));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_check_writes_nothing_in_the_folder_it_reads() {
    let dir = temp("readonly");
    fs::create_dir_all(dir.join(".trash")).unwrap();
    good(&dir, "mission_a");
    fs::write(dir.join("mission_cut.jsonl"), format!("{}{{\"cut", lines(&[header("mission_cut", "Hi.")]))).unwrap();
    fs::write(dir.join(".trash").join("mission_u.jsonl"), "junk\n").unwrap();
    let before = as_it_is(&dir);
    let done = check_history(&dir).unwrap();
    assert_eq!(done.files(), 3);
    assert_eq!(as_it_is(&dir), before);
    let _ = fs::remove_dir_all(&dir);
}
