//! YOUR PHONE ON THE SAME WI-FI (Electron `main/phone-host.ts`, `main/phone-scout-cards.ts`, `main/ledger-writes.ts`,
//! `shared/phone.ts`).
//!
//! One small page served on the home network. Off means no listener and no pairing key. On, the page HTML itself is
//! served to anyone who asks (it holds nothing), and every `/api/*` request requires the 192-bit pairing key from the
//! QR code's `#fragment` in `Authorization: Bearer <key>`, checked in constant time over SHA-256 digests before any
//! request body is read.
//!
//! The pairing key is kept sealed in [`SecretBox`], never written in plain text to `phone.json`, never printed in `Debug`,
//! and scrubbed from errors via [`crate::secrets::scrub_secrets`]. Tests pass a fake `SecretBox` and a fake listener so
//! no test touches the real keychain or binds a real port.

use crate::own_models::SecretBox;
use crate::protocol_tags::unwrap_protocol_tags;
use crate::scout::types::{ScoutCard, ScoutOpen, ScoutUrgency};
use crate::secrets::scrub_secrets;
use regex::Regex;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub const PHONE_FILE: &str = "phone.json";
/// Fixed port so a phone's saved address still works after Locust restarts; any free port (`0`) when taken.
pub const PHONE_PORT: u16 = 47740;
/// Largest request body a phone may send.
pub const MAX_BODY: usize = 32 * 1024;
pub const MAX_MESSAGE: usize = 20_000;
/// Wrong keys from one remote address before it is refused for a while.
pub const MAX_WRONG: usize = 20;
pub const WRONG_WINDOW_MS: i64 = 10 * 60_000;

pub const PHONE_MAX_CONVERSATIONS: usize = 20;
pub const PHONE_MAX_TURNS: usize = 8;
pub const PHONE_MAX_ANSWER: usize = 8_000;
pub const PHONE_MAX_SCOUT_CARDS: usize = 10;
const SNIPPET_MAX: usize = 160;
const TITLE_MAX: usize = 120;

/// A 192-bit base64url pairing key whose `Debug` never leaks the secret.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingKey(String);

impl PairingKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PairingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PairingKey([redacted])")
    }
}

pub fn valid_pairing_key(key: &str) -> bool {
    key.len() == 32 && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn base64url_no_pad(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 0x3f) as usize] as char);
        }
    }
    out
}

pub fn generate_pairing_key() -> Result<PairingKey, String> {
    let mut bytes = [0u8; 24];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| format!("Could not generate pairing key: {e}"))?;
    Ok(PairingKey(base64url_no_pad(&bytes)))
}

/// Constant-time comparison over fixed-length 32-byte SHA-256 digests so unequal byte lengths (such as UTF-8 vs
/// Latin-1 multi-byte characters) never panic or leak length.
pub fn same_key(supplied: &[u8], expected: &str) -> bool {
    let a = Sha256::digest(supplied);
    let b = Sha256::digest(expected.as_bytes());
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Scrubs both known secret patterns and the active pairing key from any error message.
pub fn scrub_phone_error(err: &str, key: Option<&str>) -> String {
    let (scrubbed, _) = scrub_secrets(err);
    let mut text = crate::secrets::redact_secrets(&scrubbed);
    if let Some(k) = key.filter(|k| !k.is_empty()) {
        text = text.replace(k, "[redacted]");
    }
    text
}

// ---- shared/phone.ts & phone-host.ts data types --------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PhoneSettings {
    pub enabled: bool,
    pub url: Option<String>,
    pub addresses: Vec<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhonePlain {
    pub said: String,
    pub careful: bool,
}

impl PhonePlain {
    pub fn to_json(&self) -> Value {
        json!({ "said": self.said, "careful": self.careful })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhoneWaiting {
    pub approval_id: String,
    pub mission_id: String,
    pub summary: String,
    pub requested_at: String,
    pub name: Option<String>,
    pub exact: Option<String>,
    pub plain: Option<PhonePlain>,
}

impl PhoneWaiting {
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("approvalId".into(), json!(self.approval_id));
        obj.insert("missionId".into(), json!(self.mission_id));
        obj.insert("summary".into(), json!(self.summary));
        obj.insert("requestedAt".into(), json!(self.requested_at));
        if let Some(name) = &self.name {
            obj.insert("name".into(), json!(name));
        }
        if let Some(exact) = &self.exact {
            obj.insert("exact".into(), json!(exact));
        }
        if let Some(plain) = &self.plain {
            obj.insert("plain".into(), plain.to_json());
        }
        Value::Object(obj)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhoneConversation {
    pub mission_id: String,
    pub title: String,
    pub agent: String,
    pub live: bool,
    pub phase: String,
    pub at: String,
    pub snippet: Option<String>,
}

impl PhoneConversation {
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("missionId".into(), json!(self.mission_id));
        obj.insert("title".into(), json!(self.title));
        obj.insert("agent".into(), json!(self.agent));
        obj.insert("live".into(), json!(self.live));
        obj.insert("phase".into(), json!(self.phase));
        obj.insert("at".into(), json!(self.at));
        if let Some(snippet) = &self.snippet {
            obj.insert("snippet".into(), json!(snippet));
        }
        Value::Object(obj)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhoneTurn {
    pub mission_id: String,
    pub asked: Option<String>,
    pub answer: Option<String>,
    pub agent: String,
    pub at: String,
    pub phase: String,
}

impl PhoneTurn {
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("missionId".into(), json!(self.mission_id));
        if let Some(asked) = &self.asked {
            obj.insert("asked".into(), json!(asked));
        }
        if let Some(answer) = &self.answer {
            obj.insert("answer".into(), json!(answer));
        }
        obj.insert("agent".into(), json!(self.agent));
        obj.insert("at".into(), json!(self.at));
        obj.insert("phase".into(), json!(self.phase));
        Value::Object(obj)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhoneScoutCard {
    pub card_id: String,
    pub said: String,
    pub lines: Option<Vec<String>>,
    pub now: bool,
    pub at: String,
    pub url: Option<String>,
}

impl PhoneScoutCard {
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("cardId".into(), json!(self.card_id));
        obj.insert("said".into(), json!(self.said));
        if let Some(lines) = &self.lines {
            obj.insert("lines".into(), json!(lines));
        }
        obj.insert("now".into(), json!(self.now));
        obj.insert("at".into(), json!(self.at));
        if let Some(url) = &self.url {
            obj.insert("url".into(), json!(url));
        }
        Value::Object(obj)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PhoneState {
    pub waiting: Vec<PhoneWaiting>,
    pub conversations: Vec<PhoneConversation>,
    pub scout: Option<Vec<PhoneScoutCard>>,
}

impl PhoneState {
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert(
            "waiting".into(),
            Value::Array(self.waiting.iter().map(PhoneWaiting::to_json).collect()),
        );
        obj.insert(
            "conversations".into(),
            Value::Array(self.conversations.iter().map(PhoneConversation::to_json).collect()),
        );
        if let Some(scout) = &self.scout {
            obj.insert(
                "scout".into(),
                Value::Array(scout.iter().map(PhoneScoutCard::to_json).collect()),
            );
        }
        Value::Object(obj)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhoneSent {
    Ok,
    Err { message: String },
}

impl PhoneSent {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Ok => json!({ "ok": true }),
            Self::Err { message } => json!({ "ok": false, "message": message }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhoneAnswer {
    pub status: u16,
    pub data: Value,
}

/// What the phone host asks of Locust (state, turns, card answer, send, dismiss Scout card).
pub trait PhoneCalls: Send + Sync {
    fn state(&self) -> PhoneState;
    fn turns(&self, mission_id: &str) -> Option<Vec<PhoneTurn>>;
    fn answer_card(&self, approval_id: &str, approve: bool) -> bool;
    fn send(&self, mission_id: &str, text: &str) -> PhoneSent;
    fn dismiss_scout(&self, _card_id: &str) -> Option<bool> {
        None
    }
}

// ---- phone-scout-cards.ts ------------------------------------------------------------------------------------------

fn github_card_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^https://github\.com/[\w./#?=&%-]+$").expect("valid github url regex")
    })
}

fn clip_chars(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        text.to_owned()
    } else {
        let head: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

/// Scout's cards as the phone shows them (`main/phone-scout-cards.ts`): newest first, up to 10, GitHub link only,
/// never the card's quoted command, Scout's model's take, or a draft for a teammate.
pub fn phone_scout_cards(cards: &[ScoutCard]) -> Vec<PhoneScoutCard> {
    let mut sorted: Vec<&ScoutCard> = cards.iter().collect();
    sorted.sort_by(|a, b| b.at.cmp(&a.at));
    sorted
        .into_iter()
        .take(PHONE_MAX_SCOUT_CARDS)
        .map(|card| {
            let url = match &card.fact.open {
                Some(ScoutOpen::Url { url, .. }) if github_card_url_re().is_match(url) => {
                    Some(url.clone())
                }
                _ => None,
            };
            let lines = card
                .fact
                .lines
                .as_ref()
                .filter(|l| !l.is_empty())
                .map(|l| l.iter().take(8).cloned().collect());
            PhoneScoutCard {
                card_id: card.card_id.clone(),
                said: clip_chars(&card.fact.said, 400),
                lines,
                now: card.fact.urgency == ScoutUrgency::Now,
                at: card.at.clone(),
                url,
            }
        })
        .collect()
}

// ---- shared/phone.ts conversations, turns, and answerOf ------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct PhoneMission {
    pub mission_id: String,
    pub prompt: String,
    pub runtime: String,
    pub created_at: String,
    pub last_updated_at: String,
    pub phase: String,
    pub continues_from_mission_id: Option<String>,
    pub continues_from_reason: Option<String>,
    pub started_by_kind: Option<String>,
    pub events: Vec<Value>,
}

/// The answer as Locust's own MCP tools read it (the last whole message), or the one still arriving on a live turn.
pub fn answer_of(events: &[Value]) -> Option<String> {
    let mut joined: Vec<(String, String, bool)> = Vec::new();
    for event in events {
        if event["type"] != "message.delta" {
            continue;
        }
        let payload = &event["payload"];
        let item_id = payload["itemId"].as_str().unwrap_or_default().to_owned();
        let text = payload["text"].as_str().unwrap_or_default();
        let fin = payload["final"] == true;
        if let Some(existing) = joined.iter_mut().find(|(id, _, _)| id == &item_id) {
            if payload["operation"] == "replace" {
                existing.1 = text.to_owned();
            } else {
                existing.1.push_str(text);
            }
            existing.2 = existing.2 || fin;
        } else {
            joined.push((item_id, text.to_owned(), fin));
        }
    }
    let chosen = joined
        .iter()
        .rfind(|(_, _, fin)| *fin)
        .or_else(|| joined.last())?;
    let unwrapped = unwrap_protocol_tags(&chosen.1).trim().to_owned();
    if unwrapped.is_empty() {
        return None;
    }
    if unwrapped.chars().count() > PHONE_MAX_ANSWER {
        let prefix: String = unwrapped.chars().take(PHONE_MAX_ANSWER).collect();
        Some(format!("{prefix}…"))
    } else {
        Some(unwrapped)
    }
}

fn asked_of(mission: &PhoneMission) -> Option<&str> {
    if let Some(kind) = mission.started_by_kind.as_deref()
        && kind != "terminal"
    {
        return None;
    }
    if mission.continues_from_reason.as_deref() == Some("route-switch") {
        return None;
    }
    Some(&mission.prompt)
}

fn one_line(text: &str, max: usize) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    clip_chars(&flat, max)
}

fn who_of(mission: &PhoneMission, names: Option<&dyn Fn(&str) -> Option<String>>) -> String {
    names
        .and_then(|f| f(&mission.mission_id))
        .unwrap_or_else(|| crate::carry_on::display_name(&mission.runtime).to_owned())
}

/// The conversations a phone lists (`shared/phone.ts::phoneConversations`): chains of turns, newest first, each named
/// by its first turn's words; side questions are excluded.
pub fn phone_conversations(
    missions: &[PhoneMission],
    live_mission_ids: &[String],
    names: Option<&dyn Fn(&str) -> Option<String>>,
) -> (Vec<PhoneConversation>, BTreeMap<String, Vec<PhoneMission>>) {
    let listed: Vec<&PhoneMission> = missions
        .iter()
        .filter(|m| m.started_by_kind.as_deref() != Some("side"))
        .collect();
    let by_id: HashMap<&str, &PhoneMission> =
        listed.iter().map(|m| (m.mission_id.as_str(), *m)).collect();

    let root_of = |mission: &PhoneMission| -> String {
        let mut seen = HashSet::new();
        let mut at = mission;
        loop {
            seen.insert(at.mission_id.as_str());
            let Some(prev_id) = at.continues_from_mission_id.as_deref() else {
                return at.mission_id.clone();
            };
            let Some(before) = by_id.get(prev_id) else {
                return at.mission_id.clone();
            };
            if seen.contains(before.mission_id.as_str()) {
                return at.mission_id.clone();
            }
            at = *before;
        }
    };

    let mut chains: BTreeMap<String, Vec<PhoneMission>> = BTreeMap::new();
    for mission in listed {
        let root = root_of(mission);
        chains.entry(root).or_default().push(mission.clone());
    }

    let live: HashSet<&str> = live_mission_ids.iter().map(String::as_str).collect();
    let mut list = Vec::new();
    for (root_id, turns) in &mut chains {
        turns.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let root = by_id.get(root_id.as_str()).copied().unwrap_or(&turns[0]);
        let newest = turns.last().expect("non-empty chain");
        let answer = answer_of(&newest.events);
        let title = asked_of(root)
            .or_else(|| turns.iter().find_map(|t| asked_of(t)))
            .unwrap_or("A conversation Locust started");
        let is_live = live.contains(newest.mission_id.as_str());
        list.push(PhoneConversation {
            mission_id: newest.mission_id.clone(),
            title: one_line(title, TITLE_MAX),
            agent: who_of(newest, names),
            live: is_live,
            phase: if is_live {
                "running".to_owned()
            } else {
                newest.phase.clone()
            },
            at: newest.last_updated_at.clone(),
            snippet: answer.map(|a| one_line(&a, SNIPPET_MAX)),
        });
    }
    list.sort_by(|a, b| b.at.cmp(&a.at));
    list.truncate(PHONE_MAX_CONVERSATIONS);
    (list, chains)
}

/// One conversation's newest turns, oldest first, from any of its turns (`shared/phone.ts::phoneTurns`).
pub fn phone_turns(
    chains: &BTreeMap<String, Vec<PhoneMission>>,
    mission_id: &str,
    live_mission_ids: &[String],
    names: Option<&dyn Fn(&str) -> Option<String>>,
) -> Option<Vec<PhoneTurn>> {
    let chain = chains
        .values()
        .find(|turns| turns.iter().any(|t| t.mission_id == mission_id))?;
    let live: HashSet<&str> = live_mission_ids.iter().map(String::as_str).collect();
    let skip = chain.len().saturating_sub(PHONE_MAX_TURNS);
    Some(
        chain[skip..]
            .iter()
            .map(|turn| PhoneTurn {
                mission_id: turn.mission_id.clone(),
                asked: asked_of(turn).map(str::to_owned),
                answer: answer_of(&turn.events),
                agent: who_of(turn, names),
                at: turn.created_at.clone(),
                phase: if live.contains(turn.mission_id.as_str()) {
                    "running".to_owned()
                } else {
                    turn.phase.clone()
                },
            })
            .collect(),
    )
}

// ---- ledger-writes.ts: write counter and keyedOnce cache -----------------------------------------------------------

/// Tracks writes before and after they land so a phone poll does not cache an in-flight write.
#[derive(Default)]
pub struct LedgerWriteCounter {
    count: AtomicU64,
}

impl LedgerWriteCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn writes(&self) -> u64 {
        self.count.load(Ordering::Acquire)
    }

    pub fn record_write<T>(&self, work: impl FnOnce() -> T) -> T {
        self.count.fetch_add(1, Ordering::AcqRel);
        let result = work();
        self.count.fetch_add(1, Ordering::AcqRel);
        result
    }

    pub fn begin_write(&self) {
        self.count.fetch_add(1, Ordering::AcqRel);
    }

    pub fn end_write(&self) {
        self.count.fetch_add(1, Ordering::AcqRel);
    }
}

/// One value, rebuilt only when its key changes; a build in flight is shared, and a failed build is not cached so the
/// next ask tries again.
pub struct KeyedOnce<T> {
    held: Mutex<Option<(String, T)>>,
}

impl<T: Clone> Default for KeyedOnce<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> KeyedOnce<T> {
    pub fn new() -> Self {
        Self { held: Mutex::new(None) }
    }

    pub fn get_or_build<E>(&self, key: &str, build: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
        // The lock is held through the build, so a second ask for the same key waits and shares it.
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((kept_key, val)) = held.as_ref()
            && kept_key == key
        {
            return Ok(val.clone());
        }
        let val = build()?;
        *held = Some((key.to_owned(), val.clone()));
        Ok(val)
    }
}

// ---- Network interfaces & addresses --------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkInterface {
    pub name: String,
    pub address: String,
    pub is_ipv4: bool,
    pub internal: bool,
}

fn virtual_interface_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)vethernet|virtualbox|vmware|hyper-v|wsl|docker|loopback|vbox")
            .expect("valid virtual interface regex")
    })
}

fn home_interface_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)wi-?fi|wlan|wireless|ethernet").expect("valid home interface regex")
    })
}

/// The addresses a phone on the same network can reach: IPv4 private ranges (and Tailscale's 100.64/10), never a
/// virtual switch's, home Wi-Fi and Ethernet first.
pub fn phone_addresses(interfaces: &[NetworkInterface]) -> Vec<String> {
    let mut found: Vec<(u8, usize, String)> = Vec::new();
    for (idx, entry) in interfaces.iter().enumerate() {
        if !entry.is_ipv4 || entry.internal || virtual_interface_re().is_match(&entry.name) {
            continue;
        }
        let mut parts = entry.address.split('.');
        let (Some(a), Some(b), Some(_), Some(_), None) = (
            parts.next().and_then(|s| s.parse::<u8>().ok()),
            parts.next().and_then(|s| s.parse::<u8>().ok()),
            parts.next().and_then(|s| s.parse::<u8>().ok()),
            parts.next().and_then(|s| s.parse::<u8>().ok()),
            parts.next(),
        ) else {
            continue;
        };
        let home = if a == 192 && b == 168 {
            0
        } else if a == 10 {
            1
        } else if a == 172 && (16..=31).contains(&b) {
            2
        } else if a == 100 && (64..=127).contains(&b) {
            3
        } else {
            continue;
        };
        let bonus = if home_interface_re().is_match(&entry.name) { 0 } else { 1 };
        found.push((home * 2 + bonus, idx, entry.address.clone()));
    }
    found.sort_by_key(|(rank, idx, _)| (*rank, *idx));
    let mut dedup = Vec::new();
    for (_, _, addr) in found {
        if !dedup.contains(&addr) {
            dedup.push(addr);
        }
    }
    dedup
}

// ---- HTTP request validation & routing -----------------------------------------------------------------------------

fn host_header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:\d{1,3}(?:\.\d{1,3}){3}|localhost|127\.0\.0\.1)(?::\d{1,5})?$")
            .expect("valid host regex")
    })
}

fn card_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^card_[0-9a-f]{12}$").expect("valid cardId regex"))
}

pub fn valid_host_header(host: &str) -> bool {
    host_header_re().is_match(host)
}

pub fn page_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("content-type", "text/html; charset=utf-8"),
        ("cache-control", "no-store"),
        ("referrer-policy", "no-referrer"),
        ("x-content-type-options", "nosniff"),
        (
            "content-security-policy",
            "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; manifest-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    ]
}

pub fn json_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("content-type", "application/json; charset=utf-8"),
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
    ]
}

/// Routes one authenticated API call (`main/phone-host.ts::answerPhone`).
pub fn answer_phone(
    calls: &dyn PhoneCalls,
    method: &str,
    path: &str,
    input: Option<&Value>,
) -> Option<PhoneAnswer> {
    let prefixed = if path.starts_with('/') {
        format!("http://phone{path}")
    } else {
        format!("http://phone/{path}")
    };
    let parsed = url::Url::parse(&prefixed).ok()?;
    let pathname = parsed.path();
    if method == "GET" {
        if pathname == "/api/state" {
            return Some(PhoneAnswer {
                status: 200,
                data: calls.state().to_json(),
            });
        }
        if pathname == "/api/turns" {
            let id = parsed
                .query_pairs()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default();
            return Some(match calls.turns(&id) {
                None => PhoneAnswer {
                    status: 404,
                    data: json!({ "message": "That conversation is not in Locust any more." }),
                },
                Some(turns) => PhoneAnswer {
                    status: 200,
                    data: json!({ "turns": turns.iter().map(PhoneTurn::to_json).collect::<Vec<_>>() }),
                },
            });
        }
        return None;
    }
    let fields = input.and_then(Value::as_object);
    if pathname == "/api/decide"
        && let Some(obj) = fields
        && let (Some(approval_id), Some(approve)) = (
            obj.get("approvalId").and_then(Value::as_str),
            obj.get("approve").and_then(Value::as_bool),
        )
    {
        return Some(PhoneAnswer {
            status: 200,
            data: json!({ "ok": calls.answer_card(approval_id, approve) }),
        });
    }
    if pathname == "/api/scout/dismiss"
        && let Some(obj) = fields
        && let Some(card_id) = obj.get("cardId").and_then(Value::as_str)
        && card_id_re().is_match(card_id)
        && let Some(ok) = calls.dismiss_scout(card_id)
    {
        return Some(PhoneAnswer {
            status: 200,
            data: json!({ "ok": ok }),
        });
    }
    if pathname == "/api/send"
        && let Some(obj) = fields
        && let (Some(mission_id), Some(raw_text)) = (
            obj.get("missionId").and_then(Value::as_str),
            obj.get("text").and_then(Value::as_str),
        )
    {
        let text = raw_text.trim();
        if text.is_empty() || text.encode_utf16().count() > MAX_MESSAGE {
            return Some(PhoneAnswer {
                status: 400,
                data: json!({ "ok": false, "message": "Write a message first." }),
            });
        }
        return Some(PhoneAnswer {
            status: 200,
            data: calls.send(mission_id, text).to_json(),
        });
    }
    None
}

#[derive(Clone, Debug)]
pub struct PhoneHttpRequest {
    pub method: String,
    pub path: String,
    /// Lowercased header names; values kept as raw bytes so non-UTF8 Latin-1 headers never fail before auth.
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
    pub remote_addr: String,
}

impl PhoneHttpRequest {
    pub fn header_bytes(&self, name: &str) -> Option<&[u8]> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_slice())
    }

    pub fn header_str(&self, name: &str) -> Option<&str> {
        self.header_bytes(name).and_then(|b| std::str::from_utf8(b).ok())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhoneHttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl PhoneHttpResponse {
    fn empty(status: u16, extra_headers: &[(&str, &str)]) -> Self {
        Self {
            status,
            headers: extra_headers
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            body: Vec::new(),
        }
    }

    fn json(status: u16, value: &Value) -> Self {
        Self {
            status,
            headers: json_headers()
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
            body: value.to_string().into_bytes(),
        }
    }

    fn html(body: String) -> Self {
        Self {
            status: 200,
            headers: page_headers()
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
            body: body.into_bytes(),
        }
    }
}

/// Core request handler (`main/phone-host.ts::phoneRequestHandler`): checks `Host`, serves the page, enforces
/// per-IP wrong-key lockout, authenticates `Authorization: Bearer` in constant time before parsing any body, checks
/// `Content-Type` and body size, and re-checks the key before executing `POST`.
pub struct PhoneRequestHandler {
    key: Arc<dyn Fn() -> Option<String> + Send + Sync>,
    page: Arc<dyn Fn() -> String + Send + Sync>,
    calls: Arc<dyn PhoneCalls>,
    now_ms: Arc<dyn Fn() -> i64 + Send + Sync>,
    wrong: Mutex<HashMap<String, Vec<i64>>>,
}

impl PhoneRequestHandler {
    pub fn new(
        key: Arc<dyn Fn() -> Option<String> + Send + Sync>,
        page: Arc<dyn Fn() -> String + Send + Sync>,
        calls: Arc<dyn PhoneCalls>,
        now_ms: Arc<dyn Fn() -> i64 + Send + Sync>,
    ) -> Self {
        Self {
            key,
            page,
            calls,
            now_ms,
            wrong: Mutex::new(HashMap::new()),
        }
    }

    /// Checks `Host`, serves `/` and `/index.html`, or authenticates `/api/*` headers BEFORE reading the body.
    /// Returns `Ok(response)` if handled without needing a body, or `Err(expected_key)` to proceed to reading body.
    pub fn check_head(&self, req: &PhoneHttpRequest) -> Result<PhoneHttpResponse, String> {
        let host = req.header_str("host").unwrap_or("");
        if !valid_host_header(host) {
            return Ok(PhoneHttpResponse::empty(421, &[("connection", "close")]));
        }
        let parsed = url::Url::parse(&format!(
            "http://phone{}",
            if req.path.starts_with('/') {
                req.path.clone()
            } else {
                format!("/{}", req.path)
            }
        ))
        .unwrap_or_else(|_| url::Url::parse("http://phone/").expect("valid fallback url"));
        let pathname = parsed.path();
        if req.method == "GET" && (pathname == "/" || pathname == "/index.html") {
            return Ok(PhoneHttpResponse::html((self.page)()));
        }
        if !pathname.starts_with("/api/") {
            return Ok(PhoneHttpResponse::empty(404, &[]));
        }
        let now = (self.now_ms)();
        let mut wrong = self.wrong.lock().unwrap_or_else(|e| e.into_inner());
        let recent: Vec<i64> = wrong
            .get(&req.remote_addr)
            .into_iter()
            .flatten()
            .copied()
            .filter(|at| now - *at < WRONG_WINDOW_MS)
            .collect();
        if recent.len() >= MAX_WRONG {
            wrong.insert(req.remote_addr.clone(), recent);
            return Ok(PhoneHttpResponse::json(
                429,
                &json!({ "message": "Too many wrong keys. Scan the code in Locust again in a few minutes." }),
            ));
        }
        let expected = (self.key)();
        let supplied = req
            .header_bytes("authorization")
            .and_then(|b| b.strip_prefix(b"Bearer "))
            .unwrap_or(&[]);
        let authed = expected
            .as_deref()
            .is_some_and(|exp| same_key(supplied, exp));
        if !authed {
            let mut next = recent;
            next.push(now);
            wrong.insert(req.remote_addr.clone(), next);
            return Ok(PhoneHttpResponse::json(
                401,
                &json!({ "message": "This phone is not paired. Scan the code in Locust’s Settings again." }),
            ));
        }
        Err(expected.expect("authed implies expected is Some"))
    }

    /// Handles a complete request (`check_head` + method + content-type + body + key-recheck + `answer_phone`).
    pub fn handle(&self, req: &PhoneHttpRequest) -> PhoneHttpResponse {
        let expected = match self.check_head(req) {
            Ok(resp) => return resp,
            Err(exp) => exp,
        };
        let target = if req.path.starts_with('/') {
            req.path.as_str()
        } else {
            "/"
        };
        let pathname = url::Url::parse(&format!("http://phone{target}"))
            .map(|u| u.path().to_owned())
            .unwrap_or_else(|_| "/".to_owned());
        if req.method == "GET" {
            if let Some(answer) = answer_phone(self.calls.as_ref(), "GET", target, None) {
                return PhoneHttpResponse::json(answer.status, &answer.data);
            }
        }
        if req.method != "POST" {
            return PhoneHttpResponse::empty(405, &[]);
        }
        let content_type = req.header_str("content-type").unwrap_or("");
        if !content_type.starts_with("application/json") {
            return PhoneHttpResponse::empty(415, &[]);
        }
        if req.body.len() > MAX_BODY {
            return PhoneHttpResponse::json(
                400,
                &json!({ "message": "That message could not be read." }),
            );
        }
        let Ok(input) = serde_json::from_slice::<Value>(&req.body) else {
            return PhoneHttpResponse::json(
                400,
                &json!({ "message": "That message could not be read." }),
            );
        };
        if (self.key)().as_deref() != Some(expected.as_str()) {
            return PhoneHttpResponse::json(
                401,
                &json!({ "message": "This phone was unpaired." }),
            );
        }
        match answer_phone(self.calls.as_ref(), "POST", &pathname, Some(&input)) {
            Some(answer) => PhoneHttpResponse::json(answer.status, &answer.data),
            None => PhoneHttpResponse::empty(404, &[]),
        }
    }
}

// ---- Listener abstraction + PhoneHost ------------------------------------------------------------------------------

pub trait PhoneBound: Send + Sync {
    fn port(&self) -> u16;
    fn stop(&self);
}

pub trait PhoneListenerFactory: Send + Sync {
    fn bind(&self, port: u16, handler: Arc<PhoneRequestHandler>) -> Result<Box<dyn PhoneBound>, String>;
}

/// Writes `text` to an owner-only (`0o600` on Unix) temp file created fresh, then renames into place.
fn write_private_file(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&temp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&temp, path)
}

#[derive(Default)]
struct HostState {
    bound: Option<Box<dyn PhoneBound>>,
    port: Option<u16>,
    key: Option<PairingKey>,
    sealed_key: Option<String>,
    disposed: bool,
}

pub struct PhoneHostOptions {
    pub directory: PathBuf,
    pub page: Arc<dyn Fn() -> String + Send + Sync>,
    pub calls: Arc<dyn PhoneCalls>,
    pub secrets: Arc<dyn SecretBox>,
    pub listener: Arc<dyn PhoneListenerFactory>,
    pub refusal: Arc<dyn Fn() -> Option<String> + Send + Sync>,
    pub addresses: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
    pub port: u16,
    pub now_ms: Arc<dyn Fn() -> i64 + Send + Sync>,
}

/// Owns the local Wi-Fi phone listener, its `SecretBox`-protected pairing key, and `phone.json` persistence.
pub struct PhoneHost {
    file: PathBuf,
    page: Arc<dyn Fn() -> String + Send + Sync>,
    calls: Arc<dyn PhoneCalls>,
    secrets: Arc<dyn SecretBox>,
    listener: Arc<dyn PhoneListenerFactory>,
    refusal: Arc<dyn Fn() -> Option<String> + Send + Sync>,
    addresses: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
    preferred_port: u16,
    now_ms: Arc<dyn Fn() -> i64 + Send + Sync>,
    state: Arc<Mutex<HostState>>,
    /// Serializes `load`, `set_enabled`, `forget`, and `dispose`.
    ops: Mutex<()>,
}

impl PhoneHost {
    pub fn new(options: PhoneHostOptions) -> Self {
        Self {
            file: options.directory.join(PHONE_FILE),
            page: options.page,
            calls: options.calls,
            secrets: options.secrets,
            listener: options.listener,
            refusal: options.refusal,
            addresses: options.addresses,
            preferred_port: options.port,
            now_ms: options.now_ms,
            state: Arc::new(Mutex::new(HostState::default())),
            ops: Mutex::new(()),
        }
    }

    fn build_settings(&self, st: &HostState, message: Option<String>) -> PhoneSettings {
        let found = (self.addresses)();
        let enabled = st.bound.is_some();
        let url = if enabled
            && let (Some(key), Some(port), Some(first)) = (&st.key, st.port, found.first())
        {
            Some(format!("http://{first}:{port}/#{}", key.as_str()))
        } else {
            None
        };
        let addresses = match st.port {
            Some(port) => found.iter().map(|a| format!("http://{a}:{port}")).collect(),
            None => Vec::new(),
        };
        PhoneSettings {
            enabled,
            url,
            addresses,
            message: message.or_else(|| (self.refusal)()),
        }
    }

    pub fn settings(&self) -> PhoneSettings {
        let _op = self.ops.lock().unwrap_or_else(|e| e.into_inner());
        let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.build_settings(&st, None)
    }

    fn save_state(&self, enabled: bool, sealed: Option<&str>) -> Result<(), String> {
        let mut obj = Map::new();
        obj.insert("enabled".into(), json!(enabled));
        if let Some(s) = sealed {
            obj.insert("key".into(), json!(s));
        }
        write_private_file(&self.file, &Value::Object(obj).to_string())
            .map_err(|e| scrub_phone_error(&e.to_string(), None))
    }

    fn start_locked(&self, st: &mut HostState) -> Result<(), String> {
        if st.bound.is_some() {
            return Ok(());
        }
        if st.key.is_none() {
            if !self.secrets.available() {
                return Err("Locust could not open its page on your network. It is still off.".into());
            }
            let generated = generate_pairing_key()?;
            let sealed = self
                .secrets
                .seal(generated.as_str())
                .map_err(|e| scrub_phone_error(&e, Some(generated.as_str())))?;
            st.key = Some(generated);
            st.sealed_key = Some(sealed);
        }
        let state_ref = self.state.clone();
        let handler = Arc::new(PhoneRequestHandler::new(
            Arc::new(move || {
                state_ref
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .key
                    .as_ref()
                    .map(|k| k.as_str().to_owned())
            }),
            self.page.clone(),
            self.calls.clone(),
            self.now_ms.clone(),
        ));
        let bound = match self.listener.bind(self.preferred_port, handler.clone()) {
            Ok(b) => b,
            Err(_) if self.preferred_port != 0 => self.listener.bind(0, handler)?,
            Err(e) => return Err(e),
        };
        st.port = Some(bound.port());
        st.bound = Some(bound);
        Ok(())
    }

    fn stop_locked(st: &mut HostState) {
        let bound = st.bound.take();
        st.port = None;
        if let Some(b) = bound {
            b.stop();
        }
    }

    pub fn load(&self) -> PhoneSettings {
        let _op = self.ops.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if st.disposed || st.bound.is_some() {
            return self.build_settings(&st, None);
        }
        if (self.refusal)().is_some() {
            return self.build_settings(&st, None);
        }
        if let Ok(text) = std::fs::read_to_string(&self.file)
            && let Ok(saved) = serde_json::from_str::<Value>(&text)
        {
            if let Some(sealed) = saved["key"].as_str().filter(|s| !s.is_empty())
                && self.secrets.available()
                && let Ok(opened) = self.secrets.open(sealed)
                && valid_pairing_key(&opened)
            {
                st.key = Some(PairingKey(opened));
                st.sealed_key = Some(sealed.to_owned());
            }
            if saved["enabled"] == true && st.key.is_some() {
                let _ = self.start_locked(&mut st);
            }
        }
        self.build_settings(&st, None)
    }

    pub fn set_enabled(&self, enabled: bool) -> PhoneSettings {
        let _op = self.ops.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if st.disposed {
            return self.build_settings(&st, Some("That setting was not changed.".into()));
        }
        if enabled && (self.refusal)().is_some() {
            return self.build_settings(&st, None);
        }
        if enabled {
            let had_key = st.sealed_key.is_some();
            if let Err(err) = self.start_locked(&mut st) {
                Self::stop_locked(&mut st);
                if !had_key {
                    if let Some(sealed) = st.sealed_key.take() {
                        self.secrets.forget(&sealed);
                    }
                    st.key = None;
                }
                let _ = err;
                return self.build_settings(
                    &st,
                    Some("Locust could not open its page on your network. It is still off.".into()),
                );
            }
            let sealed = st.sealed_key.clone();
            if self.save_state(true, sealed.as_deref()).is_err() {
                Self::stop_locked(&mut st);
                if !had_key {
                    if let Some(old) = st.sealed_key.take() {
                        self.secrets.forget(&old);
                    }
                    st.key = None;
                }
                return self.build_settings(
                    &st,
                    Some("Locust could not open its page on your network. It is still off.".into()),
                );
            }
            self.build_settings(&st, None)
        } else {
            Self::stop_locked(&mut st);
            let old_sealed = st.sealed_key.take();
            st.key = None;
            if self.save_state(false, None).is_err() {
                if let Some(s) = old_sealed {
                    self.secrets.forget(&s);
                }
                return self.build_settings(
                    &st,
                    Some("Locust could not open its page on your network. It is still off.".into()),
                );
            }
            if let Some(s) = old_sealed {
                self.secrets.forget(&s);
            }
            self.build_settings(&st, None)
        }
    }

    /// Generates a new pairing key, sealing it into `SecretBox`; every previously paired phone is unpaired at once.
    pub fn forget(&self) -> PhoneSettings {
        let _op = self.ops.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if st.disposed {
            return self.build_settings(&st, None);
        }
        if !self.secrets.available() {
            return self.build_settings(
                &st,
                Some("The new code could not be saved. The old one still works.".into()),
            );
        }
        let Ok(next) = generate_pairing_key() else {
            return self.build_settings(
                &st,
                Some("The new code could not be saved. The old one still works.".into()),
            );
        };
        let Ok(sealed_next) = self.secrets.seal(next.as_str()) else {
            return self.build_settings(
                &st,
                Some("The new code could not be saved. The old one still works.".into()),
            );
        };
        let enabled = st.bound.is_some();
        if self.save_state(enabled, Some(&sealed_next)).is_err() {
            self.secrets.forget(&sealed_next);
            return self.build_settings(
                &st,
                Some("The new code could not be saved. The old one still works.".into()),
            );
        }
        let old_sealed = st.sealed_key.replace(sealed_next);
        st.key = Some(next);
        if let Some(old) = old_sealed {
            self.secrets.forget(&old);
        }
        self.build_settings(&st, None)
    }

    pub fn dispose(&self) {
        let _op = self.ops.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.disposed = true;
        Self::stop_locked(&mut st);
    }
}

impl Drop for PhoneHost {
    fn drop(&mut self) {
        self.dispose();
    }
}

#[cfg(test)]
#[path = "phone_host_tests.rs"]
mod tests;
