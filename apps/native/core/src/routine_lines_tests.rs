//! Electron's routine card lines: `a-paused-routine-says-so.test.ts`, `a-missed-routine-says-missed-and-run-now.test.tsx`
//! (its line) and `a-routine-says-what-it-runs-on.test.tsx` ("the routine row"), with their values.

use crate::engine::Teammate;
use crate::routines::schedule::{missed_line, summary};
use crate::routines::{routine_chain, routine_chain_with_models};
use chrono::{Local, TimeZone};
use serde_json::{json, Value};

fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> chrono::DateTime<Local> {
    Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
}

/// Electron's `saved`: Morning digest, every hour, last run 2 Oct 08:00 (months there count from 0: 9 is October).
fn saved(over: Value) -> Value {
    let mut r = json!({
        "routineId": "rt_1", "name": "Morning digest", "teammateId": "tm_wren",
        "route": {"runtime": "cursor", "model": "composer-2.5", "mode": "ask"},
        "steps": ["Say good morning."], "learnedFrom": [],
        "createdAt": at(2026, 10, 2, 8, 0).to_rfc3339(), "lastRunAt": at(2026, 10, 2, 8, 0).to_rfc3339(),
        "runs": 0, "schedule": {"kind": "every", "hours": 1}
    });
    for (k, v) in over.as_object().unwrap() {
        r[k] = v.clone();
    }
    r
}

#[test]
fn a_paused_routine_says_so_on_its_schedule_chip_in_place_of_its_next_time() {
    assert_eq!(summary(&saved(json!({"paused": true})), at(2026, 10, 3, 9, 0)).as_deref(), Some("every hour · paused"));
}

#[test]
fn a_paused_routine_counts_its_next_time_from_a_resume() {
    let resumed = saved(json!({"resumedAt": at(2026, 10, 3, 8, 30).to_rfc3339()}));
    assert_eq!(summary(&resumed, at(2026, 10, 3, 9, 0)).as_deref(), Some("every hour · next 09:30"));
}

#[test]
fn a_missed_routine_says_missed_0800_today() {
    let now = at(2026, 10, 3, 9, 0);
    let missed = at(2026, 10, 3, 8, 0).to_rfc3339();
    assert_eq!(missed_line(&missed, now).as_deref(), Some("Missed, 08:00 today"));
    let routine = saved(json!({"schedule": {"kind": "daily", "at": "08:00"}, "lastRunAt": missed, "missedAt": missed}));
    assert_eq!(summary(&routine, now).as_deref(), Some("Missed, 08:00 today"));
    assert_eq!(missed_line(&at(2026, 10, 2, 8, 0).to_rfc3339(), now).as_deref(), Some("Missed, 08:00 yesterday"));
    assert_eq!(missed_line(&at(2026, 9, 29, 8, 0).to_rfc3339(), now).as_deref(), Some("Missed, Tue 08:00"));
    assert_eq!(missed_line("not a time", now), None);
}

fn mate(id: &str, name: &str, route: Option<(&str, &str)>) -> Teammate {
    Teammate {
        id: id.into(),
        name: name.into(),
        role: "Code".into(),
        runtime: route.map(|r| r.0.into()),
        model: route.map(|r| r.1.into()),
        account: None,
        busy: false,
        runs_in: String::new(),
        folder: None,
    }
}

fn team() -> Vec<Teammate> {
    vec![
        mate("tm_wren", "Wren", Some(("cursor", "grok-4.7-high"))),
        mate("tm_sable", "Sable", Some(("opencode", "opencode/nemotron-3.5-lightning-free"))),
    ]
}

#[test]
fn the_routine_row_names_each_teammate_with_the_model_their_steps_run_on() {
    let chain = routine_chain_with_models(&json!({"teammateId": "tm_wren", "steps": ["a", "b"], "handOffs": [{}, {"teammateId": "tm_sable", "check": true}]}), &team());
    assert_eq!(chain, "Wren (Grok 4.7 High) → Sable (Nemotron 3.5 Lightning Free) (checks)");
}

#[test]
fn the_routine_row_names_one_shared_model_once_after_the_chain_so_checks_stays_on_the_line() {
    let muse = Some(("opencode", "opencode/muse-spark-1.3-contributor-free"));
    let same = vec![mate("tm_wren", "Wren", muse), mate("tm_atlas", "Atlas", muse), mate("tm_sable", "Sable", muse)];
    let three = routine_chain_with_models(
        &json!({"teammateId": "tm_wren", "steps": ["a", "b", "c"], "handOffs": [{}, {"teammateId": "tm_atlas"}, {"teammateId": "tm_sable", "check": true}]}),
        &same,
    );
    assert_eq!(three, "Wren → Atlas → Sable (checks), all on Muse Spark 1.3 Contributor Free");
    assert_eq!(three.matches("Muse Spark").count(), 1);
    let two = routine_chain_with_models(&json!({"teammateId": "tm_wren", "steps": ["a", "b"], "handOffs": [{}, {"teammateId": "tm_sable", "check": true}]}), &same);
    assert!(two.starts_with("Wren → Sable (checks), both on Muse Spark"), "{two}");
}

#[test]
fn the_routine_row_still_names_each_model_when_the_routes_differ_even_one_model_through_two_tools() {
    let two_tools = vec![mate("tm_wren", "Wren", Some(("claude", "claude-sonnet-5-5"))), mate("tm_sable", "Sable", Some(("antigravity", "claude-sonnet-5-5")))];
    let chain = routine_chain_with_models(&json!({"teammateId": "tm_wren", "steps": ["a", "b"], "handOffs": [{}, {"teammateId": "tm_sable", "check": true}]}), &two_tools);
    assert!(chain.starts_with("Wren (") && chain.contains(") → Sable (") && chain.ends_with(") (checks)"), "{chain}");
    assert!(!chain.contains("both on") && !chain.contains("all on"));
}

#[test]
fn the_routine_row_names_the_one_teammate_of_a_plain_routine_with_its_model_too() {
    let plain = json!({"teammateId": "tm_wren", "steps": ["a"]});
    assert_eq!(routine_chain_with_models(&plain, &team()), "Wren (Grok 4.7 High)");
    assert_eq!(routine_chain(&plain, &team()), None);
}
