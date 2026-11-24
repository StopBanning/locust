// Carries the reconciliation cases of packages/mission-store/test/checkpoint.test.ts (Electron 4ce2f37a) onto
// `carry_on::reconcile`, Rust's `reconcileMission`. Electron makes its events with its Codex `exec --json` reader, which
// Rust does not have (Rust runs Codex through its app server), so each event here is the one that reader emits for the
// same record, written out: `item.started` of a `command_execution` is a `tool.started` named `shell`, an agent message
// that grows is a `replace` and then an `append` of what was added.
use crate::carry_on::reconcile;
use serde_json::{Value, json};

const NOW: &str = "2026-08-31T16:00:00.000Z";
const CHECKPOINT_AT: &str = "2026-08-31T16:05:00.000Z";

fn metadata() -> Value {
    json!({ "missionId": "mission_1", "runId": "run_1", "prompt": "Inspect the workspace without changing it.", "runtime": "codex",
        "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0-alpha.7.2",
        "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": NOW })
}
fn event(sequence: u64, kind: &str, payload: Value) -> Value {
    json!({ "id": format!("run_1:codex:{sequence}"), "runId": "run_1", "missionId": "mission_1", "sequence": sequence, "occurredAt": NOW,
        "sourceAdapter": "codex", "runtimeThreadId": "thread_1", "type": kind, "payload": payload })
}
fn started(command: &str) -> Value {
    event(2, "tool.started", json!({ "itemId": "call_1", "toolKind": "command_execution", "name": "shell", "command": command, "phase": "started" }))
}
/// A tool call that starts and reports an outcome: nothing is in doubt.
fn settled() -> Vec<Value> {
    vec![
        event(1, "run.started", json!({ "runtimeThreadId": "thread_1" })),
        started("ls"),
        event(3, "tool.completed", json!({ "itemId": "call_1", "toolKind": "command_execution", "name": "shell", "command": "ls", "exitCode": 0, "phase": "completed", "status": "completed" })),
        event(4, "message.delta", json!({ "itemId": "answer", "operation": "replace", "text": "Durable result", "final": true })),
    ]
}
/// A tool call that starts and never reports: the case the checkpoint exists for.
fn unsettled() -> Vec<Value> {
    vec![event(1, "run.started", json!({ "runtimeThreadId": "thread_1" })), started("send-invoice")]
}
fn stub(sequence: u64, id: &str, kind: &str) -> Value {
    json!({ "id": id, "runId": "run_1", "missionId": "mission_1", "sequence": sequence, "occurredAt": NOW, "sourceAdapter": "codex",
        "type": kind, "payload": { "stepKind": "turn", "evidence": {} } })
}
fn digest(events: &[Value]) -> String {
    reconcile(&metadata(), events, false, "manual", 1, CHECKPOINT_AT)["transcriptDigest"].as_str().unwrap().to_owned()
}

#[test]
fn a_settled_mission_is_reported_safe_to_resume_elsewhere() {
    let checkpoint = reconcile(&metadata(), &settled(), false, "route-limit", 1, CHECKPOINT_AT);
    for (key, value) in [
        ("missionId", json!("mission_1")), ("runId", json!("run_1")), ("epoch", json!(1)), ("reason", json!("route-limit")),
        ("resumeSafety", json!("safe")), ("settledActions", json!(["call_1"])), ("unsettledActions", json!([])),
        ("assistantSummary", json!("Durable result")), ("runtimeThreadId", json!("thread_1")), ("createdAt", json!(CHECKPOINT_AT)),
    ] {
        assert_eq!(checkpoint[key], value, "{key}");
    }
    assert_eq!(checkpoint["reconciledThroughSequence"], settled().len());
    let digest = checkpoint["transcriptDigest"].as_str().unwrap();
    assert!(digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)), "{digest}");
}

#[test]
fn a_mission_is_never_called_safe_while_an_action_never_reported_an_outcome() {
    let checkpoint = reconcile(&metadata(), &unsettled(), false, "route-limit", 1, CHECKPOINT_AT);
    assert_eq!(checkpoint["resumeSafety"], "approval-required");
    assert_eq!(checkpoint["settledActions"], json!([]));
    assert_eq!(checkpoint["unsettledActions"], json!([{ "itemId": "call_1", "toolKind": "command_execution", "name": "shell", "startedAt": NOW, "startedAtSequence": 2 }]));
    assert!(checkpoint["safetyReason"].as_str().unwrap().contains("never reported an outcome"));
}

// The checkpoint half of "reports an unreadable ledger as unsafe": a ledger read short of its end.
#[test]
fn a_ledger_read_short_of_its_end_is_unsafe() {
    let checkpoint = reconcile(&metadata(), &settled(), true, "route-limit", 1, CHECKPOINT_AT);
    assert_eq!(checkpoint["resumeSafety"], "unsafe");
    assert!(checkpoint["safetyReason"].as_str().unwrap().contains("incomplete"));
}

#[test]
fn an_appended_message_is_rebuilt_rather_than_kept_as_its_last_fragment() {
    let events = [
        event(1, "run.started", json!({ "runtimeThreadId": "thread_1" })),
        event(2, "message.delta", json!({ "itemId": "answer", "operation": "replace", "text": "Part one. ", "final": false })),
        event(3, "message.delta", json!({ "itemId": "answer", "operation": "append", "text": "Part two.", "final": true })),
    ];
    assert_eq!(reconcile(&metadata(), &events, false, "manual", 1, CHECKPOINT_AT)["assistantSummary"], "Part one. Part two.");
}

#[test]
fn different_transcripts_get_different_digests() {
    let a = [stub(1, "call_1", "step.started"), stub(2, "call_2", "step.started")];
    let different_ids = [stub(1, "call_1", "step.started"), stub(2, "call_9", "step.started")];
    let different_types = [stub(1, "call_1", "step.started"), stub(2, "call_2", "step.completed")];
    assert_ne!(digest(&a), digest(&different_ids));
    assert_ne!(digest(&a), digest(&different_types));
    assert_eq!(digest(&a), digest(&a.clone()));
    // The delimiter: without it sequence 1 with id "2a" and sequence 12 with id "a" hash the same bytes.
    assert_ne!(digest(&[stub(1, "2a", "step.started")]), digest(&[stub(12, "a", "step.started")]));
}

// --- on disk: Rust's createCheckpoint is `reconcile` over what `ledger::Tail` reads, then `append_checkpoint`. ---

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn seeded() -> (Temp, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("locust-checkpoint-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let mut writer = crate::ledger_write::LedgerWriter::create_runtime(&store, "mission_1", "Inspect the workspace without changing it.", "account-default", "codex", "codex", &[], None).unwrap();
    for e in settled() {
        writer.append(e["type"].as_str().unwrap(), e["payload"].clone(), Some("thread_1")).unwrap();
    }
    let path = writer.path.clone();
    (Temp(root), path)
}
/// What a stopped turn's checkpoint is made from: the file as it reads now, and whether every line of it read.
fn checkpoint_of(path: &std::path::Path, reason: &str, epoch: u64, at: &str) -> Value {
    let lines = std::fs::read_to_string(path).unwrap().lines().filter(|l| !l.trim().is_empty()).count();
    let records = crate::ledger::Tail::new(path, 0).poll();
    let metadata = records.iter().find(|r| r["recordType"] == "mission.created").unwrap()["metadata"].clone();
    let events: Vec<Value> = records.iter().filter_map(|r| r.get("event").cloned()).collect();
    reconcile(&metadata, &events, records.len() < lines, reason, epoch, at)
}

#[test]
fn a_checkpoint_survives_a_fresh_reader_and_keeps_its_epoch_order() {
    let (_t, path) = seeded();
    for (epoch, reason, at) in [(1, "route-limit", "2026-08-31T16:05:00.000Z"), (2, "manual", "2026-08-31T16:06:00.000Z")] {
        crate::ledger_write::append_checkpoint(&path, &checkpoint_of(&path, reason, epoch, at)).unwrap();
    }
    let mission = crate::mission_export::recover_mission_from_path(&path).unwrap();
    assert_eq!(mission["issues"], json!([]));
    assert_eq!(mission["phase"], "interrupted");
    let order: Vec<_> = mission["checkpoints"].as_array().unwrap().iter().map(|c| (c["epoch"].clone(), c["reason"].clone())).collect();
    assert_eq!(order, [(json!(1), json!("route-limit")), (json!(2), json!("manual"))]);
    // Checkpoints take a ledger sequence, not an event sequence: the transcript is unchanged by them.
    assert_eq!(mission["events"].as_array().unwrap().len(), settled().len());
}

#[test]
fn a_checkpoint_describes_what_survived_on_disk_after_an_interrupted_write() {
    let (_t, path) = seeded();
    let text = std::fs::read_to_string(&path).unwrap();
    let kept: Vec<&str> = text.lines().filter(|l| !l.is_empty()).take(3).collect();
    std::fs::write(&path, format!("{}\n", kept.join("\n"))).unwrap();
    let checkpoint = checkpoint_of(&path, "route-limit", 1, CHECKPOINT_AT);
    assert_eq!(checkpoint["reconciledThroughSequence"], 2);
    assert_eq!(checkpoint["resumeSafety"], "approval-required");
    let open: Vec<_> = checkpoint["unsettledActions"].as_array().unwrap().iter().map(|a| a["itemId"].clone()).collect();
    assert_eq!(open, [json!("call_1")]);
}

#[test]
fn a_record_the_reader_refuses_makes_the_checkpoint_unsafe() {
    let (_t, path) = seeded();
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("{\"not\":\"a record\"}\n");
    std::fs::write(&path, text).unwrap();
    let checkpoint = checkpoint_of(&path, "route-limit", 1, CHECKPOINT_AT);
    assert_eq!(checkpoint["resumeSafety"], "unsafe");
    assert!(checkpoint["safetyReason"].as_str().unwrap().contains("incomplete"));
}

// case: reports an unreadable ledger as unsafe, and refuses to write past the break
#[test]
fn an_unreadable_ledger_is_unsafe_and_its_checkpoint_is_not_written_past_the_break() {
    let (_t, path) = seeded();
    let mut text = std::fs::read_to_string(&path).unwrap();
    // A record the reader refuses stops recovery there, so the events are a prefix of what happened.
    text.push_str("{\"not\":\"a record\"}\n");
    std::fs::write(&path, &text).unwrap();
    let before = std::fs::read(&path).unwrap();
    let checkpoint = checkpoint_of(&path, "route-limit", 1, CHECKPOINT_AT);
    assert_eq!(checkpoint["resumeSafety"], "unsafe");
    assert!(checkpoint["safetyReason"].as_str().unwrap().contains("incomplete"));
    // The app's writer (what a route switch appends with) refuses, and nothing was appended: a record written after
    // the break is unreachable to recovery, which is the defect the ledger exists to prevent.
    assert!(crate::ledger_write::append_checkpoint(&path, &checkpoint).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    // And the same writer refuses the break even when handed a checkpoint that calls itself safe.
    let mut claimed = checkpoint.clone();
    claimed["resumeSafety"] = json!("safe");
    assert!(crate::ledger_write::append_checkpoint(&path, &claimed).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

// ---- checkpoint reader parity: through the ledger reader (`ledger::read_ledger`, what the history and the checkpoint
// writer read with), a checkpoint record written after the seeded mission's events ----

/// Electron's `sample()`, for the seeded mission's own ids.
fn sample(path: &std::path::Path) -> Value {
    let header = crate::ledger::read_ledger(path).unwrap().records[0].clone();
    json!({ "schemaVersion": 1, "missionId": header["metadata"]["missionId"], "runId": header["metadata"]["runId"], "epoch": 1,
        "reason": "route-limit", "reconciledThroughSequence": 2, "settledActions": [], "unsettledActions": [], "resumeSafety": "safe",
        "safetyReason": "Every recorded action reported an outcome.", "transcriptDigest": "a".repeat(64), "assistantSummary": "",
        "createdAt": CHECKPOINT_AT })
}

/// Writes `checkpoint` as the ledger's next record, as a writer would, and says whether the reader keeps it.
fn kept(path: &std::path::Path, checkpoint: &Value) -> bool {
    let read = crate::ledger::read_ledger(path).unwrap();
    let next = read.records.iter().filter_map(|r| r["ledgerSequence"].as_u64()).max().unwrap() + 1;
    let record = json!({ "schemaVersion": read.records[0]["schemaVersion"], "recordType": "mission.checkpoint", "ledgerSequence": next,
        "occurredAt": checkpoint["createdAt"], "checkpoint": checkpoint });
    let before = std::fs::read_to_string(path).unwrap();
    std::fs::write(path, format!("{before}{record}\n")).unwrap();
    let after = crate::ledger::read_ledger(path).unwrap();
    std::fs::write(path, before).unwrap();
    let held = after.records.iter().any(|r| r["recordType"] == "mission.checkpoint");
    assert_eq!(held, after.issues.is_empty(), "a refused checkpoint is an invalid record");
    held
}

// case: accepts a well-formed checkpoint
#[test]
fn the_reader_keeps_a_well_formed_checkpoint() {
    let (_t, path) = seeded();
    assert!(kept(&path, &sample(&path)));
}

// case: refuses a checkpoint that claims safety while listing an unknown action
#[test]
fn the_reader_refuses_a_safe_checkpoint_that_lists_an_open_action() {
    let (_t, path) = seeded();
    let mut lying = sample(&path);
    lying["unsettledActions"] = json!([{ "itemId": "call_1", "toolKind": "command_execution", "name": "shell", "startedAt": NOW, "startedAtSequence": 2 }]);
    assert!(!kept(&path, &lying));
    // CONTROL: the same open action under an honest verdict is kept.
    lying["resumeSafety"] = json!("approval-required");
    assert!(kept(&path, &lying));
}

// case: refuses checkpoints belonging to another mission or run
#[test]
fn the_reader_refuses_a_checkpoint_of_another_mission_or_run() {
    let (_t, path) = seeded();
    for (key, other) in [("missionId", "mission_2"), ("runId", "run_2")] {
        let mut foreign = sample(&path);
        foreign[key] = json!(other);
        assert!(!kept(&path, &foreign), "{key}");
    }
}

// case: refuses a malformed digest, epoch, reason, or safety value
#[test]
fn the_reader_refuses_a_bad_digest_epoch_reason_or_safety() {
    let (_t, path) = seeded();
    for (key, bad) in [("transcriptDigest", json!("nope")), ("epoch", json!(0)), ("reason", json!("whenever")), ("resumeSafety", json!("fine"))] {
        let mut malformed = sample(&path);
        malformed[key] = bad;
        assert!(!kept(&path, &malformed), "{key}");
    }
}

// case: stops recovery at a checkpoint whose epoch is out of order
#[test]
fn recovery_stops_at_a_checkpoint_whose_epoch_is_out_of_order() {
    let (_t, path) = seeded();
    crate::ledger_write::append_checkpoint(&path, &checkpoint_of(&path, "route-limit", 1, CHECKPOINT_AT)).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = text.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect();
    let mut last: Value = serde_json::from_str(lines.last().unwrap()).unwrap();
    last["checkpoint"]["epoch"] = json!(7);
    *lines.last_mut().unwrap() = last.to_string();
    std::fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();
    let read = crate::ledger::read_ledger(&path).unwrap();
    assert!(!read.records.iter().any(|r| r["recordType"] == "mission.checkpoint"));
    assert!(read.issues.iter().any(|i| i.code() == "invalid-record"), "{:?}", read.issues);
    // And the transcript the window reads from that file says the record was damaged.
    assert!(crate::ledger::read_transcript(&path).issues.iter().any(|i| i.code() == "invalid-record"));
}
