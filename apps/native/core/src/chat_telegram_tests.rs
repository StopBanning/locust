//! Parity and Electron carry-over tests for `apps/desktop/src/main/teammates-answer-their-person-in-telegram.test.ts` (14 cases).
//! Uses an in-process fake Telegram Bot API and a fake Electron-compatible base64 secret box; never contacts Telegram or the OS keychain.

use super::*;
use crate::approval::{Answer, Decision, Kind, Question, QuestionOption, Request};
use crate::chat_apps::ChatAppSettings;
use crate::chat_http::{HttpClient, HttpRequest, HttpResponse};
use crate::chat_link::{ChatAlert, ChatLink, ChatLinkOptions, ChatMission, ChatStart, ChatTeammate, TokenInput};
use crate::own_models::SecretBox;
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

/// Stand-in for Electron's `safeStorage` base64-encoded ciphertext (`Buffer.from(xor).toString('base64')`).
struct ElectronSecretBox {
    asked: Mutex<Vec<String>>,
}

impl ElectronSecretBox {
    fn new() -> Arc<Self> {
        Arc::new(Self { asked: Mutex::new(Vec::new()) })
    }
    fn seal_electron(text: &str) -> String {
        let xored: Vec<u8> = text.bytes().map(|b| b ^ 0x5a).collect();
        b64_encode(&xored)
    }
}

impl SecretBox for ElectronSecretBox {
    fn available(&self) -> bool {
        self.asked.lock().unwrap().push("available".into());
        true
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        self.asked.lock().unwrap().push("seal".into());
        Ok(Self::seal_electron(text))
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        self.asked.lock().unwrap().push("open".into());
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

struct FakeTelegram {
    token: String,
    webhook: Option<String>,
    updates: Mutex<VecDeque<Value>>,
    sent: Mutex<Vec<Value>>,
    edits: Mutex<Vec<Value>>,
    commands: Mutex<Vec<Vec<Value>>>,
    actions: Mutex<Vec<Value>>,
    answers: Mutex<Vec<Value>>,
    polls: AtomicU64,
    reject_html_once: AtomicBool,
    next_msg_id: AtomicI64,
    next_update_id: AtomicI64,
}

impl FakeTelegram {
    fn new(webhook: Option<&str>) -> Arc<Self> {
        Arc::new(Self {
            token: "123456789:AAHfakefakefakefakefakefakefakefake1".into(),
            webhook: webhook.map(str::to_owned),
            updates: Mutex::new(VecDeque::new()),
            sent: Mutex::new(Vec::new()),
            edits: Mutex::new(Vec::new()),
            commands: Mutex::new(Vec::new()),
            actions: Mutex::new(Vec::new()),
            answers: Mutex::new(Vec::new()),
            polls: AtomicU64::new(0),
            reject_html_once: AtomicBool::new(false),
            next_msg_id: AtomicI64::new(500),
            next_update_id: AtomicI64::new(1),
        })
    }

    fn say(&self, text: &str, from_id: i64, chat_id: i64, chat_type: &str, reply_to: Option<i64>, date_secs: u64) -> i64 {
        let uid = self.next_update_id.fetch_add(1, Ordering::SeqCst);
        let mid = uid * 10;
        let (first, username) = if from_id == 9999 { ("Mallory", "mallory") } else { ("Sam", "sam_here") };
        let mut msg = json!({
            "message_id": mid,
            "from": { "id": from_id, "is_bot": false, "first_name": first, "username": username },
            "chat": { "id": chat_id, "type": chat_type },
            "date": date_secs,
            "text": text,
        });
        if let Some(r) = reply_to {
            msg["reply_to_message"] = json!({ "message_id": r });
        }
        self.updates.lock().unwrap().push_back(json!({ "update_id": uid, "message": msg }));
        mid
    }

    fn press(&self, message_id: i64, data: &str, from_id: i64, chat_id: i64) {
        let uid = self.next_update_id.fetch_add(1, Ordering::SeqCst);
        self.updates.lock().unwrap().push_back(json!({
            "update_id": uid,
            "callback_query": {
                "id": format!("cb_{uid}"),
                "from": { "id": from_id, "is_bot": false, "first_name": if from_id == 9999 { "Mallory" } else { "Sam" } },
                "message": { "message_id": message_id, "chat": { "id": chat_id } },
                "data": data,
            }
        }));
    }

    fn sent_list(&self) -> Vec<Value> {
        self.sent.lock().unwrap().clone()
    }
    fn edits_list(&self) -> Vec<Value> {
        self.edits.lock().unwrap().clone()
    }
    fn answers_list(&self) -> Vec<Value> {
        self.answers.lock().unwrap().clone()
    }
}

impl HttpClient for FakeTelegram {
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
            "getMe" => json!({"ok": true, "result": {"id": 123456789, "username": "locust_test_bot", "first_name": "Locust test bot"}}),
            "getWebhookInfo" => json!({"ok": true, "result": {"url": self.webhook.as_deref().unwrap_or("")}}),
            "getUpdates" => {
                self.polls.fetch_add(1, Ordering::SeqCst);
                let offset = body["offset"].as_i64().unwrap_or(0);
                let mut q = self.updates.lock().unwrap();
                let mut batch = Vec::new();
                while let Some(u) = q.pop_front() {
                    if u["update_id"].as_i64().unwrap_or(0) >= offset {
                        batch.push(u);
                    }
                }
                if batch.is_empty() {
                    drop(q);
                    std::thread::sleep(Duration::from_millis(10));
                }
                json!({"ok": true, "result": batch})
            }
            "sendMessage" => {
                if body["parse_mode"].as_str() == Some("HTML") && self.reject_html_once.swap(false, Ordering::SeqCst) {
                    return Ok(HttpResponse {
                        status: 400,
                        headers: vec![],
                        body: json!({"ok": false, "error_code": 400, "description": "Bad Request: can't parse entities"}).to_string().into_bytes(),
                    });
                }
                let mid = self.next_msg_id.fetch_add(1, Ordering::SeqCst);
                let mut recorded = body;
                recorded["message_id"] = json!(mid);
                self.sent.lock().unwrap().push(recorded);
                json!({"ok": true, "result": {"message_id": mid}})
            }
            "editMessageText" => {
                self.edits.lock().unwrap().push(body);
                json!({"ok": true, "result": true})
            }
            "sendChatAction" => {
                self.actions.lock().unwrap().push(body);
                json!({"ok": true, "result": true})
            }
            "setMyCommands" => {
                if let Some(arr) = body["commands"].as_array() {
                    self.commands.lock().unwrap().push(arr.clone());
                }
                json!({"ok": true, "result": true})
            }
            "answerCallbackQuery" => {
                self.answers.lock().unwrap().push(body);
                json!({"ok": true, "result": true})
            }
            _ => json!({"ok": false, "error_code": 404, "description": "Not Found"}),
        };
        Ok(HttpResponse { status: 200, headers: vec![], body: resp_val.to_string().into_bytes() })
    }
}

fn wait_until(mut cond: impl FnMut() -> bool, what: &str) {
    for _ in 0..250 {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

struct Harness {
    dir: PathBuf,
    tg: Arc<FakeTelegram>,
    link: Arc<ChatLink>,
    started: Arc<Mutex<Vec<(String, String, Option<String>)>>>,
    decided: Arc<Mutex<Vec<Answer>>>,
    missions: Arc<Mutex<HashMap<String, ChatMission>>>,
    live: Arc<Mutex<std::collections::HashSet<String>>>,
    waiting: Arc<Mutex<Vec<(Request, Option<String>)>>>,
    refusal: Arc<Mutex<Option<String>>>,
    clock: Arc<AtomicU64>,
    code: Option<String>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.link.dispose();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Harness {
    fn new(team: Vec<ChatTeammate>, webhook: Option<&str>) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let dir = std::env::temp_dir().join(format!("locust-tg-p-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(&dir).unwrap();
        let tg = FakeTelegram::new(webhook);
        let started = Arc::new(Mutex::new(Vec::new()));
        let decided = Arc::new(Mutex::new(Vec::new()));
        let missions = Arc::new(Mutex::new(HashMap::new()));
        let live = Arc::new(Mutex::new(std::collections::HashSet::new()));
        let waiting: Arc<Mutex<Vec<(Request, Option<String>)>>> = Arc::new(Mutex::new(Vec::new()));
        let refusal: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        let clock = Arc::new(AtomicU64::new(now_ms));
        let next = Arc::new(AtomicU64::new(0));
        let link_slot: Arc<Mutex<Option<Arc<ChatLink>>>> = Arc::new(Mutex::new(None));

        let link = create_telegram_link(
            TelegramOptions { http: tg.clone(), api: Some(Arc::new(|| "http://127.0.0.1:1".into())), poll_seconds: Some(1) },
            ChatLinkOptions {
                directory: dir.clone(),
                secrets: ElectronSecretBox::new(),
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
                now: {
                    let c = clock.clone();
                    Some(Arc::new(move || c.load(Ordering::SeqCst)))
                },
                typing_ms: Some(30),
            },
        );
        *link_slot.lock().unwrap() = Some(link.clone());
        Self { dir, tg, link, started, decided, missions, live, waiting, refusal, clock, code: None }
    }

    fn paired(team: Vec<ChatTeammate>) -> Self {
        let mut h = Self::new(team, None);
        let s = h.link.set_token(TokenInput::Single(h.tg.token.clone()));
        let code = s.link.unwrap().url.split("start=").nth(1).unwrap().to_owned();
        let now_s = h.clock.load(Ordering::SeqCst) / 1000;
        h.tg.say(&format!("/start {code}"), 77, 77, "private", None, now_s);
        wait_until(|| h.tg.sent_list().iter().any(|m| m["text"].as_str().unwrap_or("").starts_with("Paired with Locust on <b>DESK-PC</b>")), "pairing message");
        h.code = Some(code);
        h
    }

    fn now_secs(&self) -> u64 {
        self.clock.load(Ordering::SeqCst) / 1000
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

fn make_card(approval_id: &str, mission_id: &str, summary: &str, detail: &str) -> Request {
    Request {
        approval_id: approval_id.into(),
        mission_id: mission_id.into(),
        runtime: "claude".into(),
        kind: Kind::Command,
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

// case: pairs nothing with a link past its ten minutes
#[test]
fn pairs_nothing_with_a_link_past_its_ten_minutes() {
    let h = Harness::new(vec![ada(), wren()], None);
    let s = h.link.set_token(TokenInput::Single(h.tg.token.clone()));
    let code = s.link.unwrap().url.split("start=").nth(1).unwrap().to_owned();
    h.clock.fetch_add(11 * 60_000, Ordering::SeqCst);
    assert!(h.link.settings().link.is_none());
    h.tg.say(&format!("/start {code}"), 77, 77, "private", None, h.now_secs());
    wait_until(|| h.tg.sent_list().iter().any(|m| m["text"].as_str().unwrap_or("").contains("does not pair this chat")), "old link refused");
    assert!(h.link.settings().chat.is_none());
}

// case: checks a token with Telegram, keeps it encrypted, and refuses one it does not know or another program reads
#[test]
fn checks_a_token_with_telegram_keeps_it_encrypted_and_refuses_one_it_does_not_know_or_another_program_reads() {
    let h = Harness::new(vec![ada(), wren()], None);
    assert!(h.link.set_token(TokenInput::Single("not a token".into())).message.unwrap().contains("not a bot\u{2019}s token"));
    assert!(h.link.set_token(TokenInput::Single("123456789:AAHwrongwrongwrongwrongwrongwrongwro".into())).message.unwrap().contains("Telegram does not know that token"));
    let kept = h.link.set_token(TokenInput::Single(format!("  {} ", h.tg.token)));
    let bot = kept.bot.unwrap();
    assert_eq!(bot.username, "locust_test_bot");
    assert_eq!(bot.name, "Locust test bot");
    assert!(kept.link.unwrap().url.starts_with("https://t.me/locust_test_bot?start="));
    assert!(kept.message.is_none());
    let file = std::fs::read_to_string(h.dir.join(TELEGRAM_FILE)).unwrap();
    assert!(!file.contains(&h.tg.token));
    assert!(!file.contains(h.tg.token.split(':').nth(1).unwrap()));
    wait_until(|| h.link.settings().connected, "connected");

    let hooked = Harness::new(vec![ada(), wren()], Some("https://example.com/hook"));
    assert!(hooked.link.set_token(TokenInput::Single(hooked.tg.token.clone())).message.unwrap().contains("sends its messages to another program"));
    assert!(hooked.link.settings().bot.is_none());
}

// case: pairs the one chat that opens the link, once, and ignores everyone else
#[test]
fn pairs_the_one_chat_that_opens_the_link_once_and_ignores_everyone_else() {
    let h = Harness::paired(vec![ada(), wren()]);
    let s = h.link.settings();
    let chat = s.chat.unwrap();
    assert_eq!(chat.name, "Sam");
    assert_eq!(chat.username.as_deref(), Some("sam_here"));
    assert!(s.link.is_none());
    wait_until(|| !h.tg.commands.lock().unwrap().is_empty(), "command menu");
    let cmds: Vec<String> = h.tg.commands.lock().unwrap().last().unwrap().iter().map(|c| c["command"].as_str().unwrap().to_owned()).collect();
    assert_eq!(cmds, vec!["ada", "wren", "who", "new", "waiting", "help"]);

    let before = h.tg.sent_list().len();
    let now_s = h.now_secs();
    h.tg.say(&format!("/start {}", h.code.as_deref().unwrap()), 9999, 9999, "private", None, now_s);
    h.tg.say("/ada delete everything", 9999, 9999, "private", None, now_s);
    h.tg.say("/ada and this", 9999, 77, "private", None, now_s);
    wait_until(|| h.tg.sent_list().len() > before, "wrong link said");
    std::thread::sleep(Duration::from_millis(60));
    assert!(h.started.lock().unwrap().is_empty());
    let after = &h.tg.sent_list()[before..];
    assert!(after.iter().all(|m| m["chat_id"].as_i64() == Some(9999)));
    assert!(after.last().unwrap()["text"].as_str().unwrap().contains("does not pair this chat"));
    h.tg.say("/ada in a group", 77, -100, "group", None, now_s);
    std::thread::sleep(Duration::from_millis(60));
    assert!(h.started.lock().unwrap().is_empty());
}

// case: sends /ada and a message to Ada, answers in the chat, and carries the conversation on
#[test]
fn sends_ada_and_a_message_to_ada_answers_in_the_chat_and_carries_the_conversation_on() {
    let h = Harness::paired(vec![ada(), wren()]);
    let asked = h.tg.say("/ada what changed in tides today?", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 1, "first turn");
    assert_eq!(h.started.lock().unwrap()[0], ("mate_ada".into(), "what changed in tides today?".into(), None));
    wait_until(|| !h.tg.actions.lock().unwrap().is_empty(), "typing");
    h.end("mission_1", "**Three** commits:\n\n- `fix: tides`\n- docs\n\n```ts\nconst a = 1 < 2\n```", "completed");
    let answer = h.tg.sent_list().last().unwrap().clone();
    assert_eq!(answer["reply_parameters"]["message_id"].as_i64(), Some(asked));
    let html = answer["text"].as_str().unwrap();
    assert!(html.contains("<b>Ada</b>"));
    assert!(html.contains("<b>Three</b> commits"));
    assert!(html.contains("<pre><code class=\"language-ts\">const a = 1 &lt; 2</code></pre>"));

    h.tg.say("and yesterday?", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 2, "follow-up turn");
    assert_eq!(h.started.lock().unwrap()[1], ("mate_ada".into(), "and yesterday?".into(), Some("mission_1".into())));
    h.end("mission_2", "Two more.", "completed");

    h.tg.say("/new", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.tg.sent_list().last().unwrap()["text"].as_str().unwrap_or("").contains("starts a new conversation with Ada"), "/new");
    h.tg.say("start over", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 3, "fresh turn");
    assert_eq!(h.started.lock().unwrap()[2], ("mate_ada".into(), "start over".into(), None));
    h.end("mission_3", "Fresh.", "completed");

    let answer_mid = answer["message_id"].as_i64().unwrap();
    h.tg.say("more on this", 77, 77, "private", Some(answer_mid), h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 4, "reply turn");
    assert_eq!(h.started.lock().unwrap()[3], ("mate_ada".into(), "more on this".into(), Some("mission_1".into())));

    h.tg.say("/wren", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.tg.sent_list().last().unwrap()["text"].as_str().unwrap_or("").contains("Writing to Wren"), "switch to Wren");
    h.tg.say("run the tests", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 5, "Wren turn");
    assert_eq!(h.started.lock().unwrap()[4], ("mate_wren".into(), "run the tests".into(), None));
}

// case: takes anything shaped like a key out of an answer before Telegram has it
#[test]
fn takes_anything_shaped_like_a_key_out_of_an_answer_before_telegram_has_it() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.tg.say("/ada the token?", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    let fake_pat = ["ghp", "abcdefghijklmnopqrstuvwxyz0123456789"].join("_");
    h.end("mission_1", &format!("It is {fake_pat} in .env."), "completed");
    let sent = h.tg.sent_list().last().unwrap()["text"].as_str().unwrap().to_owned();
    assert!(!sent.contains(&fake_pat));
    assert!(sent.contains("[secret-shaped text removed]"));
}

// case: asks who a message is for, once, when nobody is being written to
#[test]
fn asks_who_a_message_is_for_once_when_nobody_is_being_written_to() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.tg.say("can someone look at the build?", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.tg.sent_list().iter().any(|m| m["text"].as_str() == Some("Who is this for?")), "who question");
    let asked = h.tg.sent_list().into_iter().find(|m| m["text"].as_str() == Some("Who is this for?")).unwrap();
    let rows = asked["reply_markup"]["inline_keyboard"].as_array().unwrap();
    let labels: Vec<&str> = rows.iter().map(|r| r[0]["text"].as_str().unwrap()).collect();
    assert_eq!(labels, vec!["Ada", "Wren"]);
    let wren_data = rows[1][0]["callback_data"].as_str().unwrap();
    h.tg.press(asked["message_id"].as_i64().unwrap(), wren_data, 77, 77);
    wait_until(|| h.started.lock().unwrap().len() == 1, "held message sent");
    assert_eq!(h.started.lock().unwrap()[0], ("mate_wren".into(), "can someone look at the build?".into(), None));
    assert_eq!(h.tg.edits_list().last().unwrap()["text"].as_str(), Some("To <b>Wren</b>."));

    let alone = Harness::paired(vec![ada()]);
    alone.tg.say("hello", 77, 77, "private", None, alone.now_secs());
    wait_until(|| alone.started.lock().unwrap().len() == 1, "only teammate");
    assert_eq!(alone.started.lock().unwrap()[0].0, "mate_ada");
}

// case: brings a card from its own turn with Allow once and Deny, and answers it by the window’s path
#[test]
fn brings_a_card_from_its_own_turn_with_allow_once_and_deny_and_answers_it_by_the_windows_path() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.tg.say("/wren run the tests", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    let fake_pat = ["ghp", "abcdefghijklmnopqrstuvwxyz0123456789"].join("_");
    let req = make_card("appr_1", "mission_1", &format!("Run npm test --token={fake_pat} in tides"), "npm test");
    h.waiting.lock().unwrap().push((req.clone(), Some("mate_wren".into())));
    h.link.card_raised(&req, Some("Wren"));
    wait_until(|| h.tg.sent_list().iter().any(|m| m["text"].as_str().unwrap_or("").contains("Wren needs your approval")), "card");
    let posted = h.tg.sent_list().into_iter().find(|m| m["text"].as_str().unwrap_or("").contains("Wren needs your approval")).unwrap();
    let row0 = posted["reply_markup"]["inline_keyboard"][0].as_array().unwrap();
    let labels: Vec<&str> = row0.iter().map(|b| b["text"].as_str().unwrap()).collect();
    assert_eq!(labels, vec!["Allow once", "Deny"]);
    let allow = row0[0]["callback_data"].as_str().unwrap().to_owned();
    let deny = row0[1]["callback_data"].as_str().unwrap().to_owned();
    assert!(!posted["text"].as_str().unwrap().contains(&fake_pat));
    assert!(posted["text"].as_str().unwrap().contains("[secret-shaped text removed]"));

    h.link.card_raised(&make_card("appr_window", "mission_window", "Run npm test", "npm test"), Some("Ada"));
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(h.tg.sent_list().iter().filter(|m| m["text"].as_str().unwrap_or("").contains("needs your approval")).count(), 1);

    let mid = posted["message_id"].as_i64().unwrap();
    h.tg.press(mid, &allow, 9999, 9999);
    wait_until(|| h.tg.answers_list().len() == 1, "stranger answered");
    assert!(h.decided.lock().unwrap().is_empty());

    h.tg.press(mid, &allow, 77, 77);
    wait_until(|| h.decided.lock().unwrap().len() == 1, "decision recorded");
    assert!(matches!(
        &h.decided.lock().unwrap()[0],
        Answer::Decision { approval_id, decision: Decision::ApproveOnce, .. } if approval_id == "appr_1"
    ));
    wait_until(|| h.tg.edits_list().iter().any(|e| e["message_id"].as_i64() == Some(mid)), "card settled");
    let edited = h.tg.edits_list().into_iter().find(|e| e["message_id"].as_i64() == Some(mid)).unwrap();
    assert!(edited["text"].as_str().unwrap().contains("<b>Allowed once.</b>"));
    assert_eq!(edited["reply_markup"], json!({"inline_keyboard": []}));

    h.tg.press(mid, &deny, 77, 77);
    wait_until(|| h.tg.answers_list().len() == 3, "second press answered");
    assert_eq!(h.decided.lock().unwrap().len(), 1);
    assert!(h.tg.answers_list().last().unwrap()["text"].as_str().unwrap_or("").contains("already answered"));
}

// case: says the exact command with what it is for, so Allow once is pressed on words that say it (0.840)
#[test]
fn says_the_exact_command_with_what_it_is_for_so_allow_once_is_pressed_on_words_that_say_it() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.tg.say("/wren push the fix", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    let req = make_card("appr_push", "mission_1", "Run a command", "git push --force origin main");
    h.waiting.lock().unwrap().push((req.clone(), Some("mate_wren".into())));
    h.link.card_raised(&req, Some("Wren"));
    wait_until(|| h.tg.sent_list().iter().any(|m| m["text"].as_str().unwrap_or("").contains("Wren needs your approval")), "card");
    let posted = h.tg.sent_list().into_iter().find(|m| m["text"].as_str().unwrap_or("").contains("Wren needs your approval")).unwrap();
    let plain = plain_of(posted["text"].as_str().unwrap());
    assert!(plain.contains("Run a command\ngit push --force origin main\nReplaces main on origin with this computer\u{2019}s version: commits there that are not on this computer are lost."), "{plain}");
}

// case: tells the person here what needs them from any run, once they turn it on (0.837)
#[test]
fn tells_the_person_here_what_needs_them_from_any_run_once_they_turn_it_on() {
    let h = Harness::paired(vec![ada(), wren()]);
    let req = make_card("appr_away", "mission_window", "Run npm test in tides", "npm test");
    h.waiting.lock().unwrap().push((req.clone(), None));
    let alert = ChatAlert {
        cards: vec![(req, Some("Ada".into()))],
        lines: vec![("Ivy hit the Codex limit".into(), "Pick where the work carries on.".into())],
    };
    h.link.alert(alert.clone());
    std::thread::sleep(Duration::from_millis(50));
    assert!(!h.tg.sent_list().iter().any(|m| m["text"].as_str().unwrap_or("").contains("Codex limit")));
    assert_eq!(h.link.set_alerts(true).alerts, Some(true));
    h.link.alert(alert);
    wait_until(|| h.tg.sent_list().iter().any(|m| m["text"].as_str().unwrap_or("").contains("Ada needs your approval")), "alert card");
    wait_until(|| h.tg.sent_list().iter().any(|m| plain_of(m["text"].as_str().unwrap_or("")) == "Ivy hit the Codex limit\nPick where the work carries on."), "alert line");
}

// case: says in the chat what became of a card answered in the window, or whose turn ended
#[test]
fn says_in_the_chat_what_became_of_a_card_answered_in_the_window_or_whose_turn_ended() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.tg.say("/ada look", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    let first = make_card("appr_a", "mission_1", "Run npm test", "npm test");
    let second = make_card("appr_b", "mission_1", "Edit README.md", "");
    h.waiting.lock().unwrap().push((first.clone(), None));
    h.waiting.lock().unwrap().push((second.clone(), None));
    h.link.card_raised(&first, Some("Ada"));
    h.link.card_raised(&second, Some("Ada"));
    wait_until(|| h.tg.sent_list().iter().filter(|m| m["text"].as_str().unwrap_or("").contains("needs your approval")).count() == 2, "two cards");
    h.link.card_settled("appr_a", crate::chat_link::SettledHow::Denied);
    wait_until(|| h.tg.edits_list().iter().any(|e| e["text"].as_str().unwrap_or("").contains("Denied in Locust.")), "denied in window");
    h.end("mission_1", "Stopped there.", "interrupted");
    wait_until(|| h.tg.edits_list().iter().any(|e| e["text"].as_str().unwrap_or("").contains("The turn ended before this was answered.")), "ended turn");
    assert!(h.tg.sent_list().last().unwrap()["text"].as_str().unwrap().contains("Ada\u{2019}s turn stopped before it finished."));
}

// case: answers a question with its options, or a reply in words, but never a secret one
#[test]
fn answers_a_question_with_its_options_or_a_reply_in_words_but_never_a_secret_one() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.tg.say("/ada plan it", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    let mut choose = make_card("appr_q", "mission_1", "Which branch?", "");
    choose.kind = Kind::Question;
    choose.cwd = None;
    choose.questions = vec![Question {
        id: "q1".into(),
        header: Some("Branch".into()),
        question: "Which branch should I use?".into(),
        options: vec![
            QuestionOption { label: "main".into(), description: None },
            QuestionOption { label: "release".into(), description: None },
        ],
        is_other: true,
        is_secret: false,
    }];
    let mut secret = make_card("appr_s", "mission_1", "A password", "");
    secret.kind = Kind::Question;
    secret.cwd = None;
    secret.questions = vec![Question {
        id: "q2".into(),
        header: None,
        question: "The database password?".into(),
        options: vec![],
        is_other: true,
        is_secret: true,
    }];
    h.waiting.lock().unwrap().push((choose.clone(), None));
    h.waiting.lock().unwrap().push((secret.clone(), None));
    h.link.card_raised(&choose, Some("Ada"));
    h.link.card_raised(&secret, Some("Ada"));
    wait_until(|| h.tg.sent_list().iter().any(|m| m["text"].as_str().unwrap_or("").contains("Which branch")), "question");
    wait_until(|| h.tg.sent_list().iter().any(|m| m["text"].as_str().unwrap_or("").contains("The database password?")), "secret question");
    let asked = h.tg.sent_list().into_iter().find(|m| m["text"].as_str().unwrap_or("").contains("Which branch")).unwrap();
    let hidden = h.tg.sent_list().into_iter().find(|m| m["text"].as_str().unwrap_or("").contains("The database password?")).unwrap();
    assert!(hidden.get("reply_markup").is_none());
    assert!(hidden["text"].as_str().unwrap().contains("Answer it in Locust on your computer."));
    let rows = asked["reply_markup"]["inline_keyboard"].as_array().unwrap();
    assert_eq!(rows.iter().map(|r| r[0]["text"].as_str().unwrap()).collect::<Vec<_>>(), vec!["main", "release"]);
    assert!(asked["text"].as_str().unwrap().contains("Or reply to this message with an answer of your own."));

    h.tg.say("not-a-real-secret", 77, 77, "private", Some(hidden["message_id"].as_i64().unwrap()), h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 2, "reply to secret taken as message");
    assert!(h.decided.lock().unwrap().is_empty());

    let release_data = rows[1][0]["callback_data"].as_str().unwrap();
    h.tg.press(asked["message_id"].as_i64().unwrap(), release_data, 77, 77);
    wait_until(|| h.decided.lock().unwrap().len() == 1, "option chosen");
    let expected = vec![("q1".to_owned(), vec!["release".to_owned()])];
    assert!(matches!(
        &h.decided.lock().unwrap()[0],
        Answer::Answers { approval_id, answers } if approval_id == "appr_q" && answers == &expected
    ));
}

// case: starts nothing from a message that waited while Locust was closed, and acts on each message once
#[test]
fn starts_nothing_from_a_message_that_waited_while_locust_was_closed_and_acts_on_each_message_once() {
    let h = Harness::paired(vec![ada(), wren()]);
    let stale_secs = (h.clock.load(Ordering::SeqCst) - 20 * 60_000) / 1000;
    h.tg.say("/ada deploy now", 77, 77, "private", None, stale_secs);
    wait_until(|| h.tg.sent_list().last().unwrap()["text"].as_str().unwrap_or("").contains("came while Locust was closed"), "stale message");
    assert!(h.started.lock().unwrap().is_empty());

    h.tg.say("/ada once", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn once");
    wait_until(
        || std::fs::read_to_string(h.dir.join(TELEGRAM_FILE)).map(|s| s.contains("mission_1")).unwrap_or(false),
        "turn persisted",
    );
    h.link.dispose();
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(h.dir.join(TELEGRAM_FILE)).unwrap()).unwrap();
    assert!(saved["offset"].as_i64().unwrap_or(0) > 0);

    let again_started = h.started.clone();
    let again = create_telegram_link(
        TelegramOptions { http: h.tg.clone(), api: Some(Arc::new(|| "http://127.0.0.1:1".into())), poll_seconds: Some(1) },
        ChatLinkOptions {
            directory: h.dir.clone(),
            secrets: ElectronSecretBox::new(),
            refusal: Arc::new(|| None),
            computer: Arc::new(|| "DESK-PC".into()),
            team: Arc::new(|| vec![ada()]),
            start: Arc::new(move |tid, text, follow| {
                again_started.lock().unwrap().push((tid.to_owned(), text.to_owned(), follow.map(str::to_owned)));
                ChatStart::Ok { mission_id: "mission_again".into() }
            }),
            read: Arc::new(|_| None),
            live: Arc::new(|_| false),
            carried: Arc::new(|_| false),
            waiting: Arc::new(Vec::new),
            decide: Arc::new(|_| false),
            changed: None,
            note: None,
            now: {
                let c = h.clock.clone();
                Some(Arc::new(move || c.load(Ordering::SeqCst)))
            },
            typing_ms: Some(30),
        },
    );
    again.load();
    let polls_before = h.tg.polls.load(Ordering::SeqCst);
    wait_until(|| h.tg.polls.load(Ordering::SeqCst) > polls_before + 2, "polling again");
    assert_eq!(h.started.lock().unwrap().len(), 1);
    assert!(h.tg.sent_list().last().unwrap()["text"].as_str().unwrap().contains("could not be read"));
    again.dispose();
}

// case: stops at the organization’s word, and forgets the token and the chat on Remove
#[test]
fn stops_at_the_organizations_word_and_forgets_the_token_and_the_chat_on_remove() {
    let h = Harness::paired(vec![ada(), wren()]);
    *h.refusal.lock().unwrap() = Some("Teammates in chat apps are off. Set by your organization.".into());
    h.tg.say("/ada anything", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.tg.sent_list().last().unwrap()["text"].as_str().unwrap_or("").contains("Teammates in chat apps are off"), "refusal");
    assert!(h.started.lock().unwrap().is_empty());
    assert!(h.link.set_token(TokenInput::Single(h.tg.token.clone())).message.unwrap().contains("Teammates in chat apps are off"));
    *h.refusal.lock().unwrap() = None;
    let removed = h.link.remove();
    assert_eq!(removed, ChatAppSettings::default());
    assert!(!h.dir.join(TELEGRAM_FILE).exists());
}

// case: sends only markup Telegram takes, and the plain words when it refuses some
#[test]
fn sends_only_markup_telegram_takes_and_the_plain_words_when_it_refuses_some() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.tg.say("/ada show me", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    let before = h.tg.sent_list().len();
    h.tg.reject_html_once.store(true, Ordering::SeqCst);
    let long = format!("{}\n\n| a | b |\n|---|---|\n| 1 | 2 |", "A long line of findings. ".repeat(400));
    h.end("mission_1", &long, "completed");
    let parts = h.tg.sent_list()[before..].to_vec();
    assert!(parts.len() > 1);
    // First part fell back to plain text after Telegram returned 400 parse error.
    assert!(parts[0].get("parse_mode").is_none());
    assert!(parts[0]["text"].as_str().unwrap().starts_with("Ada"));
    assert!(parts.last().unwrap()["text"].as_str().unwrap().contains("<pre>| a | b |\n| 1 | 2 |</pre>"));
}

/// Carry-over proof: a `telegram.json` written by Electron (0.831 numeric ids + base64-sealed token)
/// is read, decrypted, connected, and answers `/ada` in Rust without re-pairing.
#[test]
fn electron_telegram_pairing_and_encrypted_token_carry_over_to_rust() {
    let h = Harness::new(vec![ada(), wren()], None);
    let sealed = ElectronSecretBox::seal_electron(&h.tg.token);
    let electron_file = json!({
        "schemaVersion": 1,
        "token": sealed,
        "bot": { "id": 123456789, "username": "locust_test_bot", "name": "Locust test bot" },
        "chat": { "id": 77, "userId": 77, "name": "Sam", "username": "sam_here", "pairedAt": "2026-11-01T10:00:00.000Z" },
        "offset": 1,
        "alerts": true
    });
    std::fs::write(h.dir.join(TELEGRAM_FILE), electron_file.to_string()).unwrap();

    assert!(h.link.keeps_key());
    h.link.load();
    wait_until(|| h.link.settings().connected, "carried-over Telegram connected");
    let settings = h.link.settings();
    assert_eq!(settings.bot.unwrap().username, "locust_test_bot");
    assert_eq!(settings.chat.unwrap().name, "Sam");
    assert_eq!(settings.alerts, Some(true));

    h.tg.say("/ada carried over?", 77, 77, "private", None, h.now_secs());
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn from carried-over chat");
    assert_eq!(h.started.lock().unwrap()[0], ("mate_ada".into(), "carried over?".into(), None));
    h.end("mission_1", "Yes, still here.", "completed");
    assert!(h.tg.sent_list().last().unwrap()["text"].as_str().unwrap().contains("Yes, still here."));
}
