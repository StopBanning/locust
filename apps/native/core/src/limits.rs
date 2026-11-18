//! WHICH RUNTIMES AND ACCOUNTS ARE AT THEIR USAGE LIMIT, AND WHEN THAT LIMIT LIFTS
//! (Electron: `main/mission-history.ts` `limitedRuntimesFrom` and `planOutSentence`, `shared/limit-lifts.ts`
//! `readingHasQuotaLeft` and `limitHasReset`, `renderer/src/status.ts` `routeRowTag`).
//!
//! Per runtime and account (`agent_accounts::usage_key`), the latest event in time across the native and Electron
//! mission ledgers decides: a `route.limit_detected` of kind `quota-exhausted` puts it at its limit, a later
//! `run.completed` on the same runtime and account takes it off, and a limit whose named reset (`resets <ISO time>`)
//! has passed is over so the next run can find out.

use crate::store::Store;
use chrono::{Datelike, Local, Timelike, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Antigravity's live reading (`antigravityUsageText`) names at least one window, and every window has some left
/// (`shared/limit-lifts.ts` `readingHasQuotaLeft`).
pub fn reading_has_quota_left(said: &str) -> bool {
    let mut count = 0usize;
    for cap in crate::invariants::regex(r"(\d+)% left").captures_iter(said) {
        count += 1;
        if cap[1].parse::<u64>().unwrap_or(0) == 0 {
            return false;
        }
    }
    count > 0
}

/// The UTC epoch milliseconds of a `resets <ISO time>` clause in `said`, or `None` when none is present or it cannot
/// be parsed as an RFC 3339 instant.
pub fn reset_millis(said: &str) -> Option<i64> {
    let reset = crate::invariants::regex(r"resets (\S+)").captures(said)?.get(1)?.as_str();
    chrono::DateTime::parse_from_rfc3339(reset).ok().map(|dt| dt.timestamp_millis())
}

/// The limit named a reset time Locust can read (`resets <ISO time>`), and it has passed
/// (`shared/limit-lifts.ts` `limitHasReset`). A time it cannot read never passes.
pub fn limit_has_reset(said: &str, now_ms: i64) -> bool {
    reset_millis(said).is_some_and(|at| at <= now_ms)
}

fn event_usage_key(metadata: &Value, event: &Value) -> Option<String> {
    let adapter = event
        .get("sourceAdapter")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| metadata.get("runtime").and_then(Value::as_str).filter(|s| !s.is_empty()))?;
    let account = metadata
        .get("resolvedRouteId")
        .and_then(Value::as_str)
        .and_then(crate::agent_accounts::account_from_route);
    Some(crate::agent_accounts::usage_key(adapter, account))
}

/// Which runtimes and accounts are still at their usage limit at `now_ms` (`main/mission-history.ts`
/// `limitedRuntimesFrom(missions, now)`).
pub fn limited_runtimes_from_at(missions: &[Vec<Value>], now_ms: i64) -> HashMap<String, String> {
    let mut latest: HashMap<String, (String, Option<String>)> = HashMap::new();
    for records in missions {
        let Some(metadata) = records.iter().find_map(|r| {
            (r.get("recordType").and_then(Value::as_str) == Some("mission.created"))
                .then(|| r.get("metadata"))
                .flatten()
        }) else {
            continue;
        };
        for event in records.iter().filter_map(|r| r.get("event")) {
            let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
            let payload = &event["payload"];
            let (relevant, said) = if kind == "route.limit_detected"
                && payload.get("kind").and_then(Value::as_str) == Some("quota-exhausted")
            {
                (true, payload.get("message").and_then(Value::as_str).map(str::to_owned))
            } else if kind == "run.completed" {
                (true, None)
            } else {
                (false, None)
            };
            if !relevant {
                continue;
            }
            let Some(at) = event.get("occurredAt").and_then(Value::as_str) else {
                continue;
            };
            let Some(key) = event_usage_key(metadata, event) else {
                continue;
            };
            if latest.get(&key).is_none_or(|(prev, _)| at > prev.as_str()) {
                latest.insert(key, (at.to_owned(), said));
            }
        }
    }
    latest
        .into_iter()
        .filter_map(|(k, (_, said))| said.filter(|s| !limit_has_reset(s, now_ms)).map(|s| (k, s)))
        .collect()
}

/// Drop any limit whose live usage reading says every named window has quota left (`reading_has_quota_left`).
pub fn lift_limits_from_readings(limited: &mut HashMap<String, String>, readings: &HashMap<String, String>) {
    limited.retain(|key, _| {
        let runtime = key.split(':').next().unwrap_or(key);
        readings
            .get(key)
            .or_else(|| readings.get(runtime))
            .is_none_or(|reading| !reading_has_quota_left(reading))
    });
}

/// Which runtimes and accounts are still at their usage limit right now (`limitedRuntimesFrom(missions)`).
pub fn limited_runtimes_from(missions: &[Vec<Value>]) -> HashMap<String, String> {
    limited_runtimes_from_at(missions, Utc::now().timestamp_millis())
}

/// Why a plan cannot take a run at `now_ms`, said for a person, or `None` (`main/mission-history.ts` `planOutSentence`).
/// `key` is `agent_accounts::usage_key(runtime, account)`.
pub fn plan_out_sentence_at(missions: &[Vec<Value>], key: &str, runtime_name: &str, now_ms: i64) -> Option<String> {
    let said = limited_runtimes_from_at(missions, now_ms).remove(key)?;
    let until = match reset_millis(&said).and_then(chrono::DateTime::from_timestamp_millis) {
        Some(dt) => {
            let local = dt.with_timezone(&Local);
            let (pm, h12) = local.hour12();
            format!(
                " until {}, {} {}, {}:{:02} {}",
                local.format("%a"),
                local.format("%b"),
                local.day(),
                h12,
                local.minute(),
                if pm { "PM" } else { "AM" }
            )
        }
        None => String::new(),
    };
    Some(format!("their {runtime_name} plan is used up{until}."))
}

/// Why a plan cannot take a run right now (`planOutSentence(missions, key, runtimeName)`).
pub fn plan_out_sentence(missions: &[Vec<Value>], key: &str, runtime_name: &str) -> Option<String> {
    plan_out_sentence_at(missions, key, runtime_name, Utc::now().timestamp_millis())
}

/// Read limited runtimes at `now_ms` from both the native and Electron mission ledgers (`store.root()` and
/// `store.electron()`), lifting Antigravity when a cached live `/usage` reading shows quota left.
pub fn limited_runtimes_at(store: &Store, now_ms: i64) -> HashMap<String, String> {
    let Ok(missions) = crate::usage_ledger::collect_missions(store) else {
        return HashMap::new();
    };
    let records: Vec<Vec<Value>> = missions.into_iter().map(|(_, r)| r).collect();
    let mut limited = limited_runtimes_from_at(&records, now_ms);
    if let Some(reading) = crate::antigravity_usage::cached_usage_text() {
        let readings = HashMap::from([("antigravity".to_owned(), reading)]);
        lift_limits_from_readings(&mut limited, &readings);
    }
    limited
}

/// Read limited runtimes right now from both the native and Electron mission ledgers.
pub fn limited_runtimes(store: &Store) -> HashMap<String, String> {
    limited_runtimes_at(store, Utc::now().timestamp_millis())
}

/// Read limited runtimes from a single `mission-ledger` directory at `now_ms`.
pub fn limited_runtimes_in_dir(dir: &Path, now_ms: i64) -> HashMap<String, String> {
    let Ok(missions) = crate::usage_ledger::collect_dir_records(dir) else {
        return HashMap::new();
    };
    let records: Vec<Vec<Value>> = missions.into_iter().map(|(_, r)| r).collect();
    limited_runtimes_from_at(&records, now_ms)
}

/// `routeRowTag` (`renderer/src/status.ts`): `AT LIMIT` outranks `ACTIVE`, and only a selectable row may be `ACTIVE`.
pub fn row_tag(status: &crate::agents::RowStatus, is_active: bool) -> &'static str {
    if status.tag == "AT LIMIT" {
        return status.tag;
    }
    if is_active && status.selectable {
        "ACTIVE"
    } else {
        status.tag
    }
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
