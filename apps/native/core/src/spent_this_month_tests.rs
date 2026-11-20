//! Electron's `what-a-teammate-spent-this-month.test.ts`: the Team card added up only the newest twenty conversations.
//! Rust's total reads every ledger (`usage_ledger::usage_turns`) and counts a run in the month it ended.
use super::*;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn id(i: usize) -> String {
    format!("mission_{i:03}")
}

/// Thirty runs: Wren's odd ones priced at a dollar, Juno's even ones a subscription's; runs 1 and 2 ended in August.
fn store() -> (Temp, crate::store::Store) {
    let root = std::env::temp_dir().join(format!("locust-spend-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("native")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    for i in 1..=30 {
        let at = if i <= 2 { format!("2026-08-{}T12:00:00.000Z", 10 + i) } else { format!("2026-09-{:02}T12:00:00.000Z", i.min(28)) };
        let usage = if i % 2 == 1 { serde_json::json!({"usd": 1, "inputTokens": 900, "outputTokens": 40}) } else { serde_json::json!({"usd": 3, "inputTokens": 900, "outputTokens": 40, "billing": "subscription"}) };
        let created = serde_json::json!({"schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": at, "metadata": {"missionId": id(i), "runId": format!("run_{i}"), "prompt": format!("Question {i}"), "runtime": "opencode", "model": "opencode/some-paid-model", "requestedRouteId": "opencode", "resolvedRouteId": "opencode-account:default", "cliVersion": "test", "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": at}});
        let done = serde_json::json!({"schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": 2, "occurredAt": at, "event": {"id": format!("e_{i}"), "runId": format!("run_{i}"), "missionId": id(i), "sequence": 1, "type": "run.completed", "occurredAt": at, "sourceAdapter": "opencode", "payload": {"usage": usage}}});
        std::fs::write(dir.join(format!("{}.jsonl", id(i))), format!("{created}\n{done}\n")).unwrap();
    }
    (Temp(root), store)
}

fn owner(conversation: &str) -> Option<&'static str> {
    let i: usize = conversation.strip_prefix("mission_")?.parse().ok()?;
    (i <= 29).then_some(if i % 2 == 1 { "tm_wren" } else { "tm_juno" })
}

// case: counts every priced run of the month, not only the newest twenty, and nothing a plan paid for
#[test]
fn counts_every_priced_run_of_the_month_and_nothing_a_plan_paid_for() {
    let (_t, store) = store();
    let turns = crate::usage_ledger::usage_turns(&store).unwrap();
    assert_eq!(spent_by(&turns, "2026-09", |c| owner(c) == Some("tm_wren")).usd, Some(14.0));
    assert_eq!(spent_by(&turns, "2026-09", |c| owner(c) == Some("tm_juno")).usd, None);
}

// case: counts a run in the month it ended
#[test]
fn counts_a_run_in_the_month_it_ended() {
    let (_t, store) = store();
    let turns = crate::usage_ledger::usage_turns(&store).unwrap();
    assert_eq!(spent_by(&turns, "2026-08", |c| owner(c) == Some("tm_wren")).usd, Some(1.0));
    assert_eq!(spent_by(&turns, "2026-10", |_| true).usd, None);
}
