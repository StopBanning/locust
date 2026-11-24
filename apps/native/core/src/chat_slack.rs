//! Slack Socket Mode + Web API transport and ChatApp (Electron `main/slack.ts`, 0.832):
//! `apps.connections.open`, envelope acks, `chat.postMessage` / `chat.update`, `eyes` working reactions, and `users.info`.

use crate::chat_apps::{ChatAppBot, ChatAppCode, ChatAppId, ChatAppLink, SlackTokens, pairing_code_in};
use crate::chat_http::{HttpClient, HttpRequest, WsCallbacks, WsClient, WsConnection};
use crate::chat_link::{
    ButtonStyle, ChatApp, ChatBot, ChatEvents, ChatIncoming, ChatLink, ChatLinkOptions, ChatPress, ChatSendExtra,
    ChatTransport, ChatWords, ParsedCommand, TokenInput, create_chat_link,
};
use crate::chat_slack_fmt::{
    SLACK_MESSAGE_MAX, is_slack_app_token, is_slack_bot_token, slack_escape, slack_messages, slack_plain,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const SLACK_FILE: &str = "slack.json";
pub const SLACK_API: &str = "https://slack.com";
const TOKEN_REFUSED: &str = "Slack no longer takes the app\u{2019}s tokens. Paste them again from the app\u{2019}s settings: Remove, then set it up again.";
const REFUSED: &[&str] = &["invalid_auth", "not_authed", "account_inactive", "token_revoked", "token_expired", "not_allowed_token_type"];

#[derive(Clone)]
pub struct SlackOptions {
    pub http: Arc<dyn HttpClient>,
    pub ws: Arc<dyn WsClient>,
    pub api: Option<Arc<dyn Fn() -> String + Send + Sync>>,
}

#[derive(Debug, Clone)]
pub struct SlackError {
    pub error: String,
    pub status: u16,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn random_8_digits() -> String {
    let mut b = [0u8; 4];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    let val = u32::from_le_bytes(b) % 100_000_000;
    format!("{val:08}")
}

fn tokens_of(secret: &str) -> Option<SlackTokens> {
    let v: Value = serde_json::from_str(secret).ok()?;
    let bot = v["bot"].as_str()?;
    let app = v["app"].as_str()?;
    if is_slack_bot_token(bot) && is_slack_app_token(app) {
        Some(SlackTokens { bot: bot.to_owned(), app: app.to_owned() })
    } else {
        None
    }
}

fn slack_web(opts: &SlackOptions, token: &str, method: &str, args: &[(&str, Value)]) -> Result<Value, SlackError> {
    let base = opts.api.as_ref().map(|f| f()).unwrap_or_else(|| SLACK_API.to_owned());
    let url = format!("{}/api/{method}", base.trim_end_matches('/'));
    let mut pairs: Vec<(String, String)> = Vec::new();
    for (k, v) in args {
        let s = match v {
            Value::Null => continue,
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        pairs.push(((*k).to_owned(), s));
    }
    let form_body: String = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .finish();

    for attempt in 0..2 {
        let resp = opts
            .http
            .send(HttpRequest {
                method: "POST".into(),
                url: url.clone(),
                headers: vec![
                    ("authorization".into(), format!("Bearer {token}")),
                    ("content-type".into(), "application/x-www-form-urlencoded".into()),
                ],
                body: Some(form_body.as_bytes().to_vec()),
                timeout_secs: 15,
            })
            .map_err(|_| SlackError { error: "unreachable".into(), status: 0 })?;

        if resp.status == 429 && attempt == 0 {
            let secs = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
                .and_then(|(_, v)| v.parse::<u64>().ok())
                .unwrap_or(2)
                .min(30);
            std::thread::sleep(Duration::from_secs(secs));
            continue;
        }
        let data: Option<Value> = serde_json::from_slice(&resp.body).ok();
        if let Some(d) = data {
            if d["ok"] == true {
                return Ok(d);
            }
            let err = d["error"]
                .as_str()
                .map(|s| s.chars().take(80).collect())
                .unwrap_or_else(|| format!("http_{}", resp.status));
            return Err(SlackError { error: err, status: resp.status });
        }
        return Err(SlackError { error: format!("http_{}", resp.status), status: resp.status });
    }
    Err(SlackError { error: "rate_limited".into(), status: 429 })
}

fn allowed_ws_url(url: &str, local: bool) -> bool {
    if local {
        url.starts_with("ws://127.0.0.1:") || url.starts_with("ws://localhost:") || url.starts_with("wss://127.0.0.1:") || url.starts_with("wss://localhost:")
    } else {
        url.starts_with("wss://") && url.contains(".slack.com/")
    }
}

struct SocketState {
    socket: Option<Arc<dyn WsConnection>>,
    conn_id: u64,
    hello_conn: Option<u64>,
    backoff_ms: u64,
}

struct SlackTransport {
    tokens: SlackTokens,
    bot_id: String,
    options: SlackOptions,
    events: Arc<dyn ChatEvents>,
    local: bool,
    stopped: AtomicBool,
    sock: Mutex<SocketState>,
}

struct WsHandler {
    transport: Arc<SlackTransport>,
    conn_id: u64,
}

impl WsCallbacks for WsHandler {
    fn on_message(&self, text: String) {
        self.transport.received(self.conn_id, &text);
    }
    fn on_close(&self, _code: u16) {
        self.transport.closed(self.conn_id);
    }
}

impl SlackTransport {
    fn start(tokens: SlackTokens, bot: &ChatBot, options: SlackOptions, events: Arc<dyn ChatEvents>) -> Arc<Self> {
        let api_url = options.api.as_ref().map(|f| f()).unwrap_or_default();
        let local = api_url.starts_with("http://127.0.0.1:") || api_url.starts_with("http://localhost:");
        let t = Arc::new(Self {
            tokens,
            bot_id: bot.id.clone(),
            options,
            events,
            local,
            stopped: AtomicBool::new(false),
            sock: Mutex::new(SocketState { socket: None, conn_id: 0, hello_conn: None, backoff_ms: 1_000 }),
        });
        let t2 = t.clone();
        std::thread::spawn(move || t2.open());
        t
    }

    fn open(self: &Arc<Self>) {
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        {
            if crate::invariants::lock(&self.sock).socket.is_some() {
                return;
            }
        }
        let url = match slack_web(&self.options, &self.tokens.app, "apps.connections.open", &[]) {
            Ok(res) => res["url"].as_str().unwrap_or("").to_owned(),
            Err(err) => {
                if REFUSED.contains(&err.error.as_str()) {
                    self.events.trouble(TOKEN_REFUSED, true);
                    return;
                }
                self.events.trouble("Slack could not be reached. Locust keeps trying.", false);
                let wait = {
                    let mut s = crate::invariants::lock(&self.sock);
                    let w = s.backoff_ms;
                    s.backoff_ms = (s.backoff_ms * 2).min(60_000);
                    w
                };
                self.later(wait);
                return;
            }
        };
        if self.stopped.load(Ordering::SeqCst) {
            return;
        }
        if !allowed_ws_url(&url, self.local) {
            self.events.trouble("Slack gave an address Locust does not trust, so it did not connect. Locust tries again.", false);
            self.later(60_000);
            return;
        }
        let conn_id = {
            let mut s = crate::invariants::lock(&self.sock);
            if s.socket.is_some() {
                return;
            }
            s.conn_id += 1;
            s.conn_id
        };
        let handler = Arc::new(WsHandler { transport: self.clone(), conn_id });
        match self.options.ws.connect(&url, handler) {
            Ok(ws) => {
                let mut s = crate::invariants::lock(&self.sock);
                if s.conn_id == conn_id && !self.stopped.load(Ordering::SeqCst) {
                    s.socket = Some(ws);
                } else {
                    ws.close(1000, "stopped");
                }
            }
            Err(_) => {
                let wait = crate::invariants::lock(&self.sock).backoff_ms;
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

    fn incoming_of(&self, event: &Value) -> ChatIncoming {
        let ts = event["ts"].as_str().unwrap_or("").to_owned();
        let thread = event["thread_ts"].as_str().filter(|&t| t != ts).map(str::to_owned);
        let subtype = event["subtype"].as_str();
        let is_file = subtype == Some("file_share")
            || event["files"].as_array().is_some_and(|a| !a.is_empty());
        let is_bot = event["bot_id"].is_string()
            || event["user"].as_str() == Some(self.bot_id.as_str())
            || subtype.is_some_and(|st| st != "file_share" && st != "thread_broadcast");
        let text = if is_file {
            None
        } else {
            event["text"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .map(slack_plain)
        };
        let sent_at = ts
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n > 0.0)
            .map(|n| (n * 1000.0) as u64)
            .unwrap_or_else(now_ms);
        ChatIncoming {
            from: event["user"].as_str().unwrap_or("").to_owned(),
            bot: is_bot,
            place: event["channel"].as_str().unwrap_or("").to_owned(),
            direct: event["channel_type"].as_str() == Some("im"),
            id: ts,
            text,
            sent_at,
            reply_to: thread.clone(),
            answer_to: thread,
            name: None,
            username: None,
            command: None,
            respond: None,
        }
    }

    fn received(self: &Arc<Self>, conn_id: u64, raw: &str) {
        let Ok(env) = serde_json::from_str::<Value>(raw) else { return };
        if let Some(eid) = env["envelope_id"].as_str() {
            if let Some(ws) = crate::invariants::lock(&self.sock).socket.clone() {
                let _ = ws.send_text(&json!({ "envelope_id": eid }).to_string());
            }
        }
        match env["type"].as_str() {
            Some("hello") => {
                {
                    let mut s = crate::invariants::lock(&self.sock);
                    s.hello_conn = Some(conn_id);
                    s.backoff_ms = 1_000;
                }
                self.events.connected();
            }
            Some("disconnect") => {
                if env["reason"].as_str() == Some("link_disabled") {
                    self.events.trouble(
                        "Socket Mode is off for the Slack app, so Locust cannot hear it. Turn it on in the app\u{2019}s settings, under Socket Mode.",
                        true,
                    );
                    return;
                }
                let old_ws = {
                    let mut s = crate::invariants::lock(&self.sock);
                    s.socket.take()
                };
                self.open();
                if let Some(ws) = old_ws {
                    ws.close(1000, "refresh");
                }
            }
            Some("events_api") => {
                let ev = &env["payload"]["event"];
                if ev["type"].as_str() == Some("message") {
                    self.events.message(self.incoming_of(ev));
                }
            }
            Some("interactive") if env["payload"]["type"].as_str() == Some("block_actions") => {
                let p = &env["payload"];
                let action = &p["actions"][0];
                let data = action["action_id"]
                    .as_str()
                    .or_else(|| action["value"].as_str())
                    .unwrap_or("")
                    .to_owned();
                self.events.press(ChatPress {
                    from: p["user"]["id"].as_str().unwrap_or("").to_owned(),
                    place: p["channel"]["id"].as_str().or_else(|| p["container"]["channel_id"].as_str()).unwrap_or("").to_owned(),
                    message: p["message"]["ts"].as_str().or_else(|| p["container"]["message_ts"].as_str()).unwrap_or("").to_owned(),
                    data,
                    answer: Arc::new(|_| {}),
                });
            }
            _ => {}
        }
    }

    fn closed(self: &Arc<Self>, conn_id: u64) {
        let (was_hello, wait_ms) = {
            let mut s = crate::invariants::lock(&self.sock);
            if s.conn_id != conn_id {
                return;
            }
            s.socket = None;
            if self.stopped.load(Ordering::SeqCst) {
                return;
            }
            let was_hello = s.hello_conn == Some(conn_id);
            let wait = if was_hello {
                500
            } else {
                let w = s.backoff_ms;
                s.backoff_ms = (s.backoff_ms * 2).min(60_000);
                w
            };
            (was_hello, wait)
        };
        if !was_hello {
            self.events.trouble("Slack could not be reached. Locust keeps trying.", false);
        }
        self.later(wait_ms);
    }

    fn failed(&self, err: &SlackError, what: &str) {
        if REFUSED.contains(&err.error.as_str()) {
            self.events.trouble(TOKEN_REFUSED, true);
        } else if err.error == "missing_scope" {
            self.events.problem("The Slack app is missing a permission Locust needs. Make it again from Locust\u{2019}s link, then paste its tokens.");
        }
        self.events.note(&format!("{what}: {}", err.error));
    }
}

impl ChatTransport for SlackTransport {
    fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        let ws = crate::invariants::lock(&self.sock).socket.take();
        if let Some(ws) = ws {
            ws.close(1000, "stopped");
        }
    }

    fn send(&self, place: &str, text: &str, extra: &ChatSendExtra) -> Option<String> {
        let buttons: Vec<_> = extra.buttons.iter().flat_map(|r| r.iter()).take(25).collect();
        let mut args: Vec<(&str, Value)> = vec![
            ("channel", json!(place)),
            ("text", json!(text)),
            ("unfurl_links", json!(false)),
            ("unfurl_media", json!(false)),
        ];
        if let Some(ref thread) = extra.answer_to {
            args.push(("thread_ts", json!(thread)));
        }
        if !buttons.is_empty() {
            let elems: Vec<Value> = buttons
                .iter()
                .map(|b| {
                    let mut obj = json!({
                        "type": "button",
                        "text": { "type": "plain_text", "text": b.label.chars().take(75).collect::<String>(), "emoji": true },
                        "action_id": b.data,
                        "value": b.data,
                    });
                    if b.style == Some(ButtonStyle::Allow) {
                        obj["style"] = json!("primary");
                    } else if b.style == Some(ButtonStyle::Deny) {
                        obj["style"] = json!("danger");
                    }
                    obj
                })
                .collect();
            let blocks = json!([
                { "type": "section", "text": { "type": "mrkdwn", "text": text.chars().take(3_000).collect::<String>() } },
                { "type": "actions", "elements": elems },
            ]);
            args.push(("blocks", blocks));
        }
        match slack_web(&self.options, &self.tokens.bot, "chat.postMessage", &args) {
            Ok(res) => res["ts"].as_str().map(str::to_owned),
            Err(err) => {
                self.failed(&err, "a message was not sent");
                None
            }
        }
    }

    fn edit(&self, place: &str, message: &str, text: &str) {
        let args = [
            ("channel", json!(place)),
            ("ts", json!(message)),
            ("text", json!(text)),
            ("blocks", json!([])),
        ];
        if let Err(err) = slack_web(&self.options, &self.tokens.bot, "chat.update", &args) {
            self.failed(&err, "a message was not changed");
        }
    }

    fn working(&self, place: &str, message: &str, on: bool) {
        let method = if on { "reactions.add" } else { "reactions.remove" };
        let _ = slack_web(
            &self.options,
            &self.tokens.bot,
            method,
            &[("channel", json!(place)), ("timestamp", json!(message)), ("name", json!("eyes"))],
        );
    }

    fn who_is(&self, user: &str) -> Option<(Option<String>, Option<String>)> {
        let res = slack_web(&self.options, &self.tokens.bot, "users.info", &[("user", json!(user))]).ok()?;
        let person = &res["user"];
        if !person.is_object() {
            return None;
        }
        let name = person["profile"]["display_name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| person["profile"]["real_name"].as_str().filter(|s| !s.is_empty()))
            .or_else(|| person["real_name"].as_str().filter(|s| !s.is_empty()))
            .or_else(|| person["name"].as_str().filter(|s| !s.is_empty()))
            .map(str::to_owned);
        let username = person["name"].as_str().map(str::to_owned);
        Some((name, username))
    }
}

pub struct SlackApp {
    options: SlackOptions,
    words: ChatWords,
}

impl SlackApp {
    pub fn new(options: SlackOptions) -> Arc<Self> {
        Arc::new(Self {
            options,
            words: ChatWords {
                address: |_, name| format!("\u{201c}{name}:\u{201d}"),
                own: |cmd| format!("\u{201c}{cmd}\u{201d}"),
                who_line: |_, name, role| format!("*{}*, {}", slack_escape(name), slack_escape(role)),
                carry_on: "Reply in an answer\u{2019}s thread to carry on that conversation.",
                reply_to_card_or_own: "Or reply in this message\u{2019}s thread with an answer of your own.",
                reply_to_card_only: "Reply in this message\u{2019}s thread with your answer.",
                wrong_code: "That code does not pair this account. Make a new one in Locust: Settings > General > Chat apps.",
                help_more: None,
                no_token: "Paste the app\u{2019}s two tokens first.",
                unreadable: "The Slack app\u{2019}s tokens could not be read on this computer. Paste them again.",
            },
        })
    }
}

impl ChatApp for SlackApp {
    fn id(&self) -> ChatAppId {
        ChatAppId::Slack
    }
    fn file(&self) -> &'static str {
        SLACK_FILE
    }
    fn escape(&self, text: &str) -> String {
        slack_escape(text)
    }
    fn bold(&self, text: &str) -> String {
        format!("*{}*", slack_escape(text))
    }
    fn italic(&self, text: &str) -> String {
        format!("_{}_", slack_escape(text))
    }
    fn answer_parts(&self, name: &str, markdown: &str) -> Vec<String> {
        slack_messages(name, markdown, SLACK_MESSAGE_MAX)
    }
    fn parse(&self, text: &str, _bot: &ChatBot) -> Option<ParsedCommand> {
        let t = text.trim();
        let lower = t.to_lowercase();
        let stripped = lower.trim_end_matches(['.', '!', '?']);
        if matches!(stripped, "who" | "new" | "waiting" | "help") && lower.len() <= stripped.len() + 1 {
            return Some(ParsedCommand { command: stripped.to_owned(), rest: String::new() });
        }
        if let Some(after) = t.strip_prefix("new").or_else(|| t.strip_prefix("NEW")).or_else(|| t.strip_prefix("New")) {
            let after_ws = after.trim_start();
            if let Some(rest) = after_ws.strip_prefix(':') {
                let r = rest.trim();
                if !r.is_empty() {
                    return Some(ParsedCommand { command: "new".into(), rest: r.to_owned() });
                }
            }
        }
        None
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
        let open = match (bot.extra.get("app"), bot.extra.get("teamId")) {
            (Some(app), Some(team_id)) => {
                let a: String = url::form_urlencoded::byte_serialize(app.as_bytes()).collect();
                let t: String = url::form_urlencoded::byte_serialize(team_id.as_bytes()).collect();
                Some(format!("https://slack.com/app_redirect?app={a}&team={t}"))
            }
            _ => None,
        };
        ChatAppBot {
            username: bot.username.clone(),
            name: bot.name.clone(),
            invite: None,
            team: bot.extra.get("team").cloned(),
            open,
        }
    }
    fn shape(&self, given: &TokenInput) -> Result<String, String> {
        let (bot, app) = match given {
            TokenInput::Slack(t) => (t.bot.trim(), t.app.trim()),
            _ => ("", ""),
        };
        if is_slack_app_token(bot) && is_slack_bot_token(app) {
            return Err("The two tokens are in each other\u{2019}s places: the bot\u{2019}s begins xoxb-, the app\u{2019}s begins xapp-.".into());
        }
        if !is_slack_bot_token(bot) {
            return Err("That is not the bot\u{2019}s token. In your Slack app, open OAuth & Permissions, and copy the Bot User OAuth Token: it begins xoxb-.".into());
        }
        if !is_slack_app_token(app) {
            return Err("That is not the app\u{2019}s own token. In Basic Information, under App-Level Tokens, make one with the connections:write scope, and copy it: it begins xapp-.".into());
        }
        Ok(json!({ "bot": bot, "app": app }).to_string())
    }
    fn verify(&self, secret: &str) -> Result<ChatBot, String> {
        let tokens = tokens_of(secret).ok_or_else(|| "Those are not the app\u{2019}s tokens.".to_owned())?;
        let who = slack_web(&self.options, &tokens.bot, "auth.test", &[]).map_err(|err| -> String {
            if REFUSED.contains(&err.error.as_str()) {
                "Slack does not know that bot token. Copy it again from the app\u{2019}s OAuth & Permissions page: install the app to your workspace first.".into()
            } else {
                "Slack could not be reached. Check the connection, then try again.".into()
            }
        })?;
        let opened = slack_web(&self.options, &tokens.app, "apps.connections.open", &[]).map_err(|err| {
            if REFUSED.contains(&err.error.as_str()) {
                "Slack does not take that app token. Make it under Basic Information > App-Level Tokens, with the connections:write scope.".into()
            } else if err.status != 0 {
                format!("Slack would not open a connection for the app ({}). Check that Socket Mode is on in the app\u{2019}s settings.", err.error)
            } else {
                "Slack could not be reached. Check the connection, then try again.".into()
            }
        })?;
        let user_id = who["user_id"]
            .as_str()
            .ok_or_else(|| "That token is not a bot\u{2019}s. Copy the Bot User OAuth Token, which begins xoxb-.".to_owned())?;
        let app_id = opened["url"]
            .as_str()
            .and_then(|u| url::Url::parse(u).ok())
            .and_then(|u| u.query_pairs().find(|(k, _)| k == "app_id").map(|(_, v)| v.into_owned()));
        let mut extra = BTreeMap::new();
        if let Some(team) = who["team"].as_str() {
            extra.insert("team".into(), team.to_owned());
        }
        if let Some(tid) = who["team_id"].as_str() {
            extra.insert("teamId".into(), tid.to_owned());
        }
        if let Some(aid) = app_id {
            extra.insert("app".into(), aid);
        }
        let user_name = who["user"].as_str().unwrap_or("locust");
        Ok(ChatBot {
            id: user_id.to_owned(),
            username: user_name.to_owned(),
            name: user_name.to_owned(),
            extra,
        })
    }
    fn keeps(&self, secret: &str) -> bool {
        tokens_of(secret).is_some()
    }
    fn connect(&self, secret: &str, bot: &ChatBot, events: Arc<dyn ChatEvents>) -> Arc<dyn ChatTransport> {
        let tokens = tokens_of(secret).expect("valid slack tokens");
        SlackTransport::start(tokens, bot, self.options.clone(), events)
    }
    fn words(&self) -> &ChatWords {
        &self.words
    }
}

pub fn create_slack_link(slack_options: SlackOptions, link_options: ChatLinkOptions) -> Arc<ChatLink> {
    create_chat_link(SlackApp::new(slack_options), link_options)
}

#[cfg(test)]
#[path = "chat_slack_tests.rs"]
mod parity_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_http::HttpResponse;
    use crate::chat_link::{ChatStart, ChatTeammate};
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

    struct FakeSlack {
        bot_token: String,
        app_token: String,
        posted: Mutex<Vec<String>>,
        ws_sent: Mutex<Vec<Value>>,
        callbacks: Mutex<Option<Arc<dyn WsCallbacks>>>,
    }

    impl FakeSlack {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                bot_token: ["xoxb", "1000000000001", "2000000000002", &"fake".repeat(6)].join("-"),
                app_token: ["xapp", "1", "A0LOCUST01", "3000000000003", &"fake".repeat(5)].join("-"),
                posted: Mutex::new(Vec::new()),
                ws_sent: Mutex::new(Vec::new()),
                callbacks: Mutex::new(None),
            })
        }
    }

    impl HttpClient for FakeSlack {
        fn send(&self, req: HttpRequest) -> Result<HttpResponse, String> {
            let method = req.url.rsplit('/').next().unwrap_or("");
            let resp = match method {
                "auth.test" => json!({"ok": true, "user_id": "U0BOT01", "user": "locust", "team": "Acme", "team_id": "T01"}),
                "apps.connections.open" => json!({"ok": true, "url": "ws://127.0.0.1:9998/?app_id=A01"}),
                "chat.postMessage" => {
                    self.posted.lock().unwrap().push(String::from_utf8_lossy(req.body.as_deref().unwrap_or(&[])).into_owned());
                    json!({"ok": true, "ts": "1700000000.000100"})
                }
                "users.info" => json!({"ok": true, "user": {"name": "colin", "profile": {"display_name": "Colin"}}}),
                _ => json!({"ok": true}),
            };
            Ok(HttpResponse { status: 200, headers: vec![], body: resp.to_string().into_bytes() })
        }
    }

    struct FakeWsConn {
        sent: Arc<FakeSlack>,
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

    impl WsClient for FakeSlack {
        fn connect(&self, _url: &str, callbacks: Arc<dyn WsCallbacks>) -> Result<Arc<dyn WsConnection>, String> {
            *self.callbacks.lock().unwrap() = Some(callbacks.clone());
            callbacks.on_message(json!({"type": "hello"}).to_string());
            Ok(Arc::new(FakeWsConn {
                sent: Arc::new(FakeSlack {
                    bot_token: self.bot_token.clone(),
                    app_token: self.app_token.clone(),
                    posted: Mutex::new(Vec::new()),
                    ws_sent: Mutex::new(Vec::new()),
                    callbacks: Mutex::new(None),
                }),
            }))
        }
    }

    #[test]
    fn refuses_swapped_tokens_and_pairs_by_code_and_routes_named_message() {
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let dir = std::env::temp_dir().join(format!("locust-sl-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::create_dir_all(&dir);
        let fake = FakeSlack::new();
        let started = Arc::new(Mutex::new(Vec::new()));
        let link = create_slack_link(
            SlackOptions { http: fake.clone(), ws: fake.clone(), api: Some(Arc::new(|| "http://127.0.0.1:1".into())) },
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
        let swapped = link.set_token(TokenInput::Slack(SlackTokens { bot: fake.app_token.clone(), app: fake.bot_token.clone() }));
        assert!(swapped.message.unwrap().contains("each other\u{2019}s places"));

        let ok = link.set_token(TokenInput::Slack(SlackTokens { bot: fake.bot_token.clone(), app: fake.app_token.clone() }));
        let code = ok.code.unwrap().code;
        for _ in 0..30 {
            if fake.callbacks.lock().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let cb = fake.callbacks.lock().unwrap().clone().unwrap();
        let now_ts = format!("{}.000001", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
        cb.on_message(json!({
            "type": "events_api",
            "envelope_id": "env_1",
            "payload": { "event": { "type": "message", "channel": "D01", "channel_type": "im", "user": "U01", "text": code, "ts": now_ts } }
        }).to_string());
        assert_eq!(link.settings().chat.unwrap().name, "Colin");

        let now_ts2 = format!("{}.000002", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
        cb.on_message(json!({
            "type": "events_api",
            "envelope_id": "env_2",
            "payload": { "event": { "type": "message", "channel": "D01", "channel_type": "im", "user": "U01", "text": "Ada: check the build", "ts": now_ts2 } }
        }).to_string());
        assert_eq!(started.lock().unwrap().as_slice(), &[("t_ada".into(), "check the build".into())]);
        link.dispose();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
