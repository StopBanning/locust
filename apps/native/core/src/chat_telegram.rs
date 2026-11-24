//! Telegram Bot API transport and ChatApp (Electron `main/telegram.ts`, 0.831): long-polling `getUpdates`,
//! `sendMessage` with HTML and plain-text fallback, inline keyboards, and token verification via `getMe`/`getWebhookInfo`.

use crate::chat_apps::{ChatAppBot, ChatAppCode, ChatAppId, ChatAppLink};
use crate::chat_http::{HttpClient, HttpRequest};
use crate::chat_link::{
    ChatApp, ChatBot, ChatCommand, ChatEvents, ChatIncoming, ChatLink, ChatLinkOptions, ChatPress, ChatSendExtra,
    ChatTransport, ChatWords, ParsedCommand, TokenInput, create_chat_link, random_base64url,
};
use crate::chat_telegram_fmt::{
    TELEGRAM_MESSAGE_MAX, answer_messages, is_telegram_token, parsed_command, plain_of, telegram_escape,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const TELEGRAM_FILE: &str = "telegram.json";
pub const TELEGRAM_API: &str = "https://api.telegram.org";

#[derive(Clone)]
pub struct TelegramOptions {
    pub http: Arc<dyn HttpClient>,
    pub api: Option<Arc<dyn Fn() -> String + Send + Sync>>,
    pub poll_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct TelegramError {
    pub code: u16,
    pub description: String,
    pub retry_after: Option<u64>,
}

fn telegram_call(
    opts: &TelegramOptions,
    token: &str,
    method: &str,
    body: Value,
    timeout_secs: u64,
) -> Result<Value, TelegramError> {
    let base = opts
        .api
        .as_ref()
        .map(|f| f())
        .unwrap_or_else(|| TELEGRAM_API.to_owned());
    let base = base.trim_end_matches('/');
    let url = format!("{base}/bot{token}/{method}");
    let resp = opts
        .http
        .send(HttpRequest {
            method: "POST".into(),
            url,
            headers: vec![("content-type".into(), "application/json".into())],
            body: Some(body.to_string().into_bytes()),
            timeout_secs,
        })
        .map_err(|_| TelegramError {
            code: 0,
            description: "Telegram could not be reached.".into(),
            retry_after: None,
        })?;

    let data: Option<Value> = serde_json::from_slice(&resp.body).ok();
    if let Some(ref v) = data {
        if v["ok"] == true {
            return Ok(v["result"].clone());
        }
        let code = v["error_code"].as_u64().map(|n| n as u16).unwrap_or(resp.status);
        let desc = v["description"]
            .as_str()
            .map(|s| s.chars().take(300).collect())
            .unwrap_or_else(|| format!("Telegram answered {}.", resp.status));
        let retry_after = v["parameters"]["retry_after"].as_u64();
        return Err(TelegramError { code, description: desc, retry_after });
    }
    Err(TelegramError {
        code: resp.status,
        description: format!("Telegram answered {}.", resp.status),
        retry_after: None,
    })
}

fn name_of(user: &Value) -> String {
    let first = user["first_name"].as_str().filter(|s| !s.is_empty());
    let last = user["last_name"].as_str().filter(|s| !s.is_empty());
    let parts: Vec<&str> = [first, last].into_iter().flatten().collect();
    if parts.is_empty() { "Your chat".into() } else { parts.join(" ") }
}

fn incoming_of(msg: &Value) -> ChatIncoming {
    let from_obj = &msg["from"];
    let from = from_obj["id"]
        .as_i64()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let msg_id = msg["message_id"]
        .as_i64()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let place = msg["chat"]["id"]
        .as_i64()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let date = msg["date"].as_u64().unwrap_or_else(|| {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    });
    ChatIncoming {
        from,
        bot: from_obj["is_bot"] == true,
        place,
        direct: msg["chat"]["type"].as_str() == Some("private"),
        id: msg_id.clone(),
        text: msg["text"].as_str().map(str::to_owned),
        sent_at: date * 1000,
        reply_to: msg["reply_to_message"]["message_id"].as_i64().map(|n| n.to_string()),
        answer_to: Some(msg_id),
        name: Some(name_of(from_obj)),
        username: from_obj["username"].as_str().map(str::to_owned),
        command: None,
        respond: None,
    }
}

fn is_parse_error(desc: &str) -> bool {
    let lower = desc.to_lowercase();
    lower.contains("entit") || lower.contains("tag") || lower.contains("parse")
}

struct TelegramTransport {
    token: String,
    options: TelegramOptions,
    events: Arc<dyn ChatEvents>,
    stop_flag: Arc<AtomicBool>,
}

impl TelegramTransport {
    fn start(token: String, options: TelegramOptions, events: Arc<dyn ChatEvents>) -> Arc<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let transport = Arc::new(Self { token, options, events, stop_flag });
        let t = transport.clone();
        std::thread::spawn(move || t.poll_loop());
        transport
    }

    fn poll_loop(&self) {
        let poll_seconds = self.options.poll_seconds.unwrap_or(25);
        let mut backoff_ms = 2_000u64;
        let mut first = true;

        while !self.stop_flag.load(Ordering::SeqCst) {
            let mut body = json!({
                "timeout": if first { 0 } else { poll_seconds },
                "allowed_updates": ["message", "callback_query"],
            });
            if let Some(offset) = self.events.offset() {
                body["offset"] = json!(offset);
            }
            match telegram_call(&self.options, &self.token, "getUpdates", body, poll_seconds + 20) {
                Ok(updates_val) => {
                    if self.stop_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    first = false;
                    self.events.connected();
                    backoff_ms = 2_000;
                    if let Some(updates) = updates_val.as_array() {
                        if !updates.is_empty() {
                            let max_id = updates.iter().filter_map(|u| u["update_id"].as_i64()).max().unwrap_or(0);
                            self.events.set_offset(max_id + 1);
                            for one in updates {
                                if self.stop_flag.load(Ordering::SeqCst) {
                                    return;
                                }
                                if one["message"].is_object() {
                                    self.events.message(incoming_of(&one["message"]));
                                } else if one["callback_query"].is_object() {
                                    let q = &one["callback_query"];
                                    let qid = q["id"].as_str().unwrap_or("").to_owned();
                                    let opts = self.options.clone();
                                    let tok = self.token.clone();
                                    self.events.press(ChatPress {
                                        from: q["from"]["id"].as_i64().map(|n| n.to_string()).unwrap_or_default(),
                                        place: q["message"]["chat"]["id"].as_i64().map(|n| n.to_string()).unwrap_or_default(),
                                        message: q["message"]["message_id"].as_i64().map(|n| n.to_string()).unwrap_or_default(),
                                        data: q["data"].as_str().unwrap_or("").to_owned(),
                                        answer: Arc::new(move |text| {
                                            let mut b = json!({"callback_query_id": qid});
                                            if let Some(t) = text {
                                                b["text"] = json!(t);
                                            }
                                            let _ = telegram_call(&opts, &tok, "answerCallbackQuery", b, 10);
                                        }),
                                    });
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    if self.stop_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    if err.code == 401 || err.code == 404 {
                        self.events.trouble(
                            "Telegram no longer takes this bot\u{2019}s token. Paste a new one from @BotFather.",
                            true,
                        );
                        return;
                    }
                    let msg = if err.code == 409 {
                        "Another program is reading this bot\u{2019}s messages. Give Locust a bot of its own, made with @BotFather."
                    } else {
                        "Telegram could not be reached. Locust keeps trying."
                    };
                    self.events.trouble(msg, false);
                    let wait_ms = if err.code == 409 { 30_000 } else { backoff_ms };
                    let steps = (wait_ms / 100).max(1);
                    for _ in 0..steps {
                        if self.stop_flag.load(Ordering::SeqCst) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    backoff_ms = (backoff_ms * 2).min(60_000);
                }
            }
        }
    }
}

impl ChatTransport for TelegramTransport {
    fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    fn send(&self, place: &str, html: &str, extra: &ChatSendExtra) -> Option<String> {
        let chat_id: i64 = place.parse().ok()?;
        let mut body = json!({
            "chat_id": chat_id,
            "link_preview_options": { "is_disabled": true },
        });
        if let Some(ref answer_to) = extra.answer_to {
            if !extra.continued {
                if let Ok(mid) = answer_to.parse::<i64>() {
                    body["reply_parameters"] = json!({
                        "message_id": mid,
                        "allow_sending_without_reply": true,
                    });
                }
            }
        }
        if !extra.buttons.is_empty() {
            let keyboard: Vec<Vec<Value>> = extra
                .buttons
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|b| json!({ "text": b.label, "callback_data": b.data }))
                        .collect()
                })
                .collect();
            body["reply_markup"] = json!({ "inline_keyboard": keyboard });
        }

        for attempt in 0..2 {
            let mut req_body = body.clone();
            req_body["text"] = json!(html);
            req_body["parse_mode"] = json!("HTML");
            match telegram_call(&self.options, &self.token, "sendMessage", req_body, 15) {
                Ok(res) => return res["message_id"].as_i64().map(|n| n.to_string()),
                Err(err) => {
                    if err.code == 400 && is_parse_error(&err.description) {
                        let mut plain_body = body.clone();
                        plain_body["text"] = json!(plain_of(html));
                        return telegram_call(&self.options, &self.token, "sendMessage", plain_body, 15)
                            .ok()
                            .and_then(|res| res["message_id"].as_i64().map(|n| n.to_string()));
                    }
                    if err.code == 429 && attempt == 0 {
                        let secs = err.retry_after.unwrap_or(3).min(30);
                        std::thread::sleep(Duration::from_secs(secs));
                        continue;
                    }
                    if err.code == 403 {
                        self.events.problem(
                            "The chat blocked the bot, so Locust\u{2019}s messages do not reach it. Unblock it in Telegram.",
                        );
                    }
                    self.events.note(&format!("a message was not sent: {}", err.description));
                    return None;
                }
            }
        }
        None
    }

    fn edit(&self, place: &str, message_id: &str, html: &str) {
        let (Ok(chat_id), Ok(mid)) = (place.parse::<i64>(), message_id.parse::<i64>()) else {
            return;
        };
        let mut body = json!({
            "chat_id": chat_id,
            "message_id": mid,
            "link_preview_options": { "is_disabled": true },
            "reply_markup": { "inline_keyboard": [] },
            "text": html,
            "parse_mode": "HTML",
        });
        if let Err(err) = telegram_call(&self.options, &self.token, "editMessageText", body.clone(), 15) {
            if err.code == 400 && is_parse_error(&err.description) {
                body.as_object_mut().unwrap().remove("parse_mode");
                body["text"] = json!(plain_of(html));
                let _ = telegram_call(&self.options, &self.token, "editMessageText", body, 15);
            }
        }
    }

    fn typing(&self, place: &str) {
        if let Ok(chat_id) = place.parse::<i64>() {
            let _ = telegram_call(
                &self.options,
                &self.token,
                "sendChatAction",
                json!({"chat_id": chat_id, "action": "typing"}),
                10,
            );
        }
    }

    fn has_typing(&self) -> bool {
        true
    }

    fn typing_ms(&self) -> Option<u64> {
        Some(4_500)
    }

    fn commands(&self, list: &[ChatCommand]) -> Result<(), String> {
        let cmds: Vec<Value> = list
            .iter()
            .map(|c| json!({ "command": c.command, "description": c.description }))
            .collect();
        telegram_call(&self.options, &self.token, "setMyCommands", json!({ "commands": cmds }), 15)
            .map(|_| ())
            .map_err(|e| e.description)
    }
}

pub struct TelegramApp {
    options: TelegramOptions,
    words: ChatWords,
}

impl TelegramApp {
    pub fn new(options: TelegramOptions) -> Arc<Self> {
        Arc::new(Self {
            options,
            words: ChatWords {
                address: |cmd, _| format!("/{cmd}"),
                own: |cmd| format!("/{cmd}"),
                who_line: |cmd, name, role| format!("/{cmd} {}, {}", telegram_escape(name), telegram_escape(role)),
                carry_on: "Reply to an answer to carry on that conversation.",
                reply_to_card_or_own: "Or reply to this message with an answer of your own.",
                reply_to_card_only: "Reply to this message with your answer.",
                wrong_code: "That link does not pair this chat any more. Make a new one in Locust: Settings > General > Chat apps.",
                help_more: None,
                no_token: "Paste a bot\u{2019}s token first.",
                unreadable: "The bot\u{2019}s token could not be read on this computer. Paste it again.",
            },
        })
    }
}

impl ChatApp for TelegramApp {
    fn id(&self) -> ChatAppId {
        ChatAppId::Telegram
    }
    fn file(&self) -> &'static str {
        TELEGRAM_FILE
    }
    fn escape(&self, text: &str) -> String {
        telegram_escape(text)
    }
    fn bold(&self, text: &str) -> String {
        format!("<b>{}</b>", telegram_escape(text))
    }
    fn italic(&self, text: &str) -> String {
        format!("<i>{}</i>", telegram_escape(text))
    }
    fn answer_parts(&self, name: &str, markdown: &str) -> Vec<String> {
        answer_messages(name, markdown, TELEGRAM_MESSAGE_MAX)
    }
    fn parse(&self, text: &str, bot: &ChatBot) -> Option<ParsedCommand> {
        parsed_command(text, &bot.username)
    }
    fn code_in(&self, text: &str, bot: &ChatBot) -> Option<String> {
        let cmd = parsed_command(text, &bot.username)?;
        if cmd.command == "start" && !cmd.rest.is_empty() {
            Some(cmd.rest)
        } else {
            None
        }
    }
    fn fresh_code(&self) -> String {
        random_base64url(12)
    }
    fn pairing(&self, bot: &ChatBot, code: &str, until: &str) -> (Option<ChatAppLink>, Option<ChatAppCode>) {
        (
            Some(ChatAppLink {
                url: format!("https://t.me/{}?start={code}", bot.username),
                until: until.to_owned(),
            }),
            None,
        )
    }
    fn shown(&self, bot: &ChatBot) -> ChatAppBot {
        ChatAppBot {
            username: bot.username.clone(),
            name: bot.name.clone(),
            invite: None,
            team: None,
            open: None,
        }
    }
    fn shape(&self, given: &TokenInput) -> Result<String, String> {
        let candidate = match given {
            TokenInput::Single(s) => s.trim(),
            _ => "",
        };
        if is_telegram_token(candidate) {
            Ok(candidate.to_owned())
        } else {
            Err("That is not a bot\u{2019}s token. @BotFather gives one like 123456789:AAH\u{2026}, on a line of its own: copy all of it.".into())
        }
    }
    fn verify(&self, secret: &str) -> Result<ChatBot, String> {
        let map_err = |err: TelegramError| -> String {
            if err.code == 401 || err.code == 404 {
                "Telegram does not know that token. Copy it again from @BotFather.".into()
            } else {
                "Telegram could not be reached. Check the connection, then try again.".into()
            }
        };
        let bot = telegram_call(&self.options, secret, "getMe", json!({}), 15).map_err(map_err)?;
        let hook = telegram_call(&self.options, secret, "getWebhookInfo", json!({}), 15).map_err(map_err)?;
        if hook["url"].as_str().is_some_and(|u| !u.is_empty()) {
            return Err("This bot sends its messages to another program already. Make a new bot for Locust with @BotFather.".into());
        }
        Ok(ChatBot {
            id: bot["id"].as_i64().map(|n| n.to_string()).unwrap_or_default(),
            username: bot["username"].as_str().unwrap_or("").to_owned(),
            name: bot["first_name"].as_str().unwrap_or("").to_owned(),
            extra: BTreeMap::new(),
        })
    }
    fn keeps(&self, secret: &str) -> bool {
        is_telegram_token(secret)
    }
    fn connect(&self, secret: &str, _bot: &ChatBot, events: Arc<dyn ChatEvents>) -> Arc<dyn ChatTransport> {
        TelegramTransport::start(secret.to_owned(), self.options.clone(), events)
    }
    fn words(&self) -> &ChatWords {
        &self.words
    }
}

pub fn create_telegram_link(tg_options: TelegramOptions, link_options: ChatLinkOptions) -> Arc<ChatLink> {
    create_chat_link(TelegramApp::new(tg_options), link_options)
}

#[cfg(test)]
#[path = "chat_telegram_tests.rs"]
mod parity_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_http::HttpResponse;
    use crate::chat_link::{ChatMission, ChatStart, ChatTeammate};
    use crate::own_models::SecretBox;
    use std::collections::VecDeque;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU64;
    use std::sync::Mutex;

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

    struct FakeTelegramHttp {
        token: String,
        webhook: Option<String>,
        updates: Mutex<VecDeque<Value>>,
        sent: Mutex<Vec<Value>>,
        next_msg_id: AtomicU64,
        next_update_id: std::sync::atomic::AtomicI64,
    }

    impl FakeTelegramHttp {
        fn new(webhook: Option<&str>) -> Arc<Self> {
            Arc::new(Self {
                token: "123456789:AAHfakefakefakefakefakefakefakefake1".into(),
                webhook: webhook.map(str::to_owned),
                updates: Mutex::new(VecDeque::new()),
                sent: Mutex::new(Vec::new()),
                next_msg_id: AtomicU64::new(500),
                next_update_id: std::sync::atomic::AtomicI64::new(1),
            })
        }

        fn push_message(&self, from_id: i64, chat_id: i64, text: &str) {
            let mut q = self.updates.lock().unwrap();
            let uid = self.next_update_id.fetch_add(1, Ordering::SeqCst);
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            q.push_back(json!({
                "update_id": uid,
                "message": {
                    "message_id": uid * 10,
                    "from": { "id": from_id, "is_bot": false, "first_name": "Colin", "username": "colin" },
                    "chat": { "id": chat_id, "type": "private" },
                    "date": now,
                    "text": text,
                }
            }));
        }
    }

    impl HttpClient for FakeTelegramHttp {
        fn send(&self, req: HttpRequest) -> Result<HttpResponse, String> {
            let expected_prefix = format!("http://127.0.0.1:1/bot{}/", self.token);
            let Some(method) = req.url.strip_prefix(&expected_prefix) else {
                return Ok(HttpResponse {
                    status: 401,
                    headers: vec![],
                    body: json!({"ok": false, "error_code": 401, "description": "Unauthorized"}).to_string().into_bytes(),
                });
            };
            let body: Value = req.body.as_deref().and_then(|b| serde_json::from_slice(b).ok()).unwrap_or(json!({}));
            let resp_val = match method {
                "getMe" => json!({"ok": true, "result": {"id": 123456789, "username": "locust_test_bot", "first_name": "Locust"}}),
                "getWebhookInfo" => json!({"ok": true, "result": {"url": self.webhook.as_deref().unwrap_or("")}}),
                "getUpdates" => {
                    let mut q = self.updates.lock().unwrap();
                    let batch: Vec<Value> = q.drain(..).collect();
                    if batch.is_empty() {
                        drop(q);
                        std::thread::sleep(Duration::from_millis(30));
                    }
                    json!({"ok": true, "result": batch})
                }
                "sendMessage" => {
                    self.sent.lock().unwrap().push(body);
                    let mid = self.next_msg_id.fetch_add(1, Ordering::SeqCst);
                    json!({"ok": true, "result": {"message_id": mid}})
                }
                "setMyCommands" | "sendChatAction" | "editMessageText" | "answerCallbackQuery" => json!({"ok": true, "result": true}),
                _ => json!({"ok": false, "error_code": 404, "description": "Not Found"}),
            };
            Ok(HttpResponse { status: 200, headers: vec![], body: resp_val.to_string().into_bytes() })
        }
    }

    fn temp_dir(label: &str) -> PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let p = std::env::temp_dir().join(format!("locust-tg-{label}-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::create_dir_all(&p);
        p
    }

    #[test]
    fn refuses_bad_tokens_and_bots_with_an_existing_webhook() {
        let dir = temp_dir("webhook");
        let http = FakeTelegramHttp::new(Some("https://other.example/hook"));
        let link = create_telegram_link(
            TelegramOptions { http: http.clone(), api: Some(Arc::new(|| "http://127.0.0.1:1".into())), poll_seconds: Some(1) },
            ChatLinkOptions {
                directory: dir.clone(),
                secrets: Arc::new(FakeSecretBox),
                refusal: Arc::new(|| None),
                computer: Arc::new(|| "DESK-PC".into()),
                team: Arc::new(|| vec![]),
                start: Arc::new(|_, _, _| ChatStart::Err { message: "nope".into() }),
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
        let bad_shape = link.set_token(TokenInput::Single("not-a-token".into()));
        assert!(bad_shape.message.unwrap().contains("@BotFather"));
        let has_hook = link.set_token(TokenInput::Single(http.token.clone()));
        assert!(has_hook.message.unwrap().contains("another program already"));
        link.dispose();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pairs_routes_command_to_ada_and_sends_answer_back() {
        let dir = temp_dir("e2e");
        let http = FakeTelegramHttp::new(None);
        let missions: Arc<Mutex<std::collections::HashMap<String, ChatMission>>> = Arc::new(Mutex::new(Default::default()));
        let started = Arc::new(AtomicBool::new(false));
        let started_cb = started.clone();
        let link = create_telegram_link(
            TelegramOptions { http: http.clone(), api: Some(Arc::new(|| "http://127.0.0.1:1".into())), poll_seconds: Some(1) },
            ChatLinkOptions {
                directory: dir.clone(),
                secrets: Arc::new(FakeSecretBox),
                refusal: Arc::new(|| None),
                computer: Arc::new(|| "DESK-PC".into()),
                team: Arc::new(|| vec![ChatTeammate { teammate_id: "mate_ada".into(), name: "Ada".into(), role: "Research".into() }]),
                start: Arc::new(move |_, _, _| {
                    started_cb.store(true, Ordering::SeqCst);
                    ChatStart::Ok { mission_id: "m_1".into() }
                }),
                read: {
                    let ms = missions.clone();
                    Arc::new(move |id| ms.lock().unwrap().get(id).cloned())
                },
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
        let s = link.set_token(TokenInput::Single(http.token.clone()));
        let code = s.link.unwrap().url.split("start=").nth(1).unwrap().to_owned();
        http.push_message(77, 77, &format!("/start {code}"));
        for _ in 0..50 {
            if !http.sent.lock().unwrap().is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(http.sent.lock().unwrap()[0]["text"].as_str().unwrap().contains("Paired with Locust"));

        for _ in 0..50 {
            if link.settings().chat.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        http.push_message(77, 77, "/ada check tests");
        for _ in 0..100 {
            if started.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(started.load(Ordering::SeqCst), "turn was not started");
        missions.lock().unwrap().insert("m_1".into(), ChatMission {
            mission_id: "m_1".into(),
            phase: "completed".into(),
            events: vec![json!({"type": "message.delta", "payload": {"itemId": "x", "operation": "append", "text": "All green.", "final": true}})],
            started_by_kind: None,
            continues_from_mission_id: None,
            host_failure_message: None,
        });
        link.on_run_ended("m_1");
        let sent = http.sent.lock().unwrap();
        assert!(sent.iter().any(|m| m["text"].as_str().unwrap_or("").contains("All green.")));
        link.dispose();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
