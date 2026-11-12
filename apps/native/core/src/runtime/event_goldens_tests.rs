//! Event goldens: every recorded agent stream in Electron's fixtures, read by Electron's reader and by Rust's.
//!
//! `parity/events.json` is written by `scripts/event-goldens.mjs` and holds each stream's lines and the events
//! Electron's reader made from them (type, payload, thread). Rust's reader for the same agent must make the same
//! events. Streams that differ are listed in `parity/events-not-yet.txt`, which may only shrink, as with the command
//! grid. `LOCUST_WRITE_NOT_YET=1 cargo test -p locust-core event_goldens` rewrites it.

use super::Event;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Rust's reader for one stream, fed every line and then told the process ended cleanly.
fn rust_events(reader: &str, lines: &[String]) -> Vec<Event> {
    let mut out = Vec::new();
    match reader {
        "claude" => {
            let mut n = super::claude_events::ClaudeNormalizer::default();
            for l in lines {
                if let Ok(v) = serde_json::from_str::<Value>(l) {
                    out.extend(n.accept(&v));
                }
            }
            if !n.ended {
                out.push(n.finish(false, ""));
            }
        }
        "cursor" => {
            let mut n = super::cursor_events::CursorNormalizer::default();
            for l in lines {
                out.extend(n.accept_line(l));
            }
            if !n.ended {
                out.push(n.finish(false, Some(0)));
            }
        }
        "copilot" => {
            let mut n = super::copilot_events::CopilotNormalizer::default();
            for l in lines {
                out.extend(n.accept_line(l));
            }
            if !n.ended {
                out.push(n.finish(false, Some(0)));
            }
        }
        "agy" => {
            let mut n = super::agy_events::AgyNormalizer::default();
            for l in lines {
                out.extend(n.accept_line(l));
            }
            out.extend(n.finish(false, Some(0)));
        }
        "antigravity" => {
            let mut n = super::antigravity_events::TranscriptNormalizer::default();
            for l in lines {
                out.extend(n.accept_line(l));
            }
            out.extend(n.finish(false, Some(0)));
        }
        "muse" => {
            let mut n = super::muse_events::MuseNormalizer::default();
            for l in lines {
                out.extend(n.accept_line(l));
            }
            out.extend(n.finish(false, Some(0)));
        }
        "opencode" => {
            let mut n = super::opencode_events::Normalizer::new(None);
            for l in lines {
                if let Ok(v) = serde_json::from_str::<Value>(l) {
                    out.extend(n.accept(&v));
                }
            }
            out.extend(n.finish(false, Some(0), "", false));
        }
        other => panic!("no Rust reader for {other}"),
    }
    out
}

/// Fields that hold the clock, which the two runs read at different times, and `evidence.transportSequence`: the
/// line number Electron keeps with each event, which no Rust reader records yet. That one gap is counted once, in
/// `rust_keeps_no_transport_sequence_yet`, so it does not hide every other difference.
fn steady(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "at" | "occurredAt" | "startedAt" | "completedAt" | "endedAt" | "finishedAt" | "timestamp" | "transportSequence"
                    )
                })
                .map(|(k, v)| (k.clone(), steady(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(steady).collect()),
        other => other.clone(),
    }
}

fn first_json_difference(electron: &Value, rust: &Value, at: &str) -> String {
    match (electron, rust) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in a {
                match b.get(k) {
                    None => return format!("Rust lacks {at}/{k}"),
                    Some(w) if w != v => return first_json_difference(v, w, &format!("{at}/{k}")),
                    _ => {}
                }
            }
            match b.keys().find(|k| !a.contains_key(*k)) {
                Some(k) => format!("Rust adds {at}/{k}"),
                None => format!("{at} differs"),
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (v, w)) in a.iter().zip(b).enumerate() {
                if v != w {
                    return first_json_difference(v, w, &format!("{at}/{i}"));
                }
            }
            format!("{at} differs")
        }
        _ => {
            let short = |v: &Value| v.to_string().chars().take(80).collect::<String>();
            format!("{at}: Electron {}, Rust {}", short(electron), short(rust))
        }
    }
}

fn difference(stream: &Value) -> Option<String> {
    if let Some(why) = stream["refused"].as_str() {
        return Some(format!("Electron's reader refused the stream ({why})"));
    }
    let lines: Vec<String> =
        stream["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_owned()).collect();
    let rust = rust_events(stream["reader"].as_str().unwrap(), &lines);
    let electron = stream["events"].as_array().unwrap();
    let types = |list: Vec<&str>| list.join(",");
    let rust_types: Vec<&str> = rust.iter().map(|e| e.kind.as_str()).collect();
    let electron_types: Vec<&str> = electron.iter().map(|e| e["type"].as_str().unwrap()).collect();
    if rust_types != electron_types {
        let at = rust_types.iter().zip(&electron_types).position(|(a, b)| a != b).unwrap_or(rust_types.len().min(electron_types.len()));
        return Some(format!(
            "events part at #{at}: Electron {} ({} events), Rust {} ({} events)",
            types(electron_types.iter().skip(at).take(3).copied().collect()),
            electron_types.len(),
            types(rust_types.iter().skip(at).take(3).copied().collect()),
            rust_types.len()
        ));
    }
    for (i, (e, r)) in electron.iter().zip(&rust).enumerate() {
        let (want, have) = (steady(&e["payload"]), steady(&r.payload));
        if want != have {
            return Some(format!("event #{i} {}: {}", r.kind, first_json_difference(&want, &have, "payload")));
        }
        if e["thread"].as_str() != r.thread.as_deref() && e.get("thread").is_some() {
            return Some(format!("event #{i} {}: thread {:?}, Electron {}", r.kind, r.thread, e["thread"]));
        }
    }
    None
}

#[test]
fn rust_reads_each_recorded_stream_as_electron_does_or_it_is_listed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../parity");
    let goldens: Value = serde_json::from_str(&std::fs::read_to_string(root.join("events.json")).unwrap()).unwrap();
    let streams = goldens["streams"].as_array().unwrap();
    let differs: BTreeMap<String, String> = streams
        .iter()
        .filter_map(|s| difference(s).map(|d| (s["fixture"].as_str().unwrap().to_owned(), d)))
        .collect();
    let list_path = root.join("events-not-yet.txt");
    if std::env::var_os("LOCUST_WRITE_NOT_YET").is_some() {
        let mut out = format!(
            "# Recorded streams Rust does not yet read as Electron {} reads them.\n\
             # Written by LOCUST_WRITE_NOT_YET=1 cargo test -p locust-core event_goldens; may only shrink.\n",
            goldens["electronCommit"].as_str().unwrap_or("?")
        );
        for (id, why) in &differs {
            out.push_str(&format!("{id}\t{why}\n"));
        }
        std::fs::write(&list_path, out).unwrap();
    }
    let listed: BTreeSet<String> = std::fs::read_to_string(&list_path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.split('\t').next().unwrap().to_owned())
        .collect();
    let new: Vec<String> =
        differs.iter().filter(|(id, _)| !listed.contains(*id)).map(|(id, why)| format!("{id}: {why}")).collect();
    let fixed: Vec<&String> = listed.iter().filter(|id| !differs.contains_key(*id)).collect();
    assert!(new.is_empty(), "Rust now reads these streams differently from Electron:\n{}", new.join("\n"));
    assert!(
        fixed.is_empty(),
        "these streams match Electron now; take them off parity/events-not-yet.txt:\n{}",
        fixed.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
    );
}

/// The one difference `steady` hides. When a Rust reader starts recording the line number, this fails, and the
/// field comes out of `steady` so it is compared like any other.
#[test]
fn rust_keeps_no_transport_sequence_yet() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../parity");
    let goldens: Value = serde_json::from_str(&std::fs::read_to_string(root.join("events.json")).unwrap()).unwrap();
    let mut electron_has = 0;
    let mut rust_has = 0;
    for s in goldens["streams"].as_array().unwrap() {
        let lines: Vec<String> = s["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_owned()).collect();
        electron_has += s["events"].as_array().unwrap().iter().filter(|e| e["payload"]["evidence"]["transportSequence"].is_number()).count();
        rust_has += rust_events(s["reader"].as_str().unwrap(), &lines)
            .iter()
            .filter(|e| e.payload["evidence"]["transportSequence"].is_number())
            .count();
    }
    assert!(electron_has > 0);
    assert_eq!(rust_has, 0, "a Rust reader now records evidence.transportSequence; stop hiding it in steady()");
}
