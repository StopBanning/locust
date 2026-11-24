//! Parity and Electron carry-over tests for `apps/desktop/src/main/teammates-answer-their-person-in-slack.test.ts` (11 cases).
//! Uses an in-process fake Slack Web API + Socket Mode server and a fake Electron-compatible base64 secret box; never contacts Slack or the OS keychain.

use super::*;
use crate::approval::{Answer, Decision, Kind, Request};
use crate::chat_apps::{ChatAppSettings, SlackTokens};
use crate::chat_http::{HttpClient, HttpRequest, HttpResponse, WsCallbacks, WsClient, WsConnection};
use crate::chat_link::{ChatAlert, ChatLink, ChatLinkOptions, ChatMission, ChatStart, ChatTeammate, TokenInput};
use crate::own_models::SecretBox;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
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

fn slack_shaped(parts: &[&str]) -> String {
    parts.join("-")
}

const PERSON_ID: &str = "U0SAM00001";
const PERSON_CHANNEL: &str = "D0SAM00001";
const STRANGER_ID: &str = "U0MAL00001";
const STRANGER_CHANNEL: &str = "D0MAL00001";

#[derive(Clone, Debug)]
struct SentSlackMsg {
    ts: String,
    channel: String,
    text: String,
    thread_ts: Option<String>,
    blocks: Vec<Value>,
}

#[derive(Clone, Debug)]
struct EditedSlackMsg {
    ts: String,
    text: String,
    blocks: Vec<Value>,
}

#[derive(Clone, Debug)]
struct SlackReaction {
    ts: String,
    name: String,
    on: bool,
}

struct FakeSlackServer {
    bot_token: String,
    app_token: String,
    sent: Mutex<Vec<SentSlackMsg>>,
    edits: Mutex<Vec<EditedSlackMsg>>,
    reactions: Mutex<Vec<SlackReaction>>,
    envelopes: Mutex<Vec<Value>>,
    acked: Mutex<HashSet<String>>,
    hellos: AtomicU64,
    next_seq: AtomicU64,
    active_ws: Mutex<Option<Arc<dyn WsCallbacks>>>,
}

impl FakeSlackServer {
    fn new() -> Arc<Self> {
        let rep = "abcdef".repeat(3);
        Arc::new(Self {
            bot_token: slack_shaped(&["xoxb", "1000000000001", "2000000000002", &rep]),
            app_token: slack_shaped(&["xapp", "1", "A0LOCUST01", "3000000000003", &rep]),
            sent: Mutex::new(Vec::new()),
            edits: Mutex::new(Vec::new()),
            reactions: Mutex::new(Vec::new()),
            envelopes: Mutex::new(Vec::new()),
            acked: Mutex::new(HashSet::new()),
            hellos: AtomicU64::new(0),
            next_seq: AtomicU64::new(1),
            active_ws: Mutex::new(None),
        })
    }

    fn next_ts(&self) -> String {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let n = self.next_seq.fetch_add(1, Ordering::SeqCst);
        format!("{secs}.{n:06}")
    }

    fn deliver(&self, env: Value) {
        self.envelopes.lock().unwrap().push(env.clone());
        let ws = self.active_ws.lock().unwrap().clone().expect("active slack ws");
        ws.on_message(env.to_string());
    }

    fn say(
        &self,
        text: &str,
        from: &str,
        channel: &str,
        channel_type: &str,
        thread: Option<&str>,
        bot_id: Option<&str>,
        subtype: Option<&str>,
    ) -> String {
        let ts = self.next_ts();
        let eid = format!("env_{}", self.next_seq.fetch_add(1, Ordering::SeqCst));
        let mut ev = json!({
            "type": "message",
            "channel": channel,
            "channel_type": channel_type,
            "user": from,
            "text": text,
            "ts": ts,
        });
        if let Some(t) = thread {
            ev["thread_ts"] = json!(t);
        }
        if let Some(b) = bot_id {
            ev["bot_id"] = json!(b);
        }
        if let Some(st) = subtype {
            ev["subtype"] = json!(st);
        }
        self.deliver(json!({
            "type": "events_api",
            "envelope_id": eid,
            "payload": { "event": ev }
        }));
        ts
    }

    fn press(&self, message_ts: &str, action_id: &str, from: &str, channel: &str) {
        let eid = format!("env_{}", self.next_seq.fetch_add(1, Ordering::SeqCst));
        self.deliver(json!({
            "type": "interactive",
            "envelope_id": eid,
            "payload": {
                "type": "block_actions",
                "user": { "id": from },
                "channel": { "id": channel },
                "message": { "ts": message_ts },
                "actions": [{ "action_id": action_id, "value": action_id }]
            }
        }));
    }

    fn disconnect(&self, reason: &str) {
        let ws = self.active_ws.lock().unwrap().clone().expect("active slack ws");
        ws.on_message(json!({ "type": "disconnect", "reason": reason }).to_string());
    }

    fn read_channel(&self, channel: &str) -> Vec<String> {
        self.sent.lock().unwrap().iter().filter(|m| m.channel == channel).map(|m| m.text.clone()).collect()
    }
}

impl HttpClient for FakeSlackServer {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, String> {
        let method = req.url.rsplit('/').next().unwrap_or("");
        let auth = req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        let form: HashMap<String, String> = url::form_urlencoded::parse(req.body.as_deref().unwrap_or(&[]))
            .into_owned()
            .collect();

        let resp = match method {
            "auth.test" => {
                if auth != format!("Bearer {}", self.bot_token) {
                    json!({"ok": false, "error": "invalid_auth"})
                } else {
                    json!({"ok": true, "user_id": "U0BOT01", "user": "locust", "team": "Locust Test", "team_id": "T0LOCUST01"})
                }
            }
            "apps.connections.open" => {
                if auth != format!("Bearer {}", self.app_token) {
                    json!({"ok": false, "error": "invalid_auth"})
                } else {
                    json!({"ok": true, "url": "ws://127.0.0.1:9998/?app_id=A0LOCUST01"})
                }
            }
            "chat.postMessage" => {
                let ts = self.next_ts();
                let blocks: Vec<Value> = form
                    .get("blocks")
                    .and_then(|s| serde_json::from_str::<Vec<Value>>(s).ok())
                    .unwrap_or_default();
                self.sent.lock().unwrap().push(SentSlackMsg {
                    ts: ts.clone(),
                    channel: form.get("channel").cloned().unwrap_or_default(),
                    text: form.get("text").cloned().unwrap_or_default(),
                    thread_ts: form.get("thread_ts").cloned(),
                    blocks,
                });
                json!({"ok": true, "ts": ts})
            }
            "chat.update" => {
                let ts = form.get("ts").cloned().unwrap_or_default();
                let blocks: Vec<Value> = form
                    .get("blocks")
                    .and_then(|s| serde_json::from_str::<Vec<Value>>(s).ok())
                    .unwrap_or_default();
                self.edits.lock().unwrap().push(EditedSlackMsg {
                    ts: ts.clone(),
                    text: form.get("text").cloned().unwrap_or_default(),
                    blocks,
                });
                json!({"ok": true, "ts": ts})
            }
            "reactions.add" => {
                let ts = form.get("timestamp").cloned().unwrap_or_default();
                let name = form.get("name").cloned().unwrap_or_default();
                let mut r = self.reactions.lock().unwrap();
                if let Some(existing) = r.iter_mut().find(|x| x.ts == ts && x.name == name) {
                    existing.on = true;
                } else {
                    r.push(SlackReaction { ts, name, on: true });
                }
                json!({"ok": true})
            }
            "reactions.remove" => {
                let ts = form.get("timestamp").cloned().unwrap_or_default();
                let name = form.get("name").cloned().unwrap_or_default();
                let mut r = self.reactions.lock().unwrap();
                if let Some(existing) = r.iter_mut().find(|x| x.ts == ts && x.name == name) {
                    existing.on = false;
                } else {
                    r.push(SlackReaction { ts, name, on: false });
                }
                json!({"ok": true})
            }
            "users.info" => {
                let uid = form.get("user").map(String::as_str).unwrap_or("");
                if uid == STRANGER_ID {
                    json!({"ok": true, "user": {"name": "mallory", "real_name": "Mallory", "profile": {"display_name": "Mallory"}}})
                } else {
                    json!({"ok": true, "user": {"name": "sam", "real_name": "Sam", "profile": {"display_name": "Sam"}}})
                }
            }
            _ => json!({"ok": true}),
        };
        Ok(HttpResponse { status: 200, headers: vec![], body: resp.to_string().into_bytes() })
    }
}

struct FakeSlackWsConn {
    server: Arc<FakeSlackServer>,
}

impl WsConnection for FakeSlackWsConn {
    fn send_text(&self, text: &str) -> Result<(), String> {
        if let Ok(v) = serde_json::from_str::<Value>(text) {
            if let Some(eid) = v["envelope_id"].as_str() {
                self.server.acked.lock().unwrap().insert(eid.to_owned());
            }
        }
        Ok(())
    }
    fn close(&self, _code: u16, _reason: &str) {}
}

struct FakeSlackWsClient(Arc<FakeSlackServer>);
impl WsClient for FakeSlackWsClient {
    fn connect(&self, _url: &str, callbacks: Arc<dyn WsCallbacks>) -> Result<Arc<dyn WsConnection>, String> {
        *self.0.active_ws.lock().unwrap() = Some(callbacks.clone());
        let server = self.0.clone();
        let cb = callbacks.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            server.hellos.fetch_add(1, Ordering::SeqCst);
            cb.on_message(json!({"type": "hello"}).to_string());
        });
        Ok(Arc::new(FakeSlackWsConn { server: self.0.clone() }))
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
    sl: Arc<FakeSlackServer>,
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
    fn new(team: Vec<ChatTeammate>) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let dir = std::env::temp_dir().join(format!("locust-sl-p-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(&dir).unwrap();
        let sl = FakeSlackServer::new();
        let started = Arc::new(Mutex::new(Vec::new()));
        let decided = Arc::new(Mutex::new(Vec::new()));
        let missions = Arc::new(Mutex::new(HashMap::new()));
        let live = Arc::new(Mutex::new(HashSet::new()));
        let waiting: Arc<Mutex<Vec<(Request, Option<String>)>>> = Arc::new(Mutex::new(Vec::new()));
        let refusal: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let next = Arc::new(AtomicU64::new(0));
        let link_slot: Arc<Mutex<Option<Arc<ChatLink>>>> = Arc::new(Mutex::new(None));

        let link = create_slack_link(
            SlackOptions {
                http: sl.clone(),
                ws: Arc::new(FakeSlackWsClient(sl.clone())),
                api: Some(Arc::new(|| "http://127.0.0.1:1".into())),
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
        Self { dir, sl, link, started, decided, missions, live, waiting, refusal }
    }

    fn paired(team: Vec<ChatTeammate>) -> Self {
        let h = Self::new(team);
        let s = h.link.set_token(TokenInput::Slack(SlackTokens {
            bot: h.sl.bot_token.clone(),
            app: h.sl.app_token.clone(),
        }));
        wait_until(|| h.sl.hellos.load(Ordering::SeqCst) > 0 && h.link.settings().connected, "slack hello");
        let code = s.code.unwrap().code;
        h.sl.say(&code, PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
        wait_until(|| h.sl.read_channel(PERSON_CHANNEL).iter().any(|t| t.starts_with("Paired with Locust on *DESK-PC*")), "pairing message");
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

// case: checks both tokens with Slack, keeps them encrypted, and says which one is wrong
#[test]
fn checks_both_tokens_with_slack_keeps_them_encrypted_and_says_which_one_is_wrong() {
    let h = Harness::new(vec![ada(), wren()]);
    let bot = h.sl.bot_token.clone();
    let app = h.sl.app_token.clone();
    assert!(h.link.set_token(TokenInput::Slack(SlackTokens { bot: "not a token".into(), app: app.clone() })).message.unwrap().contains("not the bot\u{2019}s token"));
    assert!(h.link.set_token(TokenInput::Slack(SlackTokens { bot: app.clone(), app: bot.clone() })).message.unwrap().contains("in each other\u{2019}s places"));
    assert!(h.link.set_token(TokenInput::Slack(SlackTokens { bot: bot.clone(), app: slack_shaped(&["xapp", "nope"]) })).message.unwrap().contains("not the app\u{2019}s own token"));
    assert!(h.link.set_token(TokenInput::Slack(SlackTokens { bot: format!("{bot}0"), app: app.clone() })).message.unwrap().contains("Slack does not know that bot token"));
    assert!(h.link.set_token(TokenInput::Slack(SlackTokens { bot: bot.clone(), app: format!("{app}0") })).message.unwrap().contains("Slack does not take that app token"));

    let kept = h.link.set_token(TokenInput::Slack(SlackTokens { bot: format!(" {bot} "), app: format!("{app}\n") }));
    let b = kept.bot.unwrap();
    assert_eq!(b.username, "locust");
    assert_eq!(b.name, "locust");
    assert_eq!(b.team.as_deref(), Some("Locust Test"));
    assert_eq!(b.open.as_deref(), Some("https://slack.com/app_redirect?app=A0LOCUST01&team=T0LOCUST01"));
    assert_eq!(kept.code.unwrap().code.len(), 8);
    let file = std::fs::read_to_string(h.dir.join(SLACK_FILE)).unwrap();
    assert!(!file.contains(&bot));
    assert!(!file.contains(&app));
    assert!(!file.contains("xox"));
    wait_until(|| h.link.settings().connected, "connected");
}

// case: pairs the one account that sends the code, names it, and ignores everyone else
#[test]
fn pairs_the_one_account_that_sends_the_code_names_it_and_ignores_everyone_else() {
    let h = Harness::paired(vec![ada(), wren()]);
    let chat = h.link.settings().chat.unwrap();
    assert_eq!(chat.name, "Sam");
    assert_eq!(chat.username.as_deref(), Some("sam"));
    let first_msg = h.sl.read_channel(PERSON_CHANNEL)[0].clone();
    assert!(first_msg.contains("*Locust on DESK-PC*"));
    assert!(first_msg.contains("\u{201c}Ada:\u{201d} and a message writes to Ada."));

    h.sl.say("1234 5678", STRANGER_ID, STRANGER_CHANNEL, "im", None, None, None);
    h.sl.say("Ada: delete everything", STRANGER_ID, STRANGER_CHANNEL, "im", None, None, None);
    h.sl.say("Ada: and this", STRANGER_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.sl.read_channel(STRANGER_CHANNEL).len() == 1, "wrong code said");
    assert!(h.sl.read_channel(STRANGER_CHANNEL)[0].contains("That code does not pair this account"));
    h.sl.say("Ada: in a channel", PERSON_ID, "C0GENERAL1", "channel", None, None, None);
    h.sl.say("Ada: channel on dm id", PERSON_ID, PERSON_CHANNEL, "channel", None, None, None);
    h.sl.say("Ada: from a bot", PERSON_ID, PERSON_CHANNEL, "im", None, Some("B0OTHER001"), None);
    h.sl.say("Ada: an edit", PERSON_ID, PERSON_CHANNEL, "im", None, None, Some("message_changed"));
    std::thread::sleep(Duration::from_millis(50));
    assert!(h.started.lock().unwrap().is_empty());
}

// case: takes “Ada:” to Ada, answers in the chat, and carries a conversation on in its thread
#[test]
fn takes_ada_to_ada_answers_in_the_chat_and_carries_a_conversation_on_in_its_thread() {
    let h = Harness::paired(vec![ada(), wren()]);
    let asked = h.sl.say("Ada: what changed in tides today?", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 1, "first turn");
    assert_eq!(h.started.lock().unwrap()[0], ("mate_ada".into(), "what changed in tides today?".into(), None));
    wait_until(|| h.sl.reactions.lock().unwrap().iter().any(|r| r.ts == asked && r.name == "eyes" && r.on), "eyes reaction");
    h.end("mission_1", "**Three** commits:\n\n- `fix: tides`\n- docs <b>\n\n```ts\nconst a = 1 < 2\n```", "completed");
    let answer = h.sl.sent.lock().unwrap().last().unwrap().clone();
    assert!(answer.thread_ts.is_none());
    assert!(answer.text.starts_with("*Ada*\n\n*Three* commits:\n\n\u{2022} `fix: tides`\n\u{2022} docs &lt;b&gt;"), "{}", answer.text);
    assert!(answer.text.contains("```\nconst a = 1 &lt; 2\n```"));
    assert!(!h.sl.reactions.lock().unwrap().iter().find(|r| r.ts == asked).unwrap().on);

    h.sl.say("and yesterday?", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 2, "follow-up");
    assert_eq!(h.started.lock().unwrap()[1], ("mate_ada".into(), "and yesterday?".into(), Some("mission_1".into())));
    h.end("mission_2", "Two more.", "completed");

    h.sl.say("more on this", PERSON_ID, PERSON_CHANNEL, "im", Some(&answer.ts), None, None);
    wait_until(|| h.started.lock().unwrap().len() == 3, "thread turn");
    assert_eq!(h.started.lock().unwrap()[2], ("mate_ada".into(), "more on this".into(), Some("mission_1".into())));
    h.end("mission_3", "More, here.", "completed");
    assert_eq!(h.sl.sent.lock().unwrap().last().unwrap().thread_ts.as_deref(), Some(answer.ts.as_str()));

    h.sl.say("new", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.sl.read_channel(PERSON_CHANNEL).last().is_some_and(|t| t.contains("starts a new conversation with Ada")), "new");
    h.sl.say("start over", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 4, "fresh turn");
    assert_eq!(h.started.lock().unwrap()[3], ("mate_ada".into(), "start over".into(), None));

    h.sl.say("who", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.sl.read_channel(PERSON_CHANNEL).last().is_some_and(|t| t.starts_with("*Your teammates*")), "who");
    assert!(h.sl.read_channel(PERSON_CHANNEL).last().unwrap().contains("*Wren*, Code &amp; Migrations"));
}

// case: acknowledges every envelope at once, and acts once on a message Slack sends twice
#[test]
fn acknowledges_every_envelope_at_once_and_acts_once_on_a_message_slack_sends_twice() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.sl.say("Ada: once", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn once");
    let last_env = h.sl.envelopes.lock().unwrap().last().unwrap().clone();
    h.sl.deliver(last_env);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(h.started.lock().unwrap().len(), 1);
    let acked = h.sl.acked.lock().unwrap();
    for env in h.sl.envelopes.lock().unwrap().iter() {
        let eid = env["envelope_id"].as_str().unwrap();
        assert!(acked.contains(eid), "envelope {eid} not acked");
    }
}

// case: asks who a message is for with buttons, when nobody is being written to
#[test]
fn asks_who_a_message_is_for_with_buttons_when_nobody_is_being_written_to() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.sl.say("can someone look at the build?", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.sl.sent.lock().unwrap().iter().any(|m| m.text == "Who is this for?"), "who question");
    let asked = h.sl.sent.lock().unwrap().iter().find(|m| m.text == "Who is this for?").unwrap().clone();
    let elems = asked.blocks.iter().find(|b| b["type"] == "actions").unwrap()["elements"].as_array().unwrap();
    let labels: Vec<&str> = elems.iter().map(|e| e["text"]["text"].as_str().unwrap()).collect();
    assert_eq!(labels, vec!["Ada", "Wren"]);
    let wren_action = elems[1]["action_id"].as_str().unwrap();
    h.sl.press(&asked.ts, wren_action, PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.started.lock().unwrap().len() == 1, "held message sent");
    assert_eq!(h.started.lock().unwrap()[0], ("mate_wren".into(), "can someone look at the build?".into(), None));
    let edit = h.sl.edits.lock().unwrap().last().unwrap().clone();
    assert_eq!(edit.ts, asked.ts);
    assert_eq!(edit.text, "To *Wren*.");
    assert!(edit.blocks.is_empty());
}

// case: brings a card from its own turn with Allow once and Deny, and answers it by the window’s path
#[test]
fn brings_a_card_from_its_own_turn_with_allow_once_and_deny_and_answers_it_by_the_windows_path() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.sl.say("Wren: run the tests", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    let fake_bot = slack_shaped(&["xoxb", "1000000000001", "2000000000002", &"abcdef".repeat(3)]);
    let req = make_card("appr_1", "mission_1", &format!("Run npm test --token={fake_bot} in tides"), "npm test");
    h.waiting.lock().unwrap().push((req.clone(), Some("mate_wren".into())));
    h.link.card_raised(&req, Some("Wren"));
    wait_until(|| h.sl.sent.lock().unwrap().iter().any(|m| m.text.starts_with("*Wren needs your approval*")), "card");
    let posted = h.sl.sent.lock().unwrap().iter().find(|m| m.text.starts_with("*Wren needs your approval*")).unwrap().clone();
    let elems = posted.blocks.iter().find(|b| b["type"] == "actions").unwrap()["elements"].as_array().unwrap();
    let pairs: Vec<(&str, &str)> = elems.iter().map(|e| (e["text"]["text"].as_str().unwrap(), e["style"].as_str().unwrap())).collect();
    assert_eq!(pairs, vec![("Allow once", "primary"), ("Deny", "danger")]);
    assert!(!posted.text.contains(&fake_bot));
    assert!(posted.text.contains("[secret-shaped text removed]"));

    let allow = elems[0]["action_id"].as_str().unwrap();
    let deny = elems[1]["action_id"].as_str().unwrap();
    h.sl.press(&posted.ts, allow, STRANGER_ID, STRANGER_CHANNEL);
    std::thread::sleep(Duration::from_millis(50));
    assert!(h.decided.lock().unwrap().is_empty());

    h.sl.press(&posted.ts, allow, PERSON_ID, PERSON_CHANNEL);
    wait_until(|| h.decided.lock().unwrap().len() == 1, "answer");
    assert!(matches!(
        &h.decided.lock().unwrap()[0],
        Answer::Decision { approval_id, decision: Decision::ApproveOnce, .. } if approval_id == "appr_1"
    ));
    wait_until(|| h.sl.edits.lock().unwrap().iter().any(|e| e.ts == posted.ts), "card settled");
    let edited = h.sl.edits.lock().unwrap().iter().find(|e| e.ts == posted.ts).unwrap().clone();
    assert!(edited.text.contains("*Allowed once.*"));
    assert!(edited.blocks.is_empty());

    h.sl.press(&posted.ts, deny, PERSON_ID, PERSON_CHANNEL);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(h.decided.lock().unwrap().len(), 1);
}

// case: tells the person here what needs them from any run, once they turn it on (0.837)
#[test]
fn tells_the_person_here_what_needs_them_from_any_run_once_they_turn_it_on() {
    let h = Harness::paired(vec![ada(), wren()]);
    let req = make_card("appr_away", "mission_window", "Run npm test in tides", "npm test");
    h.waiting.lock().unwrap().push((req.clone(), None));
    let alert = ChatAlert {
        cards: vec![(req, Some("Ada".into()))],
        lines: vec![("Scout".into(), "One of your pull requests needs you.".into())],
    };
    h.link.alert(alert.clone());
    std::thread::sleep(Duration::from_millis(50));
    assert!(!h.sl.sent.lock().unwrap().iter().any(|m| m.text.contains("pull requests") || m.text.contains("needs your approval")));
    assert_eq!(h.link.set_alerts(true).alerts, Some(true));
    h.link.alert(alert);
    wait_until(|| h.sl.sent.lock().unwrap().iter().any(|m| m.text.starts_with("*Ada needs your approval*")), "alert card");
    wait_until(|| h.sl.sent.lock().unwrap().iter().any(|m| m.text == "*Scout*\nOne of your pull requests needs you."), "Scout line");
}

// case: takes anything shaped like a key out of an answer before Slack has it
#[test]
fn takes_anything_shaped_like_a_key_out_of_an_answer_before_slack_has_it() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.sl.say("Ada: the tokens?", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn");
    let fake_app = slack_shaped(&["xapp", "1", "A0LOCUST01", "3000000000003", &"abcdef".repeat(3)]);
    let fake_bot = slack_shaped(&["xoxb", "1000000000001", "2000000000002", &"abcdef".repeat(3)]);
    h.end("mission_1", &format!("The app token is {fake_app} and the bot one is {fake_bot}."), "completed");
    let sent = h.sl.sent.lock().unwrap().last().unwrap().text.clone();
    assert!(!sent.contains(&fake_app));
    assert!(!sent.contains(&fake_bot));
    assert!(sent.contains("[secret-shaped text removed]"));
}

// case: splits a long answer into messages Slack takes, all in the thread it was asked in
#[test]
fn splits_a_long_answer_into_messages_slack_takes_all_in_the_thread_it_was_asked_in() {
    let h = Harness::paired(vec![ada(), wren()]);
    let root = h.sl.say("Ada: start", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 1, "first turn");
    h.end("mission_1", "Started.", "completed");
    h.sl.say("show me everything", PERSON_ID, PERSON_CHANNEL, "im", Some(&root), None, None);
    wait_until(|| h.started.lock().unwrap().len() == 2, "thread turn");
    let before = h.sl.sent.lock().unwrap().len();
    let long = format!("{}\n\n| a | b |\n|---|---|\n| 1 | 2 |", "A long line of findings. ".repeat(400));
    h.end("mission_2", &long, "completed");
    let parts = h.sl.sent.lock().unwrap()[before..].to_vec();
    assert!(parts.len() > 1);
    assert!(parts[0].text.starts_with("*Ada*"));
    assert!(parts.last().unwrap().text.contains("```\n| a | b |\n| 1 | 2 |\n```"));
    for part in &parts {
        assert!(part.text.chars().count() <= 4_000);
        assert_eq!(part.thread_ts.as_deref(), Some(root.as_str()));
    }
}

// case: moves to a new connection when Slack refreshes it, and stops when Socket Mode is turned off
#[test]
fn moves_to_a_new_connection_when_slack_refreshes_it_and_stops_when_socket_mode_is_turned_off() {
    let h = Harness::paired(vec![ada(), wren()]);
    h.sl.disconnect("refresh_requested");
    wait_until(|| h.sl.hellos.load(Ordering::SeqCst) == 2, "new connection");
    h.sl.say("Ada: still there?", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn on new connection");
    h.sl.disconnect("link_disabled");
    wait_until(|| h.link.settings().message.as_deref().unwrap_or("").contains("Socket Mode is off"), "socket mode off");
    assert!(!h.link.settings().connected);
}

// case: stops at the organization’s word, and forgets the tokens and the chat on Remove
#[test]
fn stops_at_the_organizations_word_and_forgets_the_tokens_and_the_chat_on_remove() {
    let h = Harness::paired(vec![ada(), wren()]);
    *h.refusal.lock().unwrap() = Some("Teammates in chat apps are off. Set by your organization.".into());
    h.sl.say("Ada: anything", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.sl.read_channel(PERSON_CHANNEL).last().is_some_and(|t| t.contains("Teammates in chat apps are off")), "refusal");
    assert!(h.started.lock().unwrap().is_empty());
    *h.refusal.lock().unwrap() = None;
    assert_eq!(h.link.remove(), ChatAppSettings::default());
    assert!(!h.dir.join(SLACK_FILE).exists());
}

/// Carry-over proof: a `slack.json` written by Electron (schemaVersion 1 + base64-sealed `{"bot","app"}` tokens + paired chat)
/// is read, decrypted, connected, and answers `Ada:` in Rust without re-pairing.
#[test]
fn electron_slack_pairing_and_encrypted_tokens_carry_over_to_rust() {
    let h = Harness::new(vec![ada(), wren()]);
    let secret_json = json!({ "bot": h.sl.bot_token, "app": h.sl.app_token }).to_string();
    let sealed = ElectronSecretBox::seal_electron(&secret_json);
    let electron_file = json!({
        "schemaVersion": 1,
        "token": sealed,
        "bot": {
            "id": "U0BOT01",
            "username": "locust",
            "name": "locust",
            "extra": { "team": "Locust Test", "teamId": "T0LOCUST01", "app": "A0LOCUST01" }
        },
        "chat": {
            "place": PERSON_CHANNEL,
            "user": PERSON_ID,
            "name": "Sam",
            "username": "sam",
            "pairedAt": "2026-11-01T10:00:00.000Z"
        },
        "alerts": true
    });
    std::fs::write(h.dir.join(SLACK_FILE), electron_file.to_string()).unwrap();

    assert!(h.link.keeps_key());
    h.link.load();
    wait_until(|| h.link.settings().connected, "carried-over Slack connected");
    let settings = h.link.settings();
    assert_eq!(settings.bot.unwrap().team.as_deref(), Some("Locust Test"));
    assert_eq!(settings.chat.unwrap().name, "Sam");
    assert_eq!(settings.alerts, Some(true));

    h.sl.say("Ada: carried over?", PERSON_ID, PERSON_CHANNEL, "im", None, None, None);
    wait_until(|| h.started.lock().unwrap().len() == 1, "turn from carried-over chat");
    assert_eq!(h.started.lock().unwrap()[0], ("mate_ada".into(), "carried over?".into(), None));
    h.end("mission_1", "Yes, Slack carried over.", "completed");
    assert!(h.sl.read_channel(PERSON_CHANNEL).last().unwrap().contains("Yes, Slack carried over."));
}
