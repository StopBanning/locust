//! YOUR PHONE, FROM ANYWHERE (Electron `main/phone-anywhere.ts`, 0.826).
//!
//! Connects out to Locust's relay and keeps one quiet connection open so paired phones and browsers
//! reach Locust from anywhere. Every request and answer is sealed end-to-end (`relay_crypto.rs`);
//! the relay passes along what it cannot read and keeps only hashes of tokens.

use crate::desk::{DESK_PART_CHARS, DeskJoiner, DeskPart, desk_parts};
use crate::phone_alerts::{AlertKind, PhoneAlert};
use crate::phone_host::{MAX_BODY, PhoneCalls, answer_phone};
use crate::remote_mcp::{AskKind, RemoteMcpAsk, RemoteMcpReply};
use crate::relay_crypto::{
    Direction, LinkKeys, LinkSide, aad_of, device_id_of, from_base64url, link_keys, new_pc_keys,
    open_json, pairing_proof, pairing_token, random_text, same_text, seal_json, sha256_b64url,
    to_base64url,
};
use crate::web_push::{PushSubscriptionKeys, encrypt_push};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PHONE_ANYWHERE_FILE: &str = "phone-relay.json";
pub const DEFAULT_RELAY: &str = "https://app.locust.lol";
pub const CODE_MS: i64 = 10 * 60_000;
const MAX_SKEW_MS: i64 = 10 * 60_000;
const COMMAND_MS: u64 = 10_000;
const MAX_NAME: usize = 40;
pub const NOTE_SENDS_WITHIN_MS: i64 = 12 * 60 * 60_000;
pub const NOTE_CHARS: usize = 8_000;
const NOTE_RETRY_MS: u64 = 60_000;
const NOTES_TAKEN_KEPT: usize = 200;
const MAX_FRAME_BYTES: usize = 720 * 1024;

pub const AWAY_FOUND_ALERT_TITLE: &str = "Scout";
pub const AWAY_FOUND_ALERT_BODY: &str = "Scout found something on GitHub while your computer is off.";
pub const AWAY_FOUND_ALERT_TAG: &str = "scout-away";

pub const AWAY_BRIEF_ALERT_TITLE: &str = "Scout";
pub const AWAY_BRIEF_ALERT_BODY: &str = "Your morning brief is ready.";
pub const AWAY_BRIEF_ALERT_TAG: &str = "scout-brief";

pub fn away_found_alert_json() -> Value {
    json!({
        "title": AWAY_FOUND_ALERT_TITLE,
        "body": AWAY_FOUND_ALERT_BODY,
        "tag": AWAY_FOUND_ALERT_TAG,
    })
}

pub fn away_brief_alert_json() -> Value {
    json!({
        "title": AWAY_BRIEF_ALERT_TITLE,
        "body": AWAY_BRIEF_ALERT_BODY,
        "tag": AWAY_BRIEF_ALERT_TAG,
    })
}

// ---- Public types ---------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingCode {
    pub url: String,
    pub until: String,
    pub desk: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairedDeviceView {
    pub id: String,
    pub name: String,
    pub added: String,
    pub seen: Option<String>,
    pub online: bool,
    pub alerts: bool,
    pub desk: Option<bool>,
    pub github: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhoneAnywhereSettings {
    pub enabled: bool,
    pub connected: bool,
    pub code: Option<PairingCode>,
    pub devices: Vec<PairedDeviceView>,
    pub github_login: Option<String>,
    pub message: Option<String>,
    pub night_approvals: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayLetter {
    pub id: String,
    pub at: i64,
    pub from: String,
    pub n: String,
    pub c: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookLetter {
    pub id: String,
    pub at: i64,
    pub hook: String,
    pub body: String,
    pub content_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookLettersPage {
    pub items: Vec<HookLetter>,
    pub lost: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LettersPage {
    pub items: Vec<RelayLetter>,
    pub more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteBox {
    pub id: String,
    pub at: i64,
    pub device: String,
    pub n: String,
    pub c: String,
    pub held: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteDelivery {
    Sent,
    Busy,
    Old,
    Gone,
    Failed,
}

impl NoteDelivery {
    fn held_str(self) -> Option<&'static str> {
        match self {
            Self::Old => Some("old"),
            Self::Gone => Some("gone"),
            Self::Failed => Some("failed"),
            Self::Sent | Self::Busy => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwayForPhone {
    pub key: String,
    pub repo: String,
}

// ---- Transport & callback traits ------------------------------------------------------------------------------------

pub trait RelaySocketCallbacks: Send + Sync {
    fn on_open(&self);
    fn on_message(&self, text: String);
    fn on_close(&self);
}

pub trait RelaySocket: Send + Sync {
    fn send(&self, text: String) -> Result<(), String>;
    fn close(&self);
    fn set_callbacks(&self, callbacks: Arc<dyn RelaySocketCallbacks>);
}

pub type RelayConnector = Arc<dyn Fn(&str) -> Result<Arc<dyn RelaySocket>, String> + Send + Sync>;

pub trait AwayCallbacks: Send + Sync {
    fn for_phone(&self) -> Option<AwayForPhone>;
    fn refresh(&self);
    fn letter(&self, letter: RelayLetter);
}

pub trait HooksCallbacks: Send + Sync {
    fn refresh(&self);
}

pub trait DeskCallbacks: Send + Sync {
    fn frame(&self, device: &str, asks: Vec<Value>);
    fn forget(&self, device: &str);
}

pub trait NotesCallbacks: Send + Sync {
    fn deliver(&self, mission_id: &str, text: &str) -> NoteDelivery;
}

/// Other apps from anywhere (0.841, remote-mcp.ts): an app's request the relay passed on, answered here, and the relay
/// told again whether it may take apps each time this computer connects.
pub trait McpCallbacks: Send + Sync {
    fn answer(&self, asked: RemoteMcpAsk) -> RemoteMcpReply;
    fn sync(&self);
}

pub struct PhoneAnywhereOptions {
    pub directory: PathBuf,
    pub calls: Arc<dyn PhoneCalls>,
    pub relay: Arc<dyn Fn() -> String + Send + Sync>,
    pub refusal: Option<Arc<dyn Fn() -> Option<String> + Send + Sync>>,
    pub changed: Option<Arc<dyn Fn(PhoneAnywhereSettings) + Send + Sync>>,
    pub connect: RelayConnector,
    pub now: Option<Arc<dyn Fn() -> i64 + Send + Sync>>,
    pub protect_file: Option<Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>>,
    pub retry_ms: Option<Arc<dyn Fn(u32) -> u64 + Send + Sync>>,
    pub away: Option<Arc<dyn AwayCallbacks>>,
    pub hooks: Option<Arc<dyn HooksCallbacks>>,
    pub desk: Option<Arc<dyn DeskCallbacks>>,
    pub notes: Option<Arc<dyn NotesCallbacks>>,
    pub github_user: Option<Arc<dyn Fn(&str) -> Option<u64> + Send + Sync>>,
    pub computer_name: Option<Arc<dyn Fn() -> Option<String> + Send + Sync>>,
    pub mcp: Option<Arc<dyn McpCallbacks>>,
}

// ---- Persisted state ------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct SavedDevice {
    id: String,
    name: String,
    key: String,
    added: i64,
    seen: Option<i64>,
    seq: u64,
    push: Option<PushSubscriptionKeys>,
    desk: bool,
    desk_seen: HashMap<String, u64>,
    via: Option<String>,
}

#[derive(Clone, Debug)]
struct Saved {
    relay: String,
    room: String,
    token: String,
    private_key: Option<[u8; 32]>,
    public_key: String,
    devices: Vec<SavedDevice>,
    night_approvals: bool,
    notes_taken: Vec<String>,
    closing: bool,
    github: Option<(u64, String)>,
}

#[derive(Clone, Debug)]
struct PairingState {
    secret: String,
    hash: String,
    until: i64,
    used: bool,
    desk: bool,
}

#[derive(Clone, Debug)]
enum CommandAnswer {
    Ok,
    Err(String),
    Failed,
}

struct PendingWaiters {
    commands: HashMap<String, std::sync::mpsc::Sender<CommandAnswer>>,
    letters: HashMap<String, std::sync::mpsc::Sender<Option<LettersPage>>>,
    hook_letters: HashMap<String, std::sync::mpsc::Sender<Option<HookLettersPage>>>,
    notes: HashMap<String, std::sync::mpsc::Sender<Option<Vec<NoteBox>>>>,
}

impl PendingWaiters {
    fn new() -> Self {
        Self {
            commands: HashMap::new(),
            letters: HashMap::new(),
            hook_letters: HashMap::new(),
            notes: HashMap::new(),
        }
    }

    fn fail_all(&mut self) {
        for (_, tx) in self.commands.drain() {
            let _ = tx.send(CommandAnswer::Failed);
        }
        for (_, tx) in self.letters.drain() {
            let _ = tx.send(None);
        }
        for (_, tx) in self.hook_letters.drain() {
            let _ = tx.send(None);
        }
        for (_, tx) in self.notes.drain() {
            let _ = tx.send(None);
        }
    }
}

struct InnerState {
    saved: Option<Saved>,
    keys: HashMap<String, LinkKeys>,
    socket: Option<(u64, Arc<dyn RelaySocket>)>,
    connected: bool,
    refused_by_relay: bool,
    online: HashSet<String>,
    pairing: Option<PairingState>,
    message: Option<String>,
    attempt: u32,
    retry_scheduled: bool,
    disposed: bool,
    desk_session: String,
    desk_sent: u64,
    desk_joins: HashMap<String, DeskJoiner>,
    collecting_notes: bool,
    collect_notes_again: bool,
    device_locks: HashMap<String, Arc<Mutex<()>>>,
}

struct PhoneAnywhereInner {
    file: PathBuf,
    options: PhoneAnywhereOptions,
    state: Mutex<InnerState>,
    pending: Mutex<PendingWaiters>,
    write_lock: Mutex<()>,
    serial_lock: Mutex<()>,
    // Last acknowledged app opening, scoped to a single authenticated socket.
    mcp_opened_on: Mutex<Option<u64>>,
    conn_seq: AtomicU64,
    cmd_seq: AtomicU64,
    in_flight: Mutex<Vec<std::thread::JoinHandle<()>>>,
    disposed_flag: AtomicBool,
}

#[derive(Clone)]
pub struct PhoneAnywhere {
    inner: Arc<PhoneAnywhereInner>,
}

// ---- Validation helpers ---------------------------------------------------------------------------------------------

fn iso_ms(ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn default_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn clean_name(value: Option<&Value>) -> String {
    let raw = value.and_then(Value::as_str).unwrap_or("");
    let filtered: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '\'' | '.' | ',' | '(' | ')' | '-'))
        .collect();
    let collapsed = filtered.split_whitespace().collect::<Vec<_>>().join(" ");
    let clipped: String = collapsed.chars().take(MAX_NAME).collect();
    if clipped.is_empty() {
        "Phone".to_owned()
    } else {
        clipped
    }
}

fn is_b64url_len(s: &str, min: usize, max: usize) -> bool {
    let len = s.len();
    len >= min
        && len <= max
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn push_of(value: &Value) -> Option<PushSubscriptionKeys> {
    let obj = value.as_object()?;
    let endpoint = obj.get("endpoint").and_then(Value::as_str)?;
    if !endpoint.starts_with("https://")
        || endpoint.len() > 1024
        || endpoint.chars().any(char::is_whitespace)
    {
        return None;
    }
    let p256dh = obj.get("p256dh").and_then(Value::as_str)?;
    let auth = obj.get("auth").and_then(Value::as_str)?;
    if !is_b64url_len(p256dh, 87, 87) || !is_b64url_len(auth, 22, 22) {
        return None;
    }
    Some(PushSubscriptionKeys {
        endpoint: endpoint.to_owned(),
        p256dh: p256dh.to_owned(),
        auth: auth.to_owned(),
    })
}

fn letter_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d{13}-[A-Za-z0-9_-]{8}$").unwrap())
}

fn mcp_ask_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9_-]{16}$").unwrap())
}

fn hook_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9_-]{1,80}$").unwrap())
}

fn parse_letter(value: &Value) -> Option<RelayLetter> {
    let obj = value.as_object()?;
    let id = obj.get("id").and_then(Value::as_str)?;
    if !letter_id_re().is_match(id) {
        return None;
    }
    let at = obj.get("at").and_then(Value::as_i64)?;
    let from = obj.get("from").and_then(Value::as_str)?;
    if from != "away" && from != "pc" {
        return None;
    }
    let n = obj.get("n").and_then(Value::as_str)?;
    let c = obj.get("c").and_then(Value::as_str)?;
    Some(RelayLetter {
        id: id.to_owned(),
        at,
        from: from.to_owned(),
        n: n.to_owned(),
        c: c.to_owned(),
    })
}

fn parse_hook_letter(value: &Value) -> Option<HookLetter> {
    let obj = value.as_object()?;
    let id = obj.get("id").and_then(Value::as_str)?;
    let hook = obj.get("hook").and_then(Value::as_str)?;
    let at = obj.get("at").and_then(Value::as_i64)?;
    let body = obj.get("body").and_then(Value::as_str)?;
    let content_type = obj.get("contentType").and_then(Value::as_str)?;
    if !letter_id_re().is_match(id)
        || !hook_id_re().is_match(hook)
        || body.len() > MAX_FRAME_BYTES
        || content_type.len() > 200
    {
        return None;
    }
    Some(HookLetter {
        id: id.to_owned(),
        at,
        hook: hook.to_owned(),
        body: body.to_owned(),
        content_type: content_type.to_owned(),
    })
}

fn parse_note_box(value: &Value) -> Option<NoteBox> {
    let obj = value.as_object()?;
    let id = obj.get("id").and_then(Value::as_str)?;
    let at = obj.get("at").and_then(Value::as_i64)?;
    let device = obj.get("device").and_then(Value::as_str)?;
    let n = obj.get("n").and_then(Value::as_str)?;
    let c = obj.get("c").and_then(Value::as_str)?;
    let held = match obj.get("held") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if matches!(s.as_str(), "old" | "gone" | "failed") => {
            Some(s.clone())
        }
        _ => return None,
    };
    if !letter_id_re().is_match(id) || !is_b64url_len(device, 22, 22) {
        return None;
    }
    Some(NoteBox {
        id: id.to_owned(),
        at,
        device: device.to_owned(),
        n: n.to_owned(),
        c: c.to_owned(),
        held,
    })
}

fn is_desk_ask(value: &Value) -> Option<String> {
    let obj = value.as_object()?;
    let kind = obj.get("kind").and_then(Value::as_str)?;
    let id = obj.get("id").and_then(Value::as_str)?;
    if id.is_empty() || id.len() > 64 {
        return None;
    }
    match kind {
        "hello" | "take" => Some(id.to_owned()),
        "replay" if obj.get("what").and_then(Value::as_str) == Some("approvals") => {
            Some(id.to_owned())
        }
        "call"
            if obj.get("channel").and_then(Value::as_str).is_some()
                && obj.get("args").and_then(Value::as_str).is_some() =>
        {
            Some(id.to_owned())
        }
        _ => None,
    }
}

fn parse_desk_part(value: &Value) -> Option<DeskPart> {
    let obj = value.as_object()?;
    if obj.get("v").and_then(Value::as_u64) != Some(1)
        || obj.get("kind").and_then(Value::as_str) != Some("desk-part")
    {
        return None;
    }
    let id = obj.get("id").and_then(Value::as_str)?;
    if id.len() > 64 {
        return None;
    }
    let n = obj.get("n").and_then(Value::as_u64)? as usize;
    let of = obj.get("of").and_then(Value::as_u64)? as usize;
    let text = obj.get("text").and_then(Value::as_str)?;
    Some(DeskPart {
        v: 1,
        kind: "desk-part",
        id: id.to_owned(),
        n,
        of,
        text: text.to_owned(),
    })
}

struct WrittenNote {
    nid: String,
    mission_id: String,
    text: String,
    t: i64,
}

fn parse_written_note(value: &Value) -> Option<WrittenNote> {
    let obj = value.as_object()?;
    if obj.get("v").and_then(Value::as_u64) != Some(1)
        || obj.get("kind").and_then(Value::as_str) != Some("note")
    {
        return None;
    }
    let nid = obj.get("nid").and_then(Value::as_str)?;
    if !is_b64url_len(nid, 16, 32) {
        return None;
    }
    let mission_id = obj.get("missionId").and_then(Value::as_str)?;
    if mission_id.is_empty() || mission_id.len() > 200 {
        return None;
    }
    let text = obj.get("text").and_then(Value::as_str)?;
    if text.trim().is_empty() || text.chars().count() > NOTE_CHARS {
        return None;
    }
    let t = obj.get("t").and_then(Value::as_i64)?;
    Some(WrittenNote {
        nid: nid.to_owned(),
        mission_id: mission_id.to_owned(),
        text: text.to_owned(),
        t,
    })
}

// ---- JSON serialization of Saved ------------------------------------------------------------------------------------

impl Saved {
    fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("v".into(), json!(1));
        obj.insert("relay".into(), json!(self.relay));
        obj.insert("room".into(), json!(self.room));
        obj.insert("token".into(), json!(self.token));
        if let Some(pk) = self.private_key {
            obj.insert("key".into(), json!({ "kty": "OKP", "crv": "X25519", "d": to_base64url(&pk) }));
        } else {
            obj.insert("key".into(), json!({}));
        }
        obj.insert("publicKey".into(), json!(self.public_key));
        let devices: Vec<Value> = self
            .devices
            .iter()
            .map(|d| {
                let mut m = Map::new();
                m.insert("id".into(), json!(d.id));
                m.insert("name".into(), json!(d.name));
                m.insert("key".into(), json!(d.key));
                m.insert("added".into(), json!(d.added));
                if let Some(seen) = d.seen {
                    m.insert("seen".into(), json!(seen));
                }
                m.insert("seq".into(), json!(d.seq));
                if let Some(push) = &d.push {
                    m.insert(
                        "push".into(),
                        json!({
                            "endpoint": push.endpoint,
                            "p256dh": push.p256dh,
                            "auth": push.auth,
                        }),
                    );
                }
                if d.desk {
                    m.insert("desk".into(), json!(true));
                }
                if !d.desk_seen.is_empty() {
                    m.insert("deskSeen".into(), json!(d.desk_seen));
                }
                if let Some(via) = &d.via {
                    m.insert("via".into(), json!(via));
                }
                Value::Object(m)
            })
            .collect();
        obj.insert("devices".into(), Value::Array(devices));
        if self.night_approvals {
            obj.insert("nightApprovals".into(), json!(true));
        }
        if !self.notes_taken.is_empty() {
            obj.insert("notesTaken".into(), json!(self.notes_taken));
        }
        if self.closing {
            obj.insert("closing".into(), json!(true));
        }
        if let Some((user, login)) = &self.github {
            obj.insert("github".into(), json!({ "user": user, "login": login }));
        }
        Value::Object(obj)
    }

    fn from_json(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        if obj.get("v").and_then(Value::as_u64) != Some(1) {
            return None;
        }
        let relay = obj.get("relay").and_then(Value::as_str)?;
        if !(relay.starts_with("https://") || relay.starts_with("http://")) {
            return None;
        }
        let room = obj.get("room").and_then(Value::as_str)?;
        let token = obj.get("token").and_then(Value::as_str)?;
        let public_key = obj.get("publicKey").and_then(Value::as_str)?;
        if !is_b64url_len(room, 22, 22)
            || !is_b64url_len(token, 43, 43)
            || !is_b64url_len(public_key, 43, 43)
        {
            return None;
        }
        let key_obj = obj.get("key")?.as_object()?;
        let private_key = key_obj
            .get("d")
            .and_then(Value::as_str)
            .and_then(|d| from_base64url(d).ok())
            .and_then(|bytes| <[u8; 32]>::try_from(bytes.as_slice()).ok());
        let devices_arr = obj.get("devices").and_then(Value::as_array)?;
        let mut devices = Vec::new();
        for item in devices_arr {
            let d = match item.as_object() {
                Some(o) => o,
                None => continue,
            };
            let Some(id) = d.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(name) = d.get("name").and_then(Value::as_str) else {
                continue;
            };
            let Some(key) = d.get("key").and_then(Value::as_str) else {
                continue;
            };
            let added = d.get("added").and_then(Value::as_i64).unwrap_or(0);
            let seen = d.get("seen").and_then(Value::as_i64);
            let seq = d.get("seq").and_then(Value::as_u64).unwrap_or(0);
            let push = d.get("push").and_then(push_of);
            let desk = d.get("desk").and_then(Value::as_bool) == Some(true);
            let mut desk_seen = HashMap::new();
            if let Some(ds) = d.get("deskSeen").and_then(Value::as_object) {
                for (k, v) in ds {
                    if let Some(q) = v.as_u64() {
                        desk_seen.insert(k.clone(), q);
                    }
                }
            }
            let via = d.get("via").and_then(Value::as_str).map(str::to_owned);
            devices.push(SavedDevice {
                id: id.to_owned(),
                name: name.to_owned(),
                key: key.to_owned(),
                added,
                seen,
                seq,
                push,
                desk,
                desk_seen,
                via,
            });
        }
        let night_approvals = obj.get("nightApprovals").and_then(Value::as_bool) == Some(true);
        let notes_taken = obj
            .get("notesTaken")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let closing = obj.get("closing").and_then(Value::as_bool) == Some(true);
        let github = obj.get("github").and_then(Value::as_object).and_then(|g| {
            let user = g.get("user").and_then(Value::as_u64)?;
            let login = g.get("login").and_then(Value::as_str)?;
            Some((user, login.to_owned()))
        });
        Some(Self {
            relay: relay.to_owned(),
            room: room.to_owned(),
            token: token.to_owned(),
            private_key,
            public_key: public_key.to_owned(),
            devices,
            night_approvals,
            notes_taken,
            closing,
            github,
        })
    }
}

// ---- Socket callback bridge -----------------------------------------------------------------------------------------

struct SocketBridge {
    inner: Arc<PhoneAnywhereInner>,
    conn_id: u64,
    token: String,
}

impl RelaySocketCallbacks for SocketBridge {
    fn on_open(&self) {
        let sock = {
            let st = self.inner.state.lock().unwrap();
            match &st.socket {
                Some((id, s)) if *id == self.conn_id => Some(Arc::clone(s)),
                _ => None,
            }
        };
        if let Some(s) = sock {
            let _ = s.send(json!({ "t": "auth", "as": "pc", "token": self.token }).to_string());
        }
    }

    fn on_message(&self, text: String) {
        self.inner.heard_frame(self.conn_id, &text);
    }

    fn on_close(&self) {
        let should_reconnect = {
            let mut st = self.inner.state.lock().unwrap();
            match &st.socket {
                Some((id, _)) if *id == self.conn_id => {
                    st.socket = None;
                    st.connected = false;
                    st.online.clear();
                    true
                }
                _ => false,
            }
        };
        if should_reconnect {
            self.inner.pending.lock().unwrap().fail_all();
            self.inner.tell();
            self.inner.reconnect_later();
        }
    }
}

// ---- Implementation -------------------------------------------------------------------------------------------------

impl PhoneAnywhere {
    pub fn new(options: PhoneAnywhereOptions) -> Self {
        let file = options.directory.join(PHONE_ANYWHERE_FILE);
        Self {
            inner: Arc::new(PhoneAnywhereInner {
                file,
                options,
                state: Mutex::new(InnerState {
                    saved: None,
                    keys: HashMap::new(),
                    socket: None,
                    connected: false,
                    refused_by_relay: false,
                    online: HashSet::new(),
                    pairing: None,
                    message: None,
                    attempt: 0,
                    retry_scheduled: false,
                    disposed: false,
                    desk_session: random_text(9),
                    desk_sent: 0,
                    desk_joins: HashMap::new(),
                    collecting_notes: false,
                    collect_notes_again: false,
                    device_locks: HashMap::new(),
                }),
                pending: Mutex::new(PendingWaiters::new()),
                write_lock: Mutex::new(()),
                serial_lock: Mutex::new(()),
                mcp_opened_on: Mutex::new(None),
                conn_seq: AtomicU64::new(1),
                cmd_seq: AtomicU64::new(1),
                in_flight: Mutex::new(Vec::new()),
                disposed_flag: AtomicBool::new(false),
            }),
        }
    }

    pub fn settings(&self) -> PhoneAnywhereSettings {
        let _guard = self.inner.serial_lock.lock().unwrap();
        self.inner.settings_snapshot()
    }

    pub fn connected(&self) -> bool {
        self.inner.state.lock().unwrap().connected
    }

    pub fn room(&self) -> Option<(String, String)> {
        let st = self.inner.state.lock().unwrap();
        let held = st.saved.as_ref()?;
        if held.closing {
            None
        } else {
            Some((held.relay.clone(), held.room.clone()))
        }
    }

    pub fn load(&self) -> PhoneAnywhereSettings {
        let _guard = self.inner.serial_lock.lock().unwrap();
        {
            let st = self.inner.state.lock().unwrap();
            if st.disposed || st.saved.is_some() {
                return self.inner.settings_from(&st);
            }
        }
        if let Ok(text) = fs::read_to_string(&self.inner.file)
            && let Ok(val) = serde_json::from_str::<Value>(&text)
            && let Some(held) = Saved::from_json(&val)
        {
            if held.closing {
                {
                    let mut st = self.inner.state.lock().unwrap();
                    st.saved = Some(held);
                    st.refused_by_relay = false;
                }
                self.inner.connect();
            } else {
                self.inner.start(held);
            }
        }
        self.inner.settings_snapshot()
    }

    pub fn set_enabled(&self, enabled: bool) -> PhoneAnywhereSettings {
        let _guard = self.inner.serial_lock.lock().unwrap();
        if self.inner.disposed_flag.load(Ordering::SeqCst) {
            return self.inner.settings_snapshot();
        }
        if enabled {
            if self.inner.refusal().is_some() {
                return self.inner.settings_snapshot();
            }
            let (is_closing, has_saved) = {
                let st = self.inner.state.lock().unwrap();
                (
                    st.saved.as_ref().is_some_and(|s| s.closing),
                    st.saved.is_some(),
                )
            };
            if is_closing {
                return self.inner.settings_snapshot();
            }
            if !has_saved {
                let pc = new_pc_keys();
                let relay_url = (self.inner.options.relay)()
                    .trim_end_matches('/')
                    .to_owned();
                let held = Saved {
                    relay: if relay_url.is_empty() {
                        DEFAULT_RELAY.to_owned()
                    } else {
                        relay_url
                    },
                    room: random_text(16),
                    token: random_text(32),
                    private_key: Some(pc.private_key),
                    public_key: pc.public_key,
                    devices: Vec::new(),
                    night_approvals: false,
                    notes_taken: Vec::new(),
                    closing: false,
                    github: None,
                };
                {
                    let mut st = self.inner.state.lock().unwrap();
                    st.saved = Some(held.clone());
                }
                if self.inner.persist().is_err() {
                    let mut st = self.inner.state.lock().unwrap();
                    st.saved = None;
                    st.message =
                        Some("Locust could not save what pairs your phone. It is still off.".into());
                    return self.inner.settings_from(&st);
                }
                self.inner.start(held);
            } else {
                self.inner.connect();
            }
            self.inner.new_code_inner(false);
            {
                let mut st = self.inner.state.lock().unwrap();
                st.message = None;
            }
            return self.inner.settings_snapshot();
        }

        // Turning off: forget every desk device first.
        let device_ids: Vec<String> = {
            let st = self.inner.state.lock().unwrap();
            st.saved
                .as_ref()
                .map(|s| s.devices.iter().map(|d| d.id.clone()).collect())
                .unwrap_or_default()
        };
        if let Some(desk) = &self.inner.options.desk {
            for id in &device_ids {
                desk.forget(id);
            }
        }
        let has_saved = {
            let mut st = self.inner.state.lock().unwrap();
            if let Some(held) = st.saved.as_mut()
                && !held.closing
            {
                held.private_key = None;
                held.devices.clear();
                held.closing = true;
                st.keys.clear();
                st.pairing = None;
                st.message = None;
            }
            st.saved.is_some()
        };
        if has_saved {
            let _ = self.inner.persist();
            self.inner.let_go();
        } else {
            self.inner.forget_room();
        }
        self.inner.settings_snapshot()
    }

    pub fn new_code(&self) -> PhoneAnywhereSettings {
        let _guard = self.inner.serial_lock.lock().unwrap();
        let ok = {
            let st = self.inner.state.lock().unwrap();
            !st.disposed && st.saved.as_ref().is_some_and(|s| !s.closing)
        };
        if ok {
            self.inner.new_code_inner(false);
        }
        self.inner.settings_snapshot()
    }

    /// Open in your browser (0.838): makes a one-time code for the full window (`desk: true`), opened at the relay.
    pub fn desk_code(&self) -> Option<String> {
        let made = {
            let _guard = self.inner.serial_lock.lock().unwrap();
            let ok = {
                let st = self.inner.state.lock().unwrap();
                !st.disposed && st.saved.as_ref().is_some_and(|s| !s.closing)
            };
            if ok {
                self.inner.new_code_inner(true);
            }
            self.inner.settings_snapshot()
        };
        let code = made.code?;
        if !code.desk {
            return None;
        }
        let (hash, until, connected) = {
            let st = self.inner.state.lock().unwrap();
            let p = st.pairing.as_ref()?;
            (p.hash.clone(), p.until, st.connected)
        };
        if !connected {
            return None;
        }
        if self.inner.command(json!({ "t": "pair-open", "hash": hash, "until": until })) {
            Some(code.url)
        } else {
            None
        }
    }

    /// Where to send the person to tie this computer to their GitHub account, or None with why in Settings' note.
    pub fn github_link(&self) -> Option<String> {
        let (relay, room, public_key) = {
            let mut st = self.inner.state.lock().unwrap();
            if !st.connected || st.saved.as_ref().is_none_or(|s| s.closing) {
                st.message = Some("Locust is not connected to its relay yet. Try again in a moment.".into());
                drop(st);
                self.inner.tell();
                return None;
            }
            let held = st.saved.as_ref()?;
            (held.relay.clone(), held.room.clone(), held.public_key.clone())
        };
        let link = random_text(32);
        let name = computer_name_of(
            self.inner
                .options
                .computer_name
                .as_ref()
                .and_then(|f| f())
                .as_deref(),
        );
        let answer = self.inner.answer_of(json!({
            "t": "github-open",
            "hash": sha256_b64url(&link),
            "key": public_key,
            "name": name,
        }));
        match answer {
            CommandAnswer::Ok => {
                self.inner.state.lock().unwrap().message = None;
                self.inner.tell();
                Some(format!("{relay}/v1/github/start?room={room}&link={link}"))
            }
            CommandAnswer::Err(c) if c == "github-off" => {
                self.inner.state.lock().unwrap().message =
                    Some("Locust\u{2019}s relay does not offer signing in with GitHub yet.".into());
                self.inner.tell();
                None
            }
            _ => {
                self.inner.state.lock().unwrap().message = Some(
                    "Locust\u{2019}s relay did not answer, or does not offer signing in with GitHub yet. Try again in a moment."
                        .into(),
                );
                self.inner.tell();
                None
            }
        }
    }

    /// No browser signs in with GitHub any more. Browsers already let in stay paired until removed.
    pub fn github_sign_out(&self) -> PhoneAnywhereSettings {
        let _guard = self.inner.serial_lock.lock().unwrap();
        let has_github = {
            let st = self.inner.state.lock().unwrap();
            !st.disposed && st.saved.as_ref().is_some_and(|s| s.github.is_some())
        };
        if !has_github {
            return self.inner.settings_snapshot();
        }
        if !self.inner.command(json!({ "t": "github-close" })) {
            self.inner.state.lock().unwrap().message = Some(
                "Locust\u{2019}s relay did not answer. Signing in with GitHub is still on; try again in a moment."
                    .into(),
            );
            return self.inner.settings_snapshot();
        }
        {
            let mut st = self.inner.state.lock().unwrap();
            if let Some(held) = st.saved.as_mut() {
                held.github = None;
            }
            st.message = None;
        }
        let _ = self.inner.persist();
        self.inner.tell();
        self.inner.settings_snapshot()
    }

    pub fn set_desk(&self, device: &str, on: bool) -> PhoneAnywhereSettings {
        let _guard = self.inner.serial_lock.lock().unwrap();
        let found = {
            let mut st = self.inner.state.lock().unwrap();
            if st.disposed {
                return self.inner.settings_from(&st);
            }
            let Some(held) = st.saved.as_mut() else {
                return self.inner.settings_from(&st);
            };
            if let Some(d) = held.devices.iter_mut().find(|d| d.id == device) {
                d.desk = on;
                d.desk_seen.clear();
                true
            } else {
                false
            }
        };
        if !found {
            return self.inner.settings_snapshot();
        }
        if self.inner.persist().is_err() {
            self.inner.state.lock().unwrap().message =
                Some("That setting could not be saved. It lasts until Locust closes.".into());
        }
        if !on && let Some(desk) = &self.inner.options.desk {
            desk.forget(device);
        }
        self.inner.tell();
        self.inner.settings_snapshot()
    }

    pub fn remove(&self, device: &str) -> PhoneAnywhereSettings {
        let _guard = self.inner.serial_lock.lock().unwrap();
        let (found, is_connected) = {
            let mut st = self.inner.state.lock().unwrap();
            if st.disposed {
                return self.inner.settings_from(&st);
            }
            let Some(held) = st.saved.as_mut() else {
                return self.inner.settings_from(&st);
            };
            let before = held.devices.len();
            held.devices.retain(|d| d.id != device);
            let found = held.devices.len() != before;
            if found {
                st.keys.remove(device);
                st.online.remove(device);
            }
            (found, st.connected)
        };
        if !found {
            return self.inner.settings_snapshot();
        }
        if let Some(desk) = &self.inner.options.desk {
            desk.forget(device);
        }
        let _ = self.inner.persist();
        if is_connected {
            let _ = self.inner.command(json!({ "t": "remove", "device": device }));
        }
        self.inner.settings_snapshot()
    }

    pub fn alert(&self, alert: &PhoneAlert) -> usize {
        if self.inner.refusal().is_some() {
            return 0;
        }
        let targets: Vec<(String, PushSubscriptionKeys)> = {
            let st = self.inner.state.lock().unwrap();
            let Some(held) = st.saved.as_ref() else {
                return 0;
            };
            if !st.connected {
                return 0;
            }
            held.devices
                .iter()
                .filter_map(|d| {
                    if st.online.contains(&d.id) {
                        return None;
                    }
                    d.push.clone().map(|p| (d.id.clone(), p))
                })
                .collect()
        };
        let mut sent = 0;
        let urgency = match alert.kind {
            AlertKind::Approval | AlertKind::Question => "high",
            _ => "normal",
        };
        let payload = json!({
            "title": alert.title,
            "body": alert.body,
            "tag": alert.tag,
        })
        .to_string();
        for (device_id, push) in targets {
            let Ok(encrypted) = encrypt_push(&push.p256dh, &push.auth, &payload, None) else {
                continue;
            };
            let body = to_base64url(&encrypted);
            let answer = self.inner.answer_of(json!({
                "t": "push",
                "endpoint": push.endpoint,
                "body": body,
                "urgency": urgency,
            }));
            match answer {
                CommandAnswer::Ok => sent += 1,
                CommandAnswer::Err(code) if code == "push-gone" => {
                    {
                        let mut st = self.inner.state.lock().unwrap();
                        if let Some(held) = st.saved.as_mut()
                            && let Some(d) = held.devices.iter_mut().find(|d| d.id == device_id)
                        {
                            d.push = None;
                        }
                    }
                    let _ = self.inner.persist();
                    self.inner.tell();
                }
                _ => {}
            }
        }
        sent
    }

    /// Where other apps reach this computer from anywhere (0.841): its room's MCP address, while From anywhere is on.
    pub fn mcp_address(&self) -> Option<String> {
        let st = self.inner.state.lock().unwrap();
        let held = st.saved.as_ref().filter(|held| !held.closing)?;
        Some(format!("{}/v1/room/{}/mcp", held.relay, held.room))
    }

    /// The relay takes apps for this room, or no longer: off forgets every app it registered. With no room there is
    /// nothing to turn off, and nothing can be turned on.
    pub fn mcp_open(&self, on: bool) -> bool {
        // The settings switch and a late auth sync can both ask to open. Hold our own lock
        // through the acknowledgement, without taking the socket reader's state lock.
        let mut opened_on = self.inner.mcp_opened_on.lock().unwrap();
        if !on {
            *opened_on = None;
        }
        let connection = {
            let st = self.inner.state.lock().unwrap();
            if !st.saved.as_ref().is_some_and(|held| !held.closing) {
                return !on;
            }
            if !st.connected {
                return false;
            }
            let Some((connection, _)) = st.socket.as_ref() else {
                return false;
            };
            *connection
        };
        if on && *opened_on == Some(connection) {
            return true;
        }
        let frame = json!({ "t": if on { "mcp-open" } else { "mcp-close" } });
        let accepted = matches!(self.inner.answer_of_on(frame, Some(connection)), CommandAnswer::Ok);
        if on && accepted {
            *opened_on = Some(connection);
        }
        accepted
    }

    pub fn hook_open(&self, hook: &str, hash: &str) -> bool {
        if self
            .inner
            .state
            .lock()
            .unwrap()
            .saved
            .as_ref()
            .is_some_and(|s| s.closing)
        {
            return false;
        }
        self.inner
            .command(json!({ "t": "hook-open", "hook": hook, "hash": hash }))
    }

    pub fn hook_close(&self, hook: &str) -> bool {
        self.inner.command(json!({ "t": "hook-close", "hook": hook }))
    }

    pub fn hook_collected(&self, hook: &str, letters: &[String]) -> bool {
        self.inner.command(json!({
            "t": "hook-collected",
            "hook": hook,
            "letters": letters,
        }))
    }

    pub fn hook_letters(&self, hook: &str) -> Option<HookLettersPage> {
        let id = self.inner.next_cmd_id();
        let (tx, rx) = std::sync::mpsc::channel();
        self.inner
            .pending
            .lock()
            .unwrap()
            .hook_letters
            .insert(id.clone(), tx);
        if !self
            .inner
            .send_frame(json!({ "t": "hook-letters", "hook": hook, "id": id }))
        {
            self.inner.pending.lock().unwrap().hook_letters.remove(&id);
            return None;
        }
        let res = rx.recv_timeout(Duration::from_millis(COMMAND_MS)).ok().flatten();
        self.inner.pending.lock().unwrap().hook_letters.remove(&id);
        res
    }

    pub fn away_open(&self, hash: &str) -> bool {
        if self
            .inner
            .state
            .lock()
            .unwrap()
            .saved
            .as_ref()
            .is_some_and(|s| s.closing)
        {
            return false;
        }
        self.inner.command(json!({ "t": "away-open", "hash": hash }))
    }

    pub fn away_close(&self) -> bool {
        self.inner.command(json!({ "t": "away-close" }))
    }

    pub fn keep_away_alerts(&self) -> bool {
        let devices: Vec<(String, PushSubscriptionKeys)> = {
            let st = self.inner.state.lock().unwrap();
            let Some(held) = st.saved.as_ref() else {
                return false;
            };
            if !st.connected {
                return false;
            }
            held.devices
                .iter()
                .filter_map(|d| d.push.clone().map(|p| (d.id.clone(), p)))
                .collect()
        };
        let found_str = away_found_alert_json().to_string();
        let brief_str = away_brief_alert_json().to_string();
        let mut alerts = Vec::new();
        for (id, push) in devices {
            let Ok(found_enc) = encrypt_push(&push.p256dh, &push.auth, &found_str, None) else {
                continue;
            };
            let Ok(brief_enc) = encrypt_push(&push.p256dh, &push.auth, &brief_str, None) else {
                continue;
            };
            alerts.push(json!({
                "device": id,
                "endpoint": push.endpoint,
                "found": to_base64url(&found_enc),
                "brief": to_base64url(&brief_enc),
            }));
        }
        self.inner.command(json!({ "t": "away-alerts", "alerts": alerts }))
    }

    pub fn post(&self, n: &str, c: &str) -> bool {
        self.inner.command(json!({ "t": "post", "n": n, "c": c }))
    }

    pub fn letters(&self, after: Option<&str>) -> Option<LettersPage> {
        let id = self.inner.next_cmd_id();
        let (tx, rx) = std::sync::mpsc::channel();
        self.inner
            .pending
            .lock()
            .unwrap()
            .letters
            .insert(id.clone(), tx);
        let mut frame = json!({ "t": "letters", "id": id });
        if let Some(a) = after {
            frame["after"] = json!(a);
        }
        if !self.inner.send_frame(frame) {
            self.inner.pending.lock().unwrap().letters.remove(&id);
            return None;
        }
        let res = rx.recv_timeout(Duration::from_millis(COMMAND_MS)).ok().flatten();
        self.inner.pending.lock().unwrap().letters.remove(&id);
        res
    }

    pub fn tell_desk(&self, device: &str, items: &[Value]) -> bool {
        self.inner.tell_desk(device, items)
    }

    pub fn dispose(&self) {
        self.inner.disposed_flag.store(true, Ordering::SeqCst);
        let _guard = self.inner.serial_lock.lock().unwrap();
        {
            let mut st = self.inner.state.lock().unwrap();
            st.disposed = true;
        }
        self.inner.stop_socket();
        let handles: Vec<_> = {
            let mut fl = self.inner.in_flight.lock().unwrap();
            std::mem::take(&mut *fl)
        };
        for h in handles {
            let _ = h.join();
        }
        let has_saved = self.inner.state.lock().unwrap().saved.is_some();
        if has_saved {
            let _ = self.inner.persist();
        }
    }
}

impl PhoneAnywhereInner {
    fn now(&self) -> i64 {
        self.options
            .now
            .as_ref()
            .map(|f| f())
            .unwrap_or_else(default_now)
    }

    fn refusal(&self) -> Option<String> {
        self.options.refusal.as_ref().and_then(|f| f())
    }

    fn next_cmd_id(&self) -> String {
        let n = self.cmd_seq.fetch_add(1, Ordering::Relaxed);
        format!("c{n}")
    }

    fn spawn_tracked(self: &Arc<Self>, f: impl FnOnce(Arc<Self>) + Send + 'static) {
        let this = Arc::clone(self);
        let handle = std::thread::spawn(move || f(this));
        let mut fl = self.in_flight.lock().unwrap();
        fl.retain(|h| !h.is_finished());
        fl.push(handle);
    }

    fn code_url(held: &Saved, secret: &str, desk: bool) -> String {
        if desk {
            format!("{}/#p={}.{}.{}.d", held.relay, held.room, held.public_key, secret)
        } else {
            format!("{}/#p={}.{}.{}", held.relay, held.room, held.public_key, secret)
        }
    }

    fn settings_from(&self, st: &InnerState) -> PhoneAnywhereSettings {
        let now = self.now();
        let open = st.saved.as_ref().and_then(|_| {
            st.pairing
                .as_ref()
                .filter(|p| !p.used && p.until > now)
        });
        let refusal = self.refusal();
        let closing = st.saved.as_ref().is_some_and(|s| s.closing);
        let note = if closing {
            Some("Turning off...".to_owned())
        } else {
            refusal.or_else(|| st.message.clone())
        };
        let enabled = st.saved.is_some() && !closing;
        let code = if enabled {
            open.zip(st.saved.as_ref()).map(|(p, held)| PairingCode {
                url: Self::code_url(held, &p.secret, p.desk),
                until: iso_ms(p.until),
                desk: p.desk,
            })
        } else {
            None
        };
        let devices = st
            .saved
            .as_ref()
            .map(|held| {
                held.devices
                    .iter()
                    .map(|d| PairedDeviceView {
                        id: d.id.clone(),
                        name: d.name.clone(),
                        added: iso_ms(d.added),
                        seen: d.seen.map(iso_ms),
                        online: st.online.contains(&d.id),
                        alerts: d.push.is_some(),
                        desk: if d.desk { Some(true) } else { None },
                        github: d.via.as_deref() == Some("github"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        PhoneAnywhereSettings {
            enabled,
            connected: st.connected,
            code,
            devices,
            github_login: if enabled {
                st.saved
                    .as_ref()
                    .and_then(|s| s.github.as_ref().map(|(_, l)| l.clone()))
            } else {
                None
            },
            message: note,
            night_approvals: st.saved.as_ref().is_some_and(|s| s.night_approvals),
        }
    }

    fn settings_snapshot(&self) -> PhoneAnywhereSettings {
        let st = self.state.lock().unwrap();
        self.settings_from(&st)
    }

    fn tell(&self) {
        if self.disposed_flag.load(Ordering::SeqCst) {
            return;
        }
        if let Some(cb) = &self.options.changed {
            cb(self.settings_snapshot());
        }
    }

    fn persist(&self) -> Result<(), String> {
        let _w = self.write_lock.lock().unwrap();
        let held = {
            let st = self.state.lock().unwrap();
            st.saved.clone()
        };
        let Some(held) = held else {
            return Ok(());
        };
        fs::create_dir_all(&self.options.directory).map_err(|e| e.to_string())?;
        let temp = self.file.with_extension("json.tmp");
        let _ = fs::remove_file(&temp);
        {
            let mut opts = fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            opts.open(&temp).map_err(|e| e.to_string())?;
        }
        if let Some(protect) = &self.options.protect_file {
            protect(&temp)?;
        }
        let body = serde_json::to_string(&held.to_json()).map_err(|e| e.to_string())?;
        fs::write(&temp, body).map_err(|e| e.to_string())?;
        fs::rename(&temp, &self.file).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn send_frame(&self, frame: Value) -> bool {
        self.send_frame_on(frame, None)
    }

    fn send_frame_on(&self, frame: Value, connection: Option<u64>) -> bool {
        let sock = {
            let st = self.state.lock().unwrap();
            if !st.connected {
                return false;
            }
            st.socket.as_ref().filter(|(id, _)| connection.is_none_or(|expected| expected == *id))
                .map(|(_, s)| Arc::clone(s))
        };
        let Some(s) = sock else {
            return false;
        };
        s.send(frame.to_string()).is_ok()
    }

    fn answer_of(&self, frame: Value) -> CommandAnswer {
        self.answer_of_on(frame, None)
    }

    fn answer_of_on(&self, mut frame: Value, connection: Option<u64>) -> CommandAnswer {
        let id = self.next_cmd_id();
        frame["id"] = json!(id);
        let (tx, rx) = std::sync::mpsc::channel();
        self.pending.lock().unwrap().commands.insert(id.clone(), tx);
        if !self.send_frame_on(frame, connection) {
            self.pending.lock().unwrap().commands.remove(&id);
            return CommandAnswer::Failed;
        }
        let res = rx
            .recv_timeout(Duration::from_millis(COMMAND_MS))
            .unwrap_or(CommandAnswer::Failed);
        self.pending.lock().unwrap().commands.remove(&id);
        res
    }

    fn command(&self, frame: Value) -> bool {
        matches!(self.answer_of(frame), CommandAnswer::Ok)
    }

    fn open_code(&self) {
        let frame = {
            let st = self.state.lock().unwrap();
            let now = self.now();
            st.pairing
                .as_ref()
                .filter(|p| !p.used && p.until > now)
                .map(|p| json!({ "t": "pair-open", "hash": p.hash, "until": p.until }))
        };
        if let Some(f) = frame {
            self.send_frame(f);
        }
    }

    fn new_code_inner(&self, desk: bool) {
        let secret = random_text(16);
        let hash = sha256_b64url(&pairing_token(&secret));
        let until = self.now() + CODE_MS;
        {
            let mut st = self.state.lock().unwrap();
            st.pairing = Some(PairingState {
                secret,
                hash,
                until,
                used: false,
                desk,
            });
        }
        self.open_code();
    }

    fn stop_socket(&self) {
        let old = {
            let mut st = self.state.lock().unwrap();
            st.connected = false;
            st.online.clear();
            st.socket.take().map(|(_, s)| s)
        };
        self.pending.lock().unwrap().fail_all();
        if let Some(s) = old {
            s.close();
        }
    }

    fn reconnect_later(self: &Arc<Self>) {
        let delay = {
            let mut st = self.state.lock().unwrap();
            if st.disposed || st.saved.is_none() || st.refused_by_relay || st.retry_scheduled {
                return;
            }
            st.attempt += 1;
            st.retry_scheduled = true;
            let attempt = st.attempt;
            self.options
                .retry_ms
                .as_ref()
                .map(|f| f(attempt))
                .unwrap_or_else(|| (1_000u64 << attempt.saturating_sub(1).min(6)).min(60_000))
        };
        self.spawn_tracked(move |this| {
            let deadline = std::time::Instant::now() + Duration::from_millis(delay);
            while std::time::Instant::now() < deadline {
                if this.disposed_flag.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            {
                let mut st = this.state.lock().unwrap();
                st.retry_scheduled = false;
                if st.disposed || st.saved.is_none() || st.refused_by_relay {
                    return;
                }
            }
            this.connect();
        });
    }

    fn forget_room(&self) {
        self.stop_socket();
        {
            let mut st = self.state.lock().unwrap();
            st.saved = None;
            st.keys.clear();
            st.pairing = None;
            st.message = None;
            st.refused_by_relay = false;
        }
        let _w = self.write_lock.lock().unwrap();
        let _ = fs::remove_file(&self.file);
    }

    fn let_go(self: &Arc<Self>) {
        let (is_closing, refused, connected) = {
            let st = self.state.lock().unwrap();
            (
                st.saved.as_ref().is_some_and(|s| s.closing),
                st.refused_by_relay,
                st.connected,
            )
        };
        if !is_closing {
            return;
        }
        if refused {
            self.forget_room();
            return;
        }
        if connected && self.command(json!({ "t": "reset" })) {
            self.forget_room();
            return;
        }
        if connected {
            self.stop_socket();
        }
        self.reconnect_later();
    }

    fn connect(self: &Arc<Self>) {
        let (url, token) = {
            let st = self.state.lock().unwrap();
            if st.disposed
                || st.saved.is_none()
                || st.socket.is_some()
                || st.refused_by_relay
                || self.refusal().is_some()
            {
                return;
            }
            let held = st.saved.as_ref().unwrap();
            let ws_base = if let Some(rest) = held.relay.strip_prefix("https://") {
                format!("wss://{rest}")
            } else if let Some(rest) = held.relay.strip_prefix("http://") {
                format!("ws://{rest}")
            } else {
                held.relay.clone()
            };
            (format!("{ws_base}/v1/room/{}", held.room), held.token.clone())
        };
        let made = match (self.options.connect)(&url) {
            Ok(s) => s,
            Err(_) => {
                self.reconnect_later();
                return;
            }
        };
        let conn_id = self.conn_seq.fetch_add(1, Ordering::Relaxed);
        {
            let mut st = self.state.lock().unwrap();
            if st.disposed || st.saved.is_none() {
                drop(st);
                made.close();
                return;
            }
            st.socket = Some((conn_id, Arc::clone(&made)));
        }
        let bridge = Arc::new(SocketBridge {
            inner: Arc::clone(self),
            conn_id,
            token,
        });
        made.set_callbacks(bridge);
    }

    fn start(self: &Arc<Self>, held: Saved) {
        let mut keys = HashMap::new();
        if let Some(pk) = held.private_key {
            for d in &held.devices {
                if let Ok(lk) = link_keys(&pk, &d.key, &held.room, LinkSide::Pc) {
                    keys.insert(d.id.clone(), lk);
                }
            }
        }
        {
            let mut st = self.state.lock().unwrap();
            st.keys = keys;
            st.saved = Some(held);
            st.refused_by_relay = false;
        }
        self.connect();
    }

    fn heard_frame(self: &Arc<Self>, conn_id: u64, text: &str) {
        if text == "pong" {
            return;
        }
        {
            let st = self.state.lock().unwrap();
            match &st.socket {
                Some((id, _)) if *id == conn_id => {}
                _ => return,
            }
        }
        let Ok(frame) = serde_json::from_str::<Value>(text) else {
            return;
        };
        let Some(t) = frame.get("t").and_then(Value::as_str) else {
            return;
        };
        match t {
            "ok" if frame.get("as").and_then(Value::as_str) == Some("pc") => {
                let closing = {
                    let mut st = self.state.lock().unwrap();
                    st.connected = true;
                    st.attempt = 0;
                    st.message = None;
                    st.online.clear();
                    if st.saved.as_ref().is_some_and(|s| s.closing) {
                        true
                    } else {
                        if let Some(arr) = frame.get("devices").and_then(Value::as_array) {
                            for dev in arr {
                                if let Some(s) = dev.as_str() {
                                    st.online.insert(s.to_owned());
                                }
                            }
                        }
                        false
                    }
                };
                if closing {
                    self.spawn_tracked(|this| {
                        let _g = this.serial_lock.lock().unwrap();
                        this.let_go();
                        this.tell();
                    });
                    return;
                }
                self.open_code();
                self.tell();
                if let Some(away) = &self.options.away {
                    away.refresh();
                }
                if let Some(hooks) = &self.options.hooks {
                    hooks.refresh();
                }
                if let Some(mcp) = self.options.mcp.clone() {
                    // Never on the line's own thread: the relay's answer comes back on it.
                    self.spawn_tracked(move |_| mcp.sync());
                }
                self.trigger_collect_notes();
            }
            "mcp-ask" => {
                // An app's request (0.841): answered, however long the person takes over a card; the relay holds the rest.
                let Some(mcp) = self.options.mcp.clone() else { return };
                let ask = frame.get("ask").and_then(Value::as_str).filter(|ask| mcp_ask_re().is_match(ask));
                let kind = match frame.get("kind").and_then(Value::as_str) {
                    Some("connect") => Some(AskKind::Connect),
                    Some("token") => Some(AskKind::Token),
                    Some("call") => Some(AskKind::Call),
                    _ => None,
                };
                let body = frame.get("body").and_then(Value::as_str);
                let auth = frame.get("auth");
                let (Some(ask), Some(kind), Some(body)) = (ask, kind, body) else { return };
                if auth.is_some_and(|auth| !auth.is_string()) {
                    return;
                }
                let asked = RemoteMcpAsk {
                    ask: ask.to_owned(),
                    kind,
                    body: body.to_owned(),
                    auth: auth.and_then(Value::as_str).map(str::to_owned),
                };
                self.spawn_tracked(move |this| {
                    let ask = asked.ask.clone();
                    let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mcp.answer(asked)))
                        .unwrap_or_else(|_| RemoteMcpReply { status: 500, body: json!({ "error": "locust-failed" }).to_string() });
                    this.send_frame(json!({ "t": "mcp-answer", "ask": ask, "status": answer.status, "body": answer.body }));
                });
            }
            "notes-changed" => {
                self.trigger_collect_notes();
            }
            "github" => {
                if let Some(account) = frame.get("account") {
                    self.sync_github(account);
                }
            }
            "notes" => {
                if let (Some(id), Some(items)) = (
                    frame.get("id").and_then(Value::as_str),
                    frame.get("items").and_then(Value::as_array),
                ) {
                    let parsed: Vec<NoteBox> = items.iter().filter_map(parse_note_box).collect();
                    if let Some(tx) = self.pending.lock().unwrap().notes.remove(id) {
                        let _ = tx.send(Some(parsed));
                    }
                }
            }
            "done" => {
                if let Some(id) = frame.get("id").and_then(Value::as_str)
                    && let Some(tx) = self.pending.lock().unwrap().commands.remove(id)
                {
                    let _ = tx.send(CommandAnswer::Ok);
                }
            }
            "hook-letter" => {
                if let Some(hooks) = &self.options.hooks {
                    hooks.refresh();
                }
            }
            "hook-letters" => {
                if let (Some(id), Some(items), Some(lost)) = (
                    frame.get("id").and_then(Value::as_str),
                    frame.get("items").and_then(Value::as_array),
                    frame.get("lost").and_then(Value::as_u64),
                ) {
                    let parsed: Vec<HookLetter> =
                        items.iter().filter_map(parse_hook_letter).collect();
                    if let Some(tx) = self.pending.lock().unwrap().hook_letters.remove(id) {
                        let _ = tx.send(Some(HookLettersPage {
                            items: parsed,
                            lost,
                        }));
                    }
                }
            }
            "letters" => {
                if let (Some(id), Some(items)) = (
                    frame.get("id").and_then(Value::as_str),
                    frame.get("items").and_then(Value::as_array),
                ) {
                    let parsed: Vec<RelayLetter> = items.iter().filter_map(parse_letter).collect();
                    let more = frame.get("more").and_then(Value::as_bool) == Some(true);
                    if let Some(tx) = self.pending.lock().unwrap().letters.remove(id) {
                        let _ = tx.send(Some(LettersPage {
                            items: parsed,
                            more,
                        }));
                    }
                }
            }
            "letter" => {
                if let Some(item) = frame.get("item").and_then(parse_letter)
                    && let Some(away) = &self.options.away
                {
                    away.letter(item);
                }
            }
            "err" => {
                if let Some(id) = frame.get("id").and_then(Value::as_str)
                    && let Some(tx) = self.pending.lock().unwrap().commands.remove(id)
                {
                    let ans = match frame.get("code").and_then(Value::as_str) {
                        Some(c) => CommandAnswer::Err(c.to_owned()),
                        None => CommandAnswer::Failed,
                    };
                    let _ = tx.send(ans);
                    return;
                }
                let code = frame.get("code").and_then(Value::as_str);
                let (was_connected, closing) = {
                    let mut st = self.state.lock().unwrap();
                    let wc = st.connected;
                    if !wc && code == Some("auth") {
                        st.refused_by_relay = true;
                        st.message = Some("Locust’s relay no longer knows this computer. Turn this off and on again, then scan the new code.".into());
                    }
                    (wc, st.saved.as_ref().is_some_and(|s| s.closing))
                };
                if !was_connected && code == Some("auth") {
                    self.stop_socket();
                    if closing {
                        self.spawn_tracked(|this| {
                            let _g = this.serial_lock.lock().unwrap();
                            this.let_go();
                            this.tell();
                        });
                    }
                    self.tell();
                }
            }
            "presence" => {
                if let (Some(device), Some(on)) = (
                    frame.get("device").and_then(Value::as_str),
                    frame.get("on").and_then(Value::as_bool),
                ) {
                    let now = self.now();
                    {
                        let mut st = self.state.lock().unwrap();
                        if on {
                            st.online.insert(device.to_owned());
                        } else {
                            st.online.remove(device);
                        }
                        if let Some(held) = st.saved.as_mut()
                            && let Some(d) = held.devices.iter_mut().find(|d| d.id == device)
                        {
                            d.seen = Some(now);
                        }
                    }
                    self.tell();
                }
            }
            "msg" => {
                let (Some(from), Some(n), Some(c)) = (
                    frame.get("from").and_then(Value::as_str),
                    frame.get("n").and_then(Value::as_str),
                    frame.get("c").and_then(Value::as_str),
                ) else {
                    return;
                };
                if from.starts_with("pairing:") {
                    if let Some(k) = frame.get("k").and_then(Value::as_str) {
                        let from = from.to_owned();
                        let k = k.to_owned();
                        let n = n.to_owned();
                        let c = c.to_owned();
                        self.spawn_tracked(move |this| {
                            this.pairing_hello(&from, &k, &n, &c);
                        });
                    }
                    return;
                }
                let device = from.to_owned();
                let n = n.to_owned();
                let c = c.to_owned();
                self.spawn_tracked(move |this| {
                    this.handle_device_message(&device, &n, &c);
                });
            }
            _ => {}
        }
    }

    fn pairing_hello(&self, from: &str, key: &str, n: &str, c: &str) {
        let (room, pk) = {
            let st = self.state.lock().unwrap();
            let Some(held) = st.saved.as_ref() else {
                return;
            };
            let Some(pk) = held.private_key else {
                return;
            };
            (held.room.clone(), pk)
        };
        let device = device_id_of(key);
        let Ok(link) = link_keys(&pk, key, &room, LinkSide::Pc) else {
            return;
        };
        let Ok(said) = open_json(&link.open, n, c, &aad_of(&room, &device, Direction::DeviceToPc))
        else {
            return;
        };
        let Some(hello) = said.as_object() else {
            return;
        };
        if hello.get("v").and_then(Value::as_u64) != Some(1) {
            return;
        }
        let kind = hello.get("kind").and_then(Value::as_str);
        if kind == Some("signin") {
            self.signed_in(from, &device, key, link, hello);
            return;
        }
        if kind != Some("pair") {
            return;
        }
        let Some(proof) = hello.get("proof").and_then(Value::as_str) else {
            return;
        };
        let now = self.now();
        let desk_flag = {
            let mut st = self.state.lock().unwrap();
            let Some(code) = st.pairing.as_mut() else {
                return;
            };
            if code.used || code.until <= now {
                return;
            }
            if !same_text(proof, &pairing_proof(&code.secret)) {
                return;
            }
            code.used = true;
            code.desk
        };
        let name = clean_name(hello.get("name"));
        let token = random_text(32);
        let allowed = self.command(json!({
            "t": "allow",
            "device": device,
            "hash": sha256_b64url(&token),
        }));
        if !allowed {
            let mut st = self.state.lock().unwrap();
            if let Some(code) = st.pairing.as_mut() {
                code.used = false;
            }
            st.message = Some("Locust’s relay did not take the new phone. Make a new code and scan it again.".into());
            drop(st);
            self.tell();
            return;
        }
        let Ok(reply) = seal_json(
            &link.seal,
            &json!({ "v": 1, "kind": "paired", "device": device, "token": token }),
            &aad_of(&room, &device, Direction::PcToDevice),
        ) else {
            return;
        };
        {
            let mut st = self.state.lock().unwrap();
            st.pairing = None;
            let Some(held) = st.saved.as_mut() else {
                return;
            };
            held.devices.retain(|d| d.id != device);
            let now = self.now();
            held.devices.push(SavedDevice {
                id: device.clone(),
                name,
                key: key.to_owned(),
                added: now,
                seen: Some(now),
                seq: 0,
                push: None,
                desk: desk_flag,
                desk_seen: HashMap::new(),
                via: None,
            });
            st.keys.insert(device, link);
            st.message = None;
        }
        if self.persist().is_err() {
            self.state.lock().unwrap().message =
                Some("The new phone could not be saved. It works until Locust closes.".into());
        }
        // Once the phone gets its token it can send right away. Its local key must
        // already be installed; closing the pairing offer can wait for its own ack.
        self.send_frame(json!({ "t": "send", "to": from, "n": reply.n, "c": reply.c }));
        let _ = self.command(json!({ "t": "pair-close" }));
        self.tell();
    }

    fn signed_in(
        &self,
        from: &str,
        device: &str,
        key: &str,
        link: LinkKeys,
        hello: &Map<String, Value>,
    ) {
        let refuse = |words: &str| {
            let room = {
                let st = self.state.lock().unwrap();
                st.saved.as_ref().map(|s| s.room.clone())
            };
            if let Some(room) = room
                && let Ok(reply) = seal_json(
                    &link.seal,
                    &json!({ "v": 1, "kind": "refused", "message": words }),
                    &aad_of(&room, device, Direction::PcToDevice),
                )
            {
                self.send_frame(json!({ "t": "send", "to": from, "n": reply.n, "c": reply.c }));
            }
        };
        let (room, account) = {
            let st = self.state.lock().unwrap();
            let Some(held) = st.saved.as_ref() else {
                return;
            };
            if held.closing {
                refuse("Locust on your computer is not signed in with GitHub any more. Pair this browser with a code instead.");
                return;
            }
            (held.room.clone(), held.github.clone())
        };
        let Some((account_user, account_login)) = account else {
            refuse("Locust on your computer is not signed in with GitHub any more. Pair this browser with a code instead.");
            return;
        };
        let Some(gh_token) = hello.get("github").and_then(Value::as_str) else {
            return;
        };
        if !is_github_token(gh_token) {
            return;
        }
        let github_user = self
            .options
            .github_user
            .as_ref()
            .and_then(|f| f(gh_token));
        let Some(user) = github_user else {
            refuse("Locust on your computer could not check your account with GitHub. Sign in again in a moment.");
            return;
        };
        if user != account_user {
            refuse(&format!(
                "Locust on your computer is signed in with a different GitHub account ({account_login}). Sign in as that account."
            ));
            return;
        }
        let name = clean_name(hello.get("name"));
        let token = random_text(32);
        let allowed = self.command(json!({
            "t": "allow",
            "device": device,
            "hash": sha256_b64url(&token),
        }));
        if !allowed {
            refuse("Locust\u{2019}s relay did not take this browser. Sign in with GitHub again.");
            return;
        }
        let Ok(reply) = seal_json(
            &link.seal,
            &json!({ "v": 1, "kind": "paired", "device": device, "token": token }),
            &aad_of(&room, device, Direction::PcToDevice),
        ) else {
            return;
        };
        self.send_frame(json!({ "t": "send", "to": from, "n": reply.n, "c": reply.c }));
        {
            let mut st = self.state.lock().unwrap();
            let Some(held) = st.saved.as_mut() else {
                return;
            };
            held.devices.retain(|d| d.id != device);
            let now = self.now();
            held.devices.push(SavedDevice {
                id: device.to_owned(),
                name,
                key: key.to_owned(),
                added: now,
                seen: Some(now),
                seq: 0,
                push: None,
                desk: true,
                desk_seen: HashMap::new(),
                via: Some("github".into()),
            });
            st.keys.insert(device.to_owned(), link);
            st.message = None;
        }
        let _ = self.persist();
        self.tell();
    }

    fn sync_github(&self, value: &Value) {
        let next = if value.as_bool() == Some(false) {
            None
        } else {
            let Some(parsed) = parse_github_account(value) else {
                return;
            };
            Some(parsed)
        };
        let changed = {
            let mut st = self.state.lock().unwrap();
            let Some(held) = st.saved.as_mut() else {
                return;
            };
            if held.closing || held.github == next {
                false
            } else {
                held.github = next;
                true
            }
        };
        if changed {
            let _ = self.persist();
            self.tell();
        }
    }

    fn handle_device_message(&self, device: &str, n: &str, c: &str) {
        let dev_lock = {
            let mut st = self.state.lock().unwrap();
            Arc::clone(
                st.device_locks
                    .entry(device.to_owned())
                    .or_insert_with(|| Arc::new(Mutex::new(()))),
            )
        };
        let _dev_guard = dev_lock.lock().unwrap();

        let (room, link) = {
            let st = self.state.lock().unwrap();
            let Some(held) = st.saved.as_ref() else {
                return;
            };
            let Some(&lk) = st.keys.get(device) else {
                return;
            };
            (held.room.clone(), lk)
        };
        let Ok(mut said) = open_json(&link.open, n, c, &aad_of(&room, device, Direction::DeviceToPc))
        else {
            return;
        };
        if let Some(part) = parse_desk_part(&said) {
            let whole = {
                let mut st = self.state.lock().unwrap();
                let joiner = st
                    .desk_joins
                    .entry(device.to_owned())
                    .or_default();
                joiner.push(&part)
            };
            let Some(text) = whole else {
                return;
            };
            let Ok(parsed) = serde_json::from_str::<Value>(&text) else {
                return;
            };
            said = parsed;
        }
        if said.get("v").and_then(Value::as_u64) == Some(1)
            && said.get("kind").and_then(Value::as_str) == Some("desk")
        {
            self.handle_desk_frame(device, &said);
            return;
        }
        // Parse phone request
        let Some(obj) = said.as_object() else {
            return;
        };
        if obj.get("v").and_then(Value::as_u64) != Some(1)
            || obj.get("kind").and_then(Value::as_str) != Some("req")
        {
            return;
        }
        let Some(req_id) = obj.get("id").and_then(Value::as_str) else {
            return;
        };
        if req_id.is_empty() || req_id.len() > 64 {
            return;
        }
        let Some(seq) = obj.get("seq").and_then(Value::as_u64) else {
            return;
        };
        if seq == 0 {
            return;
        }
        let Some(t) = obj.get("t").and_then(Value::as_i64) else {
            return;
        };
        let Some(path) = obj.get("path").and_then(Value::as_str) else {
            return;
        };
        if !path.starts_with("/api/") || path.len() > 300 {
            return;
        }
        let body = obj.get("body");
        if let Some(b) = body
            && !b.is_object()
        {
            return;
        }

        // Check and advance seq atomically
        let now = self.now();
        {
            let mut st = self.state.lock().unwrap();
            let Some(held) = st.saved.as_mut() else {
                return;
            };
            let Some(entry) = held.devices.iter_mut().find(|d| d.id == device) else {
                return;
            };
            if seq <= entry.seq {
                return;
            }
            entry.seq = seq;
            entry.seen = Some(now);
        }
        let _ = self.persist();

        let (status, data) = if let Some(refused) = self.refusal() {
            (403u16, json!({ "message": refused }))
        } else if (t - now).abs() > MAX_SKEW_MS {
            (
                400u16,
                json!({ "message": "This phone’s clock is too far from this computer’s. Check the time on both." }),
            )
        } else if body.is_some_and(|b| b.to_string().len() > MAX_BODY) {
            (400u16, json!({ "message": "That message could not be read." }))
        } else if path == "/api/alerts"
            && let Some(b) = body
        {
            let off = b.get("off").and_then(Value::as_bool) == Some(true);
            let push = if off { None } else { push_of(b) };
            if !off && push.is_none() {
                (400u16, json!({ "message": "That alert sign-up could not be read." }))
            } else {
                {
                    let mut st = self.state.lock().unwrap();
                    if let Some(held) = st.saved.as_mut()
                        && let Some(d) = held.devices.iter_mut().find(|d| d.id == device)
                    {
                        d.push = push;
                    }
                }
                let _ = self.persist();
                self.tell();
                if let Some(away) = &self.options.away {
                    away.refresh();
                }
                (200u16, json!({ "ok": true }))
            }
        } else if path == "/api/away" {
            let away = self.options.away.as_ref().and_then(|a| a.for_phone());
            match away {
                None => (200u16, json!({ "off": true })),
                Some(info) => (200u16, json!({ "key": info.key, "repo": info.repo })),
            }
        } else {
            let method = if body.is_none() { "GET" } else { "POST" };
            match answer_phone(&*self.options.calls, method, path, body) {
                Some(ans) => (ans.status, ans.data),
                None => (404u16, json!({ "message": "Locust does not know that request." })),
            }
        };

        if let Ok(boxed) = seal_json(
            &link.seal,
            &json!({ "v": 1, "kind": "res", "id": req_id, "status": status, "data": data }),
            &aad_of(&room, device, Direction::PcToDevice),
        ) {
            self.send_frame(json!({ "t": "send", "to": device, "n": boxed.n, "c": boxed.c }));
        }
    }

    fn handle_desk_frame(&self, device: &str, frame: &Value) {
        let Some(desk) = &self.options.desk else {
            return;
        };
        let Some(obj) = frame.as_object() else {
            return;
        };
        let Some(s) = obj.get("s").and_then(Value::as_str) else {
            return;
        };
        if !is_b64url_len(s, 8, 32) {
            return;
        }
        let Some(q) = obj.get("q").and_then(Value::as_u64) else {
            return;
        };
        if q == 0 {
            return;
        }
        let Some(t) = obj.get("t").and_then(Value::as_i64) else {
            return;
        };
        let Some(items) = obj.get("items").and_then(Value::as_array) else {
            return;
        };
        if items.len() > 200 {
            return;
        }

        let now = self.now();
        let refusal = self.refusal();
        let allowed_desk = {
            let st = self.state.lock().unwrap();
            let Some(held) = st.saved.as_ref() else {
                return;
            };
            let Some(entry) = held.devices.iter().find(|d| d.id == device) else {
                return;
            };
            entry.desk && refusal.is_none()
        };
        if !allowed_desk {
            let msg = refusal.unwrap_or_else(|| {
                "This browser may not open Locust’s window. On your computer, turn on Full window for it in Settings › General › Your phone.".to_owned()
            });
            let replies: Vec<Value> = items
                .iter()
                .filter_map(is_desk_ask)
                .map(|id| json!({ "kind": "res", "id": id, "ok": false, "refused": true, "message": msg }))
                .collect();
            self.tell_desk(device, &replies);
            return;
        }
        if (t - now).abs() > MAX_SKEW_MS {
            return;
        }
        {
            let mut st = self.state.lock().unwrap();
            let Some(held) = st.saved.as_mut() else {
                return;
            };
            let Some(entry) = held.devices.iter_mut().find(|d| d.id == device) else {
                return;
            };
            if let Some(&last) = entry.desk_seen.get(s)
                && q <= last
            {
                return;
            }
            entry.desk_seen.insert(s.to_owned(), q);
            if entry.desk_seen.len() > 16 {
                let mut pairs: Vec<(String, u64)> = entry.desk_seen.drain().collect();
                pairs.sort_by(|a, b| b.1.cmp(&a.1));
                pairs.truncate(16);
                entry.desk_seen = pairs.into_iter().collect();
            }
            entry.seen = Some(now);
        }
        let asks: Vec<Value> = items
            .iter()
            .filter(|v| is_desk_ask(v).is_some())
            .cloned()
            .collect();
        desk.frame(device, asks);
    }

    fn tell_desk(&self, device: &str, items: &[Value]) -> bool {
        if items.is_empty() {
            return false;
        }
        let (room, link, session, q) = {
            let mut st = self.state.lock().unwrap();
            if !st.connected {
                return false;
            }
            let Some(room) = st.saved.as_ref().map(|s| s.room.clone()) else {
                return false;
            };
            let Some(&lk) = st.keys.get(device) else {
                return false;
            };
            st.desk_sent += 1;
            (room, lk, st.desk_session.clone(), st.desk_sent)
        };
        let frame_val = json!({
            "v": 1,
            "kind": "desk",
            "s": session,
            "q": q,
            "t": self.now(),
            "items": items,
        });
        let text = frame_val.to_string();
        let boxes: Vec<Value> = if text.chars().count() <= DESK_PART_CHARS {
            vec![frame_val]
        } else {
            desk_parts(&format!("{session}-{q}"), &text)
                .into_iter()
                .map(|p| {
                    json!({
                        "v": p.v,
                        "kind": p.kind,
                        "id": p.id,
                        "n": p.n,
                        "of": p.of,
                        "text": p.text,
                    })
                })
                .collect()
        };
        let aad = aad_of(&room, device, Direction::PcToDevice);
        for b in boxes {
            let Ok(sealed) = seal_json(&link.seal, &b, &aad) else {
                return false;
            };
            if !self.send_frame(json!({ "t": "send", "to": device, "n": sealed.n, "c": sealed.c }))
            {
                return false;
            }
        }
        true
    }

    fn waiting_notes(&self) -> Option<Vec<NoteBox>> {
        let id = self.next_cmd_id();
        let (tx, rx) = std::sync::mpsc::channel();
        self.pending.lock().unwrap().notes.insert(id.clone(), tx);
        if !self.send_frame(json!({ "t": "notes", "id": id })) {
            self.pending.lock().unwrap().notes.remove(&id);
            return None;
        }
        let res = rx.recv_timeout(Duration::from_millis(COMMAND_MS)).ok().flatten();
        self.pending.lock().unwrap().notes.remove(&id);
        res
    }

    fn trigger_collect_notes(self: &Arc<Self>) {
        if self.options.notes.is_none() {
            return;
        }
        {
            let mut st = self.state.lock().unwrap();
            if st.collecting_notes {
                st.collect_notes_again = true;
                return;
            }
            st.collecting_notes = true;
            st.collect_notes_again = false;
        }
        self.spawn_tracked(|this| loop {
            this.collect_notes_once();
            let again = {
                let mut st = this.state.lock().unwrap();
                if st.collect_notes_again && st.connected && !st.disposed {
                    st.collect_notes_again = false;
                    true
                } else {
                    st.collecting_notes = false;
                    false
                }
            };
            if !again {
                break;
            }
        });
    }

    fn collect_notes_once(self: &Arc<Self>) {
        let Some(notes_cb) = &self.options.notes else {
            return;
        };
        if self.refusal().is_some() {
            return;
        }
        {
            let st = self.state.lock().unwrap();
            if !st.connected
                || st.saved.is_none()
                || st.saved.as_ref().is_some_and(|s| s.closing)
            {
                return;
            }
        }
        let Some(items) = self.waiting_notes() else {
            return;
        };
        let mut collected: Vec<String> = Vec::new();
        let mut held_out: Vec<Value> = Vec::new();
        let mut any_busy = false;
        for item in items {
            if item.held.is_some() {
                continue;
            }
            let (room, link) = {
                let st = self.state.lock().unwrap();
                let Some(room) = st.saved.as_ref().map(|s| s.room.clone()) else {
                    return;
                };
                let Some(&lk) = st.keys.get(&item.device) else {
                    continue;
                };
                (room, lk)
            };
            let opened = open_json(
                &link.open,
                &item.n,
                &item.c,
                &aad_of(&room, &item.device, Direction::DeviceToPc),
            )
            .ok();
            let Some(written) = opened.as_ref().and_then(parse_written_note) else {
                held_out.push(json!({ "note": item.id, "why": "failed" }));
                continue;
            };
            let already_taken = {
                let st = self.state.lock().unwrap();
                st.saved
                    .as_ref()
                    .is_some_and(|s| s.notes_taken.iter().any(|nid| nid == &written.nid))
            };
            if already_taken {
                collected.push(item.id);
                continue;
            }
            let oldest_at = item.at.min(written.t);
            if self.now() - oldest_at > NOTE_SENDS_WITHIN_MS {
                held_out.push(json!({ "note": item.id, "why": "old" }));
                continue;
            }
            // Record as taken before delivery so a crash between the two never sends twice.
            {
                let mut st = self.state.lock().unwrap();
                let Some(saved) = st.saved.as_mut() else {
                    return;
                };
                saved.notes_taken.push(written.nid.clone());
                if saved.notes_taken.len() > NOTES_TAKEN_KEPT {
                    let drop_n = saved.notes_taken.len() - NOTES_TAKEN_KEPT;
                    saved.notes_taken.drain(..drop_n);
                }
            }
            if self.persist().is_err() {
                let mut st = self.state.lock().unwrap();
                if let Some(saved) = st.saved.as_mut() {
                    saved.notes_taken.retain(|nid| nid != &written.nid);
                }
                continue;
            }
            let outcome = notes_cb.deliver(&written.mission_id, written.text.trim());
            if outcome == NoteDelivery::Sent {
                collected.push(item.id);
                continue;
            }
            {
                let mut st = self.state.lock().unwrap();
                if let Some(saved) = st.saved.as_mut() {
                    saved.notes_taken.retain(|nid| nid != &written.nid);
                }
            }
            let _ = self.persist();
            if outcome == NoteDelivery::Busy {
                any_busy = true;
            } else if let Some(why) = outcome.held_str() {
                held_out.push(json!({ "note": item.id, "why": why }));
            }
        }
        if !collected.is_empty() || !held_out.is_empty() {
            let _ = self.command(json!({
                "t": "notes-collected",
                "notes": collected,
                "held": held_out,
            }));
        }
        if any_busy && !self.disposed_flag.load(Ordering::SeqCst) {
            self.spawn_tracked(|this| {
                let deadline = std::time::Instant::now() + Duration::from_millis(NOTE_RETRY_MS);
                while std::time::Instant::now() < deadline {
                    if this.disposed_flag.load(Ordering::SeqCst)
                        || !this.state.lock().unwrap().connected
                    {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                this.trigger_collect_notes();
            });
        }
    }
}

// ---- The routine links' relay (index.ts: `hookRelay = phoneAnywhere`) ---------------------------------------------

/// The phone link is the routine links' relay: one line to Locust's relay carries both. Nothing here fails outright; a
/// link the relay did not confirm is `false`, as Electron's promises resolve.
impl crate::routine_hooks::HookRelay for PhoneAnywhere {
    fn room(&self) -> Option<crate::routine_hooks::HookRoom> {
        PhoneAnywhere::room(self).map(|(relay, room)| crate::routine_hooks::HookRoom { relay, room })
    }
    fn connected(&self) -> bool {
        PhoneAnywhere::connected(self)
    }
    fn hook_open(&self, hook: &str, hash: &str) -> Result<bool, String> {
        Ok(PhoneAnywhere::hook_open(self, hook, hash))
    }
    fn hook_close(&self, hook: &str) -> Result<bool, String> {
        Ok(PhoneAnywhere::hook_close(self, hook))
    }
    fn hook_letters(&self, hook: &str) -> Result<Option<crate::routine_hooks::HookLettersPage>, String> {
        Ok(PhoneAnywhere::hook_letters(self, hook).map(|page| crate::routine_hooks::HookLettersPage {
            items: page
                .items
                .into_iter()
                .map(|letter| crate::routine_hooks::HookLetter {
                    id: letter.id,
                    at: letter.at,
                    hook: letter.hook,
                    body: letter.body,
                    content_type: letter.content_type,
                })
                .collect(),
            lost: page.lost,
        }))
    }
    fn hook_collected(&self, hook: &str, ids: &[String]) -> Result<bool, String> {
        Ok(PhoneAnywhere::hook_collected(self, hook, ids))
    }
}

fn computer_name_of(value: Option<&str>) -> String {
    let raw = value.unwrap_or("");
    let filtered: String = raw
        .chars()
        .filter(|c| {
            c.is_alphanumeric()
                || matches!(c, ' ' | '\'' | '.' | ',' | '(' | ')' | '_' | '-')
        })
        .collect();
    let collapsed: String = filtered.split_whitespace().collect::<Vec<_>>().join(" ");
    let clipped: String = collapsed.chars().take(60).collect();
    if clipped.is_empty() {
        "Computer".to_owned()
    } else {
        clipped
    }
}

fn is_github_token(text: &str) -> bool {
    (20..=255).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn parse_github_account(value: &Value) -> Option<(u64, String)> {
    let obj = value.as_object()?;
    let user = obj.get("user")?.as_u64()?;
    if user == 0 {
        return None;
    }
    let login = obj.get("login")?.as_str()?;
    let bytes = login.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 39
        || !bytes[0].is_ascii_alphanumeric()
        || !bytes.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'-')
    {
        return None;
    }
    Some((user, login.to_owned()))
}

#[cfg(test)]
mod desk_code_guard_tests {
    use super::*;
    use crate::phone_host::{PhoneCalls, PhoneSent, PhoneState, PhoneTurn};

    struct NoopCalls;
    impl PhoneCalls for NoopCalls {
        fn state(&self) -> PhoneState {
            PhoneState { waiting: Vec::new(), conversations: Vec::new(), scout: None }
        }
        fn turns(&self, _mission_id: &str) -> Option<Vec<PhoneTurn>> {
            None
        }
        fn answer_card(&self, _approval_id: &str, _approve: bool) -> bool {
            false
        }
        fn send(&self, _mission_id: &str, _text: &str) -> PhoneSent {
            PhoneSent::Ok
        }
    }

    struct AckSocket {
        callbacks: Mutex<Option<Arc<dyn RelaySocketCallbacks>>>,
    }

    impl RelaySocket for AckSocket {
        fn send(&self, text: String) -> Result<(), String> {
            let val: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let kind = val.get("t").and_then(Value::as_str).unwrap_or("");
            let id = val.get("id").and_then(Value::as_str).map(str::to_owned);
            let reply = match kind {
                "auth" => Some(json!({ "t": "ok", "as": "pc", "devices": [] })),
                "pair-open" | "reset" => id.map(|id| json!({ "t": "done", "id": id })),
                _ => None,
            };
            if let Some(reply) = reply
                && let Some(cb) = self.callbacks.lock().unwrap().clone()
            {
                std::thread::spawn(move || cb.on_message(reply.to_string()));
            }
            Ok(())
        }

        fn close(&self) {}

        fn set_callbacks(&self, callbacks: Arc<dyn RelaySocketCallbacks>) {
            *self.callbacks.lock().unwrap() = Some(callbacks.clone());
            std::thread::spawn(move || callbacks.on_open());
        }
    }

    #[test]
    fn desk_code_returns_none_when_the_code_on_show_is_not_a_desk_code() {
        let dir = std::env::temp_dir().join(format!("locust-desk-guard-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let pc = PhoneAnywhere::new(PhoneAnywhereOptions {
            directory: dir.clone(),
            calls: Arc::new(NoopCalls),
            relay: Arc::new(|| "https://relay.test".to_owned()),
            refusal: None,
            changed: None,
            connect: Arc::new(|_| {
                Ok(Arc::new(AckSocket { callbacks: Mutex::new(None) }) as Arc<dyn RelaySocket>)
            }),
            now: None,
            protect_file: Some(Arc::new(|_| Ok(()))),
            retry_ms: Some(Arc::new(|_| 20)),
            away: None,
            hooks: None,
            desk: None,
            notes: None,
            github_user: None,
            computer_name: None,
            mcp: None,
        });
        pc.set_enabled(true);
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !pc.connected() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(pc.connected());
        // An ordinary (non-desk) code is currently on show.
        assert!(pc.settings().code.as_ref().is_some_and(|c| !c.desk));
        // Mark disposed while still connected so desk_code does not replace the ordinary code.
        pc.inner.state.lock().unwrap().disposed = true;
        assert!(pc.desk_code().is_none());
        pc.dispose();
        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
#[path = "phone_mcp_generation_tests.rs"]
mod phone_mcp_generation_tests;
