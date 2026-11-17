//! Electron `main/a-provider-that-fails-first-is-carried-on.test.ts`, the carry: one press starts the task from the
//! start on the agent offered, told why; Without asking goes on by itself, and says so when the pick is the agent that
//! failed. Each stopped turn is a ledger as a real run writes it, in `accept-edits`.
use super::*;
use crate::provider_carry_on_tests::{busy, offline, signed_out, tool};

fn stopped(t: &Temp, id: &str, runtime: &str, events: &[Value]) {
    let store = t.store();
    let mut w = LedgerWriter::create_runtime_with(&store, id, "Fix it", "m", runtime, runtime, &[], None, &json!({"mode": "accept-edits", "sandbox": "workspace-write"})).unwrap();
    for e in events {
        let mut payload = e["payload"].clone();
        if e["type"] == "run.failed" {
            let stderr = payload["process"]["stderr"].as_str().unwrap_or("").to_owned();
            payload["runtimeTerminal"] = json!("failed");
            payload["process"] = json!({"exitCode": 1, "signal": null, "stderr": stderr, "stderrTruncated": false, "recordCount": 1, "inputDeliveryFailed": false,
                "outputLimitExceeded": false, "oversizedRecordsDropped": 0, "forcedTerminationAttempted": false, "terminationUnconfirmed": false,
                "startedAt": crate::ledger_write::now(), "finishedAt": crate::ledger_write::now()});
        }
        w.append(e["type"].as_str().unwrap(), payload, None).unwrap();
    }
    drop(w);
    crate::people::record_owner(&store, id, "tm_carry").unwrap();
}

fn team(t: &Temp, plan: Value) {
    let work = t.0.join("work");
    fs::create_dir_all(&work).unwrap();
    t.store()
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_carry", "name": "Carry", "role": "Custom", "hue": "lime",
                "createdAt": "2026-11-12T00:00:00.000Z", "folder": work.to_string_lossy(), "route": {"runtime": "claude", "model": "sonnet", "mode": "accept-edits"}}],
                "missionOwners": {"m": "tm_carry"}, "settings": {"planRunsOut": plan}}),
        )
        .unwrap();
}

/// The turns that continue `id`.
fn carried_from(t: &Temp, id: &str) -> Vec<Value> {
    ledgers(t)
        .into_iter()
        .map(|p| records(&p))
        .filter(|r| r[0]["metadata"]["continuesFrom"]["missionId"] == id)
        .map(|r| r[0]["metadata"].clone())
        .collect()
}

#[test]
fn one_press_starts_the_task_from_the_start_on_the_agent_offered_told_why() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped(&t, "mission_provider_busy", "claude", &[busy()]);
    let (id, _, mode) = carry("mission_provider_busy", "codex", None).unwrap();
    finished(&id);
    assert_eq!(mode, "accept-edits");
    let started = carried_from(&t, "mission_provider_busy");
    assert_eq!(started.len(), 1);
    assert_eq!(started[0]["runtime"], "codex");
    assert_eq!(started[0]["mode"], "accept-edits");
    assert_eq!(started[0]["startedBy"], json!({"kind": "carry-on", "from": "claude"}));
    let prompt = started[0]["prompt"].as_str().unwrap_or_default();
    assert!(prompt.contains(&crate::carry_on::carry_on_prompt("claude", crate::carry_on::Reason::Provider)), "{prompt}");

    stopped(&t, "mission_provider_same", "claude", &[busy()]);
    assert_eq!(carry("mission_provider_same", "claude", None).unwrap_err(), "Claude Code is the agent that just failed.");
    stopped(&t, "mission_provider_begun", "claude", &[tool(), busy()]);
    assert!(carry("mission_provider_begun", "codex", None).is_err());
    assert!(carried_from(&t, "mission_provider_same").is_empty() && carried_from(&t, "mission_provider_begun").is_empty());
}

#[test]
fn without_asking_goes_on_by_itself_and_says_so_when_the_pick_is_the_agent_that_failed() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, json!({"runtime": "claude", "automatic": true}));
    stopped(&t, "mission_provider_offline", "codex", &[offline()]);
    carry_on_without_asking(&t.store(), "mission_provider_offline");
    let started = carried_from(&t, "mission_provider_offline");
    assert_eq!(started.iter().map(|m| m["runtime"].clone()).collect::<Vec<_>>(), [json!("claude")]);
    for m in &started {
        if let Some(id) = m["missionId"].as_str() {
            finished(id);
        }
    }

    stopped(&t, "mission_provider_itself", "claude", &[signed_out()]);
    carry_on_without_asking(&t.store(), "mission_provider_itself");
    assert!(carried_from(&t, "mission_provider_itself").is_empty());
    let text = fs::read_to_string(path_of(&t, "mission_provider_itself")).unwrap();
    assert!(
        text.contains("Claude Code is the agent picked to carry on, and it is the one whose provider failed, so nothing was carried on."),
        "{text}"
    );
}
