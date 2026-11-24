//! LOCUST IN A BROWSER OPENS AT ONCE, FROM WHAT IT LAST SAW (Electron `renderer/src/desk-snapshot.ts`, 0.834).
//!
//! What the window read from the computer is kept, sealed with AES-256-GCM (through `crate::relay_crypto`),
//! and shown at once the next time: each read by its call and its arguments, the newest kept when there is
//! too much, one too long not kept at all, and never readable without the browser's own key.

use crate::desk_host::{DeskValue, desk_encode_args};
use crate::relay_crypto::{aes_gcm_decrypt, aes_gcm_encrypt, fill_random};
use chrono::{Local, TimeZone};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// A read worth keeping is at most this long, as JSON; the whole of what is kept at most `SNAPSHOT_TOTAL_CHARS`.
pub const SNAPSHOT_ENTRY_CHARS: usize = 512_000;
pub const SNAPSHOT_TOTAL_CHARS: usize = 4_000_000;

const AAD: &[u8] = b"locust-desk-snapshot-v1";

#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotAnswer {
    pub value: Value,
    pub at: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeskSnapshot {
    /// When it was last written, in milliseconds.
    pub at: u64,
    /// Each read's last answer, by its channel and arguments, with when it was kept.
    pub answers: BTreeMap<String, SnapshotAnswer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedSnapshot {
    pub iv: [u8; 12],
    pub ciphertext: Vec<u8>,
}

/// Key for one cached read: its channel and structured-clone encoded arguments.
pub fn snapshot_key(channel: &str, args: &[DeskValue]) -> String {
    format!("{channel}|{}", desk_encode_args(args))
}

/// The snapshot with one more answer in it: too long an answer is not kept, and the oldest go once it is too long.
pub fn snapshot_with(
    snapshot: Option<&DeskSnapshot>,
    key: &str,
    value: Value,
    now: u64,
) -> DeskSnapshot {
    let mut answers = snapshot.map(|s| s.answers.clone()).unwrap_or_default();
    let json_len = |v: &Value| v.to_string().chars().count();
    if json_len(&value) > SNAPSHOT_ENTRY_CHARS {
        answers.remove(key);
    } else {
        answers.insert(key.to_owned(), SnapshotAnswer { value, at: now });
        let sizes: BTreeMap<String, usize> = answers
            .iter()
            .map(|(name, entry)| (name.clone(), json_len(&entry.value)))
            .collect();
        let mut total: usize = sizes.values().sum();
        let mut by_age: Vec<(String, u64)> = answers
            .iter()
            .map(|(name, entry)| (name.clone(), entry.at))
            .collect();
        by_age.sort_by_key(|(_, at)| *at);
        for (name, _) in by_age {
            if total <= SNAPSHOT_TOTAL_CHARS {
                break;
            }
            if name == key {
                continue;
            }
            total = total.saturating_sub(sizes.get(&name).copied().unwrap_or(0));
            answers.remove(&name);
        }
    }
    DeskSnapshot { at: now, answers }
}

/// Generate a fresh 256-bit AES-GCM key using the OS RNG (`getrandom` via `relay_crypto`).
pub fn generate_snapshot_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    fill_random(&mut key);
    key
}

fn snapshot_to_json(snapshot: &DeskSnapshot) -> Value {
    let mut answers = Map::new();
    for (k, entry) in &snapshot.answers {
        answers.insert(
            k.clone(),
            json!({
                "value": entry.value,
                "at": entry.at,
            }),
        );
    }
    json!({
        "at": snapshot.at,
        "answers": Value::Object(answers),
    })
}

fn snapshot_from_json(raw: Value) -> Option<DeskSnapshot> {
    let obj = raw.as_object()?;
    let at = obj.get("at")?.as_u64()?;
    let answers_obj = obj.get("answers")?.as_object()?;
    let mut answers = BTreeMap::new();
    for (k, v) in answers_obj {
        let entry = v.as_object()?;
        let entry_at = entry.get("at")?.as_u64()?;
        let value = entry.get("value")?.clone();
        answers.insert(k.clone(), SnapshotAnswer { value, at: entry_at });
    }
    Some(DeskSnapshot { at, answers })
}

/// Sealed, so what is on disk is not the conversations as text (AES-256-GCM with AAD).
pub fn seal_snapshot(key: &[u8; 32], snapshot: &DeskSnapshot) -> Result<SealedSnapshot, String> {
    let mut iv = [0u8; 12];
    fill_random(&mut iv);
    let plain = snapshot_to_json(snapshot).to_string();
    let ciphertext = aes_gcm_encrypt(key, &iv, AAD, plain.as_bytes())?;
    Ok(SealedSnapshot { iv, ciphertext })
}

/// Open a sealed snapshot with its own key; returns `None` for any other key or tampered/invalid box.
pub fn open_snapshot(key: &[u8; 32], sealed: &SealedSnapshot) -> Option<DeskSnapshot> {
    let plain = aes_gcm_decrypt(key, &sealed.iv, AAD, &sealed.ciphertext).ok()?;
    let parsed: Value = serde_json::from_slice(&plain).ok()?;
    snapshot_from_json(parsed)
}

/// How long ago, as the line over the window says it: time only when today, ` on <day>` when another day.
pub fn snapshot_when(at_ms: i64, now_ms: i64) -> String {
    let Some(at_dt) = Local.timestamp_millis_opt(at_ms).single() else {
        return String::new();
    };
    let Some(now_dt) = Local.timestamp_millis_opt(now_ms).single() else {
        return String::new();
    };
    let time = at_dt.format("%-I:%M %p").to_string();
    if at_dt.date_naive() == now_dt.date_naive() {
        time
    } else {
        format!("{time} on {}", at_dt.format("%A, %b %-d"))
    }
}
