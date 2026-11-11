//! Collect usage turns from native AND Electron mission ledgers (Electron read-only).

use crate::spend::money_of_usage;
use crate::store::Store;
use crate::usage::{USAGE_READ_FAIL, UsageTurn};
use serde_json::Value;
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

fn collect_dir(dir: &Path) -> Result<Vec<TurnRead>, String> {
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
        if let Some(turn) = usage_turn_of(id, &records) {
            out.push(turn);
        }
    }
    Ok(out)
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
    let native_dir = store.root().join("mission-ledger");
    let native_reads = collect_dir(&native_dir)?;
    let mut reads = Vec::new();
    // Electron folder is read-only; do not create it.
    let electron = electron_ledger(store);
    if electron.exists() {
        reads.extend(collect_dir(&electron)?);
    }
    let mut unique: HashMap<String, TurnRead> = reads
        .into_iter()
        .map(|r| (r.mission_id.clone(), r))
        .collect();
    for read in native_reads {
        unique.insert(read.mission_id.clone(), read);
    }
    Ok(resolve_conversations(unique.into_values().collect()))
}

/// Remembered plan meter readings file (Electron usage-readings.json), read-only from both folders.
pub fn usage_windows(store: &Store) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for root in [store.root(), store.electron()] {
        let path = root.join("usage-readings.json");
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(map) = value
            .get("windows")
            .and_then(Value::as_object)
            .or(value.as_object())
        else {
            continue;
        };
        for (key, reading) in map {
            if let Some(text) = reading.as_str() {
                let reading_at = |said: &str| {
                    said.rsplit_once(" · from a run at ")
                        .map(|(_, at)| at)
                        .unwrap_or("")
                        .to_owned()
                };
                if out
                    .get(key)
                    .is_none_or(|before: &String| reading_at(text) >= reading_at(before))
                {
                    out.insert(key.clone(), text.to_owned());
                }
            }
        }
    }
    out
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
}
