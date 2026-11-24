//! Parity and Electron carry-over tests for `apps/desktop/src/main/teammates-answer-their-person-in-discord.test.ts` (14 cases).
//! Uses an in-process fake Discord REST + Gateway server and a fake Electron-compatible base64 secret box; never contacts Discord or the OS keychain.

use super::*;
use crate::approval::{Answer, Decision, Kind, Request};
use crate::chat_apps::ChatAppSettings;
use crate::chat_http::{HttpClient, HttpRequest, HttpResponse, WsCallbacks, WsClient, WsConnection};
use crate::chat_link::{ChatAlert, ChatLink, ChatLinkOptions, ChatMission, ChatStart, ChatTeammate, TokenInput};
use crate::own_models::SecretBox;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BASE64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_CHARS[((n >> 18) & 63) as usize] as char);
        out.push(BASE64_CHARS[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { BASE64_CHARS[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { BASE64_CHARS[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut vals = Vec::with_capacity(s.len());
    for b in s.bytes() {
        if b == b'=' {
            continue;
        }
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err("bad base64".into()),
        };
        vals.push(v);
    }
    let mut out = Vec::with_capacity(vals.len() * 3 / 4);
    for chunk in vals.chunks(4) {
        if chunk.len() >= 2 {
            out.push((chunk[0] << 2) | (chunk[1] >> 4));
        }
        if chunk.len() >= 3 {
            out.push((chunk[1] << 4) | (chunk[2] >> 2));
        }
        if chunk.len() == 4 {
            out.push((chunk[2] << 6) | chunk[3]);
        }
    }
    Ok(out)
}

struct ElectronSecretBox;

impl ElectronSecretBox {
    fn seal_electron(text: &str) -> String {
        let xored: Vec<u8> = text.bytes().map(|b| b ^ 0x5a).collect();
        b64_encode(&xored)
    }
}

impl SecretBox for ElectronSecretBox {
    fn available(&self) -> bool {
        true
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        Ok(Self::seal_electron(text))
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        let bytes = b64_decode(sealed)?;
        let plain: Vec<u8> = bytes.into_iter().map(|b| b ^ 0x5a).collect();
        String::from_utf8(plain).map_err(|_| "bad utf8".into())
    }
}

fn ada() -> ChatTeammate {
    ChatTeammate { teammate_id: "mate_ada".into(), name: "Ada".into(), role: "Research & Briefs".into() }
}
fn wren() -> ChatTeammate {
    ChatTeammate { teammate_id: "mate_wren".into(), name: "Wren".into(), role: "Code & Migrations".into() }
}

fn shown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn fake_discord_token() -> String {
    ["MTEwMDAwMDAwMDAwMDAwMDAwMQ", "GfakeA", "fake_fake_fake_fake_fake_fake_fake_fak"].join(".")
}

const PERSON_ID: &str = "1200000000000000001";
const PERSON_CHANNEL: &str = "1300000000000000001";
const STRANGER_ID: &str = "1200000000000000999";
const STRANGER_CHANNEL: &str = "1300000000000000999";
const APP_ID: &str = "1100000000000000001";

#[derive(Clone, Debug)]
struct SentDiscordMsg {
    id: String,
    channel: String,
    content: String,
    reply_to: Option<String>,
    components: Vec<Value>,
    allowed_mentions: Value,
}

#[derive(Clone, Debug)]
struct EditedDiscordMsg {
    id: String,
    content: String,
    components: Vec<Value>,
}

#[derive(Clone, Debug)]
struct DiscordCallback {
    id: String,
    kind: u64,
    cb_type: u64,
}

#[derive(Clone, Debug)]
struct DiscordResponse {
    interaction: String,
    id: String,
    content: String,
}

struct FakeDiscordServer {
    token: String,
    heartbeat_ms: u64,
    sent: Mutex<Vec<SentDiscordMsg>>,
    edits: Mutex<Vec<EditedDiscordMsg>>,
    commands: Mutex<Vec<Vec<Value>>>,
    typing: Mutex<Vec<String>>,
    callbacks: Mutex<Vec<DiscordCallback>>,
    responses: Mutex<Vec<DiscordResponse>>,
    intents: Mutex<Vec<u64>>,
    identified: AtomicU64,
    resumed: AtomicU64,
    heartbeats: AtomicU64,
    seq: AtomicU64,
    next_id: AtomicU64,
    active_ws: Mutex<Option<Arc<dyn WsCallbacks>>>,
    interaction_by_token: Mutex<HashMap<String, (String, u64, String)>>,
}

impl FakeDiscordServer {
    fn new(heartbeat_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            token: fake_discord_token(),
            heartbeat_ms,
            sent: Mutex::new(Vec::new()),
            edits: Mutex::new(Vec::new()),
            commands: Mutex::new(Vec::new()),
            typing: Mutex::new(Vec::new()),
            callbacks: Mutex::new(Vec::new()),
            responses: Mutex::new(Vec::new()),
            intents: Mutex::new(Vec::new()),
            identified: AtomicU64::new(0),
            resumed: AtomicU64::new(0),
            heartbeats: AtomicU64::new(0),
            seq: AtomicU64::new(1),
            next_id: AtomicU64::new(1),
            active_ws: Mutex::new(None),
            interaction_by_token: Mutex::new(HashMap::new()),
        })
    }

    fn next_snowflake(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::SeqCst);
        (((now_ms() - 1_420_070_400_000) << 22) + n).to_string()
    }

    fn dispatch(&self, event_type: &str, data: Value) {
        let s = self.seq.fetch_add(1, Ordering::SeqCst);
        let ws = self.active_ws.lock().unwrap().clone().expect("active gateway");
        ws.on_message(json!({ "op": 0, "s": s, "t": event_type, "d": data }).to_string());
    }

    fn say(&self, content: &str, from_id: &str, channel_id: &str, guild_id: Option<&str>, reply_to: Option<&str>) -> String {
        let id = self.next_snowflake();
        let (username, global_name) = if from_id == STRANGER_ID { ("mallory", "Mallory") } else { ("sam_here", "Sam") };
        let mut d = json!({
            "id": id,
            "channel_id": channel_id,
            "author": { "id": from_id, "username": username, "global_name": global_name },
            "content": content,
            "type": if reply_to.is_some() { 19 } else { 0 },
        });
        if let Some(g) = guild_id {
            d["guild_id"] = json!(g);
        }
        if let Some(r) = reply_to {
            d["message_reference"] = json!({ "message_id": r });
        }
        self.dispatch("MESSAGE_CREATE", d);
        id
    }

    fn command(&self, name: &str, message: Option<&str>, from_id: &str, channel_id: &str) -> String {
        let id = self.next_snowflake();
        let tok = format!("tok_{id}");
        let resp_msg_id = self.next_snowflake();
        self.interaction_by_token.lock().unwrap().insert(tok.clone(), (id.clone(), 2, resp_msg_id));
        let (username, global_name) = if from_id == STRANGER_ID { ("mallory", "Mallory") } else { ("sam_here", "Sam") };
        let mut data = json!({ "name": name });
        if let Some(m) = message {
            data["options"] = json!([{ "name": "message", "type": 3, "value": m }]);
        }
        self.dispatch("INTERACTION_CREATE", json!({
            "id": id,
            "token": tok,
            "application_id": APP_ID,
            "type": 2,
            "channel_id": channel_id,
            "context": 1,
            "user": { "id": from_id, "username": username, "global_name": global_name },
            "data": data,
        }));
        id
    }

    fn press(&self, message_id: &str, custom_id: &str, from_id: &str, channel_id: &str) {
        let id = self.next_snowflake();
        let tok = format!("tok_{id}");
        self.interaction_by_token.lock().unwrap().insert(tok.clone(), (id.clone(), 3, message_id.to_owned()));
        let (username, global_name) = if from_id == STRANGER_ID { ("mallory", "Mallory") } else { ("sam_here", "Sam") };
        self.dispatch("INTERACTION_CREATE", json!({
            "id": id,
            "token": tok,
            "application_id": APP_ID,
            "type": 3,
            "channel_id": channel_id,
            "context": 1,
            "user": { "id": from_id, "username": username, "global_name": global_name },
            "message": { "id": message_id },
            "data": { "custom_id": custom_id },
        }));
    }

    fn drop_ws(&self, code: u16) {
        if let Some(ws) = self.active_ws.lock().unwrap().clone() {
            ws.on_close(code);
        }
    }

    fn ask_reconnect(&self) {
        if let Some(ws) = self.active_ws.lock().unwrap().clone() {
            ws.on_message(json!({ "op": 7, "d": null }).to_string());
        }
    }

    fn read_channel(&self, channel_id: &str) -> Vec<String> {
        self.sent.lock().unwrap().iter().filter(|m| m.channel == channel_id).map(|m| m.content.clone()).collect()
    }
}

impl HttpClient for FakeDiscordServer {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, String> {
        let auth_ok = req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .map(|(_, v)| v == &format!("Bot {}", self.token));
        if let Some(false) = auth_ok {
            return Ok(HttpResponse { status: 401, headers: vec![], body: json!({"message": "401: Unauthorized"}).to_string().into_bytes() });
        }
        let path = req.url.strip_prefix("http://127.0.0.1:1/api/v10").unwrap_or(&req.url);
        let body: Value = req.body.as_deref().and_then(|b| serde_json::from_slice(b).ok()).unwrap_or(Value::Null);

        if path == "/users/@me" {
            return Ok(HttpResponse {
                status: 200,
                headers: vec![],
                body: json!({"id": APP_ID, "username": "Locust test", "discriminator": "4242", "bot": true}).to_string().into_bytes(),
            });
        }
        if path == "/applications/@me" {
            return Ok(HttpResponse {
                status: 200,
                headers: vec![],
                body: json!({"id": APP_ID, "name": "Locust test"}).to_string().into_bytes(),
            });
        }
        if path == "/gateway/bot" {
            return Ok(HttpResponse {
                status: 200,
                headers: vec![],
                body: json!({"url": "ws://127.0.0.1:9999", "session_start_limit": {"remaining": 1000}}).to_string().into_bytes(),
            });
        }
        if path.ends_with("/commands") && req.method == "PUT" {
            let cmds = body.as_array().cloned().unwrap_or_default();
            self.commands.lock().unwrap().push(cmds);
            return Ok(HttpResponse { status: 200, headers: vec![], body: b"[]".to_vec() });
        }
        if path.ends_with("/typing") && req.method == "POST" {
            self.typing.lock().unwrap().push(path.to_owned());
            return Ok(HttpResponse { status: 204, headers: vec![], body: vec![] });
        }
        if path.starts_with("/interactions/") && path.contains("/callback") {
            let parts: Vec<&str> = path.split('/').collect();
            let int_id = parts.get(2).copied().unwrap_or("").to_owned();
            let int_tok = parts.get(3).copied().unwrap_or("").to_owned();
            let (kind, resp_id) = self
                .interaction_by_token
                .lock()
                .unwrap()
                .get(&int_tok)
                .map(|(_, k, r)| (*k, r.clone()))
                .unwrap_or((2, self.next_snowflake()));
            self.callbacks.lock().unwrap().push(DiscordCallback {
                id: int_id,
                kind,
                cb_type: body["type"].as_u64().unwrap_or(0),
            });
            return Ok(HttpResponse {
                status: 200,
                headers: vec![],
                body: json!({"interaction": {"response_message_id": resp_id}}).to_string().into_bytes(),
            });
        }
        if path.starts_with("/webhooks/") && path.ends_with("/messages/@original") {
            let parts: Vec<&str> = path.split('/').collect();
            let int_tok = parts.get(3).copied().unwrap_or("");
            let (int_id, resp_id) = self
                .interaction_by_token
                .lock()
                .unwrap()
                .get(int_tok)
                .map(|(i, _, r)| (i.clone(), r.clone()))
                .unwrap_or_default();
            if req.method == "PATCH" {
                let content = body["content"].as_str().unwrap_or("").to_owned();
                self.responses.lock().unwrap().push(DiscordResponse { interaction: int_id, id: resp_id.clone(), content });
            }
            return Ok(HttpResponse {
                status: 200,
                headers: vec![],
                body: json!({"id": resp_id}).to_string().into_bytes(),
            });
        }
        if path.starts_with("/channels/") && path.ends_with("/messages") && req.method == "POST" {
            let channel = path.split('/').nth(2).unwrap_or("").to_owned();
            let id = self.next_snowflake();
            let msg = SentDiscordMsg {
                id: id.clone(),
                channel,
                content: body["content"].as_str().unwrap_or("").to_owned(),
                reply_to: body["message_reference"]["message_id"].as_str().map(str::to_owned),
                components: body["components"].as_array().cloned().unwrap_or_default(),
                allowed_mentions: body["allowed_mentions"].clone(),
            };
            self.sent.lock().unwrap().push(msg);
            return Ok(HttpResponse { status: 200, headers: vec![], body: json!({"id": id}).to_string().into_bytes() });
        }
        if path.starts_with("/channels/") && path.contains("/messages/") && req.method == "PATCH" {
            let id = path.rsplit('/').next().unwrap_or("").to_owned();
            self.edits.lock().unwrap().push(EditedDiscordMsg {
                id: id.clone(),
                content: body["content"].as_str().unwrap_or("").to_owned(),
                components: body["components"].as_array().cloned().unwrap_or_default(),
            });
            return Ok(HttpResponse { status: 200, headers: vec![], body: json!({"id": id}).to_string().into_bytes() });
        }
        Ok(HttpResponse { status: 200, headers: vec![], body: b"{}".to_vec() })
    }
}

struct FakeDiscordWsConn {
    server: Arc<FakeDiscordServer>,
    callbacks: Arc<dyn WsCallbacks>,
}

impl WsConnection for FakeDiscordWsConn {
    fn send_text(&self, text: &str) -> Result<(), String> {
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return Ok(());
        };
        let cb = self.callbacks.clone();
        match v["op"].as_u64() {
            Some(2) => {
                if let Some(i) = v["d"]["intents"].as_u64() {
                    self.server.intents.lock().unwrap().push(i);
                }
                self.server.identified.fetch_add(1, Ordering::SeqCst);
                let s = self.server.seq.fetch_add(1, Ordering::SeqCst);
                std::thread::spawn(move || {
                    cb.on_message(json!({"op": 0, "s": s, "t": "READY", "d": {"session_id": "sess_1", "resume_gateway_url": "ws://127.0.0.1:9999"}}).to_string());
                });
            }
            Some(6) => {
                self.server.resumed.fetch_add(1, Ordering::SeqCst);
                let s = self.server.seq.fetch_add(1, Ordering::SeqCst);
                std::thread::spawn(move || {
                    cb.on_message(json!({"op": 0, "s": s, "t": "RESUMED", "d": {}}).to_string());
                });
            }
            Some(1) => {
                self.server.heartbeats.fetch_add(1, Ordering::SeqCst);
                std::thread::spawn(move || {
                    cb.on_message(json!({"op": 11}).to_string());
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn close(&self, code: u16, _reason: &str) {
        let cb = self.callbacks.clone();
        std::thread::spawn(move || {
            cb.on_close(code);
        });
    }
}

impl WsClient for FakeDiscordServer {
    fn connect(&self, _url: &str, callbacks: Arc<dyn WsCallbacks>) -> Result<Arc<dyn WsConnection>, String> {
        *self.active_ws.lock().unwrap() = Some(callbacks.clone());
        let hb = self.heartbeat_ms;
        let cb_hello = callbacks.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            cb_hello.on_message(json!({"op": 10, "d": {"heartbeat_interval": hb}}).to_string());
        });
        // We need an Arc<FakeDiscordServer> to hold inside FakeDiscordWsConn; use a small proxy!
        Err("use FakeDiscordWsClient".into())
    }
}

struct FakeDiscordWsClient(Arc<FakeDiscordServer>);
impl WsClient for FakeDiscordWsClient {
    fn connect(&self, _url: &str, callbacks: Arc<dyn WsCallbacks>) -> Result<Arc<dyn WsConnection>, String> {
        *self.0.active_ws.lock().unwrap() = Some(callbacks.clone());
        let hb = self.0.heartbeat_ms;
        let cb_hello = callbacks.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            cb_hello.on_message(json!({"op": 10, "d": {"heartbeat_interval": hb}}).to_string());
        });
        Ok(Arc::new(FakeDiscordWsConn { server: self.0.clone(), callbacks }))
    }
}

fn wait_until(mut cond: impl FnMut() -> bool, what: &str) {
    for _ in 0..300 {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

struct Harness {
    dir: PathBuf,
    dc: Arc<FakeDiscordServer>,
    link: Arc<ChatLink>,
    started: Arc<Mutex<Vec<(String, String, Option<String>)>>>,
    decided: Arc<Mutex<Vec<Answer>>>,
    missions: Arc<Mutex<HashMap<String, ChatMission>>>,
    live: Arc<Mutex<HashSet<String>>>,
    waiting: Arc<Mutex<Vec<(Request, Option<String>)>>>,
    refusal: Arc<Mutex<Option<String>>>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.link.dispose();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Harness {
    fn new(team: Vec<ChatTeammate>, heartbeat_ms: u64) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let dir = std::env::temp_dir().join(format!("locust-dc-p-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(&dir).unwrap();
        let dc = FakeDiscordServer::new(heartbeat_ms);
        let started = Arc::new(Mutex::new(Vec::new()));
        let decided = Arc::new(Mutex::new(Vec::new()));
        let missions = Arc::new(Mutex::new(HashMap::new()));
        let live = Arc::new(Mutex::new(HashSet::new()));
        let waiting: Arc<Mutex<Vec<(Request, Option<String>)>>> = Arc::new(Mutex::new(Vec::new()));
        let refusal: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let next = Arc::new(AtomicU64::new(0));
        let link_slot: Arc<Mutex<Option<Arc<ChatLink>>>> = Arc::new(Mutex::new(None));

        let link = create_discord_link(
            DiscordOptions {
                http: dc.clone(),
                ws: Arc::new(FakeDiscordWsClient(dc.clone())),
                api: Some(Arc::new(|| "http://127.0.0.1:1".into())),
                version: None,
            },
            ChatLinkOptions {
                directory: dir.clone(),
                secrets: Arc::new(ElectronSecretBox),
                refusal: {
                    let r = refusal.clone();
                    Arc::new(move || r.lock().unwrap().clone())
                },
                computer: Arc::new(|| "DESK-PC".into()),
                team: Arc::new(move || team.clone()),
                start: {
                    let st = started.clone();
                    let lv = live.clone();
                    let nx = next.clone();
                    Arc::new(move |tid, text, follow| {
                        st.lock().unwrap().push((tid.to_owned(), text.to_owned(), follow.map(str::to_owned)));
                        let id = format!("mission_{}", nx.fetch_add(1, Ordering::SeqCst) + 1);
                        lv.lock().unwrap().insert(id.clone());
                        ChatStart::Ok { mission_id: id }
                    })
                },
                read: {
                    let ms = missions.clone();
                    Arc::new(move |id| ms.lock().unwrap().get(id).cloned())
                },
                live: {
                    let lv = live.clone();
                    Arc::new(move |id| lv.lock().unwrap().contains(id))
                },
                carried: Arc::new(|_| false),
                waiting: {
                    let w = waiting.clone();
                    Arc::new(move || w.lock().unwrap().clone())
                },
                decide: {
                    let dec = decided.clone();
                    let w = waiting.clone();
                    let slot = link_slot.clone();
                    Arc::new(move |answer| {
                        dec.lock().unwrap().push(answer.clone());
                        let (aid, how) = match &answer {
                            Answer::Decision { approval_id, decision, .. } => (
                                approval_id.clone(),
                                if *decision == Decision::Deny {
                                    crate::chat_link::SettledHow::Denied
                                } else {
                                    crate::chat_link::SettledHow::Allowed
                                },
                            ),
                            Answer::Answers { approval_id, .. } => (approval_id.clone(), crate::chat_link::SettledHow::Answered),
                        };
                        let mut wl = w.lock().unwrap();
                        let had = wl.iter().any(|(r, _)| r.approval_id == aid);
                        wl.retain(|(r, _)| r.approval_id != aid);
                        drop(wl);
                        if had {
                            if let Some(l) = slot.lock().unwrap().as_ref() {
                                l.card_settled(&aid, how);
                            }
                        }
                        had
                    })
                },
                changed: None,
                note: None,
                now: None,
                typing_ms: Some(30),
            },
        );
        *link_slot.lock().unwrap() = Some(link.clone());
        Self { dir, dc, link, started, decided, missions, live, waiting, refusal }
    }

    fn paired(team: Vec<ChatTeammate>, heartbeat_ms: u64) -> Self {
        let h = Self::new(team, heartbeat_ms);
        let s = h.link.set_token(TokenInput::Single(h.dc.token.clone()));
        wait_until(|| h.dc.identified.load(Ordering::SeqCst) > 0 && h.link.settings().connected, "gateway ready");
        let code = s.code.unwrap().code;
        let spaced = format!("{} {}", &code[..4], &code[4..]);
        h.dc.say(&spaced, PERSON_ID, PERSON_CHANNEL, None, None);
        wait_until(|| h.dc.read_channel(PERSON_CHANNEL).iter().any(|t| t.starts_with("Paired with Locust on **DESK-PC**")), "pairing message");
        h
    }

    fn end(&self, mission_id: &str, answer: &str, phase: &str) {
        self.live.lock().unwrap().remove(mission_id);
        self.missions.lock().unwrap().insert(
            mission_id.to_owned(),
            ChatMission {
                mission_id: mission_id.to_owned(),
                phase: phase.to_owned(),
                events: vec![json!({"type": "message.delta", "payload": {"itemId": "m1", "operation": "append", "text": answer, "final": true}})],
                started_by_kind: None,
                continues_from_mission_id: None,
                host_failure_message: None,
            },
        );
        self.link.on_run_ended(mission_id);
    }
}

fn make_card(approval_id: &str, mission_id: &str, kind: Kind, summary: &str, detail: &str) -> Request {
    Request {
        approval_id: approval_id.into(),
        mission_id: mission_id.into(),
        runtime: "claude".into(),
        kind,
        summary: summary.into(),
        detail: detail.into(),
        cwd: Some("C:/work/tides".into()),
        requested_at: "2026-11-17T00:00:00Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false,
        answer_in: None,
        claude_input: None,
        questions: vec![],
    }
}

// case: checks a token with Discord, keeps it encrypted, and reads only direct messages
#[test]
fn checks_a_token_with_discord_keeps_it_encrypted_and_reads_only_direct_messages() {
    let h = Harness::new(vec![ada(), wren()], 30_000);
    assert!(h.link.set_token(TokenInput::Single("not a token".into())).message.unwrap().contains("not a bot\u{2019}s token"));
    let wrong_tok = ["MTEwMDAwMDAwMDAwMDAwMDAwMQ", "Gwrong", "wrong_wrong_wrong_wrong_wrong_wrong_wro"].join(".");
    assert!(h.link.set_token(TokenInput::Single(wrong_tok)).message.unwrap().contains("Discord does not know that token"));
    let kept = h.link.set_token(TokenInput::Single(format!("  Bot {} ", h.dc.token)));
    let bot = kept.bot.unwrap();
    assert_eq!(bot.username, "Locust test#4242");
    assert_eq!(bot.name, "Locust test");
    assert_eq!(bot.invite.as_deref(), Some("https://discord.com/oauth2/authorize?client_id=1100000000000000001&scope=bot%20applications.commands&permissions=0"));
    assert_eq!(kept.code.unwrap().code.len(), 8);
    assert!(kept.link.is_none());
    let file = std::fs::read_to_string(h.dir.join(DISCORD_FILE)).unwrap();
    assert!(!file.contains(&h.dc.token));
    assert!(!file.contains(h.dc.token.split('.').nth(2).unwrap()));
    wait_until(|| h.link.settings().connected, "connected");
    assert_eq!(*h.dc.intents.lock().unwrap(), vec![1 << 12]);
}

// case: pairs the one account that sends the code, once, and ignores everyone else
#[test]
fn pairs_the_one_account_that_sends_the_code_once_and_ignores_everyone_else() {
    let h = Harness::paired(vec![ada(), wren()], 30_000);
    let chat = h.link.settings().chat.unwrap();
    assert_eq!(chat.name, "Sam");
    assert_eq!(chat.username.as_deref(), Some("sam_here"));
    assert!(h.link.settings().code.is_none());
    wait_until(|| !h.dc.commands.lock().unwrap().is_empty(), "commands");
    let cmds = h.dc.commands.lock().unwrap().last().unwrap().clone();
    let names: Vec<&str> = cmds.iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["ada", "wren", "who", "new", "waiting", "help"]);
    assert!(cmds.iter().all(|c| c["contexts"] == json!([1])));
    assert_eq!(cmds[0]["options"][0]["name"], json!("message"));
    assert!(cmds.iter().find(|c| c["name"] == "who").unwrap().get("options").is_none());

    h.dc.say("1234 5678", STRANGER_ID, STRANGER_CHANNEL, None, None);
    h.dc.say("/ada delete everything", STRANGER_ID, STRANGER_CHANNEL, None, None);
    h.dc.say("Ada: and this", STRANGER_ID, PERSON_CHANNEL, None, None);
    wait_until(|| h.dc.read_channel(STRANGER_CHANNEL).len() == 1, "wrong code said");
    assert!(h.dc.read_channel(STRANGER_CHANNEL)[0].contains("That code does not pair this account"));
    h.dc.say("/ada in a server", PERSON_ID, "1500000000000000001", Some("1400000000000000001"), None);
    h.dc.say("Ada: from a server", PERSON_ID, PERSON_CHANNEL, Some("1400000000000000001"), None);
    std::thread::sleep(Duration::from_millis(50));
    assert!(h.started.lock().unwrap().is_empty());
    assert_eq!(h.link.settings().chat.unwrap().name, "Sam");
}

// case: takes /ada from Discord’s list to Ada, answers as a reply, and carries the conversation on
#[test]
fn takes_ada_from_discords_list_to_ada_answers_as_a_reply_and_carries_the_conversation_on() {
    let h = Harness::paired(vec![ada(), wren()], 30_000);
    let asked_id = h.dc.command("ada", Some("what changed in tides today?"), PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.started.lock().unwrap().len() == 1, "first turn");
    assert_eq!(h.started.lock().unwrap()[0], ("mate_ada".into(), "what changed in tides today?".into(), None));
    assert!(h.dc.callbacks.lock().unwrap().iter().any(|c| c.id == asked_id && c.kind == 2 && c.cb_type == 5));
    wait_until(|| h.dc.responses.lock().unwrap().iter().any(|r| r.interaction == asked_id), "command echo");
    let echo = h.dc.responses.lock().unwrap().iter().find(|r| r.interaction == asked_id).unwrap().clone();
    assert_eq!(echo.content, "**To Ada**: what changed in tides today?");
    wait_until(|| !h.dc.typing.lock().unwrap().is_empty(), "typing");

    h.end("mission_1", "**Three** commits:\n\n- `fix: tides`\n- docs\n\n```ts\nconst a = 1 < 2\n```", "completed");
    let answer = h.dc.sent.lock().unwrap().last().unwrap().clone();
    assert_eq!(answer.reply_to.as_deref(), Some(echo.id.as_str()));
    assert!(answer.content.starts_with("**Ada**\n\n**Three** commits:"));
    assert!(answer.content.contains("```ts\nconst a = 1 < 2\n```"));
    assert_eq!(answer.allowed_mentions, json!({ "parse": [] }));

    h.dc.say("and yesterday?", PERSON_ID, PERSON_CHANNEL, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 2, "follow-up");
    assert_eq!(h.started.lock().unwrap()[1], ("mate_ada".into(), "and yesterday?".into(), Some("mission_1".into())));
    h.end("mission_2", "Two more.", "completed");

    h.dc.say("more on this", PERSON_ID, PERSON_CHANNEL, None, Some(&answer.id));
    wait_until(|| h.started.lock().unwrap().len() == 3, "reply turn");
    assert_eq!(h.started.lock().unwrap()[2], ("mate_ada".into(), "more on this".into(), Some("mission_1".into())));

    h.dc.say("Wren: run the tests", PERSON_ID, PERSON_CHANNEL, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 4, "Wren turn");
    assert_eq!(h.started.lock().unwrap()[3], ("mate_wren".into(), "run the tests".into(), None));

    let fresh_id = h.dc.command("new", None, PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.dc.responses.lock().unwrap().iter().any(|r| r.interaction == fresh_id), "/new response");
    assert_eq!(h.dc.responses.lock().unwrap().last().unwrap().content, "Your next message starts a new conversation with Wren.");
}

// case: makes a new code once too many wrong ones were sent to the bot, and the old one pairs nothing
#[test]
fn makes_a_new_code_once_too_many_wrong_ones_were_sent_to_the_bot_and_the_old_one_pairs_nothing() {
    let h = Harness::new(vec![ada(), wren()], 30_000);
    let s = h.link.set_token(TokenInput::Single(h.dc.token.clone()));
    wait_until(|| h.dc.identified.load(Ordering::SeqCst) > 0, "identified");
    let first = s.code.unwrap().code;
    let wrong = if first == "00000000" { "11111111" } else { "00000000" };
    for _ in 0..10 {
        h.dc.say(wrong, STRANGER_ID, STRANGER_CHANNEL, None, None);
    }
    wait_until(|| h.link.settings().code.as_ref().map(|c| c.code.as_str()) != Some(first.as_str()), "new code");
    assert!(h.link.settings().message.unwrap().contains("Too many wrong codes were sent to the bot"));
    assert_eq!(h.dc.read_channel(STRANGER_CHANNEL).len(), 1);
    h.dc.say(&first, PERSON_ID, PERSON_CHANNEL, None, None);
    std::thread::sleep(Duration::from_millis(50));
    assert!(h.link.settings().chat.is_none());
}

// case: answers someone else’s command with nothing of the person’s
#[test]
fn answers_someone_elses_command_with_nothing_of_the_persons() {
    let h = Harness::paired(vec![ada(), wren()], 30_000);
    let theirs = h.dc.command("ada", Some("show me the files"), STRANGER_ID, STRANGER_CHANNEL);
    wait_until(|| h.dc.responses.lock().unwrap().iter().any(|r| r.interaction == theirs), "stranger answered");
    let ans = h.dc.responses.lock().unwrap().iter().find(|r| r.interaction == theirs).unwrap().clone();
    assert_eq!(ans.content, "Locust answers only the person it is paired with.");
    assert!(h.started.lock().unwrap().is_empty());
}

// case: asks who a message is for with buttons, when nobody is being written to
#[test]
fn asks_who_a_message_is_for_with_buttons_when_nobody_is_being_written_to() {
    let h = Harness::paired(vec![ada(), wren()], 30_000);
    h.dc.say("can someone look at the build?", PERSON_ID, PERSON_CHANNEL, None, None);
    wait_until(|| h.dc.sent.lock().unwrap().iter().any(|m| m.content == "Who is this for?"), "who question");
    let asked = h.dc.sent.lock().unwrap().iter().find(|m| m.content == "Who is this for?").unwrap().clone();
    let btns = asked.components[0]["components"].as_array().unwrap();
    let labels: Vec<&str> = btns.iter().map(|b| b["label"].as_str().unwrap()).collect();
    assert_eq!(labels, vec!["Ada", "Wren"]);
    let wren_cid = btns[1]["custom_id"].as_str().unwrap();
    h.dc.press(&asked.id, wren_cid, PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.started.lock().unwrap().len() == 1, "held message sent");
    assert_eq!(h.started.lock().unwrap()[0], ("mate_wren".into(), "can someone look at the build?".into(), None));
    assert!(h.dc.callbacks.lock().unwrap().last().is_some_and(|c| c.kind == 3 && c.cb_type == 6));
    let edit = h.dc.edits.lock().unwrap().last().unwrap().clone();
    assert_eq!(edit.id, asked.id);
    assert_eq!(edit.content, "To **Wren**.");
    assert!(edit.components.is_empty());
}

// case: brings a card from its own turn with Allow once and Deny, and answers it by the window’s path
#[test]
fn brings_a_card_from_its_own_turn_with_allow_once_and_deny_and_answers_it_by_the_windows_path() {
    let h = Harness::paired(vec![ada(), wren()], 30_000);
    h.dc.command("wren", Some("run the tests"), PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    let fake_tok = ["MTEwMDAwMDAwMDAwMDAwMDAwMQ", "GabcDE", "abcdefghijklmnopqrstuvwxyz0123456789ab"].join(".");
    let req = make_card("appr_1", "mission_1", Kind::Command, &format!("Run npm test --token={fake_tok} in tides"), "npm test");
    h.waiting.lock().unwrap().push((req.clone(), Some("mate_wren".into())));
    h.link.card_raised(&req, Some("Wren"));
    wait_until(|| h.dc.sent.lock().unwrap().iter().any(|m| m.content.starts_with("**Wren needs your approval**")), "card");
    let posted = h.dc.sent.lock().unwrap().iter().find(|m| m.content.starts_with("**Wren needs your approval**")).unwrap().clone();
    let btns = posted.components[0]["components"].as_array().unwrap();
    let pairs: Vec<(&str, u64)> = btns.iter().map(|b| (b["label"].as_str().unwrap(), b["style"].as_u64().unwrap())).collect();
    assert_eq!(pairs, vec![("Allow once", 3), ("Deny", 4)]);
    assert!(!posted.content.contains("MTEwMDAw"));
    assert!(shown(&posted.content).contains("Run npm test --token=[secret-shaped text removed] in tides"));

    h.link.card_raised(&make_card("appr_window", "mission_window", Kind::Command, "Run npm test", "npm test"), Some("Ada"));
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(h.dc.sent.lock().unwrap().iter().filter(|m| m.content.contains("needs your approval")).count(), 1);

    let allow = btns[0]["custom_id"].as_str().unwrap();
    let deny = btns[1]["custom_id"].as_str().unwrap();
    h.dc.press(&posted.id, allow, STRANGER_ID, STRANGER_CHANNEL);
    wait_until(|| h.dc.callbacks.lock().unwrap().iter().filter(|c| c.kind == 3).count() == 1, "stranger press");
    assert!(h.decided.lock().unwrap().is_empty());

    h.dc.press(&posted.id, allow, PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.decided.lock().unwrap().len() == 1, "answer");
    assert!(matches!(
        &h.decided.lock().unwrap()[0],
        Answer::Decision { approval_id, decision: Decision::ApproveOnce, .. } if approval_id == "appr_1"
    ));
    wait_until(|| h.dc.edits.lock().unwrap().iter().any(|e| e.id == posted.id), "card settled");
    let edited = h.dc.edits.lock().unwrap().iter().find(|e| e.id == posted.id).unwrap().clone();
    assert!(edited.content.contains("**Allowed once.**"));
    assert!(edited.components.is_empty());

    h.dc.press(&posted.id, deny, PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.dc.callbacks.lock().unwrap().iter().filter(|c| c.kind == 3).count() == 3, "second press");
    assert_eq!(h.decided.lock().unwrap().len(), 1);
}

// case: says which file or command a card allows, and posts a card once when its turn and the alert rules both bring it (0.837)
#[test]
fn says_which_file_or_command_a_card_allows_and_posts_a_card_once_when_its_turn_and_the_alert_rules_both_bring_it() {
    let h = Harness::paired(vec![ada(), wren()], 30_000);
    h.dc.command("wren", Some("make the notes"), PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    h.link.set_alerts(true);
    let req = make_card("appr_file", "mission_1", Kind::FileChange, "Change 1 file", "notes/plan.md");
    h.waiting.lock().unwrap().push((req.clone(), Some("mate_wren".into())));
    h.link.card_raised(&req, Some("Wren"));
    h.link.alert(ChatAlert { cards: vec![(req, Some("Wren".into()))], lines: vec![] });
    wait_until(|| h.dc.sent.lock().unwrap().iter().any(|m| m.content.starts_with("**Wren needs your approval**")), "card");
    let posted = h.dc.sent.lock().unwrap().iter().find(|m| m.content.starts_with("**Wren needs your approval**")).unwrap().clone();
    assert!(shown(&posted.content).contains("Change 1 file\nnotes/plan.md"));
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(h.dc.sent.lock().unwrap().iter().filter(|m| m.content.starts_with("**Wren needs your approval**")).count(), 1);
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(h.dir.join(DISCORD_FILE)).unwrap()).unwrap();
    let matching = saved["cards"].as_array().unwrap().iter().filter(|c| c["approvalId"] == "appr_file").count();
    assert_eq!(matching, 1);
}

// case: tells the person here what needs them, from any run, only once they turn it on
#[test]
fn tells_the_person_here_what_needs_them_from_any_run_only_once_they_turn_it_on() {
    let h = Harness::paired(vec![ada(), wren()], 30_000);
    assert!(h.link.settings().alerts.is_none());
    let req = make_card("appr_away", "mission_window", Kind::Command, "Run npm test in tides", "npm test");
    h.waiting.lock().unwrap().push((req.clone(), None));
    let alert = ChatAlert {
        cards: vec![(req.clone(), Some("Ada".into()))],
        lines: vec![("Scout".into(), "A check failed on a branch.".into())],
    };
    h.link.alert(alert.clone());
    std::thread::sleep(Duration::from_millis(50));
    assert!(!h.dc.sent.lock().unwrap().iter().any(|m| m.content.contains("Scout") || m.content.contains("needs your approval")));

    assert_eq!(h.link.set_alerts(true).alerts, Some(true));
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(h.dir.join(DISCORD_FILE)).unwrap()).unwrap();
    assert_eq!(saved["alerts"], json!(true));
    h.link.alert(alert);
    wait_until(|| h.dc.sent.lock().unwrap().iter().any(|m| m.content.starts_with("**Ada needs your approval**")), "alert card");
    wait_until(|| h.dc.sent.lock().unwrap().iter().any(|m| m.content == "**Scout**\nA check failed on a branch."), "Scout line");
    let posted = h.dc.sent.lock().unwrap().iter().find(|m| m.content.starts_with("**Ada needs your approval**")).unwrap().clone();
    // Same card told again is not sent twice.
    h.link.alert(ChatAlert { cards: vec![(req, Some("Ada".into()))], lines: vec![] });
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(h.dc.sent.lock().unwrap().iter().filter(|m| m.content.contains("needs your approval")).count(), 1);
    let allow = posted.components[0]["components"][0]["custom_id"].as_str().unwrap();
    h.dc.press(&posted.id, allow, PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.decided.lock().unwrap().len() == 1, "decided");

    assert!(h.link.set_alerts(false).alerts.is_none());
    let next = make_card("appr_off", "mission_window_2", Kind::Command, "Run npm test", "npm test");
    h.waiting.lock().unwrap().push((next.clone(), None));
    h.link.alert(ChatAlert { cards: vec![(next, Some("Ada".into()))], lines: vec![("Scout".into(), "There is a new issue.".into())] });
    std::thread::sleep(Duration::from_millis(50));
    assert!(!h.dc.sent.lock().unwrap().iter().any(|m| m.content.contains("There is a new issue.")));
}

// case: takes anything shaped like a key out of an answer before Discord has it
#[test]
fn takes_anything_shaped_like_a_key_out_of_an_answer_before_discord_has_it() {
    let h = Harness::paired(vec![ada(), wren()], 30_000);
    h.dc.command("ada", Some("the token?"), PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    let fake_dc = ["MTEwMDAwMDAwMDAwMDAwMDAwMQ", "GabcDE", "abcdefghijklmnopqrstuvwxyz0123456789ab"].join(".");
    let fake_gh = ["ghp", "abcdefghijklmnopqrstuvwxyz0123456789"].join("_");
    h.end("mission_1", &format!("The bot token is {fake_dc} and the GitHub one is {fake_gh}."), "completed");
    let sent = h.dc.sent.lock().unwrap().last().unwrap().content.clone();
    assert!(!sent.contains(&fake_gh));
    assert!(!sent.contains("MTEwMDAw"));
    assert!(sent.contains("[secret-shaped text removed]"));
}

// case: splits a long answer into messages Discord takes, and a table into a block of code
#[test]
fn splits_a_long_answer_into_messages_discord_takes_and_a_table_into_a_block_of_code() {
    let h = Harness::paired(vec![ada(), wren()], 30_000);
    h.dc.command("ada", Some("show me"), PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    let before = h.dc.sent.lock().unwrap().len();
    let long = format!("{}\n\n| a | b |\n|---|---|\n| 1 | 2 |", "A long line of findings. ".repeat(200));
    h.end("mission_1", &long, "completed");
    let parts = h.dc.sent.lock().unwrap()[before..].to_vec();
    assert!(parts.len() > 1);
    assert!(parts[0].content.starts_with("**Ada**"));
    assert!(parts.last().unwrap().content.contains("```\n| a | b |\n| 1 | 2 |\n```"));
    for part in &parts {
        assert!(part.content.chars().count() <= 2000);
    }
    assert!(parts[1..].iter().all(|p| p.reply_to.is_none()));
}

// case: beats as Discord asks, and resumes the same session when the connection drops
#[test]
fn beats_as_discord_asks_and_resumes_the_same_session_when_the_connection_drops() {
    let h = Harness::paired(vec![ada(), wren()], 120);
    wait_until(|| h.dc.heartbeats.load(Ordering::SeqCst) >= 2, "heartbeats");
    h.dc.drop_ws(1006);
    wait_until(|| h.dc.resumed.load(Ordering::SeqCst) == 1, "first resume");
    assert_eq!(h.dc.identified.load(Ordering::SeqCst), 1);
    h.dc.say("Ada: still there?", PERSON_ID, PERSON_CHANNEL, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn after resume");
    h.dc.ask_reconnect();
    wait_until(|| h.dc.resumed.load(Ordering::SeqCst) == 2, "second resume");
    assert_eq!(h.dc.identified.load(Ordering::SeqCst), 1);
}

// case: stops when Discord refuses the token, and forgets the token and the commands on Remove
#[test]
fn stops_when_discord_refuses_the_token_and_forgets_the_token_and_the_commands_on_remove() {
    let h = Harness::paired(vec![ada(), wren()], 30_000);
    h.dc.drop_ws(4004);
    wait_until(|| h.link.settings().message.as_deref().unwrap_or("").contains("Discord no longer takes this bot\u{2019}s token"), "refused message");
    assert!(!h.link.settings().connected);
    let removed = h.link.remove();
    assert_eq!(removed, ChatAppSettings::default());
    assert!(!h.dir.join(DISCORD_FILE).exists());
    wait_until(|| h.dc.commands.lock().unwrap().last().is_some_and(|c| c.is_empty()), "commands cleared");
}

// case: stops at the organization’s word
#[test]
fn stops_at_the_organizations_word() {
    let h = Harness::paired(vec![ada(), wren()], 30_000);
    *h.refusal.lock().unwrap() = Some("Teammates in chat apps are off. Set by your organization.".into());
    h.dc.say("Ada: anything", PERSON_ID, PERSON_CHANNEL, None, None);
    wait_until(|| h.dc.read_channel(PERSON_CHANNEL).last().is_some_and(|t| t.contains("Teammates in chat apps are off")), "refusal");
    assert!(h.started.lock().unwrap().is_empty());
    assert!(h.link.set_token(TokenInput::Single(h.dc.token.clone())).message.unwrap().contains("Teammates in chat apps are off"));
}

/// Carry-over proof: a `discord.json` written by Electron (schemaVersion 1 + base64-sealed token + paired chat)
/// is read, decrypted, connected, and answers `Ada:` in Rust without re-pairing.
#[test]
fn electron_discord_pairing_and_encrypted_token_carry_over_to_rust() {
    let h = Harness::new(vec![ada(), wren()], 30_000);
    let sealed = ElectronSecretBox::seal_electron(&h.dc.token);
    let electron_file = json!({
        "schemaVersion": 1,
        "token": sealed,
        "bot": {
            "id": APP_ID,
            "username": "Locust test#4242",
            "name": "Locust test",
            "extra": { "application": APP_ID }
        },
        "chat": {
            "place": PERSON_CHANNEL,
            "user": PERSON_ID,
            "name": "Sam",
            "username": "sam_here",
            "pairedAt": "2026-11-01T10:00:00.000Z"
        },
        "alerts": true
    });
    std::fs::write(h.dir.join(DISCORD_FILE), electron_file.to_string()).unwrap();

    assert!(h.link.keeps_key());
    h.link.load();
    wait_until(|| h.link.settings().connected, "carried-over Discord connected");
    let settings = h.link.settings();
    assert_eq!(settings.bot.unwrap().username, "Locust test#4242");
    assert_eq!(settings.chat.unwrap().name, "Sam");
    assert_eq!(settings.alerts, Some(true));

    h.dc.say("Ada: carried over?", PERSON_ID, PERSON_CHANNEL, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn from carried-over chat");
    assert_eq!(h.started.lock().unwrap()[0], ("mate_ada".into(), "carried over?".into(), None));
    h.end("mission_1", "Yes, Discord carried over.", "completed");
    assert!(h.dc.read_channel(PERSON_CHANNEL).last().unwrap().contains("Yes, Discord carried over."));
}
