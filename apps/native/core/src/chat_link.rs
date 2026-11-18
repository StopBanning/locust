//! Shared chat-app link logic (Electron `main/chat-link.ts`, 0.831-0.832): what Discord, Slack, and Telegram
//! share — pairing one direct chat, routing messages to teammates, sending answers back, raising and settling
//! approval/question cards with Allow once / Deny, and keeping the encrypted token and state on disk.

use crate::approval::{Answer, Decision, Kind, Request};
use crate::chat_apps::{
    CHAT_LINK_MS, CHAT_STALE_MS, ChatAppBot, ChatAppChat, ChatAppCode, ChatAppId, ChatAppLink, ChatAppSettings,
    ChatTeammateRef, SlackTokens, addressed_teammate, teammate_commands,
};
use crate::command_plain::plain_command;
use crate::own_models::SecretBox;
use crate::protocol_tags::unwrap_protocol_tags;
use crate::secrets::scrub_secrets;
use chrono::{DateTime, Local, SecondsFormat, Utc};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The longest answer Locust reads for the chat; each app's own limits cut it further.
pub const MAX_ANSWER: usize = 15_000;
pub const MAX_ANSWERS_KEPT: usize = 200;
pub const MAX_CARDS_KEPT: usize = 50;
pub const MAX_TURNS_KEPT: usize = 20;
/// A turn answered here, whose carry-on or check-fix still counts as from here, for this long.
pub const SUCCESSOR_MS: u64 = 30 * 60_000;
pub const HELD_MS: u64 = 10 * 60_000;
/// After this many wrong codes sent to the bot, the code shown in Settings stops working and a new one is made.
pub const WRONG_CODES: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatTeammate {
    pub teammate_id: String,
    pub name: String,
    pub role: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMission {
    pub mission_id: String,
    pub phase: String,
    pub events: Vec<Value>,
    pub started_by_kind: Option<String>,
    pub continues_from_mission_id: Option<String>,
    pub host_failure_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatStart {
    Ok { mission_id: String },
    Err { message: String },
}

#[derive(Clone, Debug)]
pub struct ChatAlert {
    pub cards: Vec<(Request, Option<String>)>,
    pub lines: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatBot {
    pub id: String,
    pub username: String,
    pub name: String,
    pub extra: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCommand {
    pub command: String,
    pub rest: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonStyle {
    Allow,
    Deny,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatButton {
    pub label: String,
    pub data: String,
    pub style: Option<ButtonStyle>,
}

pub type ChatButtons = Vec<Vec<ChatButton>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatSendExtra {
    pub answer_to: Option<String>,
    pub continued: bool,
    pub buttons: ChatButtons,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatCommand {
    pub command: String,
    pub description: String,
    pub words: Option<String>,
}

#[derive(Clone, Debug)]
pub enum TokenInput {
    Single(String),
    Slack(SlackTokens),
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettledHow {
    Allowed,
    Denied,
    Answered,
    Gone,
}

pub type RespondFn = Arc<dyn Fn(&str, Option<&ChatButtons>) -> Option<String> + Send + Sync>;
pub type PressAnswerFn = Arc<dyn Fn(Option<&str>) + Send + Sync>;

pub struct ChatIncoming {
    pub from: String,
    pub bot: bool,
    pub place: String,
    pub direct: bool,
    pub id: String,
    pub text: Option<String>,
    pub sent_at: u64,
    pub reply_to: Option<String>,
    pub answer_to: Option<String>,
    pub name: Option<String>,
    pub username: Option<String>,
    pub command: Option<ParsedCommand>,
    pub respond: Option<RespondFn>,
}

pub struct ChatPress {
    pub from: String,
    pub place: String,
    pub message: String,
    pub data: String,
    pub answer: PressAnswerFn,
}

pub trait ChatEvents: Send + Sync {
    fn message(&self, incoming: ChatIncoming);
    fn press(&self, press: ChatPress);
    fn connected(&self);
    fn trouble(&self, message: &str, stop: bool);
    fn problem(&self, message: &str);
    fn note(&self, detail: &str);
    fn offset(&self) -> Option<i64>;
    fn set_offset(&self, offset: i64);
}

pub trait ChatTransport: Send + Sync {
    fn stop(&self);
    fn send(&self, place: &str, text: &str, extra: &ChatSendExtra) -> Option<String>;
    fn edit(&self, place: &str, message: &str, text: &str);
    fn typing(&self, _place: &str) {}
    fn has_typing(&self) -> bool {
        false
    }
    fn typing_ms(&self) -> Option<u64> {
        None
    }
    fn working(&self, _place: &str, _message: &str, _on: bool) {}
    fn commands(&self, _list: &[ChatCommand]) -> Result<(), String> {
        Ok(())
    }
    fn who_is(&self, _user: &str) -> Option<(Option<String>, Option<String>)> {
        None
    }
}

pub struct ChatWords {
    pub address: fn(&str, &str) -> String,
    pub own: fn(&str) -> String,
    pub who_line: fn(&str, &str, &str) -> String,
    pub carry_on: &'static str,
    pub reply_to_card_or_own: &'static str,
    pub reply_to_card_only: &'static str,
    pub wrong_code: &'static str,
    pub help_more: Option<&'static str>,
    pub no_token: &'static str,
    pub unreadable: &'static str,
}

pub trait ChatApp: Send + Sync {
    fn id(&self) -> ChatAppId;
    fn file(&self) -> &'static str;
    fn escape(&self, text: &str) -> String;
    fn bold(&self, text: &str) -> String;
    fn italic(&self, text: &str) -> String;
    fn answer_parts(&self, name: &str, markdown: &str) -> Vec<String>;
    fn parse(&self, text: &str, bot: &ChatBot) -> Option<ParsedCommand>;
    fn code_in(&self, text: &str, bot: &ChatBot) -> Option<String>;
    fn fresh_code(&self) -> String;
    fn pairing(&self, bot: &ChatBot, code: &str, until: &str) -> (Option<ChatAppLink>, Option<ChatAppCode>);
    fn shown(&self, bot: &ChatBot) -> ChatAppBot;
    fn shape(&self, given: &TokenInput) -> Result<String, String>;
    fn verify(&self, secret: &str) -> Result<ChatBot, String>;
    fn keeps(&self, secret: &str) -> bool;
    fn connect(&self, secret: &str, bot: &ChatBot, events: Arc<dyn ChatEvents>) -> Arc<dyn ChatTransport>;
    fn forget(&self, _secret: &str, _bot: &ChatBot) {}
    fn words(&self) -> &ChatWords;
}

pub struct ChatLinkOptions {
    pub directory: PathBuf,
    pub secrets: Arc<dyn SecretBox>,
    pub refusal: Arc<dyn Fn() -> Option<String> + Send + Sync>,
    pub computer: Arc<dyn Fn() -> String + Send + Sync>,
    pub team: Arc<dyn Fn() -> Vec<ChatTeammate> + Send + Sync>,
    pub start: Arc<dyn Fn(&str, &str, Option<&str>) -> ChatStart + Send + Sync>,
    pub read: Arc<dyn Fn(&str) -> Option<ChatMission> + Send + Sync>,
    pub live: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    pub carried: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    pub waiting: Arc<dyn Fn() -> Vec<(Request, Option<String>)> + Send + Sync>,
    pub decide: Arc<dyn Fn(Answer) -> bool + Send + Sync>,
    pub changed: Option<Arc<dyn Fn(ChatAppSettings) + Send + Sync>>,
    pub note: Option<Arc<dyn Fn(&str, &str) + Send + Sync>>,
    pub now: Option<Arc<dyn Fn() -> u64 + Send + Sync>>,
    pub typing_ms: Option<u64>,
}

#[derive(Clone, Debug)]
struct SavedChat {
    place: String,
    user: String,
    name: String,
    username: Option<String>,
    paired_at: String,
}

#[derive(Clone, Debug)]
struct SavedTurn {
    mission_id: String,
    teammate_id: String,
    answer_to: Option<String>,
    asked: Option<String>,
    at: u64,
    carried: bool,
}

#[derive(Clone, Debug)]
struct SavedQuestion {
    id: String,
    options: Vec<String>,
    words: bool,
}

#[derive(Clone, Debug)]
struct SavedCard {
    key: String,
    approval_id: String,
    mission_id: String,
    message_id: String,
    text: String,
    question: Option<SavedQuestion>,
}

#[derive(Clone, Debug)]
struct SavedAnswer {
    message_id: String,
    mission_id: String,
    teammate_id: Option<String>,
}

#[derive(Clone, Debug)]
struct SavedAnswered {
    mission_id: String,
    at: u64,
}

#[derive(Clone, Debug)]
struct Saved {
    token: String,
    bot: ChatBot,
    chat: Option<SavedChat>,
    offset: Option<i64>,
    current: Option<String>,
    fresh: Vec<String>,
    conversations: BTreeMap<String, String>,
    answers: Vec<SavedAnswer>,
    turns: Vec<SavedTurn>,
    cards: Vec<SavedCard>,
    answered: Vec<SavedAnswered>,
    alerts: bool,
}

struct LinkCode {
    code: String,
    until: u64,
}

struct HeldMessage {
    text: String,
    origin_id: String,
    origin_answer_to: Option<String>,
    at: u64,
    choices: Vec<String>,
}

struct State {
    saved: Option<Saved>,
    secret: Option<String>,
    link: Option<LinkCode>,
    wrong_codes: usize,
    connected: bool,
    message: Option<String>,
    transport: Option<Arc<dyn ChatTransport>>,
    commands_set: String,
    commands_at: u64,
    held: Option<HeldMessage>,
    answering_here: HashSet<String>,
    posting: HashSet<String>,
    wrong_code_at: HashMap<String, u64>,
    seen: Vec<String>,
    ticker_stop: Option<Arc<AtomicBool>>,
    typing_stop: Option<Arc<AtomicBool>>,
    disposed: bool,
}

pub struct ChatLink {
    app: Arc<dyn ChatApp>,
    options: ChatLinkOptions,
    file: PathBuf,
    state: Mutex<State>,
    turn_lock: Mutex<()>,
    persist_lock: Mutex<()>,
}

struct EventsAdapter {
    link: std::sync::Weak<ChatLink>,
}

impl ChatEvents for EventsAdapter {
    fn message(&self, incoming: ChatIncoming) {
        if let Some(link) = self.link.upgrade() {
            link.handle_message(incoming);
        }
    }

    fn press(&self, press: ChatPress) {
        if let Some(link) = self.link.upgrade() {
            link.handle_press(press);
        }
    }

    fn connected(&self) {
        if let Some(link) = self.link.upgrade() {
            let mut s = crate::invariants::lock(&link.state);
            if !s.connected || s.message.is_some() {
                s.connected = true;
                s.message = None;
                let settings = link.settings_from(&s);
                drop(s);
                link.emit_changed(settings);
            }
        }
    }

    fn trouble(&self, msg: &str, stop: bool) {
        if let Some(link) = self.link.upgrade() {
            let transport_to_stop = {
                let mut s = crate::invariants::lock(&link.state);
                s.connected = false;
                s.message = Some(msg.to_owned());
                if stop {
                    if let Some(flag) = s.ticker_stop.take() {
                        flag.store(true, Ordering::SeqCst);
                    }
                    s.transport.take()
                } else {
                    None
                }
            };
            if let Some(t) = transport_to_stop {
                t.stop();
            }
            let settings = link.settings();
            link.emit_changed(settings);
        }
    }

    fn problem(&self, msg: &str) {
        if let Some(link) = self.link.upgrade() {
            let settings = {
                let mut s = crate::invariants::lock(&link.state);
                s.message = Some(msg.to_owned());
                link.settings_from(&s)
            };
            link.emit_changed(settings);
        }
    }

    fn note(&self, detail: &str) {
        if let Some(link) = self.link.upgrade() {
            link.emit_note(detail);
        }
    }

    fn offset(&self) -> Option<i64> {
        let link = self.link.upgrade()?;
        let s = crate::invariants::lock(&link.state);
        s.saved.as_ref().and_then(|sv| sv.offset)
    }

    fn set_offset(&self, offset: i64) {
        if let Some(link) = self.link.upgrade() {
            let snapshot = {
                let mut s = crate::invariants::lock(&link.state);
                if let Some(sv) = s.saved.as_mut() {
                    sv.offset = Some(offset);
                }
                s.saved.clone()
            };
            let _ = link.persist_snapshot(snapshot);
        }
    }
}

pub fn create_chat_link(app: Arc<dyn ChatApp>, options: ChatLinkOptions) -> Arc<ChatLink> {
    let file = options.directory.join(app.file());
    Arc::new(ChatLink {
        app,
        options,
        file,
        state: Mutex::new(State {
            saved: None,
            secret: None,
            link: None,
            wrong_codes: 0,
            connected: false,
            message: None,
            transport: None,
            commands_set: String::new(),
            commands_at: 0,
            held: None,
            answering_here: HashSet::new(),
            posting: HashSet::new(),
            wrong_code_at: HashMap::new(),
            seen: Vec::new(),
            ticker_stop: None,
            typing_stop: None,
            disposed: false,
        }),
        turn_lock: Mutex::new(()),
        persist_lock: Mutex::new(()),
    })
}

/// The first non-empty line of `detail` (capped at 200 chars, secrets scrubbed), or None when `summary` already has it.
pub fn exact_line(summary: &str, detail: &str) -> Option<String> {
    let exact = detail.lines().find(|line| !line.trim().is_empty())?.trim();
    if summary.contains(exact) {
        return None;
    }
    let cut = if exact.chars().count() > 200 {
        let prefix: String = exact.chars().take(200).collect();
        format!("{prefix}…")
    } else {
        exact.to_owned()
    };
    Some(scrub_secrets(&cut).0)
}

/// The answer a turn gave, as the chat is told it: its last whole message, the reason it failed, or that it stopped.
pub fn chat_answer(mission: &ChatMission, name: &str) -> String {
    if mission.phase == "failed" {
        let event_why = mission
            .events
            .iter()
            .rev()
            .find(|ev| ev["type"] == "run.failed")
            .and_then(|ev| ev["payload"]["message"].as_str());
        let why = mission.host_failure_message.as_deref().or(event_why);
        return match why {
            Some(w) => format!("{name}\u{2019}s turn failed: {w}"),
            None => format!("{name}\u{2019}s turn failed."),
        };
    }
    if mission.phase != "completed" {
        return format!("{name}\u{2019}s turn stopped before it finished. What it did is in Locust.");
    }

    // Fold message.delta fragments by itemId, matching joinMessageFragments.
    let mut joined: Vec<(String, String, bool)> = Vec::new();
    for ev in &mission.events {
        if ev["type"] != "message.delta" {
            continue;
        }
        let item_id = ev["payload"]["itemId"].as_str().unwrap_or("").to_owned();
        let op = ev["payload"]["operation"].as_str().unwrap_or("append");
        let text = ev["payload"]["text"].as_str().unwrap_or("");
        let is_final = ev["payload"]["final"].as_bool().unwrap_or(false);
        if let Some(existing) = joined.iter_mut().rev().find(|(id, _, _)| *id == item_id) {
            if op == "replace" {
                existing.1 = text.to_owned();
            } else {
                existing.1.push_str(text);
            }
            existing.2 = is_final;
        } else {
            joined.push((item_id, text.to_owned(), is_final));
        }
    }
    let final_text = joined
        .iter()
        .rev()
        .find(|(_, _, is_final)| *is_final)
        .map(|(_, text, _)| unwrap_protocol_tags(text).trim().to_owned())
        .unwrap_or_default();

    if final_text.is_empty() {
        return format!("{name} finished without a written answer. What it did is in Locust.");
    }
    if final_text.chars().count() > MAX_ANSWER {
        let prefix: String = final_text.chars().take(MAX_ANSWER).collect();
        format!("{prefix}…")
    } else {
        final_text
    }
}

pub fn random_base64url(bytes_len: usize) -> String {
    let mut buf = vec![0u8; bytes_len];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    }
    encode_base64url(&buf)
}

fn encode_base64url(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4 + 2) / 3);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(TABLE[b0 >> 2] as char);
        out.push(TABLE[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[b2 & 0x3f] as char);
        }
    }
    out
}

fn now_ms_default() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn iso_at(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    DateTime::<Utc>::from_timestamp(secs, nanos)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap())
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn local_time_at(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    DateTime::<Utc>::from_timestamp(secs, nanos)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap())
        .with_timezone(&Local)
        .format("%-I:%M %p")
        .to_string()
}

fn json_id(v: &Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        Some(s.to_owned())
    } else if let Some(n) = v.as_i64() {
        Some(n.to_string())
    } else {
        v.as_u64().map(|n| n.to_string())
    }
}

fn migrated(raw: &Value) -> Option<Saved> {
    if raw["schemaVersion"].as_u64() != Some(1) {
        return None;
    }
    let token = raw["token"].as_str()?.to_owned();
    let bot_val = &raw["bot"];
    let username = bot_val["username"].as_str()?.to_owned();
    let bot_id = json_id(&bot_val["id"]).unwrap_or_default();
    let bot_name = bot_val["name"].as_str().unwrap_or("").to_owned();
    let mut extra = BTreeMap::new();
    if let Some(obj) = bot_val["extra"].as_object() {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                extra.insert(k.clone(), s.to_owned());
            }
        }
    }
    let bot = ChatBot { id: bot_id, username, name: bot_name, extra };

    let chat = if raw["chat"].is_object() {
        let c = &raw["chat"];
        Some(SavedChat {
            place: json_id(&c["place"]).or_else(|| json_id(&c["id"])).unwrap_or_default(),
            user: json_id(&c["user"]).or_else(|| json_id(&c["userId"])).unwrap_or_default(),
            name: c["name"].as_str().unwrap_or("Your chat").to_owned(),
            username: c["username"].as_str().map(str::to_owned),
            paired_at: c["pairedAt"].as_str().map(str::to_owned).unwrap_or_else(|| iso_at(0)),
        })
    } else {
        None
    };

    let offset = raw["offset"].as_i64();
    let current = raw["current"].as_str().map(str::to_owned);
    let fresh = raw["fresh"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();

    let mut conversations = BTreeMap::new();
    if let Some(obj) = raw["conversations"].as_object() {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                conversations.insert(k.clone(), s.to_owned());
            }
        }
    }

    let answers = raw["answers"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|one| SavedAnswer {
                    message_id: json_id(&one["messageId"]).unwrap_or_default(),
                    mission_id: one["missionId"].as_str().unwrap_or("").to_owned(),
                    teammate_id: one["teammateId"].as_str().map(str::to_owned),
                })
                .collect()
        })
        .unwrap_or_default();

    let turns = raw["turns"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|turn| SavedTurn {
                    mission_id: turn["missionId"].as_str().unwrap_or("").to_owned(),
                    teammate_id: turn["teammateId"].as_str().unwrap_or("").to_owned(),
                    answer_to: json_id(&turn["answerTo"]).or_else(|| json_id(&turn["replyTo"])),
                    asked: json_id(&turn["asked"]).or_else(|| json_id(&turn["replyTo"])),
                    at: turn["at"].as_u64().unwrap_or(0),
                    carried: turn["carried"] == true,
                })
                .collect()
        })
        .unwrap_or_default();

    let cards = raw["cards"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|one| {
                    let question = if one["question"].is_object() {
                        let q = &one["question"];
                        Some(SavedQuestion {
                            id: q["id"].as_str().unwrap_or("").to_owned(),
                            options: q["options"]
                                .as_array()
                                .map(|opts| opts.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
                                .unwrap_or_default(),
                            words: q["words"] == true,
                        })
                    } else {
                        None
                    };
                    SavedCard {
                        key: one["key"].as_str().unwrap_or("").to_owned(),
                        approval_id: one["approvalId"].as_str().unwrap_or("").to_owned(),
                        mission_id: one["missionId"].as_str().unwrap_or("").to_owned(),
                        message_id: json_id(&one["messageId"]).unwrap_or_default(),
                        text: one["text"].as_str().or_else(|| one["html"].as_str()).unwrap_or("").to_owned(),
                        question,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let answered = raw["answered"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|one| SavedAnswered {
                    mission_id: one["missionId"].as_str().unwrap_or("").to_owned(),
                    at: one["at"].as_u64().unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();

    let alerts = raw["alerts"] == true;

    Some(Saved {
        token,
        bot,
        chat,
        offset,
        current,
        fresh,
        conversations,
        answers,
        turns,
        cards,
        answered,
        alerts,
    })
}

fn saved_to_json(s: &Saved) -> Value {
    let mut bot_obj = json!({
        "id": s.bot.id,
        "username": s.bot.username,
        "name": s.bot.name,
    });
    if !s.bot.extra.is_empty() {
        bot_obj["extra"] = json!(s.bot.extra);
    }
    let mut out = json!({
        "schemaVersion": 1,
        "token": s.token,
        "bot": bot_obj,
    });
    if let Some(chat) = &s.chat {
        let mut c = json!({
            "place": chat.place,
            "user": chat.user,
            "name": chat.name,
            "pairedAt": chat.paired_at,
        });
        if let Some(u) = &chat.username {
            c["username"] = json!(u);
        }
        out["chat"] = c;
    }
    if let Some(offset) = s.offset {
        out["offset"] = json!(offset);
    }
    if let Some(current) = &s.current {
        out["current"] = json!(current);
    }
    if !s.fresh.is_empty() {
        out["fresh"] = json!(s.fresh);
    }
    if !s.conversations.is_empty() {
        out["conversations"] = json!(s.conversations);
    }
    if !s.answers.is_empty() {
        out["answers"] = json!(s.answers.iter().map(|a| {
            let mut obj = json!({"messageId": a.message_id, "missionId": a.mission_id});
            if let Some(t) = &a.teammate_id {
                obj["teammateId"] = json!(t);
            }
            obj
        }).collect::<Vec<_>>());
    }
    if !s.turns.is_empty() {
        out["turns"] = json!(s.turns.iter().map(|t| {
            let mut obj = json!({"missionId": t.mission_id, "teammateId": t.teammate_id, "at": t.at});
            if let Some(to) = &t.answer_to {
                obj["answerTo"] = json!(to);
            }
            if let Some(asked) = &t.asked {
                obj["asked"] = json!(asked);
            }
            if t.carried {
                obj["carried"] = json!(true);
            }
            obj
        }).collect::<Vec<_>>());
    }
    if !s.cards.is_empty() {
        out["cards"] = json!(s.cards.iter().map(|c| {
            let mut obj = json!({
                "key": c.key,
                "approvalId": c.approval_id,
                "missionId": c.mission_id,
                "messageId": c.message_id,
                "text": c.text,
            });
            if let Some(q) = &c.question {
                obj["question"] = json!({"id": q.id, "options": q.options, "words": q.words});
            }
            obj
        }).collect::<Vec<_>>());
    }
    if !s.answered.is_empty() {
        out["answered"] = json!(s.answered.iter().map(|a| json!({"missionId": a.mission_id, "at": a.at})).collect::<Vec<_>>());
    }
    if s.alerts {
        out["alerts"] = json!(true);
    }
    out
}

fn write_private_file(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = PathBuf::from(format!("{}.tmp", path.display()));
    let _ = std::fs::remove_file(&temp);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&temp)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&temp, path)
}

struct Replier<'a> {
    link: &'a ChatLink,
    respond: Option<RespondFn>,
    answer_to: Option<String>,
    responded: bool,
}

impl<'a> Replier<'a> {
    fn new(link: &'a ChatLink, respond: Option<RespondFn>, answer_to: Option<String>) -> Self {
        Self { link, respond, answer_to, responded: false }
    }

    fn pending(&self) -> bool {
        self.respond.is_some() && !self.responded
    }

    fn say(&mut self, text: &str, reply: bool, buttons: Option<&ChatButtons>) -> Option<String> {
        if !self.responded {
            if let Some(ref f) = self.respond {
                self.responded = true;
                return f(text, buttons);
            }
        }
        let extra = ChatSendExtra {
            answer_to: if reply { self.answer_to.clone() } else { None },
            continued: false,
            buttons: buttons.cloned().unwrap_or_default(),
        };
        self.link.send_raw(text, extra)
    }
}

impl ChatLink {
    fn now(&self) -> u64 {
        self.options.now.as_ref().map(|f| f()).unwrap_or_else(now_ms_default)
    }

    fn emit_changed(&self, settings: ChatAppSettings) {
        if let Some(ref cb) = self.options.changed {
            cb(settings);
        }
    }

    fn emit_note(&self, detail: &str) {
        if let Some(ref cb) = self.options.note {
            cb(self.app.id().as_str(), detail);
        }
    }

    fn settings_from(&self, s: &State) -> ChatAppSettings {
        let now = self.now();
        let bot = s.saved.as_ref().map(|sv| self.app.shown(&sv.bot));
        let chat = s.saved.as_ref().and_then(|sv| {
            sv.chat.as_ref().map(|c| ChatAppChat {
                name: c.name.clone(),
                username: c.username.clone(),
                paired_at: c.paired_at.clone(),
            })
        });
        let (link, code) = match (&s.saved, &s.link) {
            (Some(sv), Some(lc)) if lc.until > now => self.app.pairing(&sv.bot, &lc.code, &iso_at(lc.until)),
            _ => (None, None),
        };
        ChatAppSettings {
            bot,
            chat,
            link,
            code,
            connected: s.saved.is_some() && s.connected,
            message: s.message.clone(),
            alerts: s.saved.as_ref().filter(|sv| sv.alerts).map(|_| true),
        }
    }

    pub fn settings(&self) -> ChatAppSettings {
        let s = crate::invariants::lock(&self.state);
        self.settings_from(&s)
    }

    fn persist_snapshot(&self, snapshot: Option<Saved>) -> std::io::Result<()> {
        let _guard = crate::invariants::lock(&self.persist_lock);
        match snapshot {
            None => {
                let _ = std::fs::remove_file(&self.file);
                Ok(())
            }
            Some(sv) => write_private_file(&self.file, &saved_to_json(&sv).to_string()),
        }
    }

    pub fn keeps_key(&self) -> bool {
        let Ok(text) = std::fs::read_to_string(&self.file) else {
            return false;
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            return false;
        };
        v["token"].is_string()
    }

    pub fn load(self: &Arc<Self>) {
        let Ok(text) = std::fs::read_to_string(&self.file) else {
            return;
        };
        let Ok(parsed) = serde_json::from_str::<Value>(&text) else {
            return;
        };
        let Some(loaded) = migrated(&parsed) else {
            return;
        };
        let secret = if self.options.secrets.available() {
            self.options.secrets.open(&loaded.token).ok()
        } else {
            None
        };
        let valid_secret = secret.filter(|sec| self.app.keeps(sec));
        {
            let mut s = crate::invariants::lock(&self.state);
            s.saved = Some(loaded.clone());
            if valid_secret.is_none() {
                s.secret = None;
                s.message = Some(self.app.words().unreadable.to_owned());
                return;
            }
            s.secret = valid_secret;
        }
        if (self.options.refusal)().is_some() {
            return;
        }
        self.start_transport();
        for turn in &loaded.turns {
            if (self.options.live)(&turn.mission_id) || turn.carried {
                continue;
            }
            self.run_ended(&turn.mission_id);
        }
        if loaded.turns.iter().any(|t| (self.options.live)(&t.mission_id)) {
            self.typing_on();
        }
    }

    pub fn set_token(self: &Arc<Self>, given: TokenInput) -> ChatAppSettings {
        if let Some(refused) = (self.options.refusal)() {
            let mut out = self.settings();
            out.message = Some(refused);
            return out;
        }
        let secret = match self.app.shape(&given) {
            Ok(s) => s,
            Err(msg) => {
                let mut out = self.settings();
                out.message = Some(msg);
                return out;
            }
        };
        if !self.options.secrets.available() {
            let mut out = self.settings();
            out.message = Some("This computer cannot encrypt the token, so it was not kept.".into());
            return out;
        }
        let bot = match self.app.verify(&secret) {
            Ok(b) => b,
            Err(msg) => {
                let mut out = self.settings();
                out.message = Some(msg);
                return out;
            }
        };
        self.stop_transport();
        let sealed = match self.options.secrets.seal(&secret) {
            Ok(s) => s,
            Err(_) => {
                let mut out = self.settings();
                out.message = Some("The token could not be kept on this computer. Nothing changed.".into());
                return out;
            }
        };

        let snapshot = {
            let mut s = crate::invariants::lock(&self.state);
            let same = s.saved.as_ref().filter(|prev| prev.bot.id == bot.id).cloned();
            let mut next = same.unwrap_or_else(|| Saved {
                token: sealed.clone(),
                bot: bot.clone(),
                chat: None,
                offset: None,
                current: None,
                fresh: Vec::new(),
                conversations: BTreeMap::new(),
                answers: Vec::new(),
                turns: Vec::new(),
                cards: Vec::new(),
                answered: Vec::new(),
                alerts: false,
            });
            next.token = sealed;
            next.bot = bot;
            s.secret = Some(secret);
            s.commands_set.clear();
            s.message = None;
            if next.chat.is_none() {
                s.link = Some(LinkCode { code: self.app.fresh_code(), until: self.now() + CHAT_LINK_MS });
                s.wrong_codes = 0;
            }
            s.saved = Some(next.clone());
            Some(next)
        };

        if self.persist_snapshot(snapshot).is_err() {
            let mut s = crate::invariants::lock(&self.state);
            s.saved = None;
            s.secret = None;
            s.link = None;
            let mut out = self.settings_from(&s);
            out.message = Some("The token could not be kept on this computer. Nothing changed.".into());
            return out;
        }

        self.start_transport();
        let settings = self.settings();
        self.emit_changed(settings.clone());
        settings
    }

    pub fn new_link(&self) -> ChatAppSettings {
        let settings = {
            let mut s = crate::invariants::lock(&self.state);
            if s.saved.is_none() {
                let mut out = self.settings_from(&s);
                out.message = Some(self.app.words().no_token.to_owned());
                return out;
            }
            s.link = Some(LinkCode { code: self.app.fresh_code(), until: self.now() + CHAT_LINK_MS });
            s.wrong_codes = 0;
            self.settings_from(&s)
        };
        self.emit_changed(settings.clone());
        settings
    }

    pub fn remove(&self) -> ChatAppSettings {
        self.stop_transport();
        let (kept_saved, kept_secret, settings) = {
            let mut s = crate::invariants::lock(&self.state);
            if let Some(flag) = s.typing_stop.take() {
                flag.store(true, Ordering::SeqCst);
            }
            let ks = s.saved.take();
            let ksec = s.secret.take();
            s.link = None;
            s.message = None;
            s.held = None;
            (ks, ksec, self.settings_from(&s))
        };
        let _ = self.persist_snapshot(None);
        if let (Some(sv), Some(sec)) = (kept_saved, kept_secret) {
            self.app.forget(&sec, &sv.bot);
        }
        self.emit_changed(settings.clone());
        settings
    }

    pub fn set_alerts(&self, on: bool) -> ChatAppSettings {
        {
            let s = crate::invariants::lock(&self.state);
            if s.saved.is_none() {
                return self.settings_from(&s);
            }
        }
        if on {
            if let Some(refused) = (self.options.refusal)() {
                let mut out = self.settings();
                out.message = Some(refused);
                return out;
            }
        }
        let snapshot = {
            let mut s = crate::invariants::lock(&self.state);
            if let Some(sv) = s.saved.as_mut() {
                sv.alerts = on;
            }
            s.saved.clone()
        };
        if self.persist_snapshot(snapshot).is_err() {
            let mut s = crate::invariants::lock(&self.state);
            s.message = Some("That setting could not be saved. It lasts until Locust closes.".into());
        }
        let settings = self.settings();
        self.emit_changed(settings.clone());
        settings
    }

    pub fn alert(&self, said: ChatAlert) {
        {
            let s = crate::invariants::lock(&self.state);
            if !s.saved.as_ref().is_some_and(|sv| sv.alerts && sv.chat.is_some()) {
                return;
            }
        }
        if (self.options.refusal)().is_some() {
            return;
        }
        let _guard = crate::invariants::lock(&self.turn_lock);
        for (req, name) in &said.cards {
            self.post_card(req, name.as_deref());
        }
        if said.lines.is_empty() {
            return;
        }
        let body = said
            .lines
            .iter()
            .map(|(title, b)| format!("{}\n{}", self.app.bold(title), self.app.escape(b)))
            .collect::<Vec<_>>()
            .join("\n\n");
        let _ = self.send_raw(&body, ChatSendExtra::default());
    }

    pub fn card_raised(&self, request: &Request, name: Option<&str>) {
        {
            let s = crate::invariants::lock(&self.state);
            if !s.saved.as_ref().is_some_and(|sv| sv.chat.is_some()) {
                return;
            }
        }
        if self.from_here(&request.mission_id).is_none() {
            return;
        }
        self.post_card(request, name);
    }

    pub fn card_settled(&self, approval_id: &str, how: SettledHow) {
        {
            let s = crate::invariants::lock(&self.state);
            let has_card = s.saved.as_ref().is_some_and(|sv| sv.cards.iter().any(|c| c.approval_id == approval_id));
            if !has_card || s.answering_here.contains(approval_id) {
                return;
            }
        }
        let said = match how {
            SettledHow::Allowed => "Allowed in Locust.",
            SettledHow::Denied => "Denied in Locust.",
            SettledHow::Answered => "Answered in Locust.",
            SettledHow::Gone => "No longer asked.",
        };
        self.settle(approval_id, said);
    }

    pub fn on_run_ended(&self, mission_id: &str) {
        self.run_ended(mission_id);
    }

    pub fn dispose(&self) {
        {
            let mut s = crate::invariants::lock(&self.state);
            s.disposed = true;
            if let Some(flag) = s.typing_stop.take() {
                flag.store(true, Ordering::SeqCst);
            }
        }
        self.stop_transport();
    }

    fn stop_transport(&self) {
        let transport = {
            let mut s = crate::invariants::lock(&self.state);
            if let Some(flag) = s.ticker_stop.take() {
                flag.store(true, Ordering::SeqCst);
            }
            s.connected = false;
            s.transport.take()
        };
        if let Some(t) = transport {
            t.stop();
        }
    }

    fn start_transport(self: &Arc<Self>) {
        let (secret, bot, stop_flag) = {
            let mut s = crate::invariants::lock(&self.state);
            if s.disposed || s.transport.is_some() {
                return;
            }
            let (Some(sv), Some(sec)) = (s.saved.as_ref(), s.secret.as_ref()) else {
                return;
            };
            let sec = sec.clone();
            let bot = sv.bot.clone();
            let flag = Arc::new(AtomicBool::new(false));
            s.ticker_stop = Some(flag.clone());
            (sec, bot, flag)
        };
        let events: Arc<dyn ChatEvents> = Arc::new(EventsAdapter { link: Arc::downgrade(self) });
        let t = self.app.connect(&secret, &bot, events);
        {
            let mut s = crate::invariants::lock(&self.state);
            if s.disposed || s.transport.is_some() {
                drop(s);
                t.stop();
                return;
            }
            s.transport = Some(t);
        }
        let _ = (secret, bot);
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || {
            while !stop_flag.load(Ordering::SeqCst) {
                for _ in 0..600 {
                    if stop_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                let Some(link) = weak.upgrade() else { return };
                if let Some(refused) = (link.options.refusal)() {
                    {
                        let mut s = crate::invariants::lock(&link.state);
                        s.message = Some(refused);
                    }
                    link.stop_transport();
                    let settings = link.settings();
                    link.emit_changed(settings);
                    return;
                }
                link.refresh_commands(false);
            }
        });
    }

    fn send_raw(&self, text: &str, extra: ChatSendExtra) -> Option<String> {
        let (place, transport) = {
            let s = crate::invariants::lock(&self.state);
            (s.saved.as_ref()?.chat.as_ref()?.place.clone(), s.transport.clone()?)
        };
        transport.send(&place, text, &extra)
    }

    fn edit_raw(&self, message_id: &str, text: &str) {
        let (place, transport) = {
            let s = crate::invariants::lock(&self.state);
            let Some(place) = s.saved.as_ref().and_then(|sv| sv.chat.as_ref()).map(|c| c.place.clone()) else {
                return;
            };
            let Some(transport) = s.transport.clone() else { return };
            (place, transport)
        };
        transport.edit(&place, message_id, text);
    }

    fn ours(&self, team: &[ChatTeammate]) -> Vec<(String, String)> {
        let refs: Vec<ChatTeammateRef> = team
            .iter()
            .map(|m| ChatTeammateRef { teammate_id: m.teammate_id.clone(), name: m.name.clone() })
            .collect();
        teammate_commands(&refs)
    }

    fn refresh_commands(&self, force: bool) {
        let (transport, last_at, last_set) = {
            let mut s = crate::invariants::lock(&self.state);
            if !s.saved.as_ref().is_some_and(|sv| sv.chat.is_some()) {
                return;
            }
            let Some(t) = s.transport.clone() else { return };
            let now = self.now();
            if !force && now.saturating_sub(s.commands_at) < 60_000 {
                return;
            }
            s.commands_at = now;
            (t, s.commands_at, s.commands_set.clone())
        };
        let _ = last_at;
        let team = (self.options.team)();
        let commands = self.ours(&team);
        let mut list: Vec<ChatCommand> = team
            .iter()
            .take(90)
            .enumerate()
            .map(|(idx, mate)| ChatCommand {
                command: commands[idx].1.clone(),
                description: format!("Write to {} ({})", mate.name, mate.role).chars().take(256).collect(),
                words: Some(format!("Your message to {}", mate.name)),
            })
            .collect();
        list.push(ChatCommand { command: "who".into(), description: "Your teammates, and who you are writing to".into(), words: None });
        list.push(ChatCommand { command: "new".into(), description: "Start a new conversation with them".into(), words: Some("The first message of the new conversation".into()) });
        list.push(ChatCommand { command: "waiting".into(), description: "The cards waiting on you".into(), words: None });
        list.push(ChatCommand { command: "help".into(), description: "What you can do here".into(), words: None });

        let said = list
            .iter()
            .map(|c| format!("{}|{}|{}", c.command, c.description, c.words.as_deref().unwrap_or("")))
            .collect::<Vec<_>>()
            .join("\n");
        if said == last_set {
            return;
        }
        if transport.commands(&list).is_ok() {
            let mut s = crate::invariants::lock(&self.state);
            s.commands_set = said;
        }
    }

    fn help_text(&self, team: &[ChatTeammate]) -> String {
        let words = self.app.words();
        let commands = self.ours(team);
        let first = team.first();
        let example = match first {
            None => (words.address)("name", "Name"),
            Some(f) => (words.address)(&commands[0].1, &f.name),
        };
        let mut lines = vec![
            self.app.bold(&format!("Locust on {}", (self.options.computer)())),
            format!(
                "{} and a message writes to {}. After that, just write: it goes to the same teammate until you pick another.",
                self.app.escape(&example),
                first.map(|f| self.app.escape(&f.name)).unwrap_or_else(|| "that teammate".into())
            ),
            self.app.escape(words.carry_on),
            format!("{} starts a new conversation with whoever you are writing to.", self.app.escape(&(words.own)("new"))),
            format!(
                "{} lists your teammates. {} shows the cards waiting on you.",
                self.app.escape(&(words.own)("who")),
                self.app.escape(&(words.own)("waiting"))
            ),
        ];
        if let Some(more) = words.help_more {
            lines.push(self.app.escape(more));
        }
        lines.push("Locust answers while it is open on your computer. Approval cards here offer Allow once and Deny; Always stays in Locust.".into());
        lines.join("\n")
    }

    fn paired_from(&self, from: &str, place: &str) -> bool {
        let s = crate::invariants::lock(&self.state);
        s.saved.as_ref().and_then(|sv| sv.chat.as_ref()).is_some_and(|c| c.place == place && c.user == from)
    }

    fn pair(&self, incoming: &ChatIncoming) {
        let transport = {
            let s = crate::invariants::lock(&self.state);
            s.transport.clone()
        };
        let known = if incoming.name.is_none() {
            transport.as_ref().and_then(|t| t.who_is(&incoming.from))
        } else {
            None
        };
        let name = incoming
            .name
            .clone()
            .or_else(|| known.as_ref().and_then(|k| k.0.clone()))
            .unwrap_or_else(|| "Your chat".into());
        let username = incoming.username.clone().or_else(|| known.and_then(|k| k.1));
        let chat = SavedChat {
            place: incoming.place.clone(),
            user: incoming.from.clone(),
            name,
            username,
            paired_at: iso_at(self.now()),
        };
        let snapshot = {
            let mut s = crate::invariants::lock(&self.state);
            if let Some(sv) = s.saved.as_mut() {
                *sv = Saved {
                    token: sv.token.clone(),
                    bot: sv.bot.clone(),
                    chat: Some(chat),
                    offset: sv.offset,
                    current: None,
                    fresh: Vec::new(),
                    conversations: BTreeMap::new(),
                    answers: Vec::new(),
                    turns: Vec::new(),
                    cards: Vec::new(),
                    answered: Vec::new(),
                    alerts: sv.alerts,
                };
            }
            s.link = None;
            s.wrong_codes = 0;
            s.message = None;
            s.held = None;
            s.saved.clone()
        };
        let _ = self.persist_snapshot(snapshot);
        let settings = self.settings();
        self.emit_changed(settings);
        let team = (self.options.team)();
        let welcome = format!("Paired with Locust on {}.\n\n{}", self.app.bold(&(self.options.computer)()), self.help_text(&team));
        if let Some(ref respond) = incoming.respond {
            let _ = respond(&welcome, None);
        } else {
            let _ = self.send_raw(&welcome, ChatSendExtra::default());
        }
        self.refresh_commands(true);
    }

    fn wrong_code(&self, incoming: &ChatIncoming) {
        let now = self.now();
        let (rotated_settings, should_send, transport) = {
            let mut s = crate::invariants::lock(&self.state);
            s.wrong_codes += 1;
            let mut rotated = None;
            if s.wrong_codes >= WRONG_CODES && s.link.is_some() {
                s.link = Some(LinkCode { code: self.app.fresh_code(), until: now + CHAT_LINK_MS });
                s.wrong_codes = 0;
                s.message = Some("Too many wrong codes were sent to the bot, so Locust made a new one.".into());
                rotated = Some(self.settings_from(&s));
            }
            let last = s.wrong_code_at.get(&incoming.place).copied();
            let can_send = !last.is_some_and(|l| now.saturating_sub(l) < 60_000);
            if can_send {
                s.wrong_code_at.insert(incoming.place.clone(), now);
            }
            (rotated, can_send, s.transport.clone())
        };
        if let Some(settings) = rotated_settings {
            self.emit_changed(settings);
        }
        if !should_send {
            return;
        }
        let said = self.app.escape(self.app.words().wrong_code);
        if let Some(ref respond) = incoming.respond {
            let _ = respond(&said, None);
        } else if let Some(t) = transport {
            let _ = t.send(&incoming.place, &said, &ChatSendExtra::default());
        }
    }

    fn typing_on(self: &Arc<Self>) {
        let (stop_flag, interval_ms) = {
            let mut s = crate::invariants::lock(&self.state);
            if s.typing_stop.is_some() {
                return;
            }
            let Some(t) = s.transport.as_ref() else { return };
            if !t.has_typing() {
                return;
            }
            let ms = self.options.typing_ms.or_else(|| t.typing_ms()).unwrap_or(4_500);
            let flag = Arc::new(AtomicBool::new(false));
            s.typing_stop = Some(flag.clone());
            (flag, ms)
        };
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || {
            loop {
                let Some(link) = weak.upgrade() else { return };
                if stop_flag.load(Ordering::SeqCst) {
                    return;
                }
                let now = link.now();
                let waiting_on: HashSet<String> = (link.options.waiting)()
                    .into_iter()
                    .map(|(req, _)| req.mission_id)
                    .collect();
                let (place, transport, done) = {
                    let mut s = crate::invariants::lock(&link.state);
                    let mut working_any = false;
                    let mut empty = true;
                    if let Some(sv) = s.saved.as_mut() {
                        sv.turns.retain(|t| now.saturating_sub(t.at) <= 12 * 60 * 60_000 || (link.options.live)(&t.mission_id));
                        empty = sv.turns.is_empty();
                        working_any = sv.turns.iter().any(|t| (link.options.live)(&t.mission_id) && !waiting_on.contains(&t.mission_id));
                    }
                    if empty {
                        s.typing_stop = None;
                        (None, None, true)
                    } else {
                        let place = if working_any {
                            s.saved.as_ref().and_then(|sv| sv.chat.as_ref()).map(|c| c.place.clone())
                        } else {
                            None
                        };
                        (place, s.transport.clone(), false)
                    }
                };
                if done {
                    return;
                }
                if let (Some(p), Some(t)) = (place, transport) {
                    t.typing(&p);
                }
                std::thread::sleep(Duration::from_millis(interval_ms));
            }
        });
    }

    fn send_to(self: &Arc<Self>, teammate: &ChatTeammate, text: &str, origin_id: &str, origin_answer_to: Option<&str>, reply: Option<&mut Replier<'_>>, continues: Option<&str>) {
        let kept = {
            let s = crate::invariants::lock(&self.state);
            let Some(sv) = s.saved.as_ref() else { return };
            if sv.fresh.contains(&teammate.teammate_id) {
                None
            } else {
                sv.conversations.get(&teammate.teammate_id).cloned()
            }
        };
        let kept_stands = kept
            .as_ref()
            .is_some_and(|k| (self.options.live)(k) || (self.options.read)(k).is_some());
        let follow_up_of = continues.or_else(|| if kept_stands { kept.as_deref() } else { None });
        let started = (self.options.start)(&teammate.teammate_id, text, follow_up_of);
        let mission_id = match started {
            ChatStart::Ok { mission_id } => mission_id,
            ChatStart::Err { message } => {
                let escaped = self.app.escape(&message);
                if let Some(r) = reply {
                    let _ = r.say(&escaped, true, None);
                } else {
                    let extra = ChatSendExtra { answer_to: origin_answer_to.map(str::to_owned), ..Default::default() };
                    let _ = self.send_raw(&escaped, extra);
                }
                return;
            }
        };

        let now = self.now();
        let (snapshot, place, transport) = {
            let mut s = crate::invariants::lock(&self.state);
            if let Some(sv) = s.saved.as_mut() {
                sv.current = Some(teammate.teammate_id.clone());
                sv.fresh.retain(|id| id != &teammate.teammate_id);
                sv.conversations.insert(teammate.teammate_id.clone(), mission_id.clone());
                sv.turns.push(SavedTurn {
                    mission_id: mission_id.clone(),
                    teammate_id: teammate.teammate_id.clone(),
                    answer_to: origin_answer_to.map(str::to_owned),
                    asked: Some(origin_id.to_owned()),
                    at: now,
                    carried: false,
                });
                if sv.turns.len() > MAX_TURNS_KEPT {
                    let drop_n = sv.turns.len() - MAX_TURNS_KEPT;
                    sv.turns.drain(..drop_n);
                }
                sv.answers.push(SavedAnswer {
                    message_id: origin_id.to_owned(),
                    mission_id,
                    teammate_id: Some(teammate.teammate_id.clone()),
                });
                if sv.answers.len() > MAX_ANSWERS_KEPT {
                    let drop_n = sv.answers.len() - MAX_ANSWERS_KEPT;
                    sv.answers.drain(..drop_n);
                }
            }
            let place = s.saved.as_ref().and_then(|sv| sv.chat.as_ref()).map(|c| c.place.clone());
            (s.saved.clone(), place, s.transport.clone())
        };
        let _ = self.persist_snapshot(snapshot);
        if let Some(r) = reply {
            if r.pending() {
                let msg = format!("{}: {}", self.app.bold(&format!("To {}", teammate.name)), self.app.escape(text));
                let _ = r.say(&msg, false, None);
            }
        }
        if let (Some(p), Some(t)) = (place, transport) {
            t.working(&p, origin_id, true);
        }
        self.typing_on();
    }

    fn post_card(&self, request: &Request, name: Option<&str>) {
        {
            let mut s = crate::invariants::lock(&self.state);
            let Some(sv) = s.saved.as_ref() else { return };
            if sv.chat.is_none() || sv.cards.iter().any(|c| c.approval_id == request.approval_id) {
                return;
            }
            if s.posting.contains(&request.approval_id) {
                return;
            }
            s.posting.insert(request.approval_id.clone());
        }
        self.post_card_now(request, name);
        let mut s = crate::invariants::lock(&self.state);
        s.posting.remove(&request.approval_id);
    }

    fn post_card_now(&self, request: &Request, name: Option<&str>) {
        let who = name.unwrap_or("A teammate");
        let key = random_base64url(6);
        let words = self.app.words();
        let mut text: String;
        let mut buttons: ChatButtons = Vec::new();
        let mut question: Option<SavedQuestion> = None;

        if request.kind == Kind::Question {
            let asked = &request.questions;
            let only = if asked.len() == 1 { asked.first() } else { None };
            let said = asked
                .iter()
                .map(|one| {
                    let parts: Vec<&str> = [one.header.as_deref(), Some(one.question.as_str())]
                        .into_iter()
                        .flatten()
                        .filter(|p| !p.is_empty())
                        .collect();
                    self.app.escape(&scrub_secrets(&parts.join(": ")).0)
                })
                .collect::<Vec<_>>()
                .join("\n");
            let body = if said.is_empty() {
                self.app.escape(&scrub_secrets(&request.summary).0)
            } else {
                said
            };
            text = format!("{}\n{body}", self.app.bold(&format!("{who} is asking you")));
            if only.is_none_or(|q| q.is_secret) {
                text.push_str(&format!("\n\n{}", self.app.italic("Answer it in Locust on your computer.")));
            } else if let Some(only) = only {
                let labels: Vec<String> = only.options.iter().take(8).map(|o| o.label.clone()).collect();
                let free = only.options.is_empty() || only.is_other;
                buttons = labels
                    .iter()
                    .enumerate()
                    .map(|(idx, label)| {
                        vec![ChatButton {
                            label: label.chars().take(60).collect(),
                            data: format!("o:{key}:{idx}"),
                            style: None,
                        }]
                    })
                    .collect();
                if free {
                    let hint = if labels.is_empty() { words.reply_to_card_only } else { words.reply_to_card_or_own };
                    text.push_str(&format!("\n\n{}", self.app.italic(hint)));
                }
                question = Some(SavedQuestion { id: only.id.clone(), options: labels, words: free });
            }
        } else {
            let shown = exact_line(&request.summary, &request.detail);
            let plain = if request.kind == Kind::Command {
                plain_command(&request.detail)
            } else {
                None
            };
            text = format!(
                "{}\n{}{}{}\n\n{}",
                self.app.bold(&format!("{who} needs your approval")),
                self.app.escape(&scrub_secrets(&request.summary).0),
                shown.map(|s| format!("\n{}", self.app.escape(&s))).unwrap_or_default(),
                plain.map(|p| format!("\n{}", self.app.escape(&scrub_secrets(&p.said).0))).unwrap_or_default(),
                self.app.italic("The whole of it is in Locust on your computer.")
            );
            buttons = vec![vec![
                ChatButton { label: "Allow once".into(), data: format!("y:{key}"), style: Some(ButtonStyle::Allow) },
                ChatButton { label: "Deny".into(), data: format!("n:{key}"), style: Some(ButtonStyle::Deny) },
            ]];
        }

        let Some(message_id) = self.send_raw(&text, ChatSendExtra { buttons, ..Default::default() }) else {
            return;
        };
        let snapshot = {
            let mut s = crate::invariants::lock(&self.state);
            if let Some(sv) = s.saved.as_mut() {
                sv.cards.push(SavedCard {
                    key,
                    approval_id: request.approval_id.clone(),
                    mission_id: request.mission_id.clone(),
                    message_id,
                    text,
                    question,
                });
                if sv.cards.len() > MAX_CARDS_KEPT {
                    let drop_n = sv.cards.len() - MAX_CARDS_KEPT;
                    sv.cards.drain(..drop_n);
                }
            }
            s.saved.clone()
        };
        let _ = self.persist_snapshot(snapshot);
    }

    fn settle(&self, approval_id: &str, said: &str) {
        let (card, snapshot) = {
            let mut s = crate::invariants::lock(&self.state);
            let Some(sv) = s.saved.as_mut() else { return };
            let Some(pos) = sv.cards.iter().position(|c| c.approval_id == approval_id) else {
                return;
            };
            let card = sv.cards.remove(pos);
            (card, Some(sv.clone()))
        };
        let _ = self.persist_snapshot(snapshot);
        self.edit_raw(&card.message_id, &format!("{}\n\n{}", card.text, self.app.bold(said)));
    }

    fn from_here(&self, mission_id: &str) -> Option<(Option<SavedTurn>, Option<String>)> {
        let now = self.now();
        let (turn, has_candidates, carried_ids, recent_ids) = {
            let s = crate::invariants::lock(&self.state);
            let sv = s.saved.as_ref()?;
            if let Some(t) = sv.turns.iter().find(|t| t.mission_id == mission_id) {
                return Some((Some(t.clone()), None));
            }
            let carried: Vec<String> = sv.turns.iter().filter(|t| t.carried).map(|t| t.mission_id.clone()).collect();
            let recent: Vec<String> = sv
                .answered
                .iter()
                .filter(|a| now.saturating_sub(a.at) < SUCCESSOR_MS)
                .map(|a| a.mission_id.clone())
                .collect();
            (!carried.is_empty() || !recent.is_empty(), carried.is_empty() && recent.is_empty(), carried, recent)
        };
        let _ = turn;
        if has_candidates {
            return None;
        }
        let mission = (self.options.read)(mission_id)?;
        let before = mission.continues_from_mission_id?;
        let kind = mission.started_by_kind.as_deref();
        if kind != Some("carry-on") && kind != Some("check-fix") {
            return None;
        }
        if carried_ids.contains(&before) || recent_ids.contains(&before) {
            Some((None, Some(before)))
        } else {
            None
        }
    }

    fn answer_turn(&self, mission_id: &str, teammate_id: Option<&str>, origin: Option<&SavedTurn>) {
        let mission = (self.options.read)(mission_id);
        let team = (self.options.team)();
        let name = teammate_id
            .and_then(|id| team.iter().find(|m| m.teammate_id == id))
            .map(|m| m.name.as_str())
            .unwrap_or("Your teammate");
        let said = match mission {
            None => format!("{name}\u{2019}s answer could not be read. It is in Locust."),
            Some(ref m) => chat_answer(m, name),
        };
        let scrubbed = scrub_secrets(&said).0;
        let parts = self.app.answer_parts(name, &scrubbed);
        let mut sent = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            let extra = ChatSendExtra {
                answer_to: origin.and_then(|o| o.answer_to.clone()),
                continued: index > 0,
                buttons: Vec::new(),
            };
            if let Some(message_id) = self.send_raw(part, extra) {
                sent.push(SavedAnswer {
                    message_id,
                    mission_id: mission_id.to_owned(),
                    teammate_id: teammate_id.map(str::to_owned),
                });
            }
        }
        let (place, transport, snapshot) = {
            let now = self.now();
            let mut s = crate::invariants::lock(&self.state);
            let place = s.saved.as_ref().and_then(|sv| sv.chat.as_ref()).map(|c| c.place.clone());
            let transport = s.transport.clone();
            if let Some(sv) = s.saved.as_mut() {
                sv.answers.extend(sent);
                if sv.answers.len() > MAX_ANSWERS_KEPT {
                    let drop_n = sv.answers.len() - MAX_ANSWERS_KEPT;
                    sv.answers.drain(..drop_n);
                }
                sv.answered.retain(|a| now.saturating_sub(a.at) < SUCCESSOR_MS);
                sv.answered.push(SavedAnswered { mission_id: mission_id.to_owned(), at: now });
                if sv.answered.len() > MAX_TURNS_KEPT {
                    let drop_n = sv.answered.len() - MAX_TURNS_KEPT;
                    sv.answered.drain(..drop_n);
                }
                if let Some(tid) = teammate_id {
                    sv.conversations.insert(tid.to_owned(), mission_id.to_owned());
                }
            }
            (place, transport, s.saved.clone())
        };
        if let (Some(asked), Some(p), Some(t)) = (origin.and_then(|o| o.asked.as_deref()), place, transport) {
            t.working(&p, asked, false);
        }
        let _ = self.persist_snapshot(snapshot);
    }

    fn run_ended(&self, mission_id: &str) {
        let cards_for_mission: Vec<String> = {
            let s = crate::invariants::lock(&self.state);
            let Some(sv) = s.saved.as_ref() else { return };
            if sv.chat.is_none() {
                return;
            }
            sv.cards
                .iter()
                .filter(|c| c.mission_id == mission_id)
                .map(|c| c.approval_id.clone())
                .collect()
        };
        for approval_id in cards_for_mission {
            self.settle(&approval_id, "The turn ended before this was answered.");
        }
        let Some((turn, successor_of)) = self.from_here(mission_id) else {
            return;
        };
        if turn.is_some() && (self.options.carried)(mission_id) {
            let snapshot = {
                let mut s = crate::invariants::lock(&self.state);
                if let Some(sv) = s.saved.as_mut() {
                    for t in &mut sv.turns {
                        if t.mission_id == mission_id {
                            t.carried = true;
                        }
                    }
                }
                s.saved.clone()
            };
            let _ = self.persist_snapshot(snapshot);
            return;
        }
        let (origin, teammate_id) = {
            let mut s = crate::invariants::lock(&self.state);
            let sv = s.saved.as_mut();
            let origin = turn.or_else(|| {
                successor_of
                    .as_deref()
                    .and_then(|so| sv.as_ref().and_then(|v| v.turns.iter().find(|t| t.mission_id == so).cloned()))
            });
            let teammate_id = origin.as_ref().map(|o| o.teammate_id.clone()).or_else(|| {
                successor_of.as_deref().and_then(|so| {
                    sv.as_ref().and_then(|v| {
                        v.conversations.iter().find(|(_, id)| id.as_str() == so).map(|(tid, _)| tid.clone())
                    })
                })
            });
            if let Some(v) = sv {
                v.turns.retain(|t| t.mission_id != mission_id && Some(t.mission_id.as_str()) != successor_of.as_deref());
            }
            (origin, teammate_id)
        };
        self.answer_turn(mission_id, teammate_id.as_deref(), origin.as_ref());
    }

    pub fn handle_message(self: &Arc<Self>, incoming: ChatIncoming) {
        let _guard = crate::invariants::lock(&self.turn_lock);
        if !incoming.direct || incoming.bot || incoming.from.is_empty() {
            return;
        }
        let bot = {
            let mut s = crate::invariants::lock(&self.state);
            let Some(bot) = s.saved.as_ref().map(|sv| sv.bot.clone()) else { return };
            let key = format!("{}:{}", incoming.place, incoming.id);
            if s.seen.contains(&key) {
                return;
            }
            s.seen.push(key);
            if s.seen.len() > 500 {
                let drop_n = s.seen.len() - 500;
                s.seen.drain(..drop_n);
            }
            bot
        };

        let mut reply = Replier::new(self, incoming.respond.clone(), incoming.answer_to.clone());
        let text = incoming.text.as_deref().unwrap_or("").trim();

        let code = if incoming.command.is_none() && !text.is_empty() {
            self.app.code_in(text, &bot)
        } else {
            None
        };
        if let Some(code) = code {
            let matches_link = {
                let s = crate::invariants::lock(&self.state);
                s.link.as_ref().is_some_and(|l| l.until > self.now() && l.code == code)
            };
            if matches_link {
                self.pair(&incoming);
                return;
            }
            if !self.paired_from(&incoming.from, &incoming.place) {
                self.wrong_code(&incoming);
                return;
            }
        }

        if !self.paired_from(&incoming.from, &incoming.place) {
            return;
        }
        if let Some(refused) = (self.options.refusal)() {
            let _ = reply.say(&self.app.escape(&refused), false, None);
            return;
        }

        let team = (self.options.team)();
        let commands = self.ours(&team);
        if incoming.text.is_none() && incoming.command.is_none() {
            let _ = reply.say("Locust reads text here for now. Pictures and files go in through Locust on your computer.", true, None);
            return;
        }

        let command = incoming.command.clone().or_else(|| self.app.parse(text, &bot));
        if let Some(ref cmd) = command {
            if cmd.command == "start" || cmd.command == "help" {
                let _ = reply.say(&self.help_text(&team), false, None);
                return;
            }
            if cmd.command == "who" {
                let current_id = crate::invariants::lock(&self.state).saved.as_ref().and_then(|sv| sv.current.clone());
                let current = current_id.and_then(|id| team.iter().find(|m| m.teammate_id == id));
                let words = self.app.words();
                let lines: Vec<String> = team
                    .iter()
                    .enumerate()
                    .map(|(i, mate)| (words.who_line)(&commands[i].1, &mate.name, &mate.role))
                    .collect();
                let msg = if team.is_empty() {
                    "You have no teammates yet. Make one in Locust.".into()
                } else {
                    let mut out = format!("{}\n{}", self.app.bold("Your teammates"), lines.join("\n"));
                    if let Some(c) = current {
                        out.push_str(&format!("\n\nWriting to {}.", self.app.escape(&c.name)));
                    }
                    out
                };
                let _ = reply.say(&msg, false, None);
                return;
            }
            if cmd.command == "waiting" {
                let waiting = (self.options.waiting)();
                if waiting.is_empty() {
                    let _ = reply.say("Nothing is waiting on you.", false, None);
                    return;
                }
                if reply.pending() {
                    let _ = reply.say(&format!("{} waiting on you:", waiting.len()), false, None);
                }
                for (req, tid) in waiting.iter().take(5) {
                    let name = tid.as_deref().and_then(|id| team.iter().find(|m| m.teammate_id == id)).map(|m| m.name.as_str());
                    self.post_card(req, name);
                }
                if waiting.len() > 5 {
                    let _ = reply.say(&format!("{} more are waiting in Locust.", waiting.len() - 5), false, None);
                }
                return;
            }
        }

        let stale = self.now().saturating_sub(incoming.sent_at) > CHAT_STALE_MS;
        let stale_msg = || {
            format!(
                "This came while Locust was closed (sent {}), so nothing was started. Send it again to start it now.",
                local_time_at(incoming.sent_at)
            )
        };

        if let Some(ref cmd) = command {
            if cmd.command == "new" {
                let current_id = crate::invariants::lock(&self.state).saved.as_ref().and_then(|sv| sv.current.clone());
                let current = current_id.and_then(|id| team.iter().find(|m| m.teammate_id == id));
                let Some(current) = current else {
                    let _ = reply.say(
                        &format!("Pick who to write to first: {} lists your teammates.", self.app.escape(&(self.app.words().own)("who"))),
                        false,
                        None,
                    );
                    return;
                };
                let snapshot = {
                    let mut s = crate::invariants::lock(&self.state);
                    if let Some(sv) = s.saved.as_mut() {
                        if !sv.fresh.contains(&current.teammate_id) {
                            sv.fresh.push(current.teammate_id.clone());
                        }
                    }
                    s.saved.clone()
                };
                let _ = self.persist_snapshot(snapshot);
                if cmd.rest.is_empty() {
                    let _ = reply.say(&format!("Your next message starts a new conversation with {}.", self.app.escape(&current.name)), false, None);
                    return;
                }
                if stale {
                    let _ = reply.say(&stale_msg(), true, None);
                    return;
                }
                self.send_to(current, &cmd.rest, &incoming.id, incoming.answer_to.as_deref(), Some(&mut reply), None);
                return;
            }
        }

        let team_refs: Vec<ChatTeammateRef> = team
            .iter()
            .map(|m| ChatTeammateRef { teammate_id: m.teammate_id.clone(), name: m.name.clone() })
            .collect();
        let named = if command.is_none() {
            addressed_teammate(text, &team_refs)
        } else {
            None
        };
        let command_mate = command.as_ref().and_then(|cmd| {
            team.iter()
                .enumerate()
                .find(|(i, _)| commands[*i].1 == cmd.command)
                .map(|(_, m)| m)
        });
        if let Some(ref cmd) = command {
            if command_mate.is_none() {
                let words = self.app.words();
                let _ = reply.say(
                    &format!(
                        "There is no {}. {} lists your teammates.",
                        self.app.escape(&(words.address)(&cmd.command, &cmd.command)),
                        self.app.escape(&(words.own)("who"))
                    ),
                    true,
                    None,
                );
                return;
            }
        }

        let mate = command_mate.or_else(|| named.as_ref().and_then(|n| team.iter().find(|m| m.teammate_id == n.teammate_id)));
        let rest = command.as_ref().map(|c| c.rest.as_str()).or_else(|| named.as_ref().map(|n| n.rest.as_str()));
        if let (Some(mate), Some(rest)) = (mate, rest) {
            if rest.is_empty() {
                let snapshot = {
                    let mut s = crate::invariants::lock(&self.state);
                    if let Some(sv) = s.saved.as_mut() {
                        sv.current = Some(mate.teammate_id.clone());
                    }
                    s.saved.clone()
                };
                let _ = self.persist_snapshot(snapshot);
                let _ = reply.say(&format!("Writing to {}. Send your message.", self.app.escape(&mate.name)), false, None);
                return;
            }
            if stale {
                let _ = reply.say(&stale_msg(), true, None);
                return;
            }
            self.send_to(mate, rest, &incoming.id, incoming.answer_to.as_deref(), Some(&mut reply), None);
            return;
        }

        if stale {
            let _ = reply.say(&stale_msg(), true, None);
            return;
        }

        if let Some(ref replied_to) = incoming.reply_to {
            let (card_match, answered_match) = {
                let s = crate::invariants::lock(&self.state);
                let sv = s.saved.as_ref();
                let card = sv.and_then(|v| v.cards.iter().find(|c| c.message_id == *replied_to).cloned());
                let ans = sv.and_then(|v| v.answers.iter().rev().find(|a| a.message_id == *replied_to).cloned());
                (card, ans)
            };
            if let Some(card) = card_match {
                if let Some(ref q) = card.question {
                    if q.words {
                        {
                            let mut s = crate::invariants::lock(&self.state);
                            s.answering_here.insert(card.approval_id.clone());
                        }
                        let answered = (self.options.decide)(Answer::Answers {
                            approval_id: card.approval_id.clone(),
                            answers: vec![(q.id.clone(), vec![text.to_owned()])],
                        });
                        {
                            let mut s = crate::invariants::lock(&self.state);
                            s.answering_here.remove(&card.approval_id);
                        }
                        if answered {
                            self.settle(&card.approval_id, "Answered.");
                        } else {
                            self.settle(&card.approval_id, "This was already answered, or its turn ended.");
                        }
                        return;
                    }
                }
            }
            if let Some(ans) = answered_match {
                if let Some(mate) = ans.teammate_id.as_deref().and_then(|id| team.iter().find(|m| m.teammate_id == id)) {
                    self.send_to(mate, text, &incoming.id, incoming.answer_to.as_deref(), Some(&mut reply), Some(&ans.mission_id));
                    return;
                }
            }
        }

        let current_id = crate::invariants::lock(&self.state).saved.as_ref().and_then(|sv| sv.current.clone());
        let current = current_id
            .and_then(|id| team.iter().find(|m| m.teammate_id == id))
            .or_else(|| if team.len() == 1 { team.first() } else { None });
        if let Some(current) = current {
            self.send_to(current, text, &incoming.id, incoming.answer_to.as_deref(), Some(&mut reply), None);
            return;
        }

        if team.is_empty() {
            let _ = reply.say("You have no teammates yet. Make one in Locust, then write here.", false, None);
            return;
        }

        let choices: Vec<&ChatTeammate> = team.iter().take(8).collect();
        let buttons: ChatButtons = choices
            .iter()
            .enumerate()
            .map(|(i, m)| vec![ChatButton { label: m.name.chars().take(60).collect(), data: format!("t:{i}"), style: None }])
            .collect();
        {
            let mut s = crate::invariants::lock(&self.state);
            s.held = Some(HeldMessage {
                text: text.to_owned(),
                origin_id: incoming.id,
                origin_answer_to: incoming.answer_to,
                at: self.now(),
                choices: choices.iter().map(|m| m.teammate_id.clone()).collect(),
            });
        }
        let _ = reply.say("Who is this for?", true, Some(&buttons));
    }

    pub fn handle_press(self: &Arc<Self>, press: ChatPress) {
        let _guard = crate::invariants::lock(&self.turn_lock);
        if !self.paired_from(&press.from, &press.place) {
            (press.answer)(None);
            return;
        }
        if let Some(refused) = (self.options.refusal)() {
            let cut: String = refused.chars().take(190).collect();
            (press.answer)(Some(&cut));
            return;
        }

        if let Some(idx_str) = press.data.strip_prefix("t:") {
            if idx_str.len() == 1 && idx_str.bytes().all(|b| b.is_ascii_digit()) {
                let idx: usize = idx_str.parse().unwrap_or(99);
                let team = (self.options.team)();
                let waited = {
                    let mut s = crate::invariants::lock(&self.state);
                    s.held.take()
                };
                let mate = waited
                    .as_ref()
                    .and_then(|w| w.choices.get(idx))
                    .and_then(|tid| team.iter().find(|m| &m.teammate_id == tid));
                let Some(waited) = waited else {
                    (press.answer)(Some("That message waited too long. Send it again."));
                    self.edit_raw(&press.message, &format!("Who is this for? {}", self.app.italic("It waited too long: send it again.")));
                    return;
                };
                let Some(mate) = mate else {
                    (press.answer)(Some("That message waited too long. Send it again."));
                    self.edit_raw(&press.message, &format!("Who is this for? {}", self.app.italic("It waited too long: send it again.")));
                    return;
                };
                if self.now().saturating_sub(waited.at) > HELD_MS {
                    (press.answer)(Some("That message waited too long. Send it again."));
                    self.edit_raw(&press.message, &format!("Who is this for? {}", self.app.italic("It waited too long: send it again.")));
                    return;
                }
                (press.answer)(Some(&format!("To {}", mate.name)));
                self.edit_raw(&press.message, &format!("To {}.", self.app.bold(&mate.name)));
                self.send_to(mate, &waited.text, &waited.origin_id, waited.origin_answer_to.as_deref(), None, None);
                return;
            }
        }

        let Some((kind, key, opt_idx)) = parse_card_press(&press.data) else {
            (press.answer)(Some("This card was already answered, or its turn ended."));
            return;
        };
        let card = {
            let s = crate::invariants::lock(&self.state);
            s.saved.as_ref().and_then(|sv| sv.cards.iter().find(|c| c.key == key).cloned())
        };
        let Some(card) = card else {
            (press.answer)(Some("This card was already answered, or its turn ended."));
            return;
        };

        let answer = match kind {
            'o' => {
                let Some(idx) = opt_idx else { (press.answer)(None); return };
                let Some(q) = card.question.as_ref() else { (press.answer)(None); return };
                let Some(opt) = q.options.get(idx) else { (press.answer)(None); return };
                Answer::Answers {
                    approval_id: card.approval_id.clone(),
                    answers: vec![(q.id.clone(), vec![opt.clone()])],
                }
            }
            'y' => Answer::Decision { approval_id: card.approval_id.clone(), decision: Decision::ApproveOnce, reason: None },
            'n' => Answer::Decision { approval_id: card.approval_id.clone(), decision: Decision::Deny, reason: None },
            _ => { (press.answer)(None); return }
        };

        {
            let mut s = crate::invariants::lock(&self.state);
            s.answering_here.insert(card.approval_id.clone());
        }
        let answered = (self.options.decide)(answer);
        {
            let mut s = crate::invariants::lock(&self.state);
            s.answering_here.remove(&card.approval_id);
        }

        let said = if !answered {
            "This was already answered, or its turn ended.".to_owned()
        } else {
            match kind {
                'y' => "Allowed once.".to_owned(),
                'n' => "Denied.".to_owned(),
                _ => {
                    let label = card.question.as_ref().and_then(|q| opt_idx.and_then(|i| q.options.get(i))).map(String::as_str).unwrap_or("");
                    format!("Answered: {label}")
                }
            }
        };
        let short: String = said.chars().take(190).collect();
        (press.answer)(Some(&short));
        self.settle(&card.approval_id, &said);
    }
}

fn parse_card_press(data: &str) -> Option<(char, String, Option<usize>)> {
    let mut parts = data.split(':');
    let kind_str = parts.next()?;
    let key = parts.next()?;
    let opt = parts.next();
    if parts.next().is_some() {
        return None;
    }
    let kind = match kind_str {
        "y" | "n" | "o" => kind_str.chars().next()?,
        _ => return None,
    };
    if key.len() != 8 || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        return None;
    }
    let opt_idx = match opt {
        None => None,
        Some(s) if s.len() == 1 && s.bytes().all(|b| b.is_ascii_digit()) => Some(s.parse().ok()?),
        Some(_) => return None,
    };
    Some((kind, key.to_owned(), opt_idx))
}
