//! Carries `apps/desktop/src/main/usage-is-counted.test.ts` (all 10 cases).
use super::*;
use crate::usage::{UsageRange, UsageTurn, cached_percent, local_date, prompt_tokens, total_tokens, usage_summary};
use chrono::{Datelike, Local, TimeZone};
use serde_json::{Value, json};
use std::path::PathBuf;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_store() -> (Temp, Store) {
    let root = std::env::temp_dir().join(format!(
        "locust-usage-counted-{}-{}",
        std::process::id(),
        crate::invariants::unique_nanos()
    ));
    let electron = root.join("electron");
    let native = root.join("native");
    fs::create_dir_all(electron.join("mission-ledger")).unwrap();
    fs::create_dir_all(native.join("mission-ledger")).unwrap();
    let store = Store::new(native, electron).unwrap();
    (Temp(root), store)
}

fn created_record(mission_id: &str, runtime: &str, model: &str, at: &str, continues: Option<&str>) -> Value {
    let mut meta = json!({
        "missionId": mission_id,
        "runId": format!("run_{mission_id}"),
        "prompt": "Question",
        "runtime": runtime,
        "model": model,
        "requestedRouteId": runtime,
        "resolvedRouteId": format!("{runtime}-account:default"),
        "cliVersion": "test",
        "workspaceId": "ws_test",
        "sandbox": "read-only",
        "executionPolicyVersion": 1,
        "createdAt": at,
    });
    if let Some(prev) = continues {
        meta["continuesFrom"] = json!({"missionId": prev, "checkpointEpoch": 1, "reason": "follow-up"});
    }
    json!({
        "schemaVersion": 27,
        "recordType": "mission.created",
        "ledgerSequence": 1,
        "occurredAt": at,
        "metadata": meta,
    })
}

fn completed_record(mission_id: &str, runtime: &str, at: &str, payload: Value) -> Value {
    json!({
        "schemaVersion": 27,
        "recordType": "mission.event",
        "ledgerSequence": 2,
        "occurredAt": at,
        "event": {
            "id": format!("event_{mission_id}_1"),
            "runId": format!("run_{mission_id}"),
            "missionId": mission_id,
            "sequence": 1,
            "occurredAt": at,
            "sourceAdapter": runtime,
            "type": "run.completed",
            "payload": payload,
        }
    })
}

fn write_ledger(dir: &Path, id: &str, lines: &[Value]) {
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    fs::write(dir.join(format!("{id}.jsonl")), body).unwrap();
}

fn fixed_now() -> chrono::DateTime<Local> {
    Local.with_ymd_and_hms(2026, 10, 9, 18, 0, 0).single().unwrap()
}

fn day_iso(back: i64, hour: u32) -> String {
    let base = chrono::NaiveDate::from_ymd_opt(2026, 10, 9).unwrap() - chrono::Duration::days(back);
    let dt = Local
        .with_ymd_and_hms(base.year(), base.month(), base.day(), hour, 0, 0)
        .single()
        .unwrap();
    dt.to_rfc3339()
}

fn sample_turn(at: String) -> UsageTurn {
    UsageTurn {
        at,
        runtime: "claude".into(),
        model: "claude-opus-4-6".into(),
        conversation: "c1".into(),
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_write_tokens: None,
        usd: None,
        premium_requests: None,
        plan: false,
    }
}

// case: names the model the run reported, and keeps a plan’s dollars out of the money
#[test]
fn names_the_model_the_run_reported_and_keeps_a_plans_dollars_out_of_the_money() {
    let at = "2026-10-09T15:00:00.000Z";
    let read = usage_turn_of(
        "m1",
        &[
            created_record("m1", "claude", "opus", at, None),
            completed_record(
                "m1",
                "claude",
                at,
                json!({
                    "resolvedModel": "claude-opus-4-6",
                    "usage": {
                        "inputTokens": 40,
                        "cacheReadTokens": 90_000,
                        "cacheWriteTokens": 2_000,
                        "outputTokens": 800,
                        "usd": 1.4,
                        "billing": "subscription"
                    }
                }),
            ),
        ],
    )
    .unwrap();
    assert_eq!(read.turn.at, at);
    assert_eq!(read.turn.runtime, "claude");
    assert_eq!(read.turn.model, "claude-opus-4-6");
    assert_eq!(read.turn.input_tokens, Some(40));
    assert_eq!(read.turn.cache_read_tokens, Some(90_000));
    assert!(read.turn.plan);
    assert_eq!(read.turn.usd, None);
}

// case: an API key’s turn is money; a turn with no receipt still counts, at its last moment
#[test]
fn an_api_keys_turn_is_money_a_turn_with_no_receipt_still_counts_at_its_last_moment() {
    let at = "2026-10-09T16:00:00.000Z";
    let paid = usage_turn_of(
        "m2",
        &[
            created_record("m2", "claude", "haiku", at, None),
            completed_record("m2", "claude", at, json!({"usage": {"inputTokens": 900, "outputTokens": 40, "usd": 0.02}})),
        ],
    )
    .unwrap();
    assert_eq!(paid.turn.usd, Some(0.02));
    let failed = usage_turn_of("m3", &[created_record("m3", "codex", "gpt-6-luna", at, Some("m2"))]).unwrap();
    assert_eq!(failed.turn.at, at);
    assert_eq!(failed.turn.runtime, "codex");
    assert_eq!(failed.turn.model, "gpt-6-luna");
    assert_eq!(failed.continues.as_deref(), Some("m2"));
    assert_eq!(failed.turn.input_tokens, None);
}

// case: every turn the ledger holds, each named by its conversation’s first turn
#[test]
fn every_turn_the_ledger_holds_each_named_by_its_conversations_first_turn() {
    let (_t, store) = temp_store();
    let dir = store.root().join("mission-ledger");
    let at = |minute: u32| format!("2026-10-09T15:{minute:02}:00.000Z");
    write_ledger(
        &dir,
        "m_a",
        &[
            created_record("m_a", "codex", "gpt-6-luna", &at(1), None),
            completed_record("m_a", "codex", &at(2), json!({"usage": {"inputTokens": 1000, "cacheReadTokens": 600, "outputTokens": 50}})),
        ],
    );
    write_ledger(
        &dir,
        "m_b",
        &[
            created_record("m_b", "codex", "gpt-6-luna", &at(3), Some("m_a")),
            completed_record("m_b", "codex", &at(4), json!({"usage": {"inputTokens": 2000, "cacheReadTokens": 1500, "outputTokens": 70}})),
        ],
    );
    write_ledger(&dir, "m_c", &[created_record("m_c", "codex", "gpt-6-luna", &at(5), Some("m_b"))]);
    write_ledger(
        &dir,
        "m_d",
        &[
            created_record("m_d", "opencode", "opencode/space-bunny-free", &at(6), None),
            completed_record("m_d", "opencode", &at(7), json!({"usage": {"inputTokens": 300, "outputTokens": 20, "usd": 0}})),
        ],
    );

    let turns = usage_turns(&store).unwrap();
    assert_eq!(turns.len(), 4);
    let m_b = turns.iter().find(|t| t.at == at(4)).unwrap();
    assert_eq!(m_b.conversation, "m_a");
    assert_eq!(turns.iter().filter(|t| t.conversation == "m_a").count(), 3);
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-09T20:00:00.000Z")
        .unwrap()
        .with_timezone(&Local);
    assert_eq!(usage_summary(&turns, UsageRange::All, now).conversations, 2);
}

// case: reads a prompt in each runtime’s terms: Codex counts its cache inside, Claude beside
#[test]
fn reads_a_prompt_in_each_runtimes_terms_codex_counts_its_cache_inside_claude_beside() {
    assert_eq!(prompt_tokens("codex", Some(1000), Some(800), None), 1000);
    assert_eq!(prompt_tokens("claude", Some(40), Some(800), Some(160)), 1000);
}

// case: cuts the range by the day each turn ended, on this machine’s clock
#[test]
fn cuts_the_range_by_the_day_each_turn_ended_on_this_machines_clock() {
    let now = fixed_now();
    let turns = vec![
        sample_turn(day_iso(0, 12)),
        sample_turn(day_iso(6, 12)),
        sample_turn(day_iso(7, 12)),
        sample_turn(day_iso(29, 12)),
        sample_turn(day_iso(30, 12)),
    ];
    assert_eq!(usage_summary(&turns, UsageRange::Days7, now).turns, 2);
    let month = usage_summary(&turns, UsageRange::Days30, now);
    assert_eq!(month.turns, 4);
    assert_eq!(month.days.len(), 30);
    assert_eq!(month.days.last().unwrap().date, local_date(now));
    assert_eq!(month.range_days, 30);
    assert_eq!(usage_summary(&turns, UsageRange::All, now).turns, 5);
}

// case: twelve weeks of heatmap, Sunday first, ending today
#[test]
fn twelve_weeks_of_heatmap_sunday_first_ending_today() {
    let now = fixed_now();
    let summary = usage_summary(&[sample_turn(day_iso(0, 12))], UsageRange::Days7, now);
    let expected_len = 11 * 7 + now.weekday().num_days_from_sunday() as usize + 1;
    assert_eq!(summary.heatmap.len(), expected_len);
    let last = summary.heatmap.last().unwrap();
    assert_eq!(last.date, local_date(now));
    assert_eq!(last.turns, 1);
    let first_date = chrono::NaiveDate::parse_from_str(&summary.heatmap[0].date, "%Y-%m-%d").unwrap();
    assert_eq!(first_date.weekday(), chrono::Weekday::Sun);
}

// case: streaks run back from today, or from yesterday while today has nothing yet
#[test]
fn streaks_run_back_from_today_or_from_yesterday_while_today_has_nothing_yet() {
    let now = fixed_now();
    let turns = vec![
        sample_turn(day_iso(1, 12)),
        sample_turn(day_iso(2, 12)),
        sample_turn(day_iso(3, 12)),
        sample_turn(day_iso(10, 12)),
        sample_turn(day_iso(11, 12)),
    ];
    let s1 = usage_summary(&turns, UsageRange::All, now);
    assert_eq!((s1.streak_current, s1.streak_longest), (3, 3));
    let mut with_today = turns;
    with_today.push(sample_turn(day_iso(0, 12)));
    assert_eq!(usage_summary(&with_today, UsageRange::All, now).streak_current, 4);
    assert_eq!(usage_summary(&[sample_turn(day_iso(5, 12))], UsageRange::All, now).streak_current, 0);
}

// case: a model each, most turns first, and how each was paid for
#[test]
fn a_model_each_most_turns_first_and_how_each_was_paid_for() {
    let now = fixed_now();
    let mut t1 = sample_turn(day_iso(0, 12));
    t1.plan = true;
    t1.input_tokens = Some(10);
    t1.cache_read_tokens = Some(900);
    t1.output_tokens = Some(50);
    let t2 = t1.clone();
    let mut t3 = sample_turn(day_iso(1, 12));
    t3.model = "claude-haiku-4-5".into();
    t3.usd = Some(0.25);
    t3.input_tokens = Some(100);
    t3.cache_read_tokens = Some(0);
    t3.output_tokens = Some(10);
    let mut t4 = sample_turn(day_iso(1, 12));
    t4.runtime = "opencode".into();
    t4.model = "opencode/space-bunny-free".into();
    t4.input_tokens = Some(500);
    t4.output_tokens = Some(30);
    let mut t5 = sample_turn(day_iso(2, 12));
    t5.runtime = "copilot".into();
    t5.model = "auto".into();
    t5.premium_requests = Some(1.0);

    let summary = usage_summary(&[t1, t2, t3, t4, t5], UsageRange::Days30, now);
    let models: Vec<&str> = summary.models.iter().map(|m| m.model.as_str()).collect();
    assert_eq!(models, ["claude-opus-4-6", "opencode/space-bunny-free", "claude-haiku-4-5", "auto"]);
    assert_eq!(summary.models[0].turns, 2);
    assert_eq!(summary.models[0].pay.plan_turns, 2);
    assert_eq!(summary.models[0].pay.free_turns, 0);
    assert_eq!(summary.models[0].pay.usd, None);
    assert_eq!(summary.pay.usd, Some(0.25));
    assert_eq!(summary.pay.premium_requests, Some(1.0));
    assert_eq!(summary.pay.plan_turns, 2);
    assert_eq!(summary.pay.free_turns, 1);
    assert_eq!(total_tokens(&summary.models[0].tokens), 2 * (10 + 900 + 50));
}

// case: a cache share is over the turns whose runtime counts a cache, not a 0% for one that says nothing
#[test]
fn a_cache_share_is_over_the_turns_whose_runtime_counts_a_cache_not_a_0_for_one_that_says_nothing() {
    let now = fixed_now();
    let mut counted_turn = sample_turn(day_iso(0, 12));
    counted_turn.input_tokens = Some(100);
    counted_turn.cache_read_tokens = Some(900);
    counted_turn.output_tokens = Some(1);
    let counted = usage_summary(&[counted_turn.clone()], UsageRange::Days7, now);
    assert_eq!(cached_percent(&counted.tokens), Some(90));

    let mut silent_turn = sample_turn(day_iso(0, 12));
    silent_turn.runtime = "opencode".into();
    silent_turn.model = "opencode/space-bunny-free".into();
    silent_turn.input_tokens = Some(5000);
    silent_turn.output_tokens = Some(1);
    let silent = usage_summary(&[silent_turn.clone()], UsageRange::Days7, now);
    assert_eq!(cached_percent(&silent.tokens), None);

    let both = usage_summary(&[counted_turn, silent_turn], UsageRange::Days7, now);
    assert_eq!(cached_percent(&both.tokens), Some(90));
}

// case: active days, the busiest day, and nothing counted twice
#[test]
fn active_days_the_busiest_day_and_nothing_counted_twice() {
    let now = fixed_now();
    let turns = vec![
        sample_turn(day_iso(0, 9)),
        sample_turn(day_iso(0, 15)),
        sample_turn(day_iso(3, 12)),
    ];
    let summary = usage_summary(&turns, UsageRange::Days7, now);
    assert_eq!(summary.active_days, 2);
    let busiest = summary.busiest.unwrap();
    assert_eq!(busiest.date, local_date(now));
    assert_eq!(busiest.turns, 2);
    assert_eq!(summary.days.iter().map(|d| d.turns).sum::<u64>(), 3);
}
