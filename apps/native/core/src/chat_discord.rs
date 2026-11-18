//! Discord Gateway + REST transport and ChatApp (Electron `main/discord.ts`, 0.832):
//! `DIRECT_MESSAGES` intent (`1 << 12`), heartbeats + resume, slash commands, button components, and 8-digit pairing code.

use crate::chat_apps::{ChatAppBot, ChatAppCode, ChatAppId, ChatAppLink, pairing_code_in};
use crate::chat_discord_fmt::{DISCORD_MESSAGE_MAX, discord_escape, discord_invite, discord_messages, is_discord_token};
use crate::chat_http::{HttpClient, HttpRequest, WsCallbacks, WsClient, WsConnection};
use crate::chat_link::{
    ButtonStyle, ChatApp, ChatBot, ChatButtons, ChatCommand, ChatEvents, ChatIncoming, ChatLink, ChatLinkOptions,
    ChatPress, ChatSendExtra, ChatTransport, ChatWords, ParsedCommand, TokenInput, create_chat_link,
};
use chrono::DateTime;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DISCORD_FILE: &str = "discord.json";
pub const DISCORD_API: &str = "https://discord.com";
const GATEWAY: &str = "wss://gateway.discord.gg";
const INTENTS: u64 = 1 << 12;
const TOKEN_REFUSED: &str = "Discord no longer takes this bot\u{2019}s token. On the bot\u{2019}s page in the Developer Portal, press Reset Token and paste the new one.";

#[derive(Clone)]
pub struct DiscordOptions {
    pub http: Arc<dyn HttpClient>,
    pub ws: Arc<dyn WsClient>,
    pub api: Option<Arc<dyn Fn() -> String + Send + Sync>>,
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DiscordError {
    pub status: u16,
    pub code: u64,
    pub detail: String,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn made_at(id: &str) -> u64 {
    id.parse::<u64>().map(|n| (n >> 22) + 1_420_070_400_000).unwrap_or_else(|_| now_ms())
}

fn random_8_digits() -> String {
    let mut b = [0u8; 4];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    let val = u32::from_le_bytes(b) % 100_000_000;
    format!("{val:08}")
}

fn discord_rest(
    opts: &DiscordOptions,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Option<Value>, DiscordError> {
    let base = opts.api.as_ref().map(|f| f()).unwrap_or_else(|| DISCORD_API.to_owned());
    let url = format!("{}/api/v10{path}", base.trim_end_matches('/'));
    let agent = format!("DiscordBot (https://locust.lol, {})", opts.version.as_deref().unwrap_or("1"));

    for attempt in 0..2 {
        let mut headers = vec![("user-agent".into(), agent.clone())];
        if let Some(tok) = token {
            headers.push(("authorization".into(), format!("Bot {tok}")));
        }
        let body_bytes = body.as_ref().map(|b| {
            headers.push(("content-type".into(), "application/json".into()));
            b.to_string().into_bytes()
        });
        let resp = opts
            .http
            .send(HttpRequest {
                method: method.into(),
                url: url.clone(),
                headers,
                body: body_bytes,
                timeout_secs: 15,
            })
            .map_err(|_| DiscordError { status: 0, code: 0, detail: "Discord could not be reached.".into() })?;

        if resp.status == 204 {
            return Ok(None);
        }
        let data: Option<Value> = serde_json::from_slice(&resp.body).ok();
        if (200..300).contains(&resp.status) {
            return Ok(data);
        }
        if resp.status == 429 && attempt == 0 {
            let secs = data.as_ref().and_then(|d| d["retry_after"].as_f64()).unwrap_or(2.0).min(30.0);
            std::thread::sleep(Duration::from_millis((secs * 1000.0) as u64));
            continue;
        }
        let code = data.as_ref().and_then(|d| d["code"].as_u64()).unwrap_or(0);
        let detail = data
            .as_ref()
            .and_then(|d| d["message"].as_str())
            .map(|s| s.chars().take(300).collect())
            .unwrap_or_else(|| format!("Discord answered {}.", resp.status));
        return Err(DiscordError { status: resp.status, code, detail });
    }
    Err(DiscordError { status: 429, code: 0, detail: "Discord rate limited.".into() })
}

pub fn components_of(buttons: &ChatButtons) -> Vec<Value> {
    let short = buttons.len() > 1 && buttons.iter().all(|row| row.len() == 1 && row[0].label.chars().count() <= 20);
    let rows: Vec<Vec<_>> = if !short && buttons.len() <= 5 && buttons.iter().all(|row| row.len() <= 5) {
        buttons.iter().map(|r| r.iter().collect()).collect()
    } else {
        let flat: Vec<_> = buttons.iter().flat_map(|r| r.iter()).collect();
        flat.chunks(5).map(|c| c.to_vec()).collect()
    };
    rows.into_iter()
        .take(5)
        .map(|row| {
            let comps: Vec<Value> = row
                .into_iter()
                .map(|b| {
                    let style = match b.style {
                        Some(ButtonStyle::Allow) => 3,
                        Some(ButtonStyle::Deny) => 4,
                        None => 2,
                    };
                    json!({
                        "type": 2,
                        "style": style,
                        "label": b.label.chars().take(80).collect::<String>(),
                        "custom_id": b.data.chars().take(100).collect::<String>(),
                    })
                })
                .collect();
            json!({ "type": 1, "components": comps })
        })
        .collect()
}

fn body_of(text: &str, buttons: Option<&ChatButtons>) -> Value {
    let mut out = json!({
        "content": text,
        "allowed_mentions": { "parse": [] },
        "flags": 1 << 2,
    });
    if let Some(btns) = buttons {
        if !btns.is_empty() {
            out["components"] = json!(components_of(btns));
        }
    }
    out
}

fn allowed_gateway_url(url: &str, local: bool) -> bool {
    if local {
        url.starts_with("ws://127.0.0.1:") || url.starts_with("ws://localhost:") || url.starts_with("wss://127.0.0.1:") || url.starts_with("wss://localhost:")
    } else {
        url.starts_with("wss://") && url.contains(".discord.gg")
    }
}

struct GatewayState {
    socket: Option<Arc<dyn WsConnection>>,
    conn_id: u64,
    gateway: Option<String>,
    session: Option<(String, String)>,
    seq: Option<i64>,
    acked: bool,
    ready_conn: Option<u64>,
    beat_stop: Option<Arc<AtomicBool>>,
    backoff_ms: u64,
    quiet_for_ms: Option<u64>,
}

struct DiscordTransport {
    token: String,
    options: DiscordOptions,
    events: Arc<dyn ChatEvents>,
    local: bool,
    stopped: AtomicBool,
    gw: Mutex<GatewayState>,
}

struct WsHandler {
    transport: Arc<DiscordTransport>,
    conn_id: u64,
}

impl WsCallbacks for WsHandler {
    fn on_message(&self, text: String) {
        self.transport.received(self.conn_id, &text);
    }
    fn on_close(&self, code: u16) {
        self.transport.closed(self.conn_id, code);
    }
}

impl DiscordTransport {
    fn start(token: String, options: DiscordOptions, events: Arc<dyn ChatEvents>) -> Arc<Self> {
        let api_url = options.api.as_ref().map(|f| f()).unwrap_or_default();
        let local = api_url.starts_with("http://127.0.0.1:") || api_url.starts_with("http://localhost:");
        let t = Arc::new(Self {
            token,
            options,
            events,
            local,
            stopped: AtomicBool::new(false),
            gw: Mutex::new(GatewayState {
                socket: None,
                conn_id: 0,
                gateway: None,
                session: None,
                seq: None,
                acked: true,
                ready_conn: None,
                beat_stop: None,
                backoff_ms: 1_000,
                quiet_for_ms: None,
            }),
        });
        let t2 = t.clone();
        std::thread::spawn(move || t2.open());
        t
    }

    fn stop_beats(gw: &mut GatewayState) {
        if let Some(flag) = gw.beat_stop.take() {
            flag.store(true, Ordering::SeqCst);
        }
    }

    fn open(self: &Arc<Self>) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let need_gateway = {
            let gw = crate::invariants::lock(&self.gw);
            if gw.socket.is_some() {
                return;
            }
            gw.session.is_none() && gw.gateway.is_none()
        };
        if need_gateway {
            match discord_rest(&self.options, Some(&self.token), "GET", "/gateway/bot", None) {
                Ok(Some(found)) => {
                    if let Some(u) = found["url"].as_str() {
                        if allowed_gateway_url(u, self.local) {
                            crate::invariants::lock(&self.gw).gateway = Some(u.to_owned());
                        }
                    }
                    if found["session_start_limit"]["remaining"].as_u64() == Some(0) {
                        self.events.trouble("Discord asks the bot to wait before it connects again. Locust tries again later.", false);
                        let reset = found["session_start_limit"]["reset_after"].as_u64().unwrap_or(60_000).clamp(5_000, 3_600_000);
                        self.later(reset);
                        return;
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    if err.status == 401 {
                        self.events.trouble(TOKEN_REFUSED, true);
                        return;
                    }
                    self.events.trouble("Discord could not be reached. Locust keeps trying.", false);
                    let wait = {
                        let mut gw = crate::invariants::lock(&self.gw);
                        let w = gw.backoff_ms;
                        gw.backoff_ms = (gw.backoff_ms * 2).min(60_000);
                        w
                    };
                    self.later(wait);
                    return;
                }
            }
        }
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let (base, conn_id) = {
            let mut gw = crate::invariants::lock(&self.gw);
            if gw.socket.is_some() {
                return;
            }
            gw.conn_id += 1;
            let base = gw
                .session
                .as_ref()
                .map(|(_, r)| r.clone())
                .or_else(|| gw.gateway.clone())
                .unwrap_or_else(|| GATEWAY.to_owned());
            (base, gw.conn_id)
        };
        let url = format!("{}/?v=10&encoding=json", base.trim_end_matches('/'));
        let handler = Arc::new(WsHandler { transport: self.clone(), conn_id });
        match self.options.ws.connect(&url, handler) {
            Ok(ws) => {
                let mut gw = crate::invariants::lock(&self.gw);
                if gw.conn_id == conn_id && !self.stopped.load(Ordering::SeqCst) {
                    gw.socket = Some(ws);
                } else {
                    ws.close(1000, "stopped");
                }
            }
            Err(_) => {
                let wait = crate::invariants::lock(&self.gw).backoff_ms;
                self.later(wait);
            }
        }
    }

    fn later(self: &Arc<Self>, ms: u64) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        let t = self.clone();
        std::thread::spawn(move || {
            let steps = (ms / 50).max(1);
            for _ in 0..steps {
                if t.stopped.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            t.open();
        });
    }

    fn received(self: &Arc<Self>, conn_id: u64, raw: &str) {
        let Ok(payload) = serde_json::from_str::<Value>(raw) else { return };
        let op = payload["op"].as_u64();
        if let Some(s) = payload["s"].as_i64() {
            let mut gw = crate::invariants::lock(&self.gw);
            if gw.conn_id != conn_id {
                return;
            }
            gw.seq = Some(s);
        }
        match op {
            Some(10) => {
                let interval = payload["d"]["heartbeat_interval"].as_u64().unwrap_or(41_250);
                let (ws, identify_payload, beat_stop) = {
                    let mut gw = crate::invariants::lock(&self.gw);
                    if gw.conn_id != conn_id {
                        return;
                    }
                    Self::stop_beats(&mut gw);
                    gw.acked = true;
                    let flag = Arc::new(AtomicBool::new(false));
                    gw.beat_stop = Some(flag.clone());
                    let p = if let Some((ref sid, _)) = gw.session {
                        json!({ "op": 6, "d": { "token": self.token, "session_id": sid, "seq": gw.seq } })
                    } else {
                        json!({ "op": 2, "d": { "token": self.token, "intents": INTENTS, "properties": { "os": "darwin", "browser": "Locust", "device": "Locust" } } })
                    };
                    (gw.socket.clone(), p, flag)
                };
                if let Some(ws) = ws {
                    let _ = ws.send_text(&identify_payload.to_string());
                }
                let t = self.clone();
                std::thread::spawn(move || {
                    let steps = (interval / 100).max(1);
                    loop {
                        for _ in 0..steps {
                            if beat_stop.load(Ordering::SeqCst) || t.stopped.load(Ordering::SeqCst) {
                                return;
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        let (ws, seq, acked) = {
                            let mut gw = crate::invariants::lock(&t.gw);
                            if gw.conn_id != conn_id {
                                return;
                            }
                            let was_acked = gw.acked;
                            gw.acked = false;
                            (gw.socket.clone(), gw.seq, was_acked)
                        };
                        if let Some(ws) = ws {
                            if !acked {
                                ws.close(4000, "no heartbeat answer");
                                return;
                            }
                            let _ = ws.send_text(&json!({ "op": 1, "d": seq }).to_string());
                        } else {
                            return;
                        }
                    }
                });
            }
            Some(11) => {
                crate::invariants::lock(&self.gw).acked = true;
            }
            Some(1) => {
                let (ws, seq) = {
                    let gw = crate::invariants::lock(&self.gw);
                    (gw.socket.clone(), gw.seq)
                };
                if let Some(ws) = ws {
                    let _ = ws.send_text(&json!({ "op": 1, "d": seq }).to_string());
                }
            }
            Some(7) => {
                if let Some(ws) = crate::invariants::lock(&self.gw).socket.clone() {
                    ws.close(4000, "reconnect asked");
                }
            }
            Some(9) => {
                let ws = {
                    let mut gw = crate::invariants::lock(&self.gw);
                    if payload["d"] != true {
                        gw.session = None;
                        gw.seq = None;
                    }
                    gw.quiet_for_ms = Some(1_500);
                    gw.socket.clone()
                };
                if let Some(ws) = ws {
                    ws.close(4000, "invalid session");
                }
            }
            Some(0) => {
                if let Some(t) = payload["t"].as_str() {
                    self.dispatched(conn_id, t, &payload["d"]);
                }
            }
            _ => {}
        }
    }

    fn dispatched(self: &Arc<Self>, conn_id: u64, kind: &str, data: &Value) {
        match kind {
            "READY" => {
                let sid = data["session_id"].as_str().unwrap_or("").to_owned();
                {
                    let mut gw = crate::invariants::lock(&self.gw);
                    let resume = data["resume_gateway_url"]
                        .as_str()
                        .filter(|u| allowed_gateway_url(u, self.local))
                        .map(str::to_owned)
                        .or_else(|| gw.gateway.clone())
                        .unwrap_or_else(|| GATEWAY.to_owned());
                    gw.session = Some((sid, resume));
                    gw.ready_conn = Some(conn_id);
                    gw.backoff_ms = 1_000;
                }
                self.events.connected();
            }
            "RESUMED" => {
                {
                    let mut gw = crate::invariants::lock(&self.gw);
                    gw.ready_conn = Some(conn_id);
                    gw.backoff_ms = 1_000;
                }
                self.events.connected();
            }
            "MESSAGE_CREATE" => {
                let msg_id = data["id"].as_str().unwrap_or("").to_owned();
                let author = &data["author"];
                let msg_type = data["type"].as_u64();
                let is_bot = author["bot"] == true
                    || data["webhook_id"].is_string()
                    || msg_type.is_some_and(|t| t != 0 && t != 19);
                let text = data["content"].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
                let sent_at = data["timestamp"]
                    .as_str()
                    .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
                    .map(|dt| dt.timestamp_millis() as u64)
                    .unwrap_or_else(|| made_at(&msg_id));
                let reply_to = if msg_type == Some(19) {
                    data["message_reference"]["message_id"].as_str().map(str::to_owned)
                } else {
                    None
                };
                self.events.message(ChatIncoming {
                    from: author["id"].as_str().unwrap_or("").to_owned(),
                    bot: is_bot,
                    place: data["channel_id"].as_str().unwrap_or("").to_owned(),
                    direct: data["guild_id"].is_null(),
                    id: msg_id.clone(),
                    text,
                    sent_at,
                    reply_to,
                    answer_to: Some(msg_id),
                    name: author["global_name"].as_str().or_else(|| author["username"].as_str()).map(str::to_owned),
                    username: author["username"].as_str().map(str::to_owned),
                    command: None,
                    respond: None,
                });
            }
            "INTERACTION_CREATE" => {
                self.interacted(data);
            }
            _ => {}
        }
    }

    fn interacted(&self, interaction: &Value) {
        let user = if interaction["user"].is_object() { &interaction["user"] } else { &interaction["member"]["user"] };
        let place = interaction["channel_id"]
            .as_str()
            .or_else(|| interaction["channel"]["id"].as_str())
            .unwrap_or("")
            .to_owned();
        let direct = interaction["guild_id"].is_null()
            && (interaction["context"].is_null() || interaction["context"].as_u64() == Some(1));
        let int_type = interaction["type"].as_u64();
        let int_id = interaction["id"].as_str().unwrap_or("").to_owned();
        let int_token = interaction["token"].as_str().unwrap_or("").to_owned();
        let app_id = interaction["application_id"].as_str().unwrap_or("").to_owned();

        if int_type == Some(3) {
            let _ = discord_rest(&self.options, None, "POST", &format!("/interactions/{int_id}/{int_token}/callback"), Some(json!({"type": 6})));
            self.events.press(ChatPress {
                from: user["id"].as_str().unwrap_or("").to_owned(),
                place: if direct { place } else { String::new() },
                message: interaction["message"]["id"].as_str().unwrap_or("").to_owned(),
                data: interaction["data"]["custom_id"].as_str().unwrap_or("").to_owned(),
                answer: Arc::new(|_| {}),
            });
            return;
        }
        if int_type != Some(2) {
            return;
        }

        let deferred = discord_rest(
            &self.options,
            None,
            "POST",
            &format!("/interactions/{int_id}/{int_token}/callback?with_response=true"),
            Some(json!({"type": 5})),
        )
        .ok()
        .flatten();
        let orig_path = format!("/webhooks/{app_id}/{int_token}/messages/@original");
        let id = deferred
            .as_ref()
            .and_then(|d| d["interaction"]["response_message_id"].as_str().map(str::to_owned))
            .or_else(|| {
                discord_rest(&self.options, None, "GET", &orig_path, None)
                    .ok()
                    .flatten()
                    .and_then(|v| v["id"].as_str().map(str::to_owned))
            });

        let words = interaction["data"]["options"]
            .as_array()
            .and_then(|arr| arr.iter().find(|o| o["name"] == "message"))
            .and_then(|o| o["value"].as_str())
            .map(str::trim)
            .unwrap_or("")
            .to_owned();
        let name = interaction["data"]["name"].as_str().unwrap_or("").to_lowercase();
        let responded = Arc::new(AtomicBool::new(false));
        let resp_flag = responded.clone();
        let opts = self.options.clone();
        let op_clone = orig_path.clone();
        let id_clone = id.clone();

        self.events.message(ChatIncoming {
            from: user["id"].as_str().unwrap_or("").to_owned(),
            bot: user["bot"] == true,
            place,
            direct,
            id: id.clone().unwrap_or_else(|| int_id.clone()),
            text: Some(if words.is_empty() { format!("/{name}") } else { format!("/{name} {words}") }),
            sent_at: made_at(&int_id),
            reply_to: None,
            answer_to: id,
            name: user["global_name"].as_str().or_else(|| user["username"].as_str()).map(str::to_owned),
            username: user["username"].as_str().map(str::to_owned),
            command: Some(ParsedCommand { command: name, rest: words }),
            respond: Some(Arc::new(move |text, buttons| {
                resp_flag.store(true, Ordering::SeqCst);
                discord_rest(&opts, None, "PATCH", &op_clone, Some(body_of(text, buttons)))
                    .ok()
                    .flatten()
                    .and_then(|v| v["id"].as_str().map(str::to_owned))
                    .or_else(|| id_clone.clone())
            })),
        });

        if !responded.load(Ordering::SeqCst) {
            let _ = discord_rest(
                &self.options,
                None,
                "PATCH",
                &orig_path,
                Some(body_of("Locust answers only the person it is paired with.", None)),
            );
        }
    }

    fn closed(self: &Arc<Self>, conn_id: u64, code: u16) {
        let (was_ready, wait_ms) = {
            let mut gw = crate::invariants::lock(&self.gw);
            if gw.conn_id != conn_id {
                return;
            }
            Self::stop_beats(&mut gw);
            gw.socket = None;
            if self.stopped.load(Ordering::SeqCst) {
                return;
            }
            if code == 4004 {
                drop(gw);
                self.events.trouble(TOKEN_REFUSED, true);
                return;
            }
            if code == 4013 || code == 4014 {
                drop(gw);
                self.events.trouble(
                    "Discord refused what Locust asks to read: only direct messages. Make a new application and paste its bot\u{2019}s token.",
                    true,
                );
                return;
            }
            if matches!(code, 4010 | 4011 | 4012) {
                drop(gw);
                self.events.trouble("Discord closed the connection for good. Remove the bot here and add it again.", true);
                return;
            }
            if code == 4007 || code == 4009 {
                gw.session = None;
                gw.seq = None;
            }
            let was_ready = gw.ready_conn == Some(conn_id);
            let wait = gw.quiet_for_ms.take().unwrap_or(if was_ready { 500 } else { gw.backoff_ms });
            if !was_ready {
                gw.backoff_ms = (gw.backoff_ms * 2).min(60_000);
            }
            (was_ready, wait)
        };
        if !was_ready {
            self.events.trouble("Discord could not be reached. Locust keeps trying.", false);
        }
        self.later(wait_ms);
    }
}

impl ChatTransport for DiscordTransport {
    fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        let ws = {
            let mut gw = crate::invariants::lock(&self.gw);
            Self::stop_beats(&mut gw);
            gw.socket.take()
        };
        if let Some(ws) = ws {
            ws.close(1000, "Locust closed");
        }
    }

    fn send(&self, place: &str, text: &str, extra: &ChatSendExtra) -> Option<String> {
        let mut body = body_of(text, Some(&extra.buttons));
        if let Some(ref answer_to) = extra.answer_to {
            if !extra.continued {
                body["message_reference"] = json!({ "message_id": answer_to, "fail_if_not_exists": false });
            }
        }
        match discord_rest(&self.options, Some(&self.token), "POST", &format!("/channels/{place}/messages"), Some(body)) {
            Ok(Some(res)) => res["id"].as_str().map(str::to_owned),
            Ok(None) => None,
            Err(err) => {
                if err.code == 50007 {
                    self.events.problem("Discord did not deliver the bot\u{2019}s message. In Discord, allow direct messages from members of the server the bot is in, then write to it again.");
                } else if err.status == 401 {
                    self.events.trouble(TOKEN_REFUSED, true);
                }
                self.events.note(&format!("a message was not sent: {}", err.detail));
                None
            }
        }
    }

    fn edit(&self, place: &str, message: &str, text: &str) {
        let _ = discord_rest(
            &self.options,
            Some(&self.token),
            "PATCH",
            &format!("/channels/{place}/messages/{message}"),
            Some(json!({ "content": text, "components": [], "allowed_mentions": { "parse": [] } })),
        );
    }

    fn typing(&self, place: &str) {
        let _ = discord_rest(&self.options, Some(&self.token), "POST", &format!("/channels/{place}/typing"), None);
    }

    fn has_typing(&self) -> bool {
        true
    }

    fn typing_ms(&self) -> Option<u64> {
        Some(8_000)
    }

    fn commands(&self, list: &[ChatCommand]) -> Result<(), String> {
        let app = discord_rest(&self.options, Some(&self.token), "GET", "/applications/@me", None)
            .map_err(|e| e.detail)?
            .unwrap_or(Value::Null);
        let app_id = app["id"].as_str().unwrap_or("");
        let payload: Vec<Value> = list
            .iter()
            .map(|one| {
                let mut obj = json!({
                    "name": one.command,
                    "type": 1,
                    "description": one.description.chars().take(100).collect::<String>(),
                    "contexts": [1],
                    "integration_types": [0],
                });
                if let Some(ref w) = one.words {
                    obj["options"] = json!([{
                        "type": 3,
                        "name": "message",
                        "description": w.chars().take(100).collect::<String>(),
                        "required": false,
                        "max_length": 4_000,
                    }]);
                }
                obj
            })
            .collect();
        discord_rest(&self.options, Some(&self.token), "PUT", &format!("/applications/{app_id}/commands"), Some(json!(payload)))
            .map(|_| ())
            .map_err(|e| e.detail)
    }
}

pub struct DiscordApp {
    options: DiscordOptions,
    words: ChatWords,
}

impl DiscordApp {
    pub fn new(options: DiscordOptions) -> Arc<Self> {
        Arc::new(Self {
            options,
            words: ChatWords {
                address: |cmd, _| format!("/{cmd}"),
                own: |cmd| format!("/{cmd}"),
                who_line: |cmd, name, role| format!("/{cmd} · {}, {}", discord_escape(name), discord_escape(role)),
                carry_on: "Reply to an answer to carry on that conversation.",
                reply_to_card_or_own: "Or reply to this message with an answer of your own.",
                reply_to_card_only: "Reply to this message with your answer.",
                wrong_code: "That code does not pair this account. Make a new one in Locust: Settings > General > Chat apps.",
                help_more: Some("Type / here to pick a teammate from Discord\u{2019}s list."),
                no_token: "Paste your bot\u{2019}s token first.",
                unreadable: "The bot\u{2019}s token could not be read on this computer. Paste it again.",
            },
        })
    }
}

impl ChatApp for DiscordApp {
    fn id(&self) -> ChatAppId {
        ChatAppId::Discord
    }
    fn file(&self) -> &'static str {
        DISCORD_FILE
    }
    fn escape(&self, text: &str) -> String {
        discord_escape(text)
    }
    fn bold(&self, text: &str) -> String {
        format!("**{}**", discord_escape(text))
    }
    fn italic(&self, text: &str) -> String {
        format!("*{}*", discord_escape(text))
    }
    fn answer_parts(&self, name: &str, markdown: &str) -> Vec<String> {
        discord_messages(name, markdown, DISCORD_MESSAGE_MAX)
    }
    fn parse(&self, text: &str, _bot: &ChatBot) -> Option<ParsedCommand> {
        let trimmed = text.trim();
        let rest = trimmed.strip_prefix('/')?;
        let mut parts = rest.splitn(2, char::is_whitespace);
        let cmd = parts.next()?.to_lowercase();
        if cmd.is_empty() || cmd.len() > 32 || !cmd.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return None;
        }
        Some(ParsedCommand { command: cmd, rest: parts.next().unwrap_or("").trim().to_owned() })
    }
    fn code_in(&self, text: &str, _bot: &ChatBot) -> Option<String> {
        pairing_code_in(text)
    }
    fn fresh_code(&self) -> String {
        random_8_digits()
    }
    fn pairing(&self, _bot: &ChatBot, code: &str, until: &str) -> (Option<ChatAppLink>, Option<ChatAppCode>) {
        (None, Some(ChatAppCode { code: code.to_owned(), until: until.to_owned() }))
    }
    fn shown(&self, bot: &ChatBot) -> ChatAppBot {
        ChatAppBot {
            username: bot.username.clone(),
            name: bot.name.clone(),
            invite: bot.extra.get("application").map(|app| discord_invite(app)),
            team: None,
            open: Some(format!("https://discord.com/users/{}", bot.id)),
        }
    }
    fn shape(&self, given: &TokenInput) -> Result<String, String> {
        let raw = match given {
            TokenInput::Single(s) => s.trim(),
            _ => "",
        };
        let candidate = raw
            .strip_prefix("Bot ")
            .or_else(|| raw.strip_prefix("bot "))
            .map(str::trim_start)
            .unwrap_or(raw);
        if is_discord_token(candidate) {
            Ok(candidate.to_owned())
        } else {
            Err("That is not a bot\u{2019}s token. On your application\u{2019}s Bot page, press Reset Token, then copy all of the token it shows.".into())
        }
    }
    fn verify(&self, secret: &str) -> Result<ChatBot, String> {
        let map_err = |err: DiscordError| -> String {
            if err.status == 401 {
                "Discord does not know that token. Copy it again from your application\u{2019}s Bot page: Reset Token gives a new one.".into()
            } else {
                "Discord could not be reached. Check the connection, then try again.".into()
            }
        };
        let me = discord_rest(&self.options, Some(secret), "GET", "/users/@me", None)
            .map_err(map_err)?
            .unwrap_or(Value::Null);
        if me["bot"] != true {
            return Err("That token is not a bot\u{2019}s. Copy the one on your application\u{2019}s Bot page.".into());
        }
        let app = discord_rest(&self.options, Some(secret), "GET", "/applications/@me", None)
            .map_err(map_err)?
            .unwrap_or(Value::Null);
        let username = me["username"].as_str().unwrap_or("bot");
        let tag = match me["discriminator"].as_str() {
            Some(d) if d != "0" => format!("{username}#{d}"),
            _ => username.to_owned(),
        };
        let name = app["name"]
            .as_str()
            .or_else(|| me["global_name"].as_str())
            .or_else(|| me["username"].as_str())
            .unwrap_or("Your bot")
            .to_owned();
        let mut extra = BTreeMap::new();
        if let Some(app_id) = app["id"].as_str() {
            extra.insert("application".into(), app_id.to_owned());
        }
        Ok(ChatBot { id: me["id"].as_str().unwrap_or("").to_owned(), username: tag, name, extra })
    }
    fn keeps(&self, secret: &str) -> bool {
        is_discord_token(secret)
    }
    fn connect(&self, secret: &str, _bot: &ChatBot, events: Arc<dyn ChatEvents>) -> Arc<dyn ChatTransport> {
        DiscordTransport::start(secret.to_owned(), self.options.clone(), events)
    }
    fn forget(&self, secret: &str, bot: &ChatBot) {
        let app_id = bot.extra.get("application").map(String::as_str).unwrap_or(&bot.id);
        let _ = discord_rest(&self.options, Some(secret), "PUT", &format!("/applications/{app_id}/commands"), Some(json!([])));
    }
    fn words(&self) -> &ChatWords {
        &self.words
    }
}

pub fn create_discord_link(discord_options: DiscordOptions, link_options: ChatLinkOptions) -> Arc<ChatLink> {
    create_chat_link(DiscordApp::new(discord_options), link_options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_http::HttpResponse;
    use crate::chat_link::{ChatButton, ChatStart, ChatTeammate};
    use crate::own_models::SecretBox;
    use std::sync::atomic::AtomicU64;

    struct FakeSecretBox;
    impl SecretBox for FakeSecretBox {
        fn available(&self) -> bool { true }
        fn seal(&self, text: &str) -> Result<String, String> {
            let hex: String = text.bytes().map(|b| format!("{:02x}", b ^ 0x5a)).collect();
            Ok(format!("sealed:{hex}"))
        }
        fn open(&self, sealed: &str) -> Result<String, String> {
            let hex = sealed.strip_prefix("sealed:").ok_or("bad")?;
            let mut out = Vec::new();
            for i in (0..hex.len()).step_by(2) {
                out.push(u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| "hex")? ^ 0x5a);
            }
            String::from_utf8(out).map_err(|_| "utf8".into())
        }
    }

    struct FakeDiscord {
        token: String,
        sent: Mutex<Vec<Value>>,
        ws_sent: Mutex<Vec<Value>>,
        callbacks: Mutex<Option<Arc<dyn WsCallbacks>>>,
    }

    impl FakeDiscord {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                token: "MTEwMDAwMDAwMDAwMDAwMDAwMQ.GfakeA.fake_fake_fake_fake_fake_fake_fake_fak".into(),
                sent: Mutex::new(Vec::new()),
                ws_sent: Mutex::new(Vec::new()),
                callbacks: Mutex::new(None),
            })
        }
    }

    impl HttpClient for FakeDiscord {
        fn send(&self, req: HttpRequest) -> Result<HttpResponse, String> {
            let body: Value = req.body.as_deref().and_then(|b| serde_json::from_slice(b).ok()).unwrap_or(Value::Null);
            let resp = if req.url.ends_with("/users/@me") {
                json!({"id": "1100000000000000001", "username": "locust_bot", "discriminator": "0", "bot": true})
            } else if req.url.ends_with("/applications/@me") {
                json!({"id": "1100000000000000001", "name": "Locust"})
            } else if req.url.ends_with("/gateway/bot") {
                json!({"url": "ws://127.0.0.1:9999", "session_start_limit": {"remaining": 1000}})
            } else if req.url.contains("/messages") && req.method == "POST" {
                self.sent.lock().unwrap().push(body);
                json!({"id": "9001"})
            } else {
                json!({})
            };
            Ok(HttpResponse { status: 200, headers: vec![], body: resp.to_string().into_bytes() })
        }
    }

    struct FakeWsConn {
        sent: Arc<FakeDiscord>,
    }
    impl WsConnection for FakeWsConn {
        fn send_text(&self, text: &str) -> Result<(), String> {
            if let Ok(v) = serde_json::from_str::<Value>(text) {
                self.sent.ws_sent.lock().unwrap().push(v);
            }
            Ok(())
        }
        fn close(&self, _code: u16, _reason: &str) {}
    }

    impl WsClient for FakeDiscord {
        fn connect(&self, _url: &str, callbacks: Arc<dyn WsCallbacks>) -> Result<Arc<dyn WsConnection>, String> {
            *self.callbacks.lock().unwrap() = Some(callbacks.clone());
            callbacks.on_message(json!({"op": 10, "d": {"heartbeat_interval": 30000}}).to_string());
            callbacks.on_message(json!({"op": 0, "s": 1, "t": "READY", "d": {"session_id": "s1", "resume_gateway_url": "ws://127.0.0.1:9999"}}).to_string());
            Ok(Arc::new(FakeWsConn { sent: Arc::new(FakeDiscord {
                token: self.token.clone(),
                sent: Mutex::new(Vec::new()),
                ws_sent: Mutex::new(Vec::new()),
                callbacks: Mutex::new(None),
            }) }))
        }
    }

    #[test]
    fn lays_out_short_teammate_buttons_five_to_a_row_and_pairs_by_code() {
        let short_btns: ChatButtons = (0..6)
            .map(|i| vec![ChatButton { label: format!("Mate {i}"), data: format!("t:{i}"), style: None }])
            .collect();
        let laid = components_of(&short_btns);
        assert_eq!(laid.len(), 2);
        assert_eq!(laid[0]["components"].as_array().unwrap().len(), 5);
        assert_eq!(laid[1]["components"].as_array().unwrap().len(), 1);

        static SEQ: AtomicU64 = AtomicU64::new(1);
        let dir = std::env::temp_dir().join(format!("locust-dc-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::create_dir_all(&dir);
        let fake = FakeDiscord::new();
        let started = Arc::new(Mutex::new(Vec::new()));
        let link = create_discord_link(
            DiscordOptions { http: fake.clone(), ws: fake.clone(), api: Some(Arc::new(|| "http://127.0.0.1:1".into())), version: None },
            ChatLinkOptions {
                directory: dir.clone(),
                secrets: Arc::new(FakeSecretBox),
                refusal: Arc::new(|| None),
                computer: Arc::new(|| "DESK-PC".into()),
                team: Arc::new(|| vec![ChatTeammate { teammate_id: "t_ada".into(), name: "Ada".into(), role: "Research".into() }]),
                start: {
                    let st = started.clone();
                    Arc::new(move |tid, text, _| { st.lock().unwrap().push((tid.to_owned(), text.to_owned())); ChatStart::Ok { mission_id: "m1".into() } })
                },
                read: Arc::new(|_| None),
                live: Arc::new(|_| false),
                carried: Arc::new(|_| false),
                waiting: Arc::new(|| vec![]),
                decide: Arc::new(|_| false),
                changed: None,
                note: None,
                now: None,
                typing_ms: Some(100),
            },
        );
        let s = link.set_token(TokenInput::Single(fake.token.clone()));
        let code = s.code.unwrap().code;
        for _ in 0..30 {
            if fake.callbacks.lock().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let cb = fake.callbacks.lock().unwrap().clone().unwrap();
        let now_id = ((now_ms() - 1_420_070_400_000) << 22).to_string();
        let now_id2 = (((now_ms() - 1_420_070_400_000) << 22) + 1).to_string();
        cb.on_message(json!({
            "op": 0, "s": 2, "t": "MESSAGE_CREATE",
            "d": { "id": now_id, "channel_id": "dm_1", "author": { "id": "u_1", "username": "colin" }, "content": code, "type": 0 }
        }).to_string());
        assert_eq!(link.settings().chat.unwrap().name, "colin");
        cb.on_message(json!({
            "op": 0, "s": 3, "t": "MESSAGE_CREATE",
            "d": { "id": now_id2, "channel_id": "dm_1", "author": { "id": "u_1", "username": "colin" }, "content": "/ada hi", "type": 0 }
        }).to_string());
        assert_eq!(started.lock().unwrap().as_slice(), &[("t_ada".into(), "hi".into())]);
        link.dispose();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
