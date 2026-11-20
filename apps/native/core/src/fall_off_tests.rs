//! Electron's `a-conversation-does-not-fall-off-the-list.test.ts` (6 cases): a conversation whose newest turn is older
//! than the twenty most recently touched was not listed at all. Rust's list (`list_recent(usize::MAX)`, as the window
//! asks for it) reads every ledger's head and tail, with no transcript budget to confuse with a count of conversations.
use super::*;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn turn(dir: &Path, id: &str, prompt: &str, at: &str, parent: Option<&str>) {
    let mut metadata = serde_json::json!({"missionId": id, "runId": format!("run_{id}"), "prompt": prompt, "runtime": "codex", "model": "account-default",
        "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0", "workspaceId": "ws_test",
        "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": at});
    if let Some(p) = parent {
        metadata["continuesFrom"] = serde_json::json!({"missionId": p, "checkpointEpoch": 1, "reason": "follow-up"});
    }
    let created = serde_json::json!({"schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": at, "metadata": metadata});
    let done = serde_json::json!({"schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": 2, "occurredAt": at,
        "event": {"id": format!("run_{id}:1"), "runId": format!("run_{id}"), "missionId": id, "sequence": 1, "type": "run.completed", "occurredAt": at, "sourceAdapter": "codex", "payload": {}}});
    std::fs::write(dir.join(format!("{id}.jsonl")), format!("{created}\n{done}\n")).unwrap();
}

/// Twenty-five turns from today and the research conversation from five days ago.
fn ledgers() -> (Temp, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("locust-fall-off-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    for n in 0..25 {
        turn(&dir, &format!("mission_new_{n}"), &format!("turn {n}"), "2026-09-20T12:00:00.000Z", None);
    }
    turn(&dir, "mission_research", "research: what is the market doing", "2026-09-15T09:00:00.000Z", None);
    (Temp(dir.clone()), dir)
}

// case: lists a conversation older than the newest twenty
// case: carries its prompt, because the row is named from it
// case: lists every conversation exactly once
#[test]
fn lists_a_conversation_older_than_the_newest_twenty_with_its_prompt_and_each_once() {
    let (_t, dir) = ledgers();
    let listed = try_list_recent(&dir, usize::MAX).unwrap();
    assert_eq!(listed.len(), 26);
    let research = listed.iter().find(|s| s.mission_id == "mission_research").expect("the research conversation is listed");
    assert!(research.words.contains("research: what is the market doing") || research.title.contains("research"), "{research:?}");
    let ids: std::collections::HashSet<&str> = listed.iter().map(|s| s.mission_id.as_str()).collect();
    assert_eq!(ids.len(), listed.len());
}

// case: lists it WITHOUT its transcript
// case: still carries the transcript for the newest
// A Summary is a ledger's head and tail and never its events; the transcript is read when a conversation is opened
// (read_transcript), newest or old alike.
#[test]
fn a_listed_conversation_is_read_whole_only_when_it_is_opened() {
    let (_t, dir) = ledgers();
    let opened = read_transcript(dir.join("mission_research.jsonl"));
    assert_eq!(opened.metadata.as_ref().map(|m| m.mission_id.as_str()), Some("mission_research"));
}

// case: still finds an ancestor that is off the end of even the long list
#[test]
fn still_finds_an_ancestor_off_the_end_of_the_list() {
    let (_t, dir) = ledgers();
    turn(&dir, "mission_root", "the first question", "2026-08-01T09:00:00.000Z", None);
    turn(&dir, "mission_tip", "and a follow-up", "2026-09-20T12:00:01.000Z", Some("mission_root"));
    let listed = try_list_recent(&dir, 5).unwrap();
    assert!(listed.iter().any(|s| s.mission_id == "mission_tip"));
    assert_eq!(read_metadata(&dir.join("mission_root.jsonl")).map(|m| m.mission_id), Some("mission_root".to_owned()));
}
