//! Collect usage turns from native AND Electron mission ledgers (Electron read-only).

use crate::spend::money_of_usage;
use crate::store::Store;
use crate::usage::{USAGE_READ_FAIL, UsageTurn};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_FILE: u64 = 8 * 1024 * 1024;
const MAX_HOPS: usize = 10_000;

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn read_jsonl(path: &Path) -> Option<Vec<Value>> {
    let file = fs::File::open(path).ok()?;
    let mut buf = String::new();
    file.take(MAX_FILE + 1).read_to_string(&mut buf).ok()?;
    if buf.len() as u64 > MAX_FILE {
        return None;
    }
    Some(
        buf.lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    None
                } else {
                    serde_json::from_str(line).ok()
                }
            })
            .collect(),
    )
}

fn token_count(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0)
            .map(|n| n as u64)
    })
}

struct TurnRead {
    turn: UsageTurn,
    continues: Option<String>,
    mission_id: String,
}

fn usage_turn_of(mission_id: &str, records: &[Value]) -> Option<TurnRead> {
    let metadata = records.iter().find_map(|r| {
        if r.get("recordType").and_then(Value::as_str) == Some("mission.created") {
            r.get("metadata").cloned()
        } else {
            None
        }
    })?;
    let runtime = metadata.get("runtime")?.as_str()?.to_owned();
    let model_meta = metadata
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let continues = metadata
        .get("continuesFrom")
        .and_then(|c| c.get("missionId"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut last_updated = metadata
        .get("createdAt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();

    let mut completed: Option<&Value> = None;
    for record in records {
        if let Some(at) = record
            .get("event")
            .and_then(|e| e.get("occurredAt"))
            .or_else(|| record.get("failure").and_then(|e| e.get("occurredAt")))
            .or_else(|| record.get("checkpoint").and_then(|e| e.get("createdAt")))
            .or_else(|| record.get("link").and_then(|e| e.get("occurredAt")))
            .or_else(|| record.get("check").and_then(|e| e.get("occurredAt")))
            .or_else(|| record.get("approval").and_then(|e| e.get("occurredAt")))
            .and_then(Value::as_str)
        {
            last_updated = at.to_owned();
        }
        if let Some(event) = record.get("event")
            && event.get("type").and_then(Value::as_str) == Some("run.completed")
            && completed.is_none()
        {
            completed = Some(event);
        }
    }
    let (at, payload) = match completed {
        Some(event) => (
            event
                .get("occurredAt")
                .and_then(Value::as_str)
                .unwrap_or(last_updated.as_str())
                .to_owned(),
            event.get("payload").cloned().unwrap_or(Value::Null),
        ),
        None => (last_updated, Value::Null),
    };
    if at.is_empty() {
        return None;
    }
    let usage = payload
        .get("usage")
        .cloned()
        .unwrap_or(Value::Object(Default::default()));
    let reported = payload
        .get("resolvedModel")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let (money, plan) = if usage.is_object() {
        money_of_usage(&usage)
    } else {
        (crate::spend::Spend::default(), false)
    };
    Some(TurnRead {
        mission_id: mission_id.to_owned(),
        continues,
        turn: UsageTurn {
            at,
            runtime,
            model: reported.unwrap_or(model_meta),
            conversation: String::new(),
            input_tokens: usage.get("inputTokens").and_then(token_count),
            output_tokens: usage.get("outputTokens").and_then(token_count),
            cache_read_tokens: usage.get("cacheReadTokens").and_then(token_count),
            cache_write_tokens: usage.get("cacheWriteTokens").and_then(token_count),
            usd: money.usd,
            premium_requests: money.premium_requests,
            plan,
        },
    })
}

fn resolve_conversations(reads: Vec<TurnRead>) -> Vec<UsageTurn> {
    let continues: HashMap<String, Option<String>> = reads
        .iter()
        .map(|r| (r.mission_id.clone(), r.continues.clone()))
        .collect();
    let root_of = |mission_id: &str| -> String {
        let mut at = mission_id.to_owned();
        for _ in 0..MAX_HOPS {
            let Some(before) = continues.get(&at).and_then(|c| c.clone()) else {
                return at;
            };
            if !continues.contains_key(&before) {
                return at;
            }
            at = before;
        }
        at
    };
    reads
        .into_iter()
        .map(|mut r| {
            r.turn.conversation = root_of(&r.mission_id);
            r.turn
        })
        .collect()
}

fn electron_ledger(store: &Store) -> PathBuf {
    store.electron().join("mission-ledger")
}

/// Turns from native ledger + Electron ledger. Missing dirs are empty; unreadable → error words.
pub fn usage_turns(store: &Store) -> Result<Vec<UsageTurn>, String> {
    let mut reads = Vec::new();
    for (id, records) in collect_missions(store)? {
        if let Some(turn) = usage_turn_of(&id, &records) {
            reads.push(turn);
        }
    }
    Ok(resolve_conversations(reads))
}

/// USAGE CARRIED FROM ANOTHER COMPUTER (usage-carried.ts, 0.849): a numbers-only file in Locust's data folder, one turn
/// per line, added to the turns the ledger holds wherever Usage is read. Nothing in it is words, a teammate or a path.
pub const USAGE_CARRIED_FILE: &str = "usage-carried.jsonl";

/// `short`: a string of 1 to `most` characters, counted as JavaScript counts them.
fn short(value: &Value, most: usize) -> Option<&str> {
    value.as_str().filter(|s| !s.is_empty() && s.encode_utf16().count() <= most)
}

/// `Date.parse` of a carried turn's time: an ISO date or date-time.
fn a_time(at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(at).is_ok()
        || chrono::NaiveDateTime::parse_from_str(at, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
        || chrono::NaiveDate::parse_from_str(at, "%Y-%m-%d").is_ok()
}

/// `carriedTurn`: one carried turn, checked field by field, or None; what it does not name is not kept.
pub fn carried_turn(value: &Value) -> Option<UsageTurn> {
    let row = value.as_object()?;
    let field = |key: &str| row.get(key).unwrap_or(&Value::Null);
    let at = short(field("at"), 40).filter(|at| a_time(at))?;
    let runtime = short(field("runtime"), 40)
        .filter(|r| r.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))?;
    let model = short(field("model"), 200)?;
    let conversation = short(field("conversation"), 200)?;
    // A number the line names must be a count: finite and not below zero. One it does not name is left out.
    let mut counts = [None::<f64>; 6];
    for (i, key) in ["inputTokens", "outputTokens", "cacheReadTokens", "cacheWriteTokens", "usd", "premiumRequests"].iter().enumerate() {
        match row.get(*key) {
            None => {}
            Some(v) => counts[i] = Some(v.as_f64().filter(|n| n.is_finite() && *n >= 0.0)?),
        }
    }
    let plan = match row.get("plan") {
        None => false,
        Some(Value::Bool(true)) => true,
        Some(_) => return None,
    };
    let tokens = |n: Option<f64>| n.map(|n| n as u64);
    Some(UsageTurn {
        at: at.to_owned(),
        runtime: runtime.to_owned(),
        model: model.to_owned(),
        conversation: conversation.to_owned(),
        input_tokens: tokens(counts[0]),
        output_tokens: tokens(counts[1]),
        cache_read_tokens: tokens(counts[2]),
        cache_write_tokens: tokens(counts[3]),
        usd: counts[4],
        premium_requests: counts[5],
        plan,
    })
}

/// `carriedUsage`: every carried turn in the file at `path`; none when there is no file, and a line that is not a turn
/// is left out.
pub fn carried_usage(path: &Path) -> Vec<UsageTurn> {
    let Ok(text) = fs::read_to_string(path) else { return Vec::new() };
    text.split('\n')
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|row| carried_turn(&row))
        .collect()
}

/// The turns Usage adds up (index.ts USAGE_READ_CHANNEL): the ledgers' turns and what another computer carried here,
/// from Locust's data folder (Electron's, which the PC's numbers were carried into, and this app's own when it has one).
pub fn usage_turns_with_carried(store: &Store) -> Result<Vec<UsageTurn>, String> {
    let mut turns = usage_turns(store)?;
    let mut folders = vec![store.electron().to_path_buf()];
    if store.root() != store.electron() {
        folders.push(store.root().to_path_buf());
    }
    for folder in folders {
        turns.extend(carried_usage(&folder.join(USAGE_CARRIED_FILE)));
    }
    Ok(turns)
}

pub(crate) fn collect_dir_records(dir: &Path) -> Result<Vec<(String, Vec<Value>)>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(dir).map_err(|_| USAGE_READ_FAIL.to_owned())?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| USAGE_READ_FAIL.to_owned())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !safe_id(id) {
            continue;
        }
        let Some(records) = read_jsonl(&path) else {
            continue;
        };
        out.push((id.to_owned(), records));
    }
    Ok(out)
}

pub(crate) fn collect_missions(store: &Store) -> Result<Vec<(String, Vec<Value>)>, String> {
    let native_dir = store.root().join("mission-ledger");
    let native_reads = collect_dir_records(&native_dir)?;
    let mut unique: HashMap<String, Vec<Value>> = HashMap::new();
    let electron = electron_ledger(store);
    if electron.exists() {
        for (id, records) in collect_dir_records(&electron)? {
            unique.insert(id, records);
        }
    }
    for (id, records) in native_reads {
        unique.insert(id, records);
    }
    Ok(unique.into_iter().collect())
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

/// Port of mission-history.ts `readingOfWarning`.
pub fn reading_of_warning(message: &str) -> Option<String> {
    let rest = message.strip_prefix("You've used ")?;
    let (pct, rest) = rest.split_once("% of your ")?;
    if pct.is_empty() || pct.len() > 3 || !pct.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (window, reset) = match rest.split_once(" window · resets ") {
        Some((w, r)) if !r.is_empty() && !r.bytes().any(|b| b.is_ascii_whitespace()) => (w, Some(r)),
        _ => (rest.strip_suffix(" window")?, None),
    };
    if window.is_empty() {
        return None;
    }
    Some(match reset {
        Some(r) => format!("{window} window {pct}% used · resets {r}"),
        None => format!("{window} window {pct}% used"),
    })
}

/// Port of mission-history.ts `limitedRuntimesFrom`, keyed by `agent_accounts::usage_key`.
pub fn limited_runtimes_from(missions: &[Vec<Value>]) -> HashMap<String, String> {
    crate::limits::limited_runtimes_from(missions)
}

pub fn limited_runtimes(store: &Store) -> HashMap<String, String> {
    crate::limits::limited_runtimes(store)
}

/// Port of mission-history.ts `usageWindowsFrom`, keyed by `agent_accounts::usage_key`.
pub fn usage_windows_from(missions: &[Vec<Value>]) -> HashMap<String, String> {
    let mut latest: HashMap<String, (String, String)> = HashMap::new();
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
            let said = if kind == "adapter.diagnostic"
                && payload.get("code").and_then(Value::as_str).is_some_and(|c| c.ends_with(".usage_window"))
            {
                payload.get("message").and_then(Value::as_str).map(str::to_owned)
            } else if kind == "route.limit_detected"
                && payload.get("kind").and_then(Value::as_str) == Some("temporary-rate-limit")
            {
                payload.get("message").and_then(Value::as_str).and_then(reading_of_warning)
            } else {
                None
            };
            let Some(said) = said else {
                continue;
            };
            let Some(at) = event.get("occurredAt").and_then(Value::as_str) else {
                continue;
            };
            let Some(key) = event_usage_key(metadata, event) else {
                continue;
            };
            let replace = latest.get(&key).is_none_or(|(prev, _)| at > prev.as_str());
            if replace {
                latest.insert(key, (at.to_owned(), said));
            }
        }
    }
    latest
        .into_iter()
        .map(|(k, (at, said))| (k, format!("{said} · from a run at {at}")))
        .collect()
}

fn reading_at(said: &str) -> &str {
    said.rsplit_once(" · from a run at ")
        .map(|(_, at)| at)
        .unwrap_or("")
}

/// A kept readings file as a map: Electron's flat `{runtime: reading}`, or an older `{windows: {...}}`.
fn kept_readings(path: &Path) -> Option<HashMap<String, String>> {
    let value = serde_json::from_str::<Value>(&fs::read_to_string(path).ok()?).ok()?;
    let map = value.get("windows").and_then(Value::as_object).or(value.as_object())?;
    Some(map.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned()))).collect())
}

/// Each agent's newest usage reading (Electron `rememberedUsageWindows`, 0.571): the readings kept in
/// `usage-readings.json` (this app's, and Electron's beside it), with a newer one from the mission ledgers taking each
/// agent's place. What comes out is kept in this app's file, so a reading outlives the conversation that said it.
/// Every ledger is read, so the first fill needs no backfill past the newest twenty.
pub fn usage_windows(store: &Store) -> HashMap<String, String> {
    let own = kept_readings(&store.root().join("usage-readings.json"));
    let mut merged: HashMap<String, String> = HashMap::new();
    for kept in [own.clone(), kept_readings(&store.electron().join("usage-readings.json"))].into_iter().flatten() {
        for (key, text) in kept {
            if merged.get(&key).is_none_or(|before| reading_at(&text) >= reading_at(before)) {
                merged.insert(key, text);
            }
        }
    }
    if let Ok(missions) = collect_missions(store) {
        let records: Vec<Vec<Value>> = missions.into_iter().map(|(_, r)| r).collect();
        for (key, said) in usage_windows_from(&records) {
            if merged.get(&key).is_none_or(|before| reading_at(&said) >= reading_at(before)) {
                merged.insert(key, said);
            }
        }
    }
    if own.as_ref() != Some(&merged) && !(own.is_none() && merged.is_empty()) {
        // Not kept this time: the reading shown is still right, and the next read tries again.
        let _ = store.save_json("usage-readings.json", &json!(merged));
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn write_mission(dir: &Path, id: &str, lines: &[Value]) {
        fs::create_dir_all(dir).unwrap();
        let mut file = fs::File::create(dir.join(format!("{id}.jsonl"))).unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
    }

    #[test]
    fn remembered_limits_use_electron_filename_and_newest_reading() {
        let root = std::env::temp_dir().join(format!("usage-windows-{}", std::process::id()));
        fs::create_dir_all(root.join("native")).unwrap();
        fs::create_dir_all(root.join("electron")).unwrap();
        let newer = "5-hour window 20% used · from a run at 2026-11-10T12:00:00Z";
        fs::write(
            root.join("native/usage-readings.json"),
            json!({"codex":newer}).to_string(),
        )
        .unwrap();
        fs::write(root.join("electron/usage-readings.json"), json!({"codex":"5-hour window 80% used · from a run at 2026-11-09T12:00:00Z","claude":"week window 15% left"}).to_string()).unwrap();
        let store = Store::new(root.join("native"), root.join("electron")).unwrap();
        assert_eq!(usage_windows(&store)["codex"], newer);
        assert_eq!(usage_windows(&store)["claude"], "week window 15% left");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn zero_cache_and_failed_turn_date_match_electron() {
        let created = json!({"recordType":"mission.created","metadata":{"runtime":"claude","model":"opus","createdAt":"2026-10-01T00:00:00Z"}});
        let failed = json!({"event":{"type":"run.failed","occurredAt":"2026-11-10T12:00:00Z"}});
        let read = usage_turn_of("m", &[created.clone(), failed]).unwrap();
        assert_eq!(read.turn.at, "2026-11-10T12:00:00Z");
        let done = json!({"event":{"type":"run.completed","occurredAt":"2026-11-10T12:00:00Z","payload":{"usage":{"inputTokens":100,"cacheReadTokens":0}}}});
        let read = usage_turn_of("m", &[created, done]).unwrap();
        assert_eq!(read.turn.cache_read_tokens, Some(0));
        assert_eq!(crate::usage::turn_tokens(&read.turn).cache_counted, 100);
    }

    #[test]
    fn reads_native_and_electron_and_chains_conversation() {
        let root = std::env::temp_dir().join(format!("locust-usage-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let electron = root.join("electron");
        let native = root.join("native");
        fs::create_dir_all(electron.join("mission-ledger")).unwrap();
        fs::create_dir_all(native.join("mission-ledger")).unwrap();
        write_mission(
            &native.join("mission-ledger"),
            "root",
            &[
                json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"root","runtime":"codex","model":"gpt","createdAt":"2026-11-01T00:00:00.000Z"}}),
                json!({"schemaVersion":27,"recordType":"mission.event","event":{"type":"run.completed","occurredAt":"2026-11-01T00:01:00.000Z","sequence":1,"payload":{"usage":{"billing":"subscription","inputTokens":10,"outputTokens":2},"resolvedModel":"gpt-6"}}}),
            ],
        );
        write_mission(
            &electron.join("mission-ledger"),
            "child",
            &[
                json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"child","runtime":"claude","model":"opus","createdAt":"2026-11-02T00:00:00.000Z","continuesFrom":{"missionId":"root"}}}),
                json!({"schemaVersion":27,"recordType":"mission.event","event":{"type":"run.completed","occurredAt":"2026-11-02T00:01:00.000Z","sequence":1,"payload":{"usage":{"usd":0.5,"inputTokens":5,"outputTokens":1}}}}),
            ],
        );
        let store = Store::new(native, electron).unwrap();
        // Native wins if the same mission is present in both folders.
        fs::copy(
            store.root().join("mission-ledger/root.jsonl"),
            store.electron().join("mission-ledger/root.jsonl"),
        )
        .unwrap();
        let turns = usage_turns(&store).unwrap();
        assert_eq!(turns.len(), 2);
        let child = turns.iter().find(|t| t.runtime == "claude").unwrap();
        assert_eq!(child.conversation, "root");
        assert_eq!(child.usd, Some(0.5));
        let root_turn = turns.iter().find(|t| t.runtime == "codex").unwrap();
        assert!(root_turn.plan);
        assert_eq!(root_turn.model, "gpt-6");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn keeps_usage_and_exhausted_limits_separate_for_each_account() {
        let acc = "account_11111111111111111111111111111111";
        let missions = vec![
            vec![
                json!({
                    "recordType": "mission.created",
                    "metadata": {"missionId": "m_work", "runtime": "codex", "resolvedRouteId": format!("codex:gpt-5-account:{acc}")}
                }),
                json!({
                    "recordType": "mission.event",
                    "event": {"type": "route.limit_detected", "occurredAt": "2026-11-10T10:00:00Z", "sourceAdapter": "codex", "payload": {"kind": "quota-exhausted", "message": "Quota reached"}}
                }),
                json!({
                    "recordType": "mission.event",
                    "event": {"type": "adapter.diagnostic", "occurredAt": "2026-11-10T09:00:00Z", "sourceAdapter": "codex", "payload": {"code": "codex.usage_window", "message": "5-hour window 90% used"}}
                }),
            ],
            vec![
                json!({
                    "recordType": "mission.created",
                    "metadata": {"missionId": "m_default", "runtime": "codex", "resolvedRouteId": "codex:gpt-5-account:default"}
                }),
                json!({
                    "recordType": "mission.event",
                    "event": {"type": "run.completed", "occurredAt": "2026-11-10T11:00:00Z", "sourceAdapter": "codex", "payload": {}}
                }),
                json!({
                    "recordType": "mission.event",
                    "event": {"type": "route.limit_detected", "occurredAt": "2026-11-10T10:30:00Z", "sourceAdapter": "codex", "payload": {"kind": "temporary-rate-limit", "message": "You've used 15% of your 5-hour window"}}
                }),
            ],
        ];
        let work_key = crate::agent_accounts::usage_key("codex", Some(acc));
        let limited = limited_runtimes_from(&missions);
        assert_eq!(limited.get(&work_key).map(String::as_str), Some("Quota reached"));
        assert!(!limited.contains_key("codex"));

        let windows = usage_windows_from(&missions);
        assert_eq!(windows.get(&work_key).map(String::as_str), Some("5-hour window 90% used · from a run at 2026-11-10T09:00:00Z"));
        assert_eq!(windows.get("codex").map(String::as_str), Some("5-hour window 15% used · from a run at 2026-11-10T10:30:00Z"));
    }
}

#[cfg(test)]
#[path = "usage_carried_tests.rs"]
mod usage_carried_tests;
