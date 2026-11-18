//! Tests for `core/src/limits.rs`, carrying:
//! - `apps/desktop/src/main/a-limit-lifts-when-the-account-says-so.test.ts` (all 7 cases)
//! - `apps/desktop/src/main/mission-history.test.ts` ("which runtimes the record still holds at their limit", 6 cases)
//! - `apps/desktop/src/main/a-reply-waits-for-a-plan-that-is-out.test.ts` (`planOutSentence`, all 5 cases)
//! - `apps/desktop/src/renderer/src/status.test.ts` (`routeRowStatus` and `routeRowTag` AT LIMIT / ACTIVE cases)

use super::*;
use crate::agent_accounts::{account_from_route, usage_key};
use crate::agents::{Auth, Found, Integration, Probe, row_status};
use crate::store::Store;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const OUT: &str = "Antigravity's quota is used up · resets 2026-11-22T05:28:22Z";
const PLAN_OUT: &str = "Antigravity's quota is used up · resets 2026-11-09T23:52:20Z";
const HISTORY_NOW: &str = "2026-08-31T15:00:00.000Z";
const REPLY_NOW: &str = "2026-11-03T05:39:30Z";

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp() -> Temp {
    let p = std::env::temp_dir().join(format!(
        "locust-limits-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    Temp(p)
}

fn parse_ms(iso: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(iso).unwrap().timestamp_millis()
}

fn only(key: &str, said: &str) -> HashMap<String, String> {
    [(key.to_owned(), said.to_owned())].into_iter().collect()
}

/// The exact string `antigravityUsageText(answer(gemini, claude))` returns in
/// `a-limit-lifts-when-the-account-says-so.test.ts` for buckets at `2026-11-18T06:00:00Z`.
fn reading(gemini: f64, claude: f64) -> String {
    let group_text = |label: &str, rem: f64| -> String {
        let pct = (rem * 100.0).round() as u64;
        let reset = if (rem - 1.0).abs() < f64::EPSILON {
            ""
        } else {
            " · resets 2026-11-18T06:00:00.000Z"
        };
        format!("{label}: 5-hour window {pct}% left{reset} · {label}: weekly window {pct}% left{reset}")
    };
    let mut parts = vec![group_text("Gemini", gemini)];
    if claude < gemini {
        parts.push(group_text("Claude and GPT", claude));
    }
    parts.join(" · ")
}

fn out_on(at: &str, message: &str) -> Vec<Value> {
    vec![
        json!({
            "schemaVersion": 27,
            "recordType": "mission.created",
            "metadata": {
                "missionId": "mission_out",
                "runtime": "antigravity",
                "resolvedRouteId": "antigravity-account:default"
            }
        }),
        json!({
            "schemaVersion": 27,
            "recordType": "mission.event",
            "event": {
                "id": "e0",
                "sequence": 1,
                "type": "route.limit_detected",
                "occurredAt": at,
                "sourceAdapter": "antigravity",
                "payload": { "kind": "quota-exhausted", "message": message }
            }
        }),
    ]
}

// ---- a-limit-lifts-when-the-account-says-so.test.ts ----

#[test]
fn lifts_on_antigravitys_reading_of_the_account_signed_in_now_when_every_window_has_some_left() {
    assert!(reading_has_quota_left(&reading(1.0, 1.0)));
    assert!(reading_has_quota_left(&reading(0.4, 1.0)));
}

#[test]
fn stays_while_any_window_the_reading_names_is_used_up_claude_and_gpt_too() {
    assert!(!reading_has_quota_left(&reading(0.0, 1.0)));
    assert!(!reading_has_quota_left(&reading(1.0, 0.0)));
}

#[test]
fn stays_when_the_reading_names_no_window_at_all() {
    assert!(!reading_has_quota_left(""));
    assert!(!reading_has_quota_left("as of 2026-11-18T01:00:00.000Z"));
}

#[test]
fn lifts_once_the_reset_it_named_has_passed_and_not_a_moment_before() {
    assert!(!limit_has_reset(OUT, parse_ms("2026-11-22T05:28:21Z")));
    assert!(limit_has_reset(OUT, parse_ms("2026-11-22T05:28:22Z")));
}

#[test]
fn never_lifts_on_a_reset_it_cannot_read_as_a_time() {
    assert!(!limit_has_reset("weekly limit reached, resets 01:48", parse_ms("2030-01-01T00:00:00Z")));
    assert!(!limit_has_reset("You have hit your usage limit.", parse_ms("2030-01-01T00:00:00Z")));
}

#[test]
fn is_read_back_from_the_record_until_its_reset_then_not_as_on_colins_account() {
    let missions = vec![out_on("2026-11-16T04:29:59.050Z", OUT)];
    assert_eq!(limited_runtimes_from_at(&missions, parse_ms("2026-11-17T20:11:00Z")), only("antigravity", OUT));
    assert!(limited_runtimes_from_at(&missions, parse_ms("2026-11-22T06:00:00Z")).is_empty());
}

#[test]
fn control_a_limit_with_no_time_it_can_read_stays_until_a_run_finishes() {
    let missions = vec![out_on("2026-11-16T04:29:59Z", "weekly limit reached, resets 01:48")];
    assert_eq!(
        limited_runtimes_from_at(&missions, parse_ms("2030-01-01T00:00:00Z")),
        only("antigravity", "weekly limit reached, resets 01:48")
    );
}

// ---- mission-history.test.ts: which runtimes the record still holds at their limit ----

fn at_minutes(minutes: i64) -> String {
    (chrono::DateTime::parse_from_rfc3339(HISTORY_NOW).unwrap() + chrono::Duration::minutes(minutes))
        .to_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn created() -> Value {
    json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {
        "missionId": "mission_1", "runId": "run_1", "prompt": "Inspect the workspace without changing it.",
        "runtime": "codex", "model": "account-default", "requestedRouteId": "codex",
        "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0-alpha.7.2", "workspaceId": "ws_test",
        "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": HISTORY_NOW}})
}

fn recovered(events: Vec<Value>) -> Vec<Value> {
    let mut records = vec![created()];
    records.extend(events.into_iter().map(|event| json!({"schemaVersion": 27, "recordType": "mission.event", "event": event})));
    records
}

fn limit_event(runtime: &str, minutes: i64) -> Value {
    json!({"id": format!("limit-{runtime}-{minutes}"), "runId": "run_x", "missionId": "mission_x", "sequence": 1,
        "type": "route.limit_detected", "occurredAt": at_minutes(minutes), "sourceAdapter": runtime,
        "payload": {"kind": "quota-exhausted", "message": "You have hit your usage limit.", "evidence": {"redacted": true}}})
}

fn completed_event(runtime: &str, minutes: i64) -> Value {
    json!({"id": format!("done-{runtime}-{minutes}"), "runId": "run_x", "missionId": "mission_x", "sequence": 2,
        "type": "run.completed", "occurredAt": at_minutes(minutes), "sourceAdapter": runtime,
        "payload": {"process": {"exitCode": 0, "signal": null}, "evidence": {"redacted": true}}})
}

fn write_ledger(dir: &Path, id: &str, records: &[Value]) {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(format!("{id}.jsonl"));
    fs::write(&path, records.iter().map(|r| format!("{r}\n")).collect::<String>()).unwrap();
}

#[test]
fn reports_a_runtime_whose_last_word_was_its_limit_with_that_word() {
    assert_eq!(
        limited_runtimes_from(&[recovered(vec![limit_event("codex", 5)])]),
        only("codex", "You have hit your usage limit.")
    );
    assert_eq!(usage_key("codex", account_from_route("codex-account:default")), "codex");
}

#[test]
fn control_a_later_completed_run_on_that_runtime_clears_it() {
    assert!(
        limited_runtimes_from(&[
            recovered(vec![limit_event("codex", 5)]),
            recovered(vec![completed_event("codex", 9)])
        ])
        .is_empty()
    );
}

#[test]
fn decides_by_time_across_missions_not_by_file_order() {
    assert_eq!(
        limited_runtimes_from(&[
            recovered(vec![completed_event("codex", 1)]),
            recovered(vec![limit_event("codex", 3)])
        ]),
        only("codex", "You have hit your usage limit.")
    );
    assert_eq!(
        limited_runtimes_from(&[
            recovered(vec![limit_event("codex", 3)]),
            recovered(vec![completed_event("codex", 1)])
        ]),
        only("codex", "You have hit your usage limit.")
    );
}

#[test]
fn keeps_runtimes_apart_claude_finishing_says_nothing_about_codex() {
    assert_eq!(
        limited_runtimes_from(&[recovered(vec![limit_event("codex", 5), completed_event("claude", 9)])]),
        only("codex", "You have hit your usage limit.")
    );
}

#[test]
fn ignores_a_temporary_rate_limit_that_is_a_moment_not_the_account() {
    let mut temporary = limit_event("codex", 5);
    temporary["payload"] = json!({"kind": "temporary-rate-limit", "message": "slow down", "evidence": {"redacted": true}});
    assert!(limited_runtimes_from(&[recovered(vec![temporary])]).is_empty());
}

#[test]
fn rides_on_the_history_response_from_native_and_electron_ledgers_and_lifts_past_reset() {
    let root = temp();
    let store = Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    write_ledger(&store.root().join("mission-ledger"), "mission_1", &recovered(vec![limit_event("codex", 5)]));
    write_ledger(&store.electron().join("mission-ledger"), "mission_out", &out_on("2026-11-16T04:29:59.050Z", OUT));
    assert_eq!(
        limited_runtimes_at(&store, parse_ms("2026-11-17T20:11:00Z")),
        [
            ("codex".to_owned(), "You have hit your usage limit.".to_owned()),
            ("antigravity".to_owned(), OUT.to_owned()),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        limited_runtimes_at(&store, parse_ms("2026-11-22T06:00:00Z")),
        only("codex", "You have hit your usage limit.")
    );
    // usage_ledger::limited_runtimes_from delegates here, so it shares the reset-lifting rule too.
    assert!(
        crate::usage_ledger::limited_runtimes_from(&[out_on(
            "2026-11-16T04:29:59.050Z",
            "Antigravity's quota is used up · resets 2020-11-22T05:28:22Z"
        )])
        .is_empty()
    );
}

// ---- a-reply-waits-for-a-plan-that-is-out.test.ts (planOutSentence) ----

fn run_records(events: &[(&str, &str, Option<&str>, &str)], route: &str) -> Vec<Value> {
    let first_at = events.first().map(|(_, at, _, _)| *at).unwrap_or("x");
    let mut records = vec![json!({
        "schemaVersion": 27,
        "recordType": "mission.created",
        "metadata": {
            "missionId": format!("mission_{first_at}"),
            "resolvedRouteId": route
        }
    })];
    for (idx, (kind, at, message, adapter)) in events.iter().enumerate() {
        let payload = if *kind == "route.limit_detected" {
            json!({ "kind": "quota-exhausted", "message": message.unwrap_or("") })
        } else {
            json!({})
        };
        records.push(json!({
            "schemaVersion": 27,
            "recordType": "mission.event",
            "event": {
                "id": format!("e{idx}"),
                "sequence": idx + 1,
                "type": kind,
                "occurredAt": at,
                "sourceAdapter": adapter,
                "payload": payload
            }
        }));
    }
    records
}

#[test]
fn is_out_from_the_limit_the_agent_named_until_the_reset_it_named_said_with_that_day_and_time() {
    let missions = vec![run_records(
        &[("route.limit_detected", "2026-11-03T02:27:31Z", Some(PLAN_OUT), "antigravity")],
        "antigravity-account:default",
    )];
    let said = plan_out_sentence_at(&missions, "antigravity", "Antigravity", parse_ms(REPLY_NOW)).unwrap();
    assert!(said.starts_with("their Antigravity plan is used up until Mon, Nov 9, "), "{said}");
    assert!(said.ends_with('.'), "{said}");
}

#[test]
fn is_not_out_once_the_reset_has_passed_so_the_next_run_finds_out() {
    let missions = vec![run_records(
        &[("route.limit_detected", "2026-11-03T02:27:31Z", Some(PLAN_OUT), "antigravity")],
        "antigravity-account:default",
    )];
    assert_eq!(plan_out_sentence_at(&missions, "antigravity", "Antigravity", parse_ms("2026-11-10T00:00:00Z")), None);
}

#[test]
fn is_not_out_once_a_later_run_on_that_plan_finished() {
    let missions = vec![
        run_records(
            &[("route.limit_detected", "2026-11-03T02:27:31Z", Some(PLAN_OUT), "antigravity")],
            "antigravity-account:default",
        ),
        run_records(
            &[("run.completed", "2026-11-03T04:00:00Z", None, "antigravity")],
            "antigravity-account:default",
        ),
    ];
    assert_eq!(plan_out_sentence_at(&missions, "antigravity", "Antigravity", parse_ms(REPLY_NOW)), None);
}

#[test]
fn reads_only_the_plan_asked_about_and_says_plainly_when_the_agent_named_no_reset() {
    let codex_out = run_records(
        &[("route.limit_detected", "2026-11-03T02:27:31Z", Some("usage limit reached"), "codex")],
        "codex-account:default",
    );
    assert_eq!(plan_out_sentence_at(std::slice::from_ref(&codex_out), "antigravity", "Antigravity", parse_ms(REPLY_NOW)), None);
    assert_eq!(
        plan_out_sentence_at(&[codex_out], "codex", "Codex CLI", parse_ms(REPLY_NOW)),
        Some("their Codex CLI plan is used up.".to_owned())
    );
}

#[test]
fn answers_nothing_for_a_plan_with_no_record_at_all() {
    assert_eq!(plan_out_sentence_at(&[], "claude", "Claude Code", parse_ms(REPLY_NOW)), None);
}

// ---- renderer/src/status.test.ts: routeRowStatus and routeRowTag ----

fn ready_codex() -> Found {
    Found {
        id: "codex".into(),
        display_name: "Codex CLI".into(),
        installed: true,
        version: Some("0.151.0-alpha.7.2".into()),
        auth: Auth::Authenticated,
        ready: true,
        status: Probe::Ready,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

#[test]
fn a_runtime_whose_last_run_ended_on_the_usage_limit_says_so_instead_of_ready_and_outranks_active() {
    let said = "You've hit your usage limit. Try again at Sep 7th, 2026 1:57 AM.";
    let limited = row_status(&ready_codex(), Integration::Live, false, Some(said));
    assert_eq!(limited.tag, "AT LIMIT");
    assert!(limited.selectable);
    assert!(limited.detail.contains(said), "{}", limited.detail);
    // Outranks ACTIVE: the active route refusing to run is the thing to show.
    assert_eq!(row_tag(&row_status(&ready_codex(), Integration::Live, true, Some(said)), true), "AT LIMIT");
    // A runtime that is not runnable anyway keeps its own reason; the limit note must not upgrade a row to selectable.
    let mut not_ready = ready_codex();
    not_ready.ready = false;
    let signed_out = row_status(&not_ready, Integration::Live, false, Some(said));
    assert_ne!(signed_out.tag, "AT LIMIT");
    assert!(!signed_out.selectable);
}

#[test]
fn says_active_only_on_a_row_that_could_actually_run() {
    assert_eq!(row_tag(&row_status(&ready_codex(), Integration::Live, true, None), true), "ACTIVE");
    assert_eq!(row_tag(&row_status(&ready_codex(), Integration::Live, false, None), false), "READY");
    let mut missing_found = ready_codex();
    missing_found.installed = false;
    missing_found.ready = false;
    missing_found.status = Probe::Missing;
    let missing = row_status(&missing_found, Integration::Live, true, None);
    assert_eq!(row_tag(&missing, true), "NOT INSTALLED");
    let mut signed_out_found = ready_codex();
    signed_out_found.ready = false;
    signed_out_found.status = Probe::AuthRequired;
    signed_out_found.auth = Auth::Unauthenticated;
    let signed_out = row_status(&signed_out_found, Integration::Live, true, None);
    assert_eq!(row_tag(&signed_out, true), "SIGN IN");
}

#[test]
fn lift_limits_from_readings_lifts_antigravity_only_when_every_window_has_quota_left() {
    let mut limited = HashMap::from([
        ("antigravity".to_owned(), "out".to_owned()),
        ("codex".to_owned(), "out".to_owned()),
    ]);
    let readings_empty = HashMap::from([("antigravity".to_owned(), "Gemini: 5-hour window 0% left".to_owned())]);
    lift_limits_from_readings(&mut limited, &readings_empty);
    assert!(limited.contains_key("antigravity"));
    assert!(limited.contains_key("codex"));

    let readings_ok = HashMap::from([(
        "antigravity".to_owned(),
        "Gemini: 5-hour window 95% left · Gemini: weekly window 67% left".to_owned(),
    )]);
    lift_limits_from_readings(&mut limited, &readings_ok);
    assert!(!limited.contains_key("antigravity"));
    assert!(limited.contains_key("codex"));
}
