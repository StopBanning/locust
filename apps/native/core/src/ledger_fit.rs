//! What an event may hold before it is written (mission-store `fit.ts`): a long field is kept shortened, both ends
//! kept and the middle said to be missing, rather than a record too large to write ending the run. An answer is kept
//! whole up to its own, larger limit. Lengths are counted in UTF-16 units, as Electron counts them.
use serde_json::{Map, Value};

pub const LEDGER_TEXT_LIMIT: usize = 16_384;
pub const LEDGER_MESSAGE_TEXT_LIMIT: usize = 131_072;
pub const LEDGER_NAME_LIMIT: usize = 512;
pub const LEDGER_THREAD_LIMIT: usize = 2_048;
pub const LEDGER_ROUTE_LIMIT: usize = 256;
const LEDGER_JSON_ENTRIES: usize = 100;
const LEDGER_JSON_DEPTH: usize = 8;
const LEDGER_EVENT_BYTES: usize = 480 * 1024;
const STDERR_KEPT: usize = 64 * 1024;

const MARK: &str = "\n…[shortened]…\n";
const TAIL_KEPT: usize = 4_096;
const TOO_DEEP: &str = "[nested deeper than the record keeps]";
const TOO_LARGE: &str = "[too large for one record: the run produced it, the record does not keep it]";

const NAME_FIELDS: [&str; 7] = ["itemId", "itemType", "status", "toolKind", "name", "parentItemId", "code"];
const STRUCTURAL_FIELDS: [&str; 10] =
    ["stepKind", "operation", "kind", "level", "phase", "runtimeTerminal", "final", "terminal", "sessionEnded", "exitCode"];
const JSON_FIELDS: [&str; 3] = ["plan", "output", "usage"];

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// `fitText`: a NUL is never written; past `limit`, the head and the last 4,096 kept around the mark (or, for a small
/// limit, the head and an ellipsis).
pub fn fit_text(value: &str, limit: usize) -> String {
    let clean = if value.contains('\0') { value.replace('\0', "\u{FFFD}") } else { value.to_owned() };
    let all = units(&clean);
    if all.len() <= limit {
        return clean;
    }
    if limit >= TAIL_KEPT * 2 {
        let head = limit - TAIL_KEPT - units(MARK).len();
        return format!(
            "{}{MARK}{}",
            String::from_utf16_lossy(&all[..head]),
            String::from_utf16_lossy(&all[all.len() - TAIL_KEPT..])
        );
    }
    format!("{}…", String::from_utf16_lossy(&all[..limit - 1]))
}

/// `fitJson`: strings to the text limit, at most 100 entries a level, 8 levels deep.
pub fn fit_json(value: &Value, depth: usize) -> Value {
    match value {
        Value::String(s) => Value::String(fit_text(s, LEDGER_TEXT_LIMIT)),
        Value::Array(items) if !items.is_empty() => {
            if depth >= LEDGER_JSON_DEPTH {
                return Value::String(TOO_DEEP.into());
            }
            Value::Array(items.iter().take(LEDGER_JSON_ENTRIES).map(|e| fit_json(e, depth + 1)).collect())
        }
        Value::Object(map) if !map.is_empty() => {
            if depth >= LEDGER_JSON_DEPTH {
                return Value::String(TOO_DEEP.into());
            }
            let mut kept = Map::new();
            for (key, entry) in map.iter().take(LEDGER_JSON_ENTRIES) {
                kept.insert(fit_text(key, LEDGER_NAME_LIMIT), fit_json(entry, depth + 1));
            }
            Value::Object(kept)
        }
        other => other.clone(),
    }
}

fn fit_evidence(value: &Value) -> Value {
    let Value::Object(evidence) = value else { return value.clone() };
    let mut next = evidence.clone();
    if let Some(Value::String(kind)) = evidence.get("runtimeEventType") {
        next.insert("runtimeEventType".into(), Value::String(fit_text(kind, LEDGER_NAME_LIMIT)));
    }
    if let Some(raw) = evidence.get("raw") {
        let fitted = fit_json(raw, 0);
        if &fitted != raw {
            next.insert("raw".into(), fitted);
            next.insert("redacted".into(), Value::Bool(true));
        }
    }
    Value::Object(next)
}

fn fit_process(value: &Value) -> Value {
    let Value::Object(process) = value else { return value.clone() };
    match process.get("signal") {
        Some(Value::String(signal)) if units(signal).len() > 64 => {
            let mut next = process.clone();
            next.insert("signal".into(), Value::String(fit_text(signal, 64)));
            Value::Object(next)
        }
        _ => value.clone(),
    }
}

/// `fitPayload`: each field to its own limit; an answer's text (`message`) to the message limit.
fn fit_payload(payload: &Map<String, Value>, message: bool) -> Map<String, Value> {
    payload
        .iter()
        .map(|(key, value)| {
            let k = key.as_str();
            let fitted = if k == "evidence" {
                fit_evidence(value)
            } else if k == "process" {
                fit_process(value)
            } else if JSON_FIELDS.contains(&k) {
                fit_json(value, 0)
            } else if let (Value::String(s), false) = (value, STRUCTURAL_FIELDS.contains(&k)) {
                let limit = if k == "runtimeThreadId" {
                    LEDGER_THREAD_LIMIT
                } else if NAME_FIELDS.contains(&k) {
                    LEDGER_NAME_LIMIT
                } else if message && k == "text" {
                    LEDGER_MESSAGE_TEXT_LIMIT
                } else {
                    LEDGER_TEXT_LIMIT
                };
                Value::String(fit_text(s, limit))
            } else {
                value.clone()
            };
            (key.clone(), fitted)
        })
        .collect()
}

fn weight(event: &Value) -> usize {
    serde_json::to_string(event).map(|s| s.len()).unwrap_or(usize::MAX)
}

/// `fitWeight`: an event still past 480 KiB loses its raw evidence, then most of its stderr, then its JSON fields, then
/// its answer is shortened, both ends kept, until it fits.
fn fit_weight(mut event: Value) -> Value {
    if weight(&event) <= LEDGER_EVENT_BYTES {
        return event;
    }
    if let Some(evidence) = event["payload"].get_mut("evidence").and_then(Value::as_object_mut)
        && evidence.remove("raw").is_some()
    {
        evidence.insert("redacted".into(), Value::Bool(true));
    }
    if weight(&event) <= LEDGER_EVENT_BYTES {
        return event;
    }
    if let Some(process) = event["payload"].get_mut("process").and_then(Value::as_object_mut)
        && let Some(Value::String(stderr)) = process.get("stderr")
        && units(stderr).len() > STDERR_KEPT
    {
        let all = units(stderr);
        let tail = String::from_utf16_lossy(&all[all.len() - STDERR_KEPT..]);
        process.insert("stderr".into(), Value::String(tail));
        process.insert("stderrTruncated".into(), Value::Bool(true));
        if weight(&event) <= LEDGER_EVENT_BYTES {
            return event;
        }
    }
    for key in JSON_FIELDS {
        if event["payload"].get(key).is_none() {
            continue;
        }
        event["payload"][key] = Value::String(TOO_LARGE.into());
        if weight(&event) <= LEDGER_EVENT_BYTES {
            return event;
        }
    }
    // An answer whose characters fit and whose bytes do not (wide scripts, escaped control characters) is shortened,
    // both ends kept, until it does.
    while let Some(text) = event["payload"]["text"].as_str().map(str::to_owned) {
        let length = units(&text).len();
        if length <= TAIL_KEPT * 2 || weight(&event) <= LEDGER_EVENT_BYTES {
            break;
        }
        event["payload"]["text"] = Value::String(fit_text(&text, (TAIL_KEPT * 2).max(length * 7 / 10)));
    }
    event
}

/// `fitEventToLedger`: the event as the record keeps it. One that already fits comes back unchanged.
pub fn fit_event(event: Value) -> Value {
    let mut next = event;
    for key in ["cliVersion", "requestedRouteId", "resolvedRouteId"] {
        if let Some(Value::String(value)) = next.get(key)
            && units(value).len() > LEDGER_ROUTE_LIMIT
        {
            next[key] = Value::String(fit_text(value, LEDGER_ROUTE_LIMIT));
        }
    }
    if let Some(Value::String(thread)) = next.get("runtimeThreadId")
        && units(thread).len() > LEDGER_THREAD_LIMIT
    {
        next["runtimeThreadId"] = Value::String(fit_text(thread, LEDGER_THREAD_LIMIT));
    }
    let message = next["type"] == "message.delta";
    if let Some(payload) = next.get("payload").and_then(Value::as_object) {
        let fitted = fit_payload(payload, message);
        next["payload"] = Value::Object(fitted);
    }
    fit_weight(next)
}

#[cfg(test)]
#[path = "ledger_fit_tests.rs"]
mod ledger_fit_tests;
