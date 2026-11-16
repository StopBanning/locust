//! Electron's `main/mission-history.test.ts`, a case to a test, for what the Rust port has: the folder id a mission
//! carries, a streamed reply read back whole, deleting a mission, and the two readings the record keeps per runtime
//! (which runtime is at its limit, and its latest usage window). Electron's `publicRecoveredMission` projection, the
//! history's byte budget, its `HISTORY_UNAVAILABLE` answer and the workroom join are IPC payload shaping the native
//! window, which reads the ledger in process, does not have (see the row in `parity/tests.tsv`).

use crate::agent_accounts::usage_key;
use crate::ledger::{read_transcript, Item};
use crate::store::Store;
use crate::trash::{delete_mission, STILL_RUNNING};
use crate::usage_ledger::{limited_runtimes, limited_runtimes_from, reading_of_warning, usage_windows_from};
use crate::workspace_settings::workspace_id_for;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const NOW: &str = "2026-08-31T15:00:00.000Z";

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp() -> Temp {
    let p = std::env::temp_dir().join(format!(
        "locust-mission-history-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    Temp(p)
}

/// Electron's `recovered()` metadata, as the ledger's header record.
fn created() -> Value {
    json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {
        "missionId": "mission_1", "runId": "run_1", "prompt": "Inspect the workspace without changing it.",
        "runtime": "codex", "model": "account-default", "requestedRouteId": "codex",
        "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0-alpha.7.2", "workspaceId": "ws_test",
        "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": NOW}})
}

/// A mission's records: its header, then each event as a `mission.event`.
fn recovered(events: Vec<Value>) -> Vec<Value> {
    let mut records = vec![created()];
    records.extend(events.into_iter().map(|event| json!({"schemaVersion": 27, "recordType": "mission.event", "event": event})));
    records
}

fn fragment(sequence: u64, text: &str, last: bool, item_id: &str) -> Value {
    json!({"id": format!("event_{sequence}"), "runId": "run_1", "missionId": "mission_1", "sequence": sequence,
        "type": "message.delta", "occurredAt": NOW, "sourceAdapter": "cursor",
        "payload": {"itemId": item_id, "operation": "append", "text": text, "final": last, "evidence": {"redacted": true}}})
}

fn write(dir: &Path, id: &str, records: &[Value]) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(format!("{id}.jsonl"));
    fs::write(&path, records.iter().map(|r| format!("{r}\n")).collect::<String>()).unwrap();
    path
}

/// The replies the thread reads back, in order: (text, final).
fn replies(events: Vec<Value>) -> Vec<(String, bool)> {
    let root = temp();
    let path = write(&root.0, "mission_1", &recovered(events));
    read_transcript(path)
        .items
        .into_iter()
        .filter_map(|item| match item {
            Item::Assistant { text, r#final, .. } => Some((text, r#final)),
            _ => None,
        })
        .collect()
}

// ---- which folder a mission belongs to ----

#[test]
fn gives_the_same_folder_the_same_id_every_time_so_a_mission_can_be_matched_back_to_it() {
    assert_eq!(workspace_id_for(Path::new("C:/w/streaks")), workspace_id_for(Path::new("C:/w/streaks")));
    assert_ne!(workspace_id_for(Path::new("C:/w/streaks")), workspace_id_for(Path::new("C:/w/other")));
}

#[test]
fn does_not_put_the_path_itself_in_the_ledger() {
    let id = workspace_id_for(Path::new("C:/clients/acme-secret"));
    assert!(!id.contains("acme"), "{id}");
    assert_eq!(id.len(), 35, "{id}");
    assert!(id.starts_with("ws_") && id[3..].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)), "{id}");
    // Electron's `ws_` + the first 32 hex of the path's sha256, so a mission recorded there matches its folder here.
    assert_eq!(workspace_id_for(Path::new("C:/w/streaks")), "ws_440c2e2387e1bfe8842b1e3d83cf9d03");
}

// ---- a long reply survives the history window ----

#[test]
fn keeps_every_character_of_a_reply_streamed_across_many_fragments() {
    let fragments: Vec<Value> = (0..1200).map(|i| fragment(i + 1, &format!("w{i} "), i == 1199, "msg_0")).collect();
    let whole: String = (0..1200).map(|i| format!("w{i} ")).collect();
    let read = replies(fragments);
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].0, whole);
    assert!(read[0].0.starts_with("w0 w1 "));
}

#[test]
fn marks_the_message_final_from_the_last_fragment_not_the_first() {
    let read = replies(vec![fragment(1, "a", false, "msg_0"), fragment(2, "b", false, "msg_0"), fragment(3, "c", true, "msg_0")]);
    assert_eq!(read, vec![("abc".to_owned(), true)]);
    // And a first fragment that says final does not make an unfinished message final.
    let read = replies(vec![fragment(1, "a", true, "msg_0"), fragment(2, "b", false, "msg_0")]);
    assert_eq!(read, vec![("ab".to_owned(), false)]);
}

#[test]
fn honours_a_replace_which_is_how_a_runtime_rewrites_what_it_already_said() {
    let mut rewrite = fragment(2, "final answer", true, "msg_0");
    rewrite["payload"]["operation"] = json!("replace");
    let read = replies(vec![fragment(1, "draft", false, "msg_0"), rewrite]);
    assert_eq!(read, vec![("final answer".to_owned(), true)]);
}

#[test]
fn keeps_two_messages_apart_and_in_the_order_they_were_said() {
    let read = replies(vec![
        fragment(1, "first ", false, "msg_0"),
        fragment(2, "one", true, "msg_0"),
        fragment(3, "second ", false, "msg_1"),
        fragment(4, "two", true, "msg_1"),
    ]);
    let texts: Vec<&str> = read.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(texts, ["first one", "second two"]);
}

// ---- deleting a mission ----

#[test]
fn refuses_to_delete_a_mission_that_is_still_running_and_names_the_remedy() {
    let root = temp();
    let path = write(&root.0, "mission_1", &recovered(vec![]));
    assert_eq!(
        delete_mission(&root.0, "mission_1", &["mission_1".to_owned()]),
        Err("That mission is still running. Stop it first, then delete it.".to_owned())
    );
    assert_eq!(STILL_RUNNING, "That mission is still running. Stop it first, then delete it.");
    // Refused means untouched: the file is never asked to go.
    assert!(path.is_file());
    assert!(!root.0.join(".trash").exists());
}

#[test]
fn deletes_a_finished_mission_and_says_when_there_was_nothing_to_delete() {
    let root = temp();
    let path = write(&root.0, "mission_1", &recovered(vec![]));
    assert_eq!(delete_mission(&root.0, "mission_1", &[]), Ok(true));
    assert!(!path.exists());
    assert_eq!(delete_mission(&root.0, "mission_9", &[]), Ok(false));
}

#[test]
fn refuses_an_id_that_could_name_any_other_file_before_touching_the_store() {
    let root = temp();
    let ledger = root.0.join("mission-ledger");
    fs::create_dir_all(&ledger).unwrap();
    let outside = write(&root.0, "escape", &recovered(vec![]));
    assert!(delete_mission(&ledger, "../escape", &[]).is_err());
    assert!(outside.is_file());
    assert!(!ledger.join(".trash").exists());
}

// ---- which runtimes the record still holds at their limit ----

fn at(minutes: i64) -> String {
    (chrono::DateTime::parse_from_rfc3339(NOW).unwrap() + chrono::Duration::minutes(minutes))
        .to_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn limit(runtime: &str, minutes: i64) -> Value {
    json!({"id": format!("limit-{runtime}-{minutes}"), "runId": "run_x", "missionId": "mission_x", "sequence": 1,
        "type": "route.limit_detected", "occurredAt": at(minutes), "sourceAdapter": runtime,
        "payload": {"kind": "quota-exhausted", "message": "You have hit your usage limit.", "evidence": {"redacted": true}}})
}

fn completed(runtime: &str, minutes: i64) -> Value {
    json!({"id": format!("done-{runtime}-{minutes}"), "runId": "run_x", "missionId": "mission_x", "sequence": 2,
        "type": "run.completed", "occurredAt": at(minutes), "sourceAdapter": runtime,
        "payload": {"process": {"exitCode": 0, "signal": null}, "evidence": {"redacted": true}}})
}

fn only(key: &str, said: &str) -> std::collections::HashMap<String, String> {
    [(key.to_owned(), said.to_owned())].into_iter().collect()
}

#[test]
fn reports_a_runtime_whose_last_word_was_its_limit_with_that_word() {
    assert_eq!(limited_runtimes_from(&[recovered(vec![limit("codex", 5)])]), only("codex", "You have hit your usage limit."));
    // The default account's route keys the runtime alone, as Electron's `accountUsageKey` does.
    assert_eq!(usage_key("codex", crate::agent_accounts::account_from_route("codex-account:default")), "codex");
}

#[test]
fn control_a_later_completed_run_on_that_runtime_clears_it() {
    assert!(limited_runtimes_from(&[recovered(vec![limit("codex", 5)]), recovered(vec![completed("codex", 9)])]).is_empty());
}

#[test]
fn decides_by_time_across_missions_not_by_file_order() {
    // The completed run is listed first but happened earlier.
    assert_eq!(
        limited_runtimes_from(&[recovered(vec![completed("codex", 1)]), recovered(vec![limit("codex", 3)])]),
        only("codex", "You have hit your usage limit.")
    );
    // And listed the other way round, the newer limit still wins over the earlier completion read after it.
    assert_eq!(
        limited_runtimes_from(&[recovered(vec![limit("codex", 3)]), recovered(vec![completed("codex", 1)])]),
        only("codex", "You have hit your usage limit.")
    );
}

#[test]
fn keeps_runtimes_apart_claude_finishing_says_nothing_about_codex() {
    assert_eq!(
        limited_runtimes_from(&[recovered(vec![limit("codex", 5), completed("claude", 9)])]),
        only("codex", "You have hit your usage limit.")
    );
}

#[test]
fn ignores_a_temporary_rate_limit_that_is_a_moment_not_the_account() {
    let mut temporary = limit("codex", 5);
    temporary["payload"] = json!({"kind": "temporary-rate-limit", "message": "slow down", "evidence": {"redacted": true}});
    assert!(limited_runtimes_from(&[recovered(vec![temporary])]).is_empty());
}

#[test]
fn rides_on_what_the_window_reads_from_the_record() {
    let root = temp();
    let store = Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    write(&store.root().join("mission-ledger"), "mission_1", &recovered(vec![limit("codex", 5)]));
    assert_eq!(limited_runtimes(&store), only("codex", "You have hit your usage limit."));
}

// ---- the latest usage window per runtime, from the ledger ----

fn window_event(runtime: &str, at: &str, said: &str) -> Value {
    json!({"id": format!("w-{at}"), "runId": "run_1", "missionId": "mission_1", "sequence": 1, "occurredAt": at,
        "sourceAdapter": runtime, "type": "adapter.diagnostic",
        "payload": {"code": format!("{runtime}.usage_window"), "level": "info", "terminal": false, "message": said, "evidence": {"redacted": true}}})
}

#[test]
fn keeps_the_newest_reading_per_runtime_and_ignores_everything_else() {
    let missions = [
        recovered(vec![
            window_event("claude", "2026-09-05T10:00:00.000Z", "5-hour window 20% used"),
            window_event("claude", "2026-09-05T11:00:00.000Z", "5-hour window 35% used"),
        ]),
        recovered(vec![window_event("codex", "2026-09-05T09:00:00.000Z", "primary 10% used")]),
    ];
    // 0.406: each carries when it was seen, from which run's event.
    let expected: std::collections::HashMap<String, String> = [
        ("claude".to_owned(), "5-hour window 35% used · from a run at 2026-09-05T11:00:00.000Z".to_owned()),
        ("codex".to_owned(), "primary 10% used · from a run at 2026-09-05T09:00:00.000Z".to_owned()),
    ]
    .into_iter()
    .collect();
    assert_eq!(usage_windows_from(&missions), expected);
    assert!(usage_windows_from(&[recovered(vec![])]).is_empty());
}

#[test]
fn reads_a_newer_usage_warning_back_as_the_reading() {
    let warning = json!({"id": "l-2026-09-27T14:11:40.000Z", "runId": "run_1", "missionId": "mission_1", "sequence": 2,
        "occurredAt": "2026-09-27T14:11:40.000Z", "sourceAdapter": "claude", "type": "route.limit_detected",
        "payload": {"kind": "temporary-rate-limit", "message": "You've used 79% of your 7-day window · resets 2026-09-28T07:00:00.000Z", "evidence": {"redacted": true}}});
    let missions = [recovered(vec![
        window_event("claude", "2026-09-27T01:23:19.233Z", "7-day window 66% used · resets 2026-09-28T07:00:00.000Z · 5-hour window 20% used"),
        warning,
    ])];
    assert_eq!(
        usage_windows_from(&missions),
        only("claude", "7-day window 79% used · resets 2026-09-28T07:00:00.000Z · from a run at 2026-09-27T14:11:40.000Z")
    );
    assert_eq!(reading_of_warning("Your 7-day window is running low"), None);
    assert_eq!(reading_of_warning("You've used 31% of your 5-hour window").as_deref(), Some("5-hour window 31% used"));
}
