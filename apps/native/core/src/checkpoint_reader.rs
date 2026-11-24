//! mission-store's `parsedCheckpoint` (checkpoint.ts): the reader's definition of a valid checkpoint. A checkpoint that
//! does not belong to its mission and run, that names an unknown reason or safety, that has no proper digest or epoch,
//! or that claims safety while listing an action that never reported an outcome, is not a checkpoint: the ledger reader
//! (`ledger::recover`) treats it as an invalid record, and recovery stops there.
use serde_json::Value;

/// checkpoint.ts `CHECKPOINT_SCHEMA_VERSION`.
const CHECKPOINT_SCHEMA_VERSION: u64 = 1;
const REASONS: [&str; 4] = ["route-limit", "route-switch", "manual", "shutdown"];
const SAFETIES: [&str; 3] = ["safe", "approval-required", "unsafe"];
const MAX_UNSETTLED_ACTIONS: usize = 64;
const MAX_SETTLED_NAMES: usize = 40;
const MAX_CHECKPOINT_SUMMARY_LENGTH: usize = 4_000;
/// JavaScript's `Number.MAX_SAFE_INTEGER`: `isCount` takes safe integers only.
const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// isBoundedText: a string of at most `maximum` UTF-16 units, with no NUL.
fn bounded(value: &Value, maximum: usize) -> bool {
    value.as_str().is_some_and(|s| s.encode_utf16().count() <= maximum && !s.contains('\0'))
}

fn nonempty_bounded(value: &Value, maximum: usize) -> bool {
    bounded(value, maximum) && value.as_str().is_some_and(|s| !s.is_empty())
}

/// isCount: a safe integer at least `minimum`.
fn count(value: &Value, minimum: u64) -> bool {
    value.as_u64().is_some_and(|n| n >= minimum && n <= MAX_SAFE_INTEGER)
}

fn a_time(value: &Value) -> bool {
    nonempty_bounded(value, 128) && value.as_str().is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok())
}

fn digest(value: &Value) -> bool {
    value.as_str().is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

/// parsedUnsettledAction.
fn unsettled_action(value: &Value) -> bool {
    value.is_object()
        && nonempty_bounded(&value["itemId"], 512)
        && nonempty_bounded(&value["toolKind"], 512)
        && nonempty_bounded(&value["name"], 512)
        && a_time(&value["startedAt"])
        && count(&value["startedAtSequence"], 1)
}

/// parsedCheckpoint: `value` when it is a checkpoint the reader keeps for the mission `metadata` describes.
pub fn parsed_checkpoint<'a>(value: &'a Value, metadata: &Value) -> Option<&'a Value> {
    let settled = value.get("settledActions").and_then(Value::as_array)?;
    let unsettled = value.get("unsettledActions").and_then(Value::as_array)?;
    let shaped = value["schemaVersion"].as_u64() == Some(CHECKPOINT_SCHEMA_VERSION)
        && value["missionId"].is_string()
        && value["missionId"] == metadata["missionId"]
        && value["runId"].is_string()
        && value["runId"] == metadata["runId"]
        && count(&value["epoch"], 1)
        && value["reason"].as_str().is_some_and(|r| REASONS.contains(&r))
        && count(&value["reconciledThroughSequence"], 0)
        && unsettled.len() <= MAX_UNSETTLED_ACTIONS
        && value["resumeSafety"].as_str().is_some_and(|s| SAFETIES.contains(&s))
        && nonempty_bounded(&value["safetyReason"], 1_024)
        && bounded(&value["assistantSummary"], MAX_CHECKPOINT_SUMMARY_LENGTH)
        && digest(&value["transcriptDigest"])
        && a_time(&value["createdAt"])
        && value.get("runtimeThreadId").is_none_or(|t| bounded(t, 2_048));
    if !shaped || !settled.iter().all(|entry| nonempty_bounded(entry, 512)) {
        return None;
    }
    if let Some(names) = value.get("settledNames")
        && !names.as_array().is_some_and(|n| n.len() <= MAX_SETTLED_NAMES && n.iter().all(|e| nonempty_bounded(e, 512)))
    {
        return None;
    }
    if !unsettled.iter().all(unsettled_action) {
        return None;
    }
    // The one lie that would defeat the mechanism: a record that parses, reads as reassuring, and contradicts itself.
    if value["resumeSafety"] == "safe" && !unsettled.is_empty() {
        return None;
    }
    Some(value)
}

/// The ledger reader's checkpoint rule (mission-store's readLedgerFile): a checkpoint record keeps a parsed checkpoint,
/// written when it was made, the next epoch after the `kept` before it, reconciled through no more events than came.
pub fn checkpoint_record_fits(record: &Value, metadata: &Value, kept: u64, last_event_sequence: u64) -> bool {
    parsed_checkpoint(&record["checkpoint"], metadata).is_some_and(|c| {
        record["occurredAt"] == c["createdAt"]
            && c["epoch"].as_u64() == Some(kept + 1)
            && c["reconciledThroughSequence"].as_u64().is_some_and(|n| n <= last_event_sequence)
    })
}
