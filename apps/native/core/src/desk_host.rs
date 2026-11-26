//! LOCUST IN A BROWSER, ON THIS COMPUTER (Electron `main/desk-host.ts` + `shared/desk.ts` encode/decode, 0.830).
//!
//! Answers a paired browser's calls with the window's own handlers as if from the window, enforces
//! `desk::desk_access` and `desk::is_desk_event`, keeps only one active window at a time, covers
//! the local window while a browser has it, and replays waiting approval cards on request.

use crate::desk::{
    DeskAccess, desk_access, is_desk_event, on_the_computer, render_settings_page,
};
use crate::desk_snapshot::{
    DeskSnapshot, SealedSnapshot, open_snapshot, seal_snapshot, snapshot_key, snapshot_when,
    snapshot_with,
};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MISSION_APPROVAL_CHANNEL: &str = "mission-approval:request";

// ---- Structured-clone JSON codec (shared/desk.ts deskEncode / deskDecode) -------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum DeskValue {
    Undefined,
    Bytes(Vec<u8>),
    Json(Value),
}

impl DeskValue {
    pub fn to_json_lossy(&self) -> Value {
        match self {
            Self::Undefined => Value::Null,
            Self::Bytes(b) => json!({ "$bytes": to_base64(b) }),
            Self::Json(v) => v.clone(),
        }
    }
}

fn to_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 { TABLE[((n >> 6) & 0x3f) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[(n & 0x3f) as usize] as char } else { '=' });
    }
    out
}

fn from_base64(text: &str) -> Result<Vec<u8>, String> {
    let trimmed = text.trim_end_matches('=');
    let decode = |b: u8| -> Result<u32, String> {
        match b {
            b'A'..=b'Z' => Ok((b - b'A') as u32),
            b'a'..=b'z' => Ok((b - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((b - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err("Invalid base64 byte.".into()),
        }
    };
    let mut out = Vec::with_capacity((trimmed.len() * 3) / 4);
    for chunk in trimmed.as_bytes().chunks(4) {
        if chunk.len() < 2 {
            return Err("Invalid base64 length.".into());
        }
        let v0 = decode(chunk[0])?;
        let v1 = decode(chunk[1])?;
        out.push(((v0 << 2) | (v1 >> 4)) as u8);
        if chunk.len() >= 3 {
            let v2 = decode(chunk[2])?;
            out.push((((v1 & 0x0f) << 4) | (v2 >> 2)) as u8);
            if chunk.len() == 4 {
                let v3 = decode(chunk[3])?;
                out.push((((v2 & 0x03) << 6) | v3) as u8);
            }
        }
    }
    Ok(out)
}

pub fn desk_encode(value: &DeskValue) -> String {
    match value {
        DeskValue::Undefined => json!({ "$undefined": true }).to_string(),
        DeskValue::Bytes(b) => json!({ "$bytes": to_base64(b) }).to_string(),
        DeskValue::Json(v) => v.to_string(),
    }
}

pub fn desk_encode_args(args: &[DeskValue]) -> String {
    let items: Vec<Value> = args
        .iter()
        .map(|a| match a {
            DeskValue::Undefined => json!({ "$undefined": true }),
            DeskValue::Bytes(b) => json!({ "$bytes": to_base64(b) }),
            DeskValue::Json(v) => v.clone(),
        })
        .collect();
    Value::Array(items).to_string()
}

pub fn desk_decode(text: &str) -> Result<DeskValue, String> {
    let val: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    Ok(decode_value(val))
}

fn decode_value(val: Value) -> DeskValue {
    if let Value::Object(ref map) = val
        && map.len() == 1
    {
        if map.get("$undefined").and_then(Value::as_bool) == Some(true) {
            return DeskValue::Undefined;
        }
        if let Some(b64) = map.get("$bytes").and_then(Value::as_str)
            && let Ok(bytes) = from_base64(b64)
        {
            return DeskValue::Bytes(bytes);
        }
    }
    DeskValue::Json(val)
}

// ---- Protocol types -------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeskAsk {
    Hello { id: String },
    Take { id: String },
    Call { id: String, channel: String, args: String },
    Replay { id: String, what: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeskTell {
    ResOk { id: String, value: String },
    ResErr { id: String, message: String, refused: bool },
    Ev { channel: String, args: String },
    Taken { by: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeskHelloInfo {
    pub version: String,
    pub computer: String,
    pub theme: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeskSender {
    pub id: u64,
    pub main_frame_id: u64,
    pub process_id: u32,
    pub destroyed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeskWindow {
    pub destroyed: bool,
    pub web_contents: DeskSender,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeskEvent {
    pub sender_id: u64,
    pub sender_frame_id: u64,
    pub process_id: u32,
}

pub type DeskHandler = Arc<dyn Fn(&DeskEvent, &[DeskValue]) -> Result<DeskValue, String> + Send + Sync>;

/// `keepHandlers(ipc)` — wraps an IPC registry so handlers registered or removed are tracked in one map.
pub struct HandlerRegistry {
    registered: Mutex<HashMap<String, DeskHandler>>,
    kept: Arc<Mutex<HashMap<String, DeskHandler>>>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            registered: Mutex::new(HashMap::new()),
            kept: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn handle(&self, channel: &str, handler: DeskHandler) {
        self.registered
            .lock()
            .unwrap()
            .insert(channel.to_owned(), Arc::clone(&handler));
        self.kept
            .lock()
            .unwrap()
            .insert(channel.to_owned(), handler);
    }

    pub fn remove_handler(&self, channel: &str) {
        self.registered.lock().unwrap().remove(channel);
        self.kept.lock().unwrap().remove(channel);
    }

    pub fn registered_has(&self, channel: &str) -> bool {
        self.registered.lock().unwrap().contains_key(channel)
    }

    pub fn kept(&self) -> Arc<Mutex<HashMap<String, DeskHandler>>> {
        Arc::clone(&self.kept)
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---- DeskHost -------------------------------------------------------------------------------------------------------

pub struct DeskHostDeps {
    pub handlers: Arc<Mutex<HashMap<String, DeskHandler>>>,
    pub window: Arc<dyn Fn() -> Option<DeskWindow> + Send + Sync>,
    pub tell: Arc<dyn Fn(&str, &[DeskTell]) -> bool + Send + Sync>,
    pub cover: Arc<dyn Fn(bool) + Send + Sync>,
    pub hello: Arc<dyn Fn() -> Result<DeskHelloInfo, String> + Send + Sync>,
    pub waiting: Arc<dyn Fn() -> Vec<Value> + Send + Sync>,
    pub changed: Option<Arc<dyn Fn(Option<&str>) + Send + Sync>>,
}

struct DeskHostState {
    active: Option<String>,
    queues: HashMap<String, Vec<DeskTell>>,
    queue_order: Vec<String>,
    snapshot: Option<DeskSnapshot>,
}

pub struct DeskHost {
    deps: DeskHostDeps,
    state: Mutex<DeskHostState>,
}

fn error_words(err: &str) -> String {
    let trimmed = err.trim();
    if trimmed.is_empty() {
        "Locust on your computer could not do that.".to_owned()
    } else {
        trimmed.chars().take(2_000).collect()
    }
}

impl DeskHost {
    pub fn new(deps: DeskHostDeps) -> Self {
        Self {
            deps,
            state: Mutex::new(DeskHostState {
                active: None,
                queues: HashMap::new(),
                queue_order: Vec::new(),
                snapshot: None,
            }),
        }
    }

    fn queue(&self, device: &str, item: DeskTell) {
        let mut st = self.state.lock().unwrap();
        if !st.queues.contains_key(device) {
            st.queue_order.push(device.to_owned());
        }
        st.queues.entry(device.to_owned()).or_default().push(item);
    }

    fn answer(&self, device: &str, id: &str, value: &DeskValue) {
        self.queue(
            device,
            DeskTell::ResOk {
                id: id.to_owned(),
                value: desk_encode(value),
            },
        );
    }

    fn refuse(&self, device: &str, id: &str, message: &str, refused: bool) {
        self.queue(
            device,
            DeskTell::ResErr {
                id: id.to_owned(),
                message: message.to_owned(),
                refused,
            },
        );
    }

    fn set_active(&self, next: Option<String>) {
        {
            let mut st = self.state.lock().unwrap();
            st.active = next.clone();
        }
        if let Some(cb) = &self.deps.changed {
            cb(next.as_deref());
        }
    }

    pub fn active(&self) -> Option<String> {
        self.state.lock().unwrap().active.clone()
    }

    /// Render a Settings page for a browser (`for_browser = true`) or for this computer (`false`).
    pub fn settings_page(&self, page_id: &str, for_browser: bool) -> Option<String> {
        render_settings_page(page_id, for_browser)
    }

    /// Seal the cached read snapshot with the caller's 256-bit key.
    pub fn sealed_snapshot(&self, key: &[u8; 32]) -> Option<SealedSnapshot> {
        let st = self.state.lock().unwrap();
        let snap = st.snapshot.as_ref()?;
        seal_snapshot(key, snap).ok()
    }

    /// Open a sealed snapshot with the caller's 256-bit key.
    pub fn open_cached_snapshot(
        &self,
        key: &[u8; 32],
        sealed: &SealedSnapshot,
    ) -> Option<DeskSnapshot> {
        open_snapshot(key, sealed)
    }

    /// Format when the cached read snapshot was last written, relative to `now_ms`.
    pub fn snapshot_label(&self, now_ms: i64) -> Option<String> {
        let st = self.state.lock().unwrap();
        let snap = st.snapshot.as_ref()?;
        Some(snapshot_when(snap.at as i64, now_ms))
    }

    pub fn flush(&self) {
        let batches: Vec<(String, Vec<DeskTell>)> = {
            let mut st = self.state.lock().unwrap();
            let order = std::mem::take(&mut st.queue_order);
            order
                .into_iter()
                .filter_map(|dev| st.queues.remove(&dev).map(|items| (dev, items)))
                .collect()
        };
        for (device, items) in batches {
            let _ = (self.deps.tell)(&device, &items);
        }
    }

    fn handle_call(&self, device: &str, id: &str, channel: &str, args_raw: &str) {
        let parsed: Value = match serde_json::from_str(args_raw) {
            Ok(v) => v,
            Err(_) => {
                self.refuse(device, id, "That call could not be read.", false);
                return;
            }
        };
        let Some(arr) = parsed.as_array() else {
            self.refuse(device, id, "That call could not be read.", false);
            return;
        };
        if let Some(refusal) = on_the_computer(true, channel, arr) {
            self.refuse(device, id, refusal, true);
            return;
        }
        let access = desk_access(channel, arr).unwrap_or(DeskAccess::Read);
        if access == DeskAccess::Act && self.active().as_deref() != Some(device) {
            self.refuse(
                device,
                id,
                "Locust is open somewhere else now. Open it here again to do that.",
                true,
            );
            return;
        }
        let handler = self.deps.handlers.lock().unwrap().get(channel).cloned();
        let Some(handler) = handler else {
            self.refuse(
                device,
                id,
                "Locust on your computer does not know that call.",
                false,
            );
            return;
        };
        let Some(win) = (self.deps.window)() else {
            self.refuse(
                device,
                id,
                "Locust’s window on your computer is closed. Open Locust there again.",
                false,
            );
            return;
        };
        if win.destroyed || win.web_contents.destroyed {
            self.refuse(
                device,
                id,
                "Locust’s window on your computer is closed. Open Locust there again.",
                false,
            );
            return;
        }
        let event = DeskEvent {
            sender_id: win.web_contents.id,
            sender_frame_id: win.web_contents.main_frame_id,
            process_id: win.web_contents.process_id,
        };
        let decoded_args: Vec<DeskValue> = arr.iter().cloned().map(decode_value).collect();
        match handler(&event, &decoded_args) {
            Ok(val) => {
                if access == DeskAccess::Read {
                    let now_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let key = snapshot_key(channel, &decoded_args);
                    let mut st = self.state.lock().unwrap();
                    let next = snapshot_with(st.snapshot.as_ref(), &key, val.to_json_lossy(), now_ms);
                    st.snapshot = Some(next);
                }
                self.answer(device, id, &val);
            }
            Err(err) => self.refuse(device, id, &error_words(&err), false),
        }
    }

    pub fn handle(&self, device: &str, asks: &[DeskAsk]) {
        for ask in asks {
            match ask {
                DeskAsk::Hello { id } => match (self.deps.hello)() {
                    Err(_) => {
                        self.refuse(
                            device,
                            id,
                            "Locust on your computer could not say what it is.",
                            false,
                        );
                    }
                    Ok(info) => {
                        let cur = self.active();
                        let active_str = match cur.as_deref() {
                            Some(d) if d == device => "here",
                            None => "computer",
                            Some(_) => "other",
                        };
                        let mut obj = Map::new();
                        obj.insert("version".into(), json!(info.version));
                        obj.insert("computer".into(), json!(info.computer));
                        obj.insert("theme".into(), json!(info.theme));
                        obj.insert("active".into(), json!(active_str));
                        self.answer(device, id, &DeskValue::Json(Value::Object(obj)));
                    }
                },
                DeskAsk::Take { id } => {
                    let was = self.active();
                    self.set_active(Some(device.to_owned()));
                    if let Some(prev) = &was
                        && prev != device
                    {
                        self.queue(prev, DeskTell::Taken { by: "browser".into() });
                    }
                    if was.is_none() {
                        (self.deps.cover)(true);
                    }
                    self.answer(device, id, &DeskValue::Json(json!(true)));
                }
                DeskAsk::Replay { id, what } => {
                    if what == "approvals" && self.active().as_deref() == Some(device) {
                        for req in (self.deps.waiting)() {
                            self.queue(
                                device,
                                DeskTell::Ev {
                                    channel: MISSION_APPROVAL_CHANNEL.into(),
                                    args: desk_encode_args(&[DeskValue::Json(req)]),
                                },
                            );
                        }
                    }
                    self.answer(device, id, &DeskValue::Json(json!(true)));
                }
                DeskAsk::Call { id, channel, args } => {
                    self.handle_call(device, id, channel, args);
                }
            }
        }
    }

    pub fn mirror(&self, channel: &str, args: &[DeskValue]) {
        let Some(dev) = self.active() else {
            return;
        };
        if !is_desk_event(channel) {
            return;
        }
        self.queue(
            &dev,
            DeskTell::Ev {
                channel: channel.to_owned(),
                args: desk_encode_args(args),
            },
        );
    }

    pub fn take_back(&self) {
        let Some(was) = self.active() else {
            return;
        };
        self.set_active(None);
        self.queue(&was, DeskTell::Taken { by: "computer".into() });
        (self.deps.cover)(false);
    }

    pub fn forget(&self, device: &str) {
        {
            let mut st = self.state.lock().unwrap();
            st.queues.remove(device);
            st.queue_order.retain(|d| d != device);
        }
        if self.active().as_deref() == Some(device) {
            self.take_back();
        }
    }
}

// ---- On the phone link (index.ts `desk: { frame, forget }`) -----------------------------------------------------------

/// A browser's ask, as the phone link hands it on (`isDeskAsk`); anything else is `None`.
pub fn desk_ask_of(value: &Value) -> Option<DeskAsk> {
    let obj = value.as_object()?;
    let id = obj.get("id")?.as_str()?;
    if id.is_empty() || id.encode_utf16().count() > 64 {
        return None;
    }
    let id = id.to_owned();
    match obj.get("kind")?.as_str()? {
        "hello" => Some(DeskAsk::Hello { id }),
        "take" => Some(DeskAsk::Take { id }),
        "replay" => (obj.get("what")?.as_str()? == "approvals").then(|| DeskAsk::Replay { id, what: "approvals".into() }),
        "call" => {
            let channel = obj.get("channel")?.as_str()?;
            let args = obj.get("args")?.as_str()?;
            let length = channel.encode_utf16().count();
            (length > 0 && length <= 120)
                .then(|| DeskAsk::Call { id, channel: channel.to_owned(), args: args.to_owned() })
        }
        _ => None,
    }
}

/// What the computer tells a browser, as the phone link seals it (`DeskTell`).
pub fn desk_tell_json(tell: &DeskTell) -> Value {
    match tell {
        DeskTell::ResOk { id, value } => json!({ "kind": "res", "id": id, "ok": true, "value": value }),
        DeskTell::ResErr { id, message, refused } => {
            let mut said = json!({ "kind": "res", "id": id, "ok": false, "message": message });
            if *refused {
                said["refused"] = json!(true);
            }
            said
        }
        DeskTell::Ev { channel, args } => json!({ "kind": "ev", "channel": channel, "args": args }),
        DeskTell::Taken { by } => json!({ "kind": "taken", "by": by }),
    }
}

/// The phone link's `desk` (index.ts: `frame: (device, asks) => deskLink.handle(device, asks)`, `forget: (device) =>
/// deskLink.forget(device)`): a paired browser's frame is answered, and a device that may no longer have the window lets
/// it go. What each queues is told at once.
impl crate::phone_anywhere::DeskCallbacks for DeskHost {
    fn frame(&self, device: &str, asks: Vec<Value>) {
        let asks: Vec<DeskAsk> = asks.iter().filter_map(desk_ask_of).collect();
        self.handle(device, &asks);
        self.flush();
    }

    fn forget(&self, device: &str) {
        DeskHost::forget(self, device);
        self.flush();
    }
}
