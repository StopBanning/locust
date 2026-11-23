use crate::desk::{DeskJoiner, DeskPart, desk_parts};
use crate::phone_alerts::{AlertKind, PhoneAlert};
use crate::phone_anywhere::{
    AwayCallbacks, AwayForPhone, DeskCallbacks, HooksCallbacks, NOTE_SENDS_WITHIN_MS,
    NoteDelivery, NotesCallbacks, PHONE_ANYWHERE_FILE, PhoneAnywhere, PhoneAnywhereOptions,
    RelayLetter, RelaySocket, RelaySocketCallbacks, away_found_alert_json,
};
use crate::phone_host::{PhoneCalls, PhoneSent, PhoneState, PhoneTurn, PhoneWaiting};
use crate::relay_crypto::{
    Direction, LinkKeys, LinkSide, aad_of, device_id_of, from_base64url, link_keys, new_pc_keys,
    open_json, p256_generate_keypair, pairing_proof, pairing_token, random_bytes, random_text,
    same_text, seal_json, sha256_b64url, to_base64url,
};
use crate::web_push::open_push;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ---- Temp dir helper ------------------------------------------------------------------------------------------------

struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", random_text(10)));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &PathBuf {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn until(timeout_ms: u64, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while !check() {
        assert!(Instant::now() < deadline, "condition never happened");
        std::thread::sleep(Duration::from_millis(5));
    }
}

// ---- In-memory relay room (mirrors apps/relay/src/room-core.ts) -----------------------------------------------------

#[derive(Clone, Debug)]
enum PeerRole {
    New,
    Pc,
    Device(String),
    Pairing { tag: String, sent: bool },
}

impl PeerRole {
    fn address(&self) -> Option<String> {
        match self {
            Self::New => None,
            Self::Pc => Some("pc".to_owned()),
            Self::Device(d) => Some(d.clone()),
            Self::Pairing { tag, .. } => Some(format!("pairing:{tag}")),
        }
    }
}

struct RelayPeer {
    role: PeerRole,
    callbacks: Arc<Mutex<Option<Arc<dyn RelaySocketCallbacks>>>>,
}

impl RelayPeer {
    fn deliver_frame(&self, frame: Value) {
        let cb = self.callbacks.lock().unwrap().clone();
        if let Some(cb) = cb {
            let text = frame.to_string();
            std::thread::spawn(move || cb.on_message(text));
        }
    }

    fn deliver_close(&self) {
        let cb = self.callbacks.lock().unwrap().take();
        if let Some(cb) = cb {
            std::thread::spawn(move || cb.on_close());
        }
    }
}

#[derive(Clone, Debug)]
struct StoredHook {
    hash: String,
    lost: u64,
}

#[derive(Clone, Debug)]
struct StoredAwayAlert {
    device: String,
    endpoint: String,
    found: String,
    brief: String,
}

#[derive(Clone, Debug)]
struct StoredNote {
    id: String,
    at: i64,
    device: String,
    n: String,
    c: String,
    held: Option<String>,
}

#[derive(Clone, Debug)]
struct StoredLetter {
    id: String,
    at: i64,
    from: String,
    hook: Option<String>,
    n: Option<String>,
    c: Option<String>,
    body: Option<String>,
    content_type: Option<String>,
}

struct RoomState {
    owner: Option<String>,
    devices: HashMap<String, String>,
    pairing: Option<(String, i64, usize)>,
    away_hash: Option<String>,
    away_alerts: Vec<StoredAwayAlert>,
    hooks: HashMap<String, StoredHook>,
    letters: Vec<StoredLetter>,
    notes: Vec<StoredNote>,
    peers: HashMap<u64, RelayPeer>,
    next_peer: u64,
    pc_auths: usize,
    push_fn: Option<Arc<dyn Fn(&str, &str) -> u16 + Send + Sync>>,
}

impl RoomState {
    fn new(push_fn: Option<Arc<dyn Fn(&str, &str) -> u16 + Send + Sync>>) -> Self {
        Self {
            owner: None,
            devices: HashMap::new(),
            pairing: None,
            away_hash: None,
            away_alerts: Vec::new(),
            hooks: HashMap::new(),
            letters: Vec::new(),
            notes: Vec::new(),
            peers: HashMap::new(),
            next_peer: 1,
            pc_auths: 0,
            push_fn,
        }
    }

    fn stored_count(&self) -> usize {
        usize::from(self.owner.is_some())
            + self.devices.len()
            + self.hooks.len()
            + self.letters.len()
            + self.notes.len()
            + self.away_alerts.len()
    }

    fn pc_peer_id(&self, except: Option<u64>) -> Option<u64> {
        self.peers
            .iter()
            .find(|&(&id, p)| Some(id) != except && matches!(p.role, PeerRole::Pc))
            .map(|(&id, _)| id)
    }

    fn send_to(&self, peer_id: u64, frame: Value) {
        if let Some(p) = self.peers.get(&peer_id) {
            p.deliver_frame(frame);
        }
    }

    fn handle_message(&mut self, peer_id: u64, text: &str) {
        if text == "ping" {
            if let Some(p) = self.peers.get(&peer_id) {
                let cb = p.callbacks.lock().unwrap().clone();
                if let Some(cb) = cb {
                    std::thread::spawn(move || cb.on_message("pong".to_owned()));
                }
            }
            return;
        }
        let Ok(frame) = serde_json::from_str::<Value>(text) else {
            return;
        };
        let Some(t) = frame.get("t").and_then(Value::as_str) else {
            return;
        };
        let cmd_id = frame.get("id").and_then(Value::as_str).map(str::to_owned);
        let role = match self.peers.get(&peer_id) {
            Some(p) => p.role.clone(),
            None => return,
        };

        if matches!(role, PeerRole::New) {
            if t != "auth" {
                self.send_to(peer_id, json!({ "t": "err", "code": "auth" }));
                if let Some(p) = self.peers.remove(&peer_id) {
                    p.deliver_close();
                }
                return;
            }
            self.handle_auth(peer_id, &frame);
            return;
        }

        match t {
            "send" => {
                let Some(to) = frame.get("to").and_then(Value::as_str) else {
                    return;
                };
                let (Some(n), Some(c)) = (
                    frame.get("n").and_then(Value::as_str),
                    frame.get("c").and_then(Value::as_str),
                ) else {
                    return;
                };
                let k = frame.get("k").and_then(Value::as_str);
                let Some(from) = role.address() else {
                    return;
                };
                if let PeerRole::Pairing { sent, .. } = &role {
                    if to != "pc" || *sent || k.is_none() {
                        self.send_to(peer_id, json!({ "t": "err", "code": "bad-frame" }));
                        return;
                    }
                } else if k.is_some() {
                    self.send_to(peer_id, json!({ "t": "err", "code": "bad-frame" }));
                    return;
                }
                let targets: Vec<u64> = self
                    .peers
                    .iter()
                    .filter(|&(&id, p)| id != peer_id && p.role.address().as_deref() == Some(to))
                    .map(|(&id, _)| id)
                    .collect();
                if targets.is_empty() {
                    let code = if to == "pc" { "pc-offline" } else { "peer-offline" };
                    self.send_to(peer_id, json!({ "t": "err", "code": code }));
                    return;
                }
                if let Some(p) = self.peers.get_mut(&peer_id)
                    && let PeerRole::Pairing { sent, .. } = &mut p.role
                {
                    *sent = true;
                }
                let mut msg = json!({ "t": "msg", "from": from, "n": n, "c": c });
                if let Some(k) = k {
                    msg["k"] = json!(k);
                }
                for target in targets {
                    self.send_to(target, msg.clone());
                }
            }
            "allow" if matches!(role, PeerRole::Pc) => {
                if let (Some(device), Some(hash)) = (
                    frame.get("device").and_then(Value::as_str),
                    frame.get("hash").and_then(Value::as_str),
                ) {
                    self.devices.insert(device.to_owned(), hash.to_owned());
                    self.send_done(peer_id, "allow", cmd_id);
                }
            }
            "remove" if matches!(role, PeerRole::Pc) => {
                if let Some(device) = frame.get("device").and_then(Value::as_str) {
                    self.devices.remove(device);
                    self.notes.retain(|n| n.device != device);
                    self.away_alerts.retain(|a| a.device != device);
                    let doomed: Vec<u64> = self
                        .peers
                        .iter()
                        .filter(|(_, p)| matches!(&p.role, PeerRole::Device(d) if d == device))
                        .map(|(&id, _)| id)
                        .collect();
                    for id in doomed {
                        self.send_to(id, json!({ "t": "removed" }));
                        if let Some(p) = self.peers.remove(&id) {
                            p.deliver_close();
                        }
                    }
                    self.send_done(peer_id, "remove", cmd_id);
                }
            }
            "pair-open" if matches!(role, PeerRole::Pc) => {
                if let (Some(hash), Some(until)) = (
                    frame.get("hash").and_then(Value::as_str),
                    frame.get("until").and_then(Value::as_i64),
                ) {
                    self.pairing = Some((hash.to_owned(), until, 0));
                    self.send_done(peer_id, "pair-open", cmd_id);
                }
            }
            "pair-close" if matches!(role, PeerRole::Pc) => {
                self.pairing = None;
                self.send_done(peer_id, "pair-close", cmd_id);
            }
            "reset" if matches!(role, PeerRole::Pc) => {
                self.owner = None;
                self.devices.clear();
                self.pairing = None;
                self.away_hash = None;
                self.away_alerts.clear();
                self.hooks.clear();
                self.letters.clear();
                self.notes.clear();
                let others: Vec<u64> = self
                    .peers
                    .keys()
                    .copied()
                    .filter(|&id| id != peer_id)
                    .collect();
                for id in others {
                    if let Some(p) = self.peers.remove(&id) {
                        p.deliver_close();
                    }
                }
                self.send_done(peer_id, "reset", cmd_id);
            }
            "push" if matches!(role, PeerRole::Pc) => {
                let (Some(endpoint), Some(body)) = (
                    frame.get("endpoint").and_then(Value::as_str),
                    frame.get("body").and_then(Value::as_str),
                ) else {
                    return;
                };
                let status = match &self.push_fn {
                    Some(f) => f(endpoint, body),
                    None => {
                        self.send_err(peer_id, "push-off", cmd_id);
                        return;
                    }
                };
                if status == 404 || status == 410 {
                    self.send_err(peer_id, "push-gone", cmd_id);
                } else if (200..300).contains(&status) {
                    self.send_done(peer_id, "push", cmd_id);
                } else {
                    self.send_err(peer_id, "push-failed", cmd_id);
                }
            }
            "hook-open" if matches!(role, PeerRole::Pc) => {
                if let (Some(hook), Some(hash)) = (
                    frame.get("hook").and_then(Value::as_str),
                    frame.get("hash").and_then(Value::as_str),
                ) {
                    let lost = self.hooks.get(hook).map(|h| h.lost).unwrap_or(0);
                    self.hooks.insert(
                        hook.to_owned(),
                        StoredHook {
                            hash: hash.to_owned(),
                            lost,
                        },
                    );
                    self.send_done(peer_id, "hook-open", cmd_id);
                }
            }
            "hook-close" if matches!(role, PeerRole::Pc) => {
                if let Some(hook) = frame.get("hook").and_then(Value::as_str) {
                    self.hooks.remove(hook);
                    self.letters.retain(|l| l.hook.as_deref() != Some(hook));
                    self.send_done(peer_id, "hook-close", cmd_id);
                }
            }
            "hook-letters" if matches!(role, PeerRole::Pc) => {
                if let Some(hook) = frame.get("hook").and_then(Value::as_str) {
                    let items: Vec<Value> = self
                        .letters
                        .iter()
                        .filter(|l| l.hook.as_deref() == Some(hook))
                        .take(10)
                        .map(|l| {
                            json!({
                                "id": l.id,
                                "at": l.at,
                                "hook": hook,
                                "body": l.body.clone().unwrap_or_default(),
                                "contentType": l.content_type.clone().unwrap_or_default(),
                            })
                        })
                        .collect();
                    let lost = self.hooks.get(hook).map(|h| h.lost).unwrap_or(0);
                    let mut resp = json!({ "t": "hook-letters", "items": items, "lost": lost });
                    if let Some(id) = cmd_id {
                        resp["id"] = json!(id);
                    }
                    self.send_to(peer_id, resp);
                }
            }
            "hook-collected" if matches!(role, PeerRole::Pc) => {
                if let (Some(hook), Some(ids)) = (
                    frame.get("hook").and_then(Value::as_str),
                    frame.get("letters").and_then(Value::as_array),
                ) {
                    let set: Vec<&str> = ids.iter().filter_map(Value::as_str).collect();
                    self.letters.retain(|l| {
                        !(l.hook.as_deref() == Some(hook) && set.contains(&l.id.as_str()))
                    });
                    self.send_done(peer_id, "hook-collected", cmd_id);
                }
            }
            "away-open" if matches!(role, PeerRole::Pc) => {
                if let Some(hash) = frame.get("hash").and_then(Value::as_str) {
                    self.away_hash = Some(hash.to_owned());
                    self.send_done(peer_id, "away-open", cmd_id);
                }
            }
            "away-close" if matches!(role, PeerRole::Pc) => {
                self.away_hash = None;
                self.away_alerts.clear();
                self.letters.retain(|l| l.from == "hook");
                self.send_done(peer_id, "away-close", cmd_id);
            }
            "away-alerts" if matches!(role, PeerRole::Pc) => {
                if let Some(arr) = frame.get("alerts").and_then(Value::as_array) {
                    self.away_alerts = arr
                        .iter()
                        .filter_map(|v| {
                            let o = v.as_object()?;
                            let device = o.get("device")?.as_str()?.to_owned();
                            if !self.devices.contains_key(&device) {
                                return None;
                            }
                            Some(StoredAwayAlert {
                                device,
                                endpoint: o.get("endpoint")?.as_str()?.to_owned(),
                                found: o.get("found")?.as_str()?.to_owned(),
                                brief: o.get("brief")?.as_str()?.to_owned(),
                            })
                        })
                        .collect();
                    self.send_done(peer_id, "away-alerts", cmd_id);
                }
            }
            "post" if matches!(role, PeerRole::Pc) => {
                if let (Some(n), Some(c)) = (
                    frame.get("n").and_then(Value::as_str),
                    frame.get("c").and_then(Value::as_str),
                ) {
                    let at = now_ms();
                    let id = format!("{at:013}-{}", random_text(6));
                    let letter = StoredLetter {
                        id: id.clone(),
                        at,
                        from: "pc".to_owned(),
                        hook: None,
                        n: Some(n.to_owned()),
                        c: Some(c.to_owned()),
                        body: None,
                        content_type: None,
                    };
                    self.letters.push(letter);
                    self.send_done(peer_id, "post", cmd_id);
                }
            }
            "letters" if matches!(role, PeerRole::Pc | PeerRole::Device(_)) => {
                let after = frame.get("after").and_then(Value::as_str);
                let items: Vec<Value> = self
                    .letters
                    .iter()
                    .filter(|l| l.from != "hook")
                    .filter(|l| after.is_none_or(|a| l.id.as_str() > a))
                    .map(|l| {
                        json!({
                            "id": l.id,
                            "at": l.at,
                            "from": l.from,
                            "n": l.n.clone().unwrap_or_default(),
                            "c": l.c.clone().unwrap_or_default(),
                        })
                    })
                    .collect();
                let mut resp = json!({ "t": "letters", "items": items, "more": false });
                if let Some(id) = cmd_id {
                    resp["id"] = json!(id);
                }
                self.send_to(peer_id, resp);
            }
            "note" => {
                if let PeerRole::Device(device) = role
                    && let (Some(n), Some(c)) = (
                        frame.get("n").and_then(Value::as_str),
                        frame.get("c").and_then(Value::as_str),
                    )
                {
                    let at = now_ms();
                    let note_id = format!("{at:013}-{}", random_text(6));
                    self.notes.push(StoredNote {
                        id: note_id.clone(),
                        at,
                        device,
                        n: n.to_owned(),
                        c: c.to_owned(),
                        held: None,
                    });
                    let mut resp = json!({ "t": "noted", "note": note_id });
                    if let Some(id) = cmd_id {
                        resp["id"] = json!(id);
                    }
                    self.send_to(peer_id, resp);
                    if let Some(pc) = self.pc_peer_id(None) {
                        self.send_to(pc, json!({ "t": "notes-changed" }));
                    }
                }
            }
            "notes" => {
                match role {
                    PeerRole::Device(device) => {
                        let items: Vec<Value> = self
                            .notes
                            .iter()
                            .filter(|n| n.device == device)
                            .map(|n| {
                                let mut m = json!({ "id": n.id, "at": n.at });
                                if let Some(h) = &n.held {
                                    m["held"] = json!(h);
                                }
                                m
                            })
                            .collect();
                        let mut resp = json!({ "t": "my-notes", "items": items });
                        if let Some(id) = cmd_id {
                            resp["id"] = json!(id);
                        }
                        self.send_to(peer_id, resp);
                    }
                    PeerRole::Pc => {
                        let items: Vec<Value> = self
                            .notes
                            .iter()
                            .map(|n| {
                                let mut m = json!({
                                    "id": n.id,
                                    "at": n.at,
                                    "device": n.device,
                                    "n": n.n,
                                    "c": n.c,
                                });
                                if let Some(h) = &n.held {
                                    m["held"] = json!(h);
                                }
                                m
                            })
                            .collect();
                        let mut resp = json!({ "t": "notes", "items": items });
                        if let Some(id) = cmd_id {
                            resp["id"] = json!(id);
                        }
                        self.send_to(peer_id, resp);
                    }
                    _ => {}
                }
            }
            "notes-collected" if matches!(role, PeerRole::Pc) => {
                if let (Some(collected), Some(held)) = (
                    frame.get("notes").and_then(Value::as_array),
                    frame.get("held").and_then(Value::as_array),
                ) {
                    let gone: Vec<&str> = collected.iter().filter_map(Value::as_str).collect();
                    self.notes.retain(|n| !gone.contains(&n.id.as_str()));
                    for h in held {
                        if let (Some(nid), Some(why)) = (
                            h.get("note").and_then(Value::as_str),
                            h.get("why").and_then(Value::as_str),
                        ) && let Some(note) = self.notes.iter_mut().find(|n| n.id == nid)
                        {
                            note.held = Some(why.to_owned());
                        }
                    }
                    self.send_done(peer_id, "notes-collected", cmd_id);
                    let dev_peers: Vec<u64> = self
                        .peers
                        .iter()
                        .filter(|(_, p)| matches!(p.role, PeerRole::Device(_)))
                        .map(|(&id, _)| id)
                        .collect();
                    for dp in dev_peers {
                        self.send_to(dp, json!({ "t": "notes-changed" }));
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_auth(&mut self, peer_id: u64, frame: &Value) {
        let Some(as_kind) = frame.get("as").and_then(Value::as_str) else {
            return;
        };
        let Some(token) = frame.get("token").and_then(Value::as_str) else {
            return;
        };
        let hash = sha256_b64url(token);
        match as_kind {
            "pc" => {
                self.pc_auths += 1;
                if let Some(owner) = &self.owner {
                    if !same_text(owner, &hash) {
                        self.send_to(peer_id, json!({ "t": "err", "code": "auth" }));
                        if let Some(p) = self.peers.remove(&peer_id) {
                            p.deliver_close();
                        }
                        return;
                    }
                } else {
                    self.owner = Some(hash);
                }
                let old_pcs: Vec<u64> = self
                    .peers
                    .iter()
                    .filter(|&(&id, p)| id != peer_id && matches!(p.role, PeerRole::Pc))
                    .map(|(&id, _)| id)
                    .collect();
                for old in old_pcs {
                    if let Some(p) = self.peers.remove(&old) {
                        p.deliver_close();
                    }
                }
                if let Some(p) = self.peers.get_mut(&peer_id) {
                    p.role = PeerRole::Pc;
                }
                let online: Vec<String> = self
                    .peers
                    .values()
                    .filter_map(|p| match &p.role {
                        PeerRole::Device(d) => Some(d.clone()),
                        _ => None,
                    })
                    .collect();
                self.send_to(peer_id, json!({ "t": "ok", "as": "pc", "devices": online }));
                let others: Vec<u64> = self
                    .peers
                    .iter()
                    .filter(|(_, p)| matches!(p.role, PeerRole::Device(_) | PeerRole::Pairing { .. }))
                    .map(|(&id, _)| id)
                    .collect();
                for o in others {
                    self.send_to(o, json!({ "t": "presence", "pc": true }));
                }
            }
            "device" => {
                let Some(device) = frame.get("device").and_then(Value::as_str) else {
                    return;
                };
                if self.owner.is_none() {
                    self.send_to(peer_id, json!({ "t": "err", "code": "room" }));
                    return;
                }
                let ok = self
                    .devices
                    .get(device)
                    .is_some_and(|h| same_text(h, &hash));
                if !ok {
                    self.send_to(peer_id, json!({ "t": "err", "code": "auth" }));
                    return;
                }
                if let Some(p) = self.peers.get_mut(&peer_id) {
                    p.role = PeerRole::Device(device.to_owned());
                }
                let pc = self.pc_peer_id(Some(peer_id));
                self.send_to(
                    peer_id,
                    json!({ "t": "ok", "as": "device", "pc": pc.is_some() }),
                );
                if let Some(pc_id) = pc {
                    self.send_to(pc_id, json!({ "t": "presence", "device": device, "on": true }));
                }
            }
            "pairing" => {
                if self.owner.is_none() {
                    self.send_to(peer_id, json!({ "t": "err", "code": "room" }));
                    return;
                }
                let now = now_ms();
                let open = match &mut self.pairing {
                    Some((phash, until, tries)) if *until > now && *tries < 10 => {
                        *tries += 1;
                        Some(phash.clone())
                    }
                    _ => None,
                };
                let Some(phash) = open else {
                    self.send_to(peer_id, json!({ "t": "err", "code": "pairing-closed" }));
                    return;
                };
                if !same_text(&phash, &hash) {
                    self.send_to(peer_id, json!({ "t": "err", "code": "auth" }));
                    return;
                }
                let tag = random_text(9);
                if let Some(p) = self.peers.get_mut(&peer_id) {
                    p.role = PeerRole::Pairing { tag, sent: false };
                }
                let pc = self.pc_peer_id(Some(peer_id));
                self.send_to(
                    peer_id,
                    json!({ "t": "ok", "as": "pairing", "pc": pc.is_some() }),
                );
            }
            _ => {}
        }
    }

    fn handle_closed(&mut self, peer_id: u64) {
        let Some(peer) = self.peers.remove(&peer_id) else {
            return;
        };
        match peer.role {
            PeerRole::Pc if self.pc_peer_id(None).is_none() => {
                let others: Vec<u64> = self
                    .peers
                    .iter()
                    .filter(|(_, p)| matches!(p.role, PeerRole::Device(_) | PeerRole::Pairing { .. }))
                    .map(|(&id, _)| id)
                    .collect();
                for o in others {
                    self.send_to(o, json!({ "t": "presence", "pc": false }));
                }
            }
            PeerRole::Device(dev) => {
                let still_open = self
                    .peers
                    .values()
                    .any(|p| matches!(&p.role, PeerRole::Device(d) if d == &dev));
                if !still_open && let Some(pc_id) = self.pc_peer_id(None) {
                    self.send_to(pc_id, json!({ "t": "presence", "device": dev, "on": false }));
                }
            }
            _ => {}
        }
    }

    fn send_done(&self, peer_id: u64, of: &str, id: Option<String>) {
        let mut f = json!({ "t": "done", "of": of });
        if let Some(id) = id {
            f["id"] = json!(id);
        }
        self.send_to(peer_id, f);
    }

    fn send_err(&self, peer_id: u64, code: &str, id: Option<String>) {
        let mut f = json!({ "t": "err", "code": code });
        if let Some(id) = id {
            f["id"] = json!(id);
        }
        self.send_to(peer_id, f);
    }
}

#[derive(Clone)]
struct MemoryRelay {
    room: Arc<Mutex<RoomState>>,
}

struct MemoryLine {
    relay: MemoryRelay,
    peer_id: u64,
    callbacks: Arc<Mutex<Option<Arc<dyn RelaySocketCallbacks>>>>,
    closed: AtomicBool,
}

impl RelaySocket for MemoryLine {
    fn send(&self, text: String) -> Result<(), String> {
        if self.closed.load(Ordering::SeqCst) {
            return Err("closed".into());
        }
        let relay = self.relay.clone();
        let peer_id = self.peer_id;
        std::thread::spawn(move || {
            relay.room.lock().unwrap().handle_message(peer_id, &text);
        });
        Ok(())
    }

    fn close(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        self.callbacks.lock().unwrap().take();
        let relay = self.relay.clone();
        let peer_id = self.peer_id;
        std::thread::spawn(move || {
            relay.room.lock().unwrap().handle_closed(peer_id);
        });
    }

    fn set_callbacks(&self, callbacks: Arc<dyn RelaySocketCallbacks>) {
        *self.callbacks.lock().unwrap() = Some(Arc::clone(&callbacks));
        std::thread::spawn(move || callbacks.on_open());
    }
}

impl MemoryRelay {
    fn new(push_fn: Option<Arc<dyn Fn(&str, &str) -> u16 + Send + Sync>>) -> Self {
        Self {
            room: Arc::new(Mutex::new(RoomState::new(push_fn))),
        }
    }

    fn connect_line(&self) -> Arc<MemoryLine> {
        let callbacks = Arc::new(Mutex::new(None));
        let peer_id = {
            let mut st = self.room.lock().unwrap();
            let id = st.next_peer;
            st.next_peer += 1;
            st.peers.insert(
                id,
                RelayPeer {
                    role: PeerRole::New,
                    callbacks: Arc::clone(&callbacks),
                },
            );
            id
        };
        Arc::new(MemoryLine {
            relay: self.clone(),
            peer_id,
            callbacks,
            closed: AtomicBool::new(false),
        })
    }

    fn connector(&self) -> crate::phone_anywhere::RelayConnector {
        let relay = self.clone();
        Arc::new(move |_url: &str| Ok(relay.connect_line() as Arc<dyn RelaySocket>))
    }

    fn drop_all(&self) {
        let peers: Vec<(u64, RelayPeer)> = {
            let mut st = self.room.lock().unwrap();
            st.peers.drain().collect()
        };
        for (_, p) in peers {
            p.deliver_close();
        }
    }

    fn pc_auths(&self) -> usize {
        self.room.lock().unwrap().pc_auths
    }

    fn stored_count(&self) -> usize {
        self.room.lock().unwrap().stored_count()
    }

    fn stored_notes_debug(&self) -> String {
        format!("{:?}", self.room.lock().unwrap().notes)
    }

    fn hook_post(&self, secret: &str, body: &str, content_type: &str) -> u16 {
        let hash = sha256_b64url(secret);
        let mut st = self.room.lock().unwrap();
        let Some(hook_id) = st
            .hooks
            .iter()
            .find(|(_, h)| same_text(&h.hash, &hash))
            .map(|(k, _)| k.clone())
        else {
            return 404;
        };
        let at = now_ms();
        let id = format!("{at:013}-{}", random_text(6));
        st.letters.push(StoredLetter {
            id,
            at,
            from: "hook".to_owned(),
            hook: Some(hook_id.clone()),
            n: None,
            c: None,
            body: Some(body.to_owned()),
            content_type: Some(content_type.to_owned()),
        });
        if let Some(pc_id) = st.pc_peer_id(None) {
            st.send_to(pc_id, json!({ "t": "hook-letter", "hook": hook_id }));
        }
        202
    }

    fn away_get(&self, token: &str) -> (u16, Value) {
        let hash = sha256_b64url(token);
        let st = self.room.lock().unwrap();
        let Some(ah) = &st.away_hash else {
            return (403, json!({ "error": "away-off" }));
        };
        if !same_text(ah, &hash) {
            return (401, json!({ "error": "auth" }));
        }
        let pc = st.pc_peer_id(None).is_some();
        (200, json!({ "pc": pc }))
    }

    fn away_post(&self, token: &str, n: &str, c: &str, alert: Option<&str>) -> (u16, Value) {
        let hash = sha256_b64url(token);
        let mut st = self.room.lock().unwrap();
        let Some(ah) = &st.away_hash else {
            return (403, json!({ "error": "away-off" }));
        };
        if !same_text(ah, &hash) {
            return (401, json!({ "error": "auth" }));
        }
        let pc = st.pc_peer_id(None).is_some();
        let mut rung = 0usize;
        if let Some(kind) = alert
            && !pc
            && let Some(push_fn) = st.push_fn.clone()
        {
            let open_devs: Vec<String> = st
                .peers
                .values()
                .filter_map(|p| match &p.role {
                    PeerRole::Device(d) => Some(d.clone()),
                    _ => None,
                })
                .collect();
            for al in &st.away_alerts {
                if open_devs.contains(&al.device) || !st.devices.contains_key(&al.device) {
                    continue;
                }
                let body = if kind == "found" { &al.found } else { &al.brief };
                let status = push_fn(&al.endpoint, body);
                if (200..300).contains(&status) {
                    rung += 1;
                }
            }
        }
        let at = now_ms();
        let id = format!("{at:013}-{}", random_text(6));
        let letter_json = json!({ "id": id, "at": at, "from": "away", "n": n, "c": c });
        st.letters.push(StoredLetter {
            id: id.clone(),
            at,
            from: "away".to_owned(),
            hook: None,
            n: Some(n.to_owned()),
            c: Some(c.to_owned()),
            body: None,
            content_type: None,
        });
        let recipients: Vec<u64> = st
            .peers
            .iter()
            .filter(|(_, p)| matches!(p.role, PeerRole::Pc | PeerRole::Device(_)))
            .map(|(&pid, _)| pid)
            .collect();
        for pid in recipients {
            st.send_to(pid, json!({ "t": "letter", "item": letter_json }));
        }
        (200, json!({ "id": id, "rung": rung }))
    }
}

// ---- Test phone client helpers --------------------------------------------------------------------------------------

struct ClientListener {
    open_tx: Mutex<Option<mpsc::Sender<()>>>,
    msg_tx: mpsc::Sender<Value>,
}

impl RelaySocketCallbacks for ClientListener {
    fn on_open(&self) {
        if let Some(tx) = self.open_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }

    fn on_message(&self, text: String) {
        if let Ok(val) = serde_json::from_str::<Value>(&text) {
            let _ = self.msg_tx.send(val);
        }
    }

    fn on_close(&self) {}
}

struct FrameReader {
    rx: mpsc::Receiver<Value>,
    buf: Vec<Value>,
}

impl FrameReader {
    fn next_matching(&mut self, timeout_ms: u64, pred: impl Fn(&Value) -> bool) -> Option<Value> {
        if let Some(pos) = self.buf.iter().position(&pred) {
            return Some(self.buf.remove(pos));
        }
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        while Instant::now() < deadline {
            let rem = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(rem) {
                Ok(v) => {
                    if pred(&v) {
                        return Some(v);
                    }
                    self.buf.push(v);
                }
                Err(_) => return None,
            }
        }
        None
    }

    fn drain_available(&mut self) {
        while let Ok(v) = self.rx.try_recv() {
            self.buf.push(v);
        }
    }
}

struct PairedPhone {
    room: String,
    device: String,
    token: String,
    keys: LinkKeys,
}

fn pair_phone(relay: &MemoryRelay, url: &str, bad_proof: Option<&str>) -> Result<PairedPhone, String> {
    let frag = url.split("#p=").nth(1).ok_or("no #p=")?;
    let parts: Vec<&str> = frag.split('.').collect();
    let (room, pc_key, secret) = (parts[0], parts[1], parts[2]);
    let dev_keys = new_pc_keys();
    let device = device_id_of(&dev_keys.public_key);
    let keys = link_keys(&dev_keys.private_key, pc_key, room, LinkSide::Device)?;

    let line = relay.connect_line();
    let (open_tx, open_rx) = mpsc::channel();
    let (msg_tx, msg_rx) = mpsc::channel();
    line.set_callbacks(Arc::new(ClientListener {
        open_tx: Mutex::new(Some(open_tx)),
        msg_tx,
    }));
    open_rx.recv_timeout(Duration::from_secs(2)).map_err(|_| "no open")?;
    let mut reader = FrameReader { rx: msg_rx, buf: Vec::new() };

    line.send(json!({ "t": "auth", "as": "pairing", "token": pairing_token(secret) }).to_string())?;
    let ok = reader.next_matching(2_000, |_| true).ok_or("no auth reply")?;
    if ok.get("t").and_then(Value::as_str) != Some("ok") {
        let code = ok.get("code").and_then(Value::as_str).unwrap_or("unknown");
        line.close();
        return Err(format!("refused: {code}"));
    }
    let proof = bad_proof
        .map(str::to_owned)
        .unwrap_or_else(|| pairing_proof(secret));
    let hello = seal_json(
        &keys.seal,
        &json!({ "v": 1, "kind": "pair", "proof": proof, "name": "iPhone" }),
        &aad_of(room, &device, Direction::DeviceToPc),
    )?;
    line.send(
        json!({ "t": "send", "to": "pc", "n": hello.n, "c": hello.c, "k": dev_keys.public_key })
            .to_string(),
    )?;
    let answer = reader.next_matching(600, |f| f.get("t").and_then(Value::as_str) == Some("msg"));
    line.close();
    let answer = answer.ok_or("nothing came")?;
    let said = open_json(
        &keys.open,
        answer["n"].as_str().unwrap(),
        answer["c"].as_str().unwrap(),
        &aad_of(room, &device, Direction::PcToDevice),
    )?;
    Ok(PairedPhone {
        room: room.to_owned(),
        device,
        token: said["token"].as_str().unwrap().to_owned(),
        keys,
    })
}

struct PhoneSession {
    phone: PairedPhone,
    line: Arc<MemoryLine>,
    reader: FrameReader,
    ok: Value,
    seq: u64,
}

impl PhoneSession {
    fn open(relay: &MemoryRelay, phone: &PairedPhone, first_seq: u64) -> Self {
        let line = relay.connect_line();
        let (open_tx, open_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        line.set_callbacks(Arc::new(ClientListener {
            open_tx: Mutex::new(Some(open_tx)),
            msg_tx,
        }));
        open_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let mut reader = FrameReader { rx: msg_rx, buf: Vec::new() };
        line.send(
            json!({ "t": "auth", "as": "device", "device": phone.device, "token": phone.token })
                .to_string(),
        )
        .unwrap();
        let ok = reader.next_matching(2_000, |_| true).unwrap();
        Self {
            phone: PairedPhone {
                room: phone.room.clone(),
                device: phone.device.clone(),
                token: phone.token.clone(),
                keys: phone.keys,
            },
            line,
            reader,
            ok,
            seq: first_seq,
        }
    }

    fn ask(&mut self, path: &str, body: Option<Value>) -> (Value, String) {
        self.seq += 1;
        let id = format!("q{}", self.seq);
        let mut req = json!({ "v": 1, "kind": "req", "id": id, "seq": self.seq, "t": now_ms(), "path": path });
        if let Some(b) = body {
            req["body"] = b;
        }
        let boxed = seal_json(
            &self.phone.keys.seal,
            &req,
            &aad_of(&self.phone.room, &self.phone.device, Direction::DeviceToPc),
        )
        .unwrap();
        let frame = json!({ "t": "send", "to": "pc", "n": boxed.n, "c": boxed.c }).to_string();
        self.line.send(frame.clone()).unwrap();
        let ans = self
            .reader
            .next_matching(3_000, |f| f.get("t").and_then(Value::as_str) == Some("msg"))
            .expect("expected msg reply");
        let said = open_json(
            &self.phone.keys.open,
            ans["n"].as_str().unwrap(),
            ans["c"].as_str().unwrap(),
            &aad_of(&self.phone.room, &self.phone.device, Direction::PcToDevice),
        )
        .unwrap();
        assert_eq!(said["id"].as_str(), Some(id.as_str()));
        (said, frame)
    }

    fn send_note(&mut self, note: Value, custom_keys: Option<LinkKeys>, ask_id: &str) -> String {
        let keys = custom_keys.unwrap_or(self.phone.keys);
        let mut payload = note;
        payload["v"] = json!(1);
        payload["kind"] = json!("note");
        let boxed = seal_json(
            &keys.seal,
            &payload,
            &aad_of(&self.phone.room, &self.phone.device, Direction::DeviceToPc),
        )
        .unwrap();
        self.line
            .send(json!({ "t": "note", "n": boxed.n, "c": boxed.c, "id": ask_id }).to_string())
            .unwrap();
        let kept = self
            .reader
            .next_matching(3_000, |f| {
                f.get("id").and_then(Value::as_str) == Some(ask_id)
                    && matches!(f.get("t").and_then(Value::as_str), Some("noted" | "err"))
            })
            .unwrap();
        assert_eq!(kept["t"], "noted");
        kept["note"].as_str().unwrap().to_owned()
    }

    fn my_notes(&mut self, ask_id: &str) -> Vec<Value> {
        self.line
            .send(json!({ "t": "notes", "id": ask_id }).to_string())
            .unwrap();
        let resp = self
            .reader
            .next_matching(3_000, |f| {
                f.get("id").and_then(Value::as_str) == Some(ask_id)
                    && f.get("t").and_then(Value::as_str) == Some("my-notes")
            })
            .unwrap();
        resp["items"].as_array().cloned().unwrap_or_default()
    }
}

// ---- Fake PhoneCalls ------------------------------------------------------------------------------------------------

struct FakeCalls {
    decided: Mutex<Vec<(String, bool)>>,
    sent: Mutex<Vec<(String, String)>>,
}

impl FakeCalls {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            decided: Mutex::new(Vec::new()),
            sent: Mutex::new(Vec::new()),
        })
    }
}

impl PhoneCalls for FakeCalls {
    fn state(&self) -> PhoneState {
        PhoneState {
            waiting: vec![PhoneWaiting {
                approval_id: "ap1".to_owned(),
                mission_id: "m1".to_owned(),
                summary: "Run npm test".to_owned(),
                requested_at: "2026-10-23T00:00:00.000Z".to_owned(),
                name: None,
                exact: None,
                plain: None,
            }],
            conversations: Vec::new(),
            scout: None,
        }
    }

    fn turns(&self, mission_id: &str) -> Option<Vec<PhoneTurn>> {
        if mission_id == "m1" {
            Some(Vec::new())
        } else {
            None
        }
    }

    fn answer_card(&self, approval_id: &str, approve: bool) -> bool {
        self.decided
            .lock()
            .unwrap()
            .push((approval_id.to_owned(), approve));
        true
    }

    fn send(&self, mission_id: &str, text: &str) -> PhoneSent {
        self.sent
            .lock()
            .unwrap()
            .push((mission_id.to_owned(), text.to_owned()));
        PhoneSent::Ok
    }
}

fn make_pc(
    relay: &MemoryRelay,
    dir: PathBuf,
    calls: Arc<dyn PhoneCalls>,
    away: Option<Arc<dyn AwayCallbacks>>,
    hooks: Option<Arc<dyn HooksCallbacks>>,
    desk: Option<Arc<dyn DeskCallbacks>>,
    notes: Option<Arc<dyn NotesCallbacks>>,
) -> PhoneAnywhere {
    PhoneAnywhere::new(PhoneAnywhereOptions {
        directory: dir,
        calls,
        relay: Arc::new(|| "https://relay.test".to_owned()),
        refusal: None,
        changed: None,
        connect: relay.connector(),
        now: None,
        protect_file: Some(Arc::new(|_| Ok(()))),
        retry_ms: Some(Arc::new(|_| 20)),
        away,
        hooks,
        desk,
        notes,
    })
}

// ---- Tests (15 cases from apps/desktop/src/main/a-phone-pairs-and-asks-through-the-relay.test.ts) -------------------

#[test]
// case: registers a hook by hash, hears its live notice, collects offline calls and retires the old password
fn registers_a_hook_by_hash_hears_its_live_notice_collects_offline_calls_and_retires_the_old_password()
{
    struct CountHooks(AtomicUsize);
    impl HooksCallbacks for CountHooks {
        fn refresh(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let relay = MemoryRelay::new(None);
    let dir = TempDir::new("locust-hook-relay");
    let nudges = Arc::new(CountHooks(AtomicUsize::new(0)));
    let pc = make_pc(
        &relay,
        dir.path().clone(),
        FakeCalls::new(),
        None,
        Some(nudges.clone()),
        None,
        None,
    );
    pc.set_enabled(true);
    until(3_000, || pc.connected());
    let token = random_text(32);
    let hash = sha256_b64url(&token);
    assert!(pc.hook_open("rt_a", &hash));
    let before = nudges.0.load(Ordering::SeqCst);
    assert_eq!(relay.hook_post(&token, "online", "text/plain"), 202);
    until(3_000, || nudges.0.load(Ordering::SeqCst) > before);
    let page = pc.hook_letters("rt_a").unwrap();
    assert_eq!(page.lost, 0);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].hook, "rt_a");
    assert_eq!(page.items[0].body, "online");
    assert_eq!(page.items[0].content_type, "text/plain");
    let ids: Vec<String> = page.items.iter().map(|i| i.id.clone()).collect();
    assert!(pc.hook_collected("rt_a", &ids));
    assert!(pc.hook_letters("rt_a").unwrap().items.is_empty());
    pc.dispose();

    assert_eq!(relay.hook_post(&token, "offline", "application/json"), 202);
    let back = make_pc(&relay, dir.path().clone(), FakeCalls::new(), None, None, None, None);
    back.load();
    until(3_000, || back.connected());
    let offline = back.hook_letters("rt_a").unwrap();
    assert_eq!(offline.items[0].body, "offline");
    assert_eq!(offline.items[0].content_type, "application/json");
    assert!(back.hook_open("rt_a", &sha256_b64url(&random_text(32))));
    assert_eq!(relay.hook_post(&token, "old", "text/plain"), 404);
    assert!(back.hook_close("rt_a"));
    back.dispose();
}

#[test]
// case: pairs a phone by its code, then answers what the page asks and does what it may
fn pairs_a_phone_by_its_code_then_answers_what_the_page_asks_and_does_what_it_may() {
    let relay = MemoryRelay::new(None);
    let dir = TempDir::new("locust-anywhere");
    let calls = FakeCalls::new();
    let pc = make_pc(&relay, dir.path().clone(), calls.clone(), None, None, None, None);
    let on = pc.set_enabled(true);
    assert!(on.enabled);
    let url = on.code.as_ref().unwrap().url.clone();
    assert!(url.starts_with("https://relay.test/#p="));
    let parts: Vec<&str> = url.split("#p=").nth(1).unwrap().split('.').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].len(), 22);
    assert_eq!(parts[1].len(), 43);
    assert_eq!(parts[2].len(), 22);
    until(3_000, || pc.settings().connected);

    let phone = pair_phone(&relay, &url, None).unwrap();
    until(3_000, || pc.settings().devices.len() == 1);
    let shown = pc.settings();
    assert_eq!(shown.devices[0].id, phone.device);
    assert_eq!(shown.devices[0].name, "iPhone");
    assert!(shown.code.is_none());

    let mut device = PhoneSession::open(&relay, &phone, 0);
    assert_eq!(device.ok, json!({ "t": "ok", "as": "device", "pc": true }));
    let (state_res, _) = device.ask("/api/state", None);
    assert_eq!(state_res["status"], 200);
    assert_eq!(state_res["data"]["waiting"][0]["approvalId"], "ap1");
    assert_eq!(device.ask("/api/turns?id=gone", None).0["status"], 404);
    let (decided, _) = device.ask("/api/decide", Some(json!({ "approvalId": "ap1", "approve": true })));
    assert_eq!(decided["status"], 200);
    assert_eq!(decided["data"]["ok"], true);
    assert_eq!(*calls.decided.lock().unwrap(), vec![("ap1".to_owned(), true)]);
    assert_eq!(
        device.ask("/api/send", Some(json!({ "missionId": "m1", "text": "   " }))).0["status"],
        400
    );
    let (sent, _) = device.ask("/api/send", Some(json!({ "missionId": "m1", "text": "Carry on" })));
    assert_eq!(sent["status"], 200);
    assert_eq!(sent["data"]["ok"], true);
    assert_eq!(*calls.sent.lock().unwrap(), vec![("m1".to_owned(), "Carry on".to_owned())]);
    assert_eq!(device.ask("/api/anything-else", None).0["status"], 404);
    pc.dispose();
}

#[test]
// case: sends an alert, sealed for that phone's browser alone, only to a phone that turned alerts on and is not open
fn sends_an_alert_sealed_for_that_phones_browser_alone_only_to_a_phone_that_turned_alerts_on_and_is_not_open()
{
    let posted: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let answer = Arc::new(AtomicUsize::new(201));
    let posted_cb = posted.clone();
    let answer_cb = answer.clone();
    let relay = MemoryRelay::new(Some(Arc::new(move |ep: &str, body: &str| {
        posted_cb.lock().unwrap().push((ep.to_owned(), body.to_owned()));
        answer_cb.load(Ordering::SeqCst) as u16
    })));
    let dir = TempDir::new("locust-alert");
    let pc = make_pc(&relay, dir.path().clone(), FakeCalls::new(), None, None, None, None);
    let on = pc.set_enabled(true);
    until(3_000, || pc.settings().connected);
    let phone = pair_phone(&relay, &on.code.unwrap().url, None).unwrap();
    let mut device = PhoneSession::open(&relay, &phone, 0);

    let receiver = p256_generate_keypair();
    let auth = random_bytes(16);
    let endpoint = "https://fcm.googleapis.com/fcm/send/phone-1";
    let (sub, _) = device.ask(
        "/api/alerts",
        Some(json!({
            "endpoint": endpoint,
            "p256dh": to_base64url(&receiver.public_key),
            "auth": to_base64url(&auth),
        })),
    );
    assert_eq!(sub["status"], 200);
    assert_eq!(sub["data"]["ok"], true);
    until(3_000, || pc.settings().devices.first().is_some_and(|d| d.alerts));

    let alert = PhoneAlert {
        kind: AlertKind::Approval,
        title: "Mira needs your approval".to_owned(),
        body: "The turn is paused until you answer.".to_owned(),
        tag: "waiting".to_owned(),
        approval_id: None,
        who: None,
    };
    // Open on the phone: nothing pushed.
    assert_eq!(pc.alert(&alert), 0);
    assert!(posted.lock().unwrap().is_empty());

    device.line.close();
    until(3_000, || !pc.settings().devices[0].online);
    assert_eq!(pc.alert(&alert), 1);
    let sent = posted.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, endpoint);
    let raw_body = from_base64url(&sent[0].1).unwrap();
    assert!(!String::from_utf8_lossy(&raw_body).contains("Mira"));
    let opened: Value = serde_json::from_str(&open_push(&raw_body, &receiver, &auth).unwrap()).unwrap();
    assert_eq!(
        opened,
        json!({ "title": "Mira needs your approval", "body": "The turn is paused until you answer.", "tag": "waiting" })
    );

    // 410 Gone clears the subscription so nothing more is pushed.
    answer.store(410, Ordering::SeqCst);
    let scout_alert = PhoneAlert {
        kind: AlertKind::Scout,
        title: "Scout".to_owned(),
        body: "The folder’s check fails.".to_owned(),
        tag: "scout:check".to_owned(),
        approval_id: None,
        who: None,
    };
    assert_eq!(pc.alert(&scout_alert), 0);
    until(3_000, || !pc.settings().devices[0].alerts);
    assert_eq!(pc.alert(&scout_alert), 0);
    assert_eq!(posted.lock().unwrap().len(), 2);
    pc.dispose();
}

#[test]
// case: does a copied request once, and never again
fn does_a_copied_request_once_and_never_again() {
    let relay = MemoryRelay::new(None);
    let dir = TempDir::new("locust-replay");
    let calls = FakeCalls::new();
    let pc = make_pc(&relay, dir.path().clone(), calls.clone(), None, None, None, None);
    let on = pc.set_enabled(true);
    until(3_000, || pc.settings().connected);
    let phone = pair_phone(&relay, &on.code.unwrap().url, None).unwrap();
    let mut device = PhoneSession::open(&relay, &phone, 0);
    let (_, frame) = device.ask("/api/send", Some(json!({ "missionId": "m1", "text": "Only once" })));
    device.line.send(frame.clone()).unwrap();
    device.line.send(frame).unwrap();
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(calls.sent.lock().unwrap().len(), 1);
    device.reader.drain_available();
    assert!(
        !device
            .reader
            .buf
            .iter()
            .any(|f| f.get("t").and_then(Value::as_str) == Some("msg"))
    );
    pc.dispose();
}

#[test]
// case: pairs one phone per code, and none that cannot prove it saw the code
fn pairs_one_phone_per_code_and_none_that_cannot_prove_it_saw_the_code() {
    let relay = MemoryRelay::new(None);
    let dir = TempDir::new("locust-one-code");
    let pc = make_pc(&relay, dir.path().clone(), FakeCalls::new(), None, None, None, None);
    let on = pc.set_enabled(true);
    until(3_000, || pc.settings().connected);
    let url = on.code.unwrap().url;

    let bad = pair_phone(&relay, &url, Some(&random_text(32)));
    assert_eq!(bad.err().as_deref(), Some("nothing came"));
    assert!(pc.settings().devices.is_empty());

    pair_phone(&relay, &url, None).unwrap();
    until(3_000, || pc.settings().devices.len() == 1);
    let second_try = pair_phone(&relay, &url, None);
    assert_eq!(second_try.err().as_deref(), Some("refused: pairing-closed"));
    assert_eq!(pc.settings().devices.len(), 1);

    // A new code opens at the relay a moment after it is made (a person takes seconds to scan it); under a loaded
    // machine the first try can land before that, so it is retried while the relay still says closed.
    let next = pc.new_code().code.unwrap().url;
    let started = std::time::Instant::now();
    let mut paired = pair_phone(&relay, &next, None);
    while paired.as_ref().err().map(String::as_str) == Some("refused: pairing-closed") && started.elapsed().as_millis() < 3_000 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        paired = pair_phone(&relay, &next, None);
    }
    paired.unwrap();
    until(3_000, || pc.settings().devices.len() == 2);
    pc.dispose();
}

#[test]
// case: closes a removed phone, and forgets every phone and the room when it is turned off
fn closes_a_removed_phone_and_forgets_every_phone_and_the_room_when_it_is_turned_off() {
    let relay = MemoryRelay::new(None);
    let dir = TempDir::new("locust-remove");
    let pc = make_pc(&relay, dir.path().clone(), FakeCalls::new(), None, None, None, None);
    let on = pc.set_enabled(true);
    until(3_000, || pc.settings().connected);
    let phone = pair_phone(&relay, &on.code.unwrap().url, None).unwrap();
    let mut device = PhoneSession::open(&relay, &phone, 0);

    pc.remove(&phone.device);
    let removed = device
        .reader
        .next_matching(3_000, |f| f.get("t").and_then(Value::as_str) == Some("removed"));
    assert_eq!(removed, Some(json!({ "t": "removed" })));
    assert!(pc.settings().devices.is_empty());

    assert!(dir.path().join(PHONE_ANYWHERE_FILE).exists());
    let off = pc.set_enabled(false);
    assert!(!off.enabled);
    assert!(!off.connected);
    assert!(off.devices.is_empty());
    until(3_000, || relay.stored_count() == 0);
    assert!(!dir.path().join(PHONE_ANYWHERE_FILE).exists());
    pc.dispose();
}

#[test]
// case: connects again after the line drops, and keeps its phones and their numbers across a restart
fn connects_again_after_the_line_drops_and_keeps_its_phones_and_their_numbers_across_a_restart() {
    let relay = MemoryRelay::new(None);
    let dir = TempDir::new("locust-reconnect");
    let first = make_pc(&relay, dir.path().clone(), FakeCalls::new(), None, None, None, None);
    let on = first.set_enabled(true);
    until(3_000, || first.settings().connected);
    let phone = pair_phone(&relay, &on.code.unwrap().url, None).unwrap();
    let mut device = PhoneSession::open(&relay, &phone, 0);
    let (_, asked) = device.ask("/api/send", Some(json!({ "missionId": "m1", "text": "Before" })));

    let before = relay.pc_auths();
    relay.drop_all();
    until(3_000, || relay.pc_auths() > before && first.settings().connected);
    let mut again = PhoneSession::open(&relay, &phone, 1);
    assert_eq!(again.ask("/api/state", None).0["status"], 200);
    first.dispose();

    let calls2 = FakeCalls::new();
    let second = make_pc(&relay, dir.path().clone(), calls2.clone(), None, None, None, None);
    second.load();
    until(3_000, || second.settings().connected);
    assert_eq!(
        second.settings().devices.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
        vec![phone.device.as_str()]
    );
    let mut later = PhoneSession::open(&relay, &phone, 2);
    assert_eq!(
        later.ask("/api/send", Some(json!({ "missionId": "m1", "text": "After" }))).0["status"],
        200
    );
    // Replay of the pre-restart request is ignored.
    later.line.send(asked).unwrap();
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(
        *calls2.sent.lock().unwrap(),
        vec![("m1".to_owned(), "After".to_owned())]
    );
    second.dispose();
}

#[test]
// case: forgets a called link when the phone relay is turned off while it is disconnected
fn forgets_a_called_link_when_the_phone_relay_is_turned_off_while_it_is_disconnected() {
    let relay = MemoryRelay::new(None);
    let dir = TempDir::new("locust-hook-off");
    let open = Arc::new(AtomicBool::new(true));
    let open_c = open.clone();
    let relay_c = relay.clone();
    let pc = PhoneAnywhere::new(PhoneAnywhereOptions {
        directory: dir.path().clone(),
        calls: FakeCalls::new(),
        relay: Arc::new(|| "https://relay.test".to_owned()),
        refusal: None,
        changed: None,
        connect: Arc::new(move |_| {
            if !open_c.load(Ordering::SeqCst) {
                return Err("offline".into());
            }
            Ok(relay_c.connect_line() as Arc<dyn RelaySocket>)
        }),
        now: None,
        protect_file: Some(Arc::new(|_| Ok(()))),
        retry_ms: Some(Arc::new(|_| 20)),
        away: None,
        hooks: None,
        desk: None,
        notes: None,
    });
    pc.set_enabled(true);
    until(3_000, || pc.connected());
    let token = random_text(32);
    assert!(pc.hook_open("rt_a", &sha256_b64url(&token)));
    assert_eq!(relay.hook_post(&token, "while on", "text/plain"), 202);
    open.store(false, Ordering::SeqCst);
    relay.drop_all();
    until(3_000, || !pc.connected());
    pc.set_enabled(false);
    open.store(true, Ordering::SeqCst);
    until(3_000, || relay.hook_post(&token, "after off", "text/plain") == 404);
    pc.dispose();
}

#[test]
// case: says it is turning off until the relay confirms, and finishes after a restart
fn says_it_is_turning_off_until_the_relay_confirms_and_finishes_after_a_restart() {
    let relay = MemoryRelay::new(None);
    let dir = TempDir::new("locust-turning-off");
    let open = Arc::new(AtomicBool::new(true));
    let make = || {
        let open_c = open.clone();
        let relay_c = relay.clone();
        PhoneAnywhere::new(PhoneAnywhereOptions {
            directory: dir.path().clone(),
            calls: FakeCalls::new(),
            relay: Arc::new(|| "https://relay.test".to_owned()),
            refusal: None,
            changed: None,
            connect: Arc::new(move |_| {
                if !open_c.load(Ordering::SeqCst) {
                    return Err("offline".into());
                }
                Ok(relay_c.connect_line() as Arc<dyn RelaySocket>)
            }),
            now: None,
            protect_file: Some(Arc::new(|_| Ok(()))),
            retry_ms: Some(Arc::new(|_| 20)),
            away: None,
            hooks: None,
            desk: None,
            notes: None,
        })
    };

    let first = make();
    first.set_enabled(true);
    until(3_000, || first.connected());
    let token = random_text(32);
    assert!(first.hook_open("rt_a", &sha256_b64url(&token)));
    open.store(false, Ordering::SeqCst);
    relay.drop_all();
    until(3_000, || !first.connected());

    let turning = first.set_enabled(false);
    assert!(!turning.enabled);
    assert_eq!(turning.message.as_deref(), Some("Turning off..."));
    assert!(turning.code.is_none());
    assert!(first.room().is_none());
    assert!(dir.path().join(PHONE_ANYWHERE_FILE).exists());
    let try_on = first.set_enabled(true);
    assert!(!try_on.enabled);
    assert_eq!(try_on.message.as_deref(), Some("Turning off..."));
    first.dispose();

    open.store(true, Ordering::SeqCst);
    let second = make();
    let loaded = second.load();
    assert!(!loaded.enabled);
    assert_eq!(loaded.message.as_deref(), Some("Turning off..."));
    until(3_000, || !dir.path().join(PHONE_ANYWHERE_FILE).exists());
    assert_eq!(relay.hook_post(&token, "after restart", "text/plain"), 404);
    let after = second.settings();
    assert!(!after.enabled);
    assert!(after.message.is_none());
    second.dispose();
}

#[test]
// case: gives a phone the away key, keeps its alerts at the relay to ring with the PC off, and hands the PC the letters
fn gives_a_phone_the_away_key_keeps_its_alerts_at_the_relay_to_ring_with_the_pc_off_and_hands_the_pc_the_letters()
{
    struct TestAway {
        key: String,
        on: AtomicBool,
        refreshed: AtomicUsize,
        heard: Mutex<Vec<RelayLetter>>,
    }
    impl AwayCallbacks for TestAway {
        fn for_phone(&self) -> Option<AwayForPhone> {
            if self.on.load(Ordering::SeqCst) {
                Some(AwayForPhone {
                    key: self.key.clone(),
                    repo: "acme/app".to_owned(),
                })
            } else {
                None
            }
        }
        fn refresh(&self) {
            self.refreshed.fetch_add(1, Ordering::SeqCst);
        }
        fn letter(&self, letter: RelayLetter) {
            self.heard.lock().unwrap().push(letter);
        }
    }

    let posted: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let posted_cb = posted.clone();
    let relay = MemoryRelay::new(Some(Arc::new(move |ep: &str, body: &str| {
        posted_cb.lock().unwrap().push((ep.to_owned(), body.to_owned()));
        201
    })));
    let dir = TempDir::new("locust-away");
    let away_key = random_text(32);
    let away = Arc::new(TestAway {
        key: away_key.clone(),
        on: AtomicBool::new(true),
        refreshed: AtomicUsize::new(0),
        heard: Mutex::new(Vec::new()),
    });
    let pc = make_pc(
        &relay,
        dir.path().clone(),
        FakeCalls::new(),
        Some(away.clone()),
        None,
        None,
        None,
    );
    let on = pc.set_enabled(true);
    until(3_000, || pc.settings().connected);
    assert_eq!(away.refreshed.load(Ordering::SeqCst), 1);

    let phone = pair_phone(&relay, &on.code.unwrap().url, None).unwrap();
    let mut device = PhoneSession::open(&relay, &phone, 0);
    let (res_on, _) = device.ask("/api/away", None);
    assert_eq!(res_on["status"], 200);
    assert_eq!(res_on["data"], json!({ "key": away_key, "repo": "acme/app" }));
    away.on.store(false, Ordering::SeqCst);
    let (res_off, _) = device.ask("/api/away", None);
    assert_eq!(res_off["status"], 200);
    assert_eq!(res_off["data"], json!({ "off": true }));

    let receiver = p256_generate_keypair();
    let auth = random_bytes(16);
    let endpoint = "https://fcm.googleapis.com/fcm/send/phone-1";
    device.ask(
        "/api/alerts",
        Some(json!({
            "endpoint": endpoint,
            "p256dh": to_base64url(&receiver.public_key),
            "auth": to_base64url(&auth),
        })),
    );
    assert_eq!(away.refreshed.load(Ordering::SeqCst), 2);

    let token = random_text(32);
    assert!(pc.away_open(&sha256_b64url(&token)));
    assert!(pc.keep_away_alerts());
    assert!(posted.lock().unwrap().is_empty());

    let n = random_text(12);
    let c = random_text(80);
    let (status, _) = relay.away_post(&token, &n, &c, None);
    assert_eq!(status, 200);
    until(3_000, || away.heard.lock().unwrap().len() == 1);
    assert_eq!(away.heard.lock().unwrap()[0].from, "away");
    assert_eq!(away.heard.lock().unwrap()[0].n, n);
    assert_eq!(away.heard.lock().unwrap()[0].c, c);

    pc.dispose();
    device.line.close();
    until(3_000, || relay.room.lock().unwrap().peers.is_empty() && relay.away_get(&token).1["pc"] == false);
    let (_, rung) = relay.away_post(&token, &n, &c, Some("found"));
    assert_eq!(rung["rung"], 1);
    let sent = posted.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, endpoint);
    let opened: Value = serde_json::from_str(
        &open_push(&from_base64url(&sent[0].1).unwrap(), &receiver, &auth).unwrap(),
    )
    .unwrap();
    assert_eq!(opened, away_found_alert_json());

    let back = make_pc(&relay, dir.path().clone(), FakeCalls::new(), None, None, None, None);
    back.load();
    until(3_000, || back.settings().connected);
    let page = back.letters(None).unwrap();
    assert_eq!(
        page.items.iter().map(|i| i.from.as_str()).collect::<Vec<_>>(),
        vec!["away", "away"]
    );
    assert!(back.post(&n, &c));
    let after = back.letters(Some(&page.items.last().unwrap().id)).unwrap();
    assert_eq!(
        after.items.iter().map(|i| i.from.as_str()).collect::<Vec<_>>(),
        vec!["pc"]
    );
    back.dispose();
}

#[test]
// case: takes a browser’s frames only once Full window is on, in order, once each, and in parts when long
fn takes_a_browsers_frames_only_once_full_window_is_on_in_order_once_each_and_in_parts_when_long() {
    struct RecordDesk(Mutex<Vec<(String, Vec<Value>)>>);
    impl DeskCallbacks for RecordDesk {
        fn frame(&self, device: &str, asks: Vec<Value>) {
            self.0.lock().unwrap().push((device.to_owned(), asks));
        }
        fn forget(&self, _device: &str) {}
    }

    let relay = MemoryRelay::new(None);
    let dir = TempDir::new("locust-desk");
    let desk = Arc::new(RecordDesk(Mutex::new(Vec::new())));
    let pc = make_pc(
        &relay,
        dir.path().clone(),
        FakeCalls::new(),
        None,
        None,
        Some(desk.clone()),
        None,
    );
    let on = pc.set_enabled(true);
    until(3_000, || pc.settings().connected);
    let phone = pair_phone(&relay, &on.code.unwrap().url, None).unwrap();
    let mut device = PhoneSession::open(&relay, &phone, 0);
    let session = random_text(9);

    let device_line = Arc::clone(&device.line);
    let send_frame = |q: u64, items: Value| {
        let boxed = seal_json(
            &phone.keys.seal,
            &json!({ "v": 1, "kind": "desk", "s": session, "q": q, "t": now_ms(), "items": items }),
            &aad_of(&phone.room, &phone.device, Direction::DeviceToPc),
        )
        .unwrap();
        device_line
            .send(json!({ "t": "send", "to": "pc", "n": boxed.n, "c": boxed.c }).to_string())
            .unwrap();
    };
    let read_desk = |dev: &mut PhoneSession| -> Value {
        let f = dev
            .reader
            .next_matching(3_000, |f| f.get("t").and_then(Value::as_str) == Some("msg"))
            .unwrap();
        open_json(
            &phone.keys.open,
            f["n"].as_str().unwrap(),
            f["c"].as_str().unwrap(),
            &aad_of(&phone.room, &phone.device, Direction::PcToDevice),
        )
        .unwrap()
    };

    // Full window off: refused by id.
    send_frame(1, json!([{ "kind": "hello", "id": "h1" }]));
    let refused = read_desk(&mut device);
    assert_eq!(refused["v"], 1);
    assert_eq!(refused["kind"], "desk");
    assert_eq!(refused["items"][0]["id"], "h1");
    assert_eq!(refused["items"][0]["ok"], false);
    assert_eq!(refused["items"][0]["refused"], true);
    assert!(desk.0.lock().unwrap().is_empty());

    assert_eq!(pc.set_desk(&phone.device, true).devices[0].desk, Some(true));
    send_frame(2, json!([{ "kind": "hello", "id": "h2" }]));
    until(3_000, || desk.0.lock().unwrap().len() == 1);
    assert_eq!(desk.0.lock().unwrap()[0].0, phone.device);
    assert_eq!(desk.0.lock().unwrap()[0].1, vec![json!({ "kind": "hello", "id": "h2" })]);

    // Duplicate and older frames dropped.
    send_frame(2, json!([{ "kind": "hello", "id": "again" }]));
    send_frame(1, json!([{ "kind": "hello", "id": "older" }]));
    send_frame(3, json!([{ "kind": "hello", "id": "h3" }]));
    until(3_000, || desk.0.lock().unwrap().len() == 2);
    let ids: Vec<String> = desk
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|(_, asks)| asks[0]["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, vec!["h2", "h3"]);

    // Long frame from browser, in parts.
    let big_text = json!({
        "v": 1,
        "kind": "desk",
        "s": session,
        "q": 4,
        "t": now_ms(),
        "items": [{ "kind": "call", "id": "big", "channel": "mission:read", "args": json!(["x".repeat(300_000)]).to_string() }]
    })
    .to_string();
    for part in desk_parts("big-frame", &big_text) {
        let part_val = json!({ "v": part.v, "kind": part.kind, "id": part.id, "n": part.n, "of": part.of, "text": part.text });
        let boxed = seal_json(
            &phone.keys.seal,
            &part_val,
            &aad_of(&phone.room, &phone.device, Direction::DeviceToPc),
        )
        .unwrap();
        device
            .line
            .send(json!({ "t": "send", "to": "pc", "n": boxed.n, "c": boxed.c }).to_string())
            .unwrap();
    }
    until(3_000, || desk.0.lock().unwrap().len() == 3);
    assert_eq!(desk.0.lock().unwrap()[2].1[0]["id"], "big");

    // Long answer back to browser, in parts.
    assert!(pc.tell_desk(
        &phone.device,
        &[json!({ "kind": "res", "id": "big", "ok": true, "value": json!("y".repeat(500_000)).to_string() })],
    ));
    let mut joiner = DeskJoiner::default();
    let mut whole = None;
    while whole.is_none() {
        let said = read_desk(&mut device);
        assert_eq!(said["kind"], "desk-part");
        let part = DeskPart {
            v: 1,
            kind: "desk-part",
            id: said["id"].as_str().unwrap().to_owned(),
            n: said["n"].as_u64().unwrap() as usize,
            of: said["of"].as_u64().unwrap() as usize,
            text: said["text"].as_str().unwrap().to_owned(),
        };
        whole = joiner.push(&part);
    }
    let parsed: Value = serde_json::from_str(&whole.unwrap()).unwrap();
    assert_eq!(parsed["v"], 1);
    assert_eq!(parsed["kind"], "desk");
    assert_eq!(parsed["items"][0]["id"], "big");
    assert_eq!(parsed["items"][0]["ok"], true);

    assert_eq!(pc.set_desk(&phone.device, false).devices[0].desk, None);
    pc.dispose();
}

// ---- Offline notes helper -------------------------------------------------------------------------------------------

struct OffWithAPhone {
    dir: TempDir,
    phone: PairedPhone,
    session: PhoneSession,
}

fn off_with_a_phone(relay: &MemoryRelay) -> OffWithAPhone {
    let dir = TempDir::new("locust-notes");
    let first = make_pc(relay, dir.path().clone(), FakeCalls::new(), None, None, None, None);
    let on = first.set_enabled(true);
    until(3_000, || first.connected());
    let phone = pair_phone(relay, &on.code.unwrap().url, None).unwrap();
    until(3_000, || first.settings().devices.len() == 1);
    first.dispose();
    until(3_000, || relay.room.lock().unwrap().pc_peer_id(None).is_none());
    let session = PhoneSession::open(relay, &phone, 0);
    assert_eq!(session.ok, json!({ "t": "ok", "as": "device", "pc": false }));
    OffWithAPhone { dir, phone, session }
}

struct DeliverFn(Box<dyn Fn(&str, &str) -> NoteDelivery + Send + Sync>, Mutex<Vec<(String, String)>>);

impl NotesCallbacks for DeliverFn {
    fn deliver(&self, mission_id: &str, text: &str) -> NoteDelivery {
        self.1
            .lock()
            .unwrap()
            .push((mission_id.to_owned(), text.to_owned()));
        (self.0)(mission_id, text)
    }
}

#[test]
// case: sends a message left while the PC was off once it is back, through the phone's own path
fn sends_a_message_left_while_the_pc_was_off_once_it_is_back_through_the_phones_own_path() {
    let relay = MemoryRelay::new(None);
    let mut off = off_with_a_phone(&relay);
    off.session.send_note(
        json!({ "nid": random_text(16), "missionId": "m1", "text": "  Run the tests again  ", "t": now_ms() }),
        None,
        "n1",
    );
    assert!(!relay.stored_notes_debug().contains("Run the tests"));

    let deliver = Arc::new(DeliverFn(Box::new(|_, _| NoteDelivery::Sent), Mutex::new(Vec::new())));
    let pc = make_pc(
        &relay,
        off.dir.path().clone(),
        FakeCalls::new(),
        None,
        None,
        None,
        Some(deliver.clone()),
    );
    pc.load();
    until(3_000, || deliver.1.lock().unwrap().len() == 1);
    assert_eq!(
        *deliver.1.lock().unwrap(),
        vec![("m1".to_owned(), "Run the tests again".to_owned())]
    );
    let mut ask_n = 1;
    until(3_000, || {
        ask_n += 1;
        off.session.my_notes(&format!("chk{ask_n}")).is_empty()
    });
    assert_eq!(
        off.session
            .reader
            .next_matching(3_000, |f| f.get("t").and_then(Value::as_str) == Some("notes-changed")),
        Some(json!({ "t": "notes-changed" }))
    );
    pc.dispose();
}

#[test]
// case: never sends one twice, even when the relay hands the same note back again
fn never_sends_one_twice_even_when_the_relay_hands_the_same_note_back_again() {
    let relay = MemoryRelay::new(None);
    let mut off = off_with_a_phone(&relay);
    let same = json!({ "nid": random_text(16), "missionId": "m1", "text": "Once only", "t": now_ms() });
    off.session.send_note(same.clone(), None, "n1");
    off.session.send_note(same, None, "n2");

    let deliver = Arc::new(DeliverFn(Box::new(|_, _| NoteDelivery::Sent), Mutex::new(Vec::new())));
    let pc = make_pc(
        &relay,
        off.dir.path().clone(),
        FakeCalls::new(),
        None,
        None,
        None,
        Some(deliver.clone()),
    );
    pc.load();
    let mut ask_n = 1;
    until(3_000, || {
        ask_n += 1;
        off.session.my_notes(&format!("chk{ask_n}")).is_empty()
    });
    assert_eq!(deliver.1.lock().unwrap().len(), 1);
    pc.dispose();
}

#[test]
// case: holds one written more than twelve hours ago for the phone to decide, and sends it never by itself
fn holds_one_written_more_than_twelve_hours_ago_for_the_phone_to_decide_and_sends_it_never_by_itself()
{
    let relay = MemoryRelay::new(None);
    let mut off = off_with_a_phone(&relay);
    off.session.send_note(
        json!({
            "nid": random_text(16),
            "missionId": "m1",
            "text": "From yesterday",
            "t": now_ms() - NOTE_SENDS_WITHIN_MS - 60_000,
        }),
        None,
        "n1",
    );
    let deliver = Arc::new(DeliverFn(Box::new(|_, _| NoteDelivery::Sent), Mutex::new(Vec::new())));
    let pc = make_pc(
        &relay,
        off.dir.path().clone(),
        FakeCalls::new(),
        None,
        None,
        None,
        Some(deliver.clone()),
    );
    pc.load();
    let mut ask_n = 1;
    until(3_000, || {
        ask_n += 1;
        off.session
            .my_notes(&format!("chk{ask_n}"))
            .iter()
            .any(|item| item.get("held").and_then(Value::as_str) == Some("old"))
    });
    assert!(deliver.1.lock().unwrap().is_empty());
    pc.dispose();
}

#[test]
// case: holds with why one whose conversation is gone, or that does not open, and leaves a busy one waiting
fn holds_with_why_one_whose_conversation_is_gone_or_that_does_not_open_and_leaves_a_busy_one_waiting()
{
    let relay = MemoryRelay::new(None);
    let mut off = off_with_a_phone(&relay);
    let gone = off.session.send_note(
        json!({ "nid": random_text(16), "missionId": "gone", "text": "Nobody here", "t": now_ms() }),
        None,
        "n1",
    );
    let busy = off.session.send_note(
        json!({ "nid": random_text(16), "missionId": "busy", "text": "Still working", "t": now_ms() }),
        None,
        "n2",
    );
    let other_pc = new_pc_keys();
    let other_dev = new_pc_keys();
    let other_keys = link_keys(
        &other_dev.private_key,
        &other_pc.public_key,
        &off.phone.room,
        LinkSide::Device,
    )
    .unwrap();
    let strange = off.session.send_note(
        json!({ "nid": random_text(16), "missionId": "m1", "text": "Sealed wrong", "t": now_ms() }),
        Some(other_keys),
        "n3",
    );

    let deliver = Arc::new(DeliverFn(
        Box::new(|mid, _| if mid == "gone" { NoteDelivery::Gone } else { NoteDelivery::Busy }),
        Mutex::new(Vec::new()),
    ));
    let pc = make_pc(
        &relay,
        off.dir.path().clone(),
        FakeCalls::new(),
        None,
        None,
        None,
        Some(deliver.clone()),
    );
    pc.load();
    let mut ask_n = 1;
    let mut latest = Vec::new();
    until(3_000, || {
        ask_n += 1;
        latest = off.session.my_notes(&format!("chk{ask_n}"));
        latest.iter().filter(|i| i.get("held").is_some()).count() == 2
    });
    let mut states: HashMap<String, String> = HashMap::new();
    for item in latest {
        let id = item["id"].as_str().unwrap().to_owned();
        let st = item
            .get("held")
            .and_then(Value::as_str)
            .unwrap_or("waiting")
            .to_owned();
        states.insert(id, st);
    }
    assert_eq!(states.get(&gone).map(String::as_str), Some("gone"));
    assert_eq!(states.get(&busy).map(String::as_str), Some("waiting"));
    assert_eq!(states.get(&strange).map(String::as_str), Some("failed"));
    let mut called: Vec<String> = deliver
        .1
        .lock()
        .unwrap()
        .iter()
        .map(|(m, _)| m.clone())
        .collect();
    called.sort();
    assert_eq!(called, vec!["busy", "gone"]);
    pc.dispose();
}
