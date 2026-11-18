use crate::approval::{Answer, Decision, Kind, Question, QuestionOption, Request};
use crate::chat_apps::{
    ChatAppBot, ChatAppCode, ChatAppId, ChatAppLink, ChatAppSettings,
};
use crate::chat_link::{
    ChatAlert, ChatApp, ChatBot, ChatCommand, ChatEvents, ChatIncoming, ChatLink, ChatLinkOptions, ChatMission,
    ChatPress, ChatSendExtra, ChatStart, ChatTeammate, ChatTransport, ChatWords, MAX_ANSWER, ParsedCommand,
    TokenInput, chat_answer, create_chat_link, exact_line,
};
use crate::own_models::SecretBox;
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

struct FakeSecretBox;
impl SecretBox for FakeSecretBox {
    fn available(&self) -> bool {
        true
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        let hex: String = text.bytes().map(|b| format!("{:02x}", b ^ 0x5a)).collect();
        Ok(format!("sealed:{hex}"))
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        let hex = sealed.strip_prefix("sealed:").ok_or("bad seal")?;
        let mut out = Vec::with_capacity(hex.len() / 2);
        for i in (0..hex.len()).step_by(2) {
            let b = u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| "bad hex")?;
            out.push(b ^ 0x5a);
        }
        String::from_utf8(out).map_err(|_| "bad utf8".into())
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct SentMsg {
    place: String,
    text: String,
    extra: ChatSendExtra,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct EditedMsg {
    place: String,
    message_id: String,
    text: String,
}

struct FakeTransport {
    sent: Mutex<Vec<SentMsg>>,
    edited: Mutex<Vec<EditedMsg>>,
    commands: Mutex<Vec<Vec<ChatCommand>>>,
    next_id: AtomicU64,
    stopped: std::sync::atomic::AtomicBool,
}

impl FakeTransport {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            sent: Mutex::new(Vec::new()),
            edited: Mutex::new(Vec::new()),
            commands: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(100),
            stopped: std::sync::atomic::AtomicBool::new(false),
        })
    }
}

impl ChatTransport for FakeTransport {
    fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }
    fn send(&self, place: &str, text: &str, extra: &ChatSendExtra) -> Option<String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst).to_string();
        self.sent.lock().unwrap().push(SentMsg {
            place: place.to_owned(),
            text: text.to_owned(),
            extra: extra.clone(),
        });
        Some(id)
    }
    fn edit(&self, place: &str, message: &str, text: &str) {
        self.edited.lock().unwrap().push(EditedMsg {
            place: place.to_owned(),
            message_id: message.to_owned(),
            text: text.to_owned(),
        });
    }
    fn commands(&self, list: &[ChatCommand]) -> Result<(), String> {
        self.commands.lock().unwrap().push(list.to_vec());
        Ok(())
    }
}

fn test_words() -> ChatWords {
    ChatWords {
        address: |cmd, _| format!("/{cmd}"),
        own: |cmd| format!("/{cmd}"),
        who_line: |cmd, name, role| format!("/{cmd} {name}, {role}"),
        carry_on: "Reply to an answer to carry on that conversation.",
        reply_to_card_or_own: "Or reply to this message with an answer of your own.",
        reply_to_card_only: "Reply to this message with your answer.",
        wrong_code: "Wrong code.",
        help_more: None,
        no_token: "Paste a token first.",
        unreadable: "Token unreadable.",
    }
}

struct FakeApp {
    words: ChatWords,
    transport: Arc<FakeTransport>,
    events: Mutex<Option<Arc<dyn ChatEvents>>>,
    code_seq: AtomicU64,
}

impl FakeApp {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            words: test_words(),
            transport: FakeTransport::new(),
            events: Mutex::new(None),
            code_seq: AtomicU64::new(1),
        })
    }
}

impl ChatApp for FakeApp {
    fn id(&self) -> ChatAppId {
        ChatAppId::Telegram
    }
    fn file(&self) -> &'static str {
        "fake-chat.json"
    }
    fn escape(&self, text: &str) -> String {
        text.to_owned()
    }
    fn bold(&self, text: &str) -> String {
        format!("*{text}*")
    }
    fn italic(&self, text: &str) -> String {
        format!("_{text}_")
    }
    fn answer_parts(&self, name: &str, markdown: &str) -> Vec<String> {
        vec![format!("*{name}*\n\n{markdown}")]
    }
    fn parse(&self, text: &str, _bot: &ChatBot) -> Option<ParsedCommand> {
        let t = text.trim();
        let rest = t.strip_prefix('/')?;
        let mut parts = rest.splitn(2, char::is_whitespace);
        let cmd = parts.next()?.to_lowercase();
        let body = parts.next().unwrap_or("").trim().to_owned();
        Some(ParsedCommand { command: cmd, rest: body })
    }
    fn code_in(&self, text: &str, bot: &ChatBot) -> Option<String> {
        let p = self.parse(text, bot)?;
        if p.command == "start" && !p.rest.is_empty() { Some(p.rest) } else { None }
    }
    fn fresh_code(&self) -> String {
        let n = self.code_seq.fetch_add(1, Ordering::SeqCst);
        format!("code_{n}")
    }
    fn pairing(&self, bot: &ChatBot, code: &str, until: &str) -> (Option<ChatAppLink>, Option<ChatAppCode>) {
        (Some(ChatAppLink { url: format!("https://t.me/{}?start={code}", bot.username), until: until.to_owned() }), None)
    }
    fn shown(&self, bot: &ChatBot) -> ChatAppBot {
        ChatAppBot { username: bot.username.clone(), name: bot.name.clone(), invite: None, team: None, open: None }
    }
    fn shape(&self, given: &TokenInput) -> Result<String, String> {
        match given {
            TokenInput::Single(s) if s.starts_with("tok_") => Ok(s.clone()),
            _ => Err("bad token shape".into()),
        }
    }
    fn verify(&self, secret: &str) -> Result<ChatBot, String> {
        if secret == "tok_bad" {
            return Err("rejected".into());
        }
        Ok(ChatBot { id: "99".into(), username: "locust_bot".into(), name: "Locust".into(), extra: BTreeMap::new() })
    }
    fn keeps(&self, secret: &str) -> bool {
        secret.starts_with("tok_")
    }
    fn connect(&self, _secret: &str, _bot: &ChatBot, events: Arc<dyn ChatEvents>) -> Arc<dyn ChatTransport> {
        *self.events.lock().unwrap() = Some(events.clone());
        events.connected();
        self.transport.clone()
    }
    fn words(&self) -> &ChatWords {
        &self.words
    }
}

#[allow(dead_code)]
struct Harness {
    dir: PathBuf,
    app: Arc<FakeApp>,
    link: Arc<ChatLink>,
    clock: Arc<AtomicU64>,
    started: Arc<Mutex<Vec<(String, String, Option<String>)>>>,
    missions: Arc<Mutex<HashMap<String, ChatMission>>>,
    live: Arc<Mutex<std::collections::HashSet<String>>>,
    carried: Arc<Mutex<std::collections::HashSet<String>>>,
    decided: Arc<Mutex<Vec<Answer>>>,
    waiting: Arc<Mutex<Vec<(Request, Option<String>)>>>,
    refusal: Arc<Mutex<Option<String>>>,
    changes: Arc<Mutex<Vec<ChatAppSettings>>>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.link.dispose();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn temp_dir(label: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("locust-chat-link-{label}-{}-{n}", std::process::id()));
    let _ = std::fs::create_dir_all(&p);
    p
}

fn make_harness(label: &str, team_list: Vec<ChatTeammate>) -> Harness {
    let dir = temp_dir(label);
    let app = FakeApp::new();
    let clock = Arc::new(AtomicU64::new(1_700_000_000_000));
    let started = Arc::new(Mutex::new(Vec::new()));
    let missions = Arc::new(Mutex::new(HashMap::new()));
    let live = Arc::new(Mutex::new(std::collections::HashSet::new()));
    let carried = Arc::new(Mutex::new(std::collections::HashSet::new()));
    let decided = Arc::new(Mutex::new(Vec::new()));
    let waiting = Arc::new(Mutex::new(Vec::new()));
    let refusal = Arc::new(Mutex::new(None));
    let changes = Arc::new(Mutex::new(Vec::new()));
    let team_arc = Arc::new(team_list);
    let seq = Arc::new(AtomicU64::new(1));

    let link = create_chat_link(
        app.clone(),
        ChatLinkOptions {
            directory: dir.clone(),
            secrets: Arc::new(FakeSecretBox),
            refusal: {
                let r = refusal.clone();
                Arc::new(move || r.lock().unwrap().clone())
            },
            computer: Arc::new(|| "DESK-PC".into()),
            team: {
                let t = team_arc.clone();
                Arc::new(move || (*t).clone())
            },
            start: {
                let st = started.clone();
                let lv = live.clone();
                let sq = seq.clone();
                Arc::new(move |tid, text, follow| {
                    st.lock().unwrap().push((tid.to_owned(), text.to_owned(), follow.map(str::to_owned)));
                    let id = format!("m_{}", sq.fetch_add(1, Ordering::SeqCst));
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
            carried: {
                let cr = carried.clone();
                Arc::new(move |id| cr.lock().unwrap().contains(id))
            },
            waiting: {
                let wt = waiting.clone();
                Arc::new(move || wt.lock().unwrap().clone())
            },
            decide: {
                let dc = decided.clone();
                let wt = waiting.clone();
                Arc::new(move |ans| {
                    dc.lock().unwrap().push(ans.clone());
                    let id = ans.approval_id().to_owned();
                    let mut w = wt.lock().unwrap();
                    let had = w.iter().any(|(r, _)| r.approval_id == id);
                    w.retain(|(r, _)| r.approval_id != id);
                    had
                })
            },
            changed: {
                let ch = changes.clone();
                Some(Arc::new(move |s| ch.lock().unwrap().push(s)))
            },
            note: None,
            now: {
                let ck = clock.clone();
                Some(Arc::new(move || ck.load(Ordering::SeqCst)))
            },
            typing_ms: Some(1000),
        },
    );

    Harness { dir, app, link, clock, started, missions, live, carried, decided, waiting, refusal, changes }
}

fn two_mates() -> Vec<ChatTeammate> {
    vec![
        ChatTeammate { teammate_id: "t_ada".into(), name: "Ada".into(), role: "Research".into() },
        ChatTeammate { teammate_id: "t_wren".into(), name: "Wren".into(), role: "Code".into() },
    ]
}

fn incoming(from: &str, place: &str, id: &str, text: Option<&str>, sent_at: u64) -> ChatIncoming {
    ChatIncoming {
        from: from.into(),
        bot: false,
        place: place.into(),
        direct: true,
        id: id.into(),
        text: text.map(str::to_owned),
        sent_at,
        reply_to: None,
        answer_to: Some(id.into()),
        name: Some("Colin".into()),
        username: Some("colin".into()),
        command: None,
        respond: None,
    }
}

fn pair_chat(h: &Harness) {
    let settings = h.link.set_token(TokenInput::Single("tok_good".into()));
    assert!(settings.connected);
    let url = settings.link.unwrap().url;
    let code = url.split("start=").nth(1).unwrap();
    let now = h.clock.load(Ordering::SeqCst);
    h.link.handle_message(incoming("user_1", "chat_1", "1", Some(&format!("/start {code}")), now));
}

#[test]
fn chat_answer_reads_final_message_failure_stop_empty_and_truncates() {
    let completed = ChatMission {
        mission_id: "m1".into(),
        phase: "completed".into(),
        events: vec![
            json!({"type": "message.delta", "payload": {"itemId": "a", "operation": "append", "text": "Hello ", "final": false}}),
            json!({"type": "message.delta", "payload": {"itemId": "a", "operation": "append", "text": "world.", "final": true}}),
        ],
        started_by_kind: None,
        continues_from_mission_id: None,
        host_failure_message: None,
    };
    assert_eq!(chat_answer(&completed, "Ada"), "Hello world.");

    let failed = ChatMission {
        mission_id: "m2".into(),
        phase: "failed".into(),
        events: vec![json!({"type": "run.failed", "payload": {"message": "boom"}})],
        started_by_kind: None,
        continues_from_mission_id: None,
        host_failure_message: None,
    };
    assert_eq!(chat_answer(&failed, "Ada"), "Ada\u{2019}s turn failed: boom");

    let stopped = ChatMission {
        mission_id: "m3".into(),
        phase: "cancelled".into(),
        events: vec![],
        started_by_kind: None,
        continues_from_mission_id: None,
        host_failure_message: None,
    };
    assert_eq!(chat_answer(&stopped, "Ada"), "Ada\u{2019}s turn stopped before it finished. What it did is in Locust.");

    let empty = ChatMission {
        mission_id: "m4".into(),
        phase: "completed".into(),
        events: vec![],
        started_by_kind: None,
        continues_from_mission_id: None,
        host_failure_message: None,
    };
    assert_eq!(chat_answer(&empty, "Ada"), "Ada finished without a written answer. What it did is in Locust.");

    let huge = "x".repeat(MAX_ANSWER + 50);
    let long = ChatMission {
        mission_id: "m5".into(),
        phase: "completed".into(),
        events: vec![json!({"type": "message.delta", "payload": {"itemId": "a", "operation": "replace", "text": huge, "final": true}})],
        started_by_kind: None,
        continues_from_mission_id: None,
        host_failure_message: None,
    };
    let ans = chat_answer(&long, "Ada");
    assert_eq!(ans.chars().count(), MAX_ANSWER + 1);
    assert!(ans.ends_with('…'));
}

#[test]
fn exact_line_takes_first_non_empty_omits_if_in_summary_and_caps_at_200() {
    assert_eq!(exact_line("Run a command", "\n  cargo test  \nmore"), Some("cargo test".into()));
    assert_eq!(exact_line("Run cargo test", "cargo test"), None);
    let long = "a".repeat(250);
    let got = exact_line("Run", &long).unwrap();
    assert_eq!(got.chars().count(), 201);
    assert!(got.ends_with('…'));
}

#[test]
fn pairs_one_chat_ignores_strangers_and_rotates_after_ten_wrong_codes() {
    let h = make_harness("pair", two_mates());
    let s = h.link.set_token(TokenInput::Single("tok_good".into()));
    assert!(h.link.keeps_key());
    let disk = std::fs::read_to_string(h.dir.join("fake-chat.json")).unwrap();
    assert!(!disk.contains("tok_good"));

    let now = h.clock.load(Ordering::SeqCst);
    let old_url = s.link.unwrap().url;
    for i in 0..10 {
        h.link.handle_message(incoming("stranger", "chat_9", &format!("w{i}"), Some("/start wrong"), now));
    }
    let after = h.link.settings();
    assert_ne!(after.link.as_ref().unwrap().url, old_url);
    assert!(after.message.unwrap().contains("Too many wrong codes"));
    // Only 1 wrong code reply sent in the minute.
    assert_eq!(h.app.transport.sent.lock().unwrap().len(), 1);

    // Pair with the rotated code.
    let new_code = after.link.unwrap().url.split("start=").nth(1).unwrap().to_owned();
    h.link.handle_message(incoming("user_1", "chat_1", "p1", Some(&format!("/start {new_code}")), now));
    assert_eq!(h.link.settings().chat.unwrap().name, "Colin");

    // Stranger message is ignored.
    h.link.handle_message(incoming("stranger", "chat_9", "s1", Some("/ada hi"), now));
    assert!(h.started.lock().unwrap().is_empty());
}

#[test]
fn routes_commands_followups_replies_new_and_stale_messages() {
    let h = make_harness("route", two_mates());
    pair_chat(&h);
    let now = h.clock.load(Ordering::SeqCst);

    // /ada hello starts m_1
    h.link.handle_message(incoming("user_1", "chat_1", "10", Some("/ada check build"), now));
    assert_eq!(h.started.lock().unwrap().as_slice(), &[("t_ada".into(), "check build".into(), None)]);

    // Complete m_1 -> answer sent back
    h.live.lock().unwrap().remove("m_1");
    h.missions.lock().unwrap().insert("m_1".into(), ChatMission {
        mission_id: "m_1".into(),
        phase: "completed".into(),
        events: vec![json!({"type": "message.delta", "payload": {"itemId": "x", "operation": "append", "text": "Green.", "final": true}})],
        started_by_kind: None,
        continues_from_mission_id: None,
        host_failure_message: None,
    });
    h.link.on_run_ended("m_1");
    let ans_msg_id = {
        let sent = h.app.transport.sent.lock().unwrap();
        assert!(sent.last().unwrap().text.contains("Green."));
        (100 + sent.len() as u64 - 1).to_string()
    };

    // Switch to Wren with "Wren: status"
    h.link.handle_message(incoming("user_1", "chat_1", "11", Some("Wren: status"), now));
    assert_eq!(h.started.lock().unwrap().last().unwrap(), &("t_wren".into(), "status".into(), None));

    // Reply to Ada's answer -> continues m_1 with Ada even though current is Wren
    let mut rep = incoming("user_1", "chat_1", "12", Some("and lint?"), now);
    rep.reply_to = Some(ans_msg_id);
    h.link.handle_message(rep);
    assert_eq!(h.started.lock().unwrap().last().unwrap(), &("t_ada".into(), "and lint?".into(), Some("m_1".into())));

    // /new starts fresh on current (Ada)
    h.link.handle_message(incoming("user_1", "chat_1", "13", Some("/new fresh topic"), now));
    assert_eq!(h.started.lock().unwrap().last().unwrap(), &("t_ada".into(), "fresh topic".into(), None));

    // Stale message (> 10 min old) does not start a turn
    h.link.handle_message(incoming("user_1", "chat_1", "14", Some("old msg"), now - 11 * 60_000));
    assert_eq!(h.started.lock().unwrap().len(), 4);
    assert!(h.app.transport.sent.lock().unwrap().last().unwrap().text.contains("while Locust was closed"));
}

#[test]
fn raises_and_settles_approval_and_question_cards() {
    let h = make_harness("cards", two_mates());
    pair_chat(&h);
    let now = h.clock.load(Ordering::SeqCst);

    h.link.handle_message(incoming("user_1", "chat_1", "20", Some("/ada run"), now));
    let req = Request {
        approval_id: "appr_1".into(),
        mission_id: "m_1".into(),
        runtime: "claude".into(),
        kind: Kind::Command,
        summary: "Run a command".into(),
        detail: "cargo test".into(),
        cwd: None,
        requested_at: "2026-11-17T00:00:00Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        claude_input: None,
        questions: vec![],
    };
    h.waiting.lock().unwrap().push((req.clone(), Some("t_ada".into())));
    h.link.card_raised(&req, Some("Ada"));

    let (card_msg_id, allow_data) = {
        let sent = h.app.transport.sent.lock().unwrap();
        let last = sent.last().unwrap();
        assert!(last.text.contains("Ada needs your approval"));
        assert!(last.text.contains("cargo test"));
        assert_eq!(last.extra.buttons[0].len(), 2);
        assert_eq!(last.extra.buttons[0][0].label, "Allow once");
        assert_eq!(last.extra.buttons[0][1].label, "Deny");
        let msg_id = (100 + sent.len() as u64 - 1).to_string();
        (msg_id, last.extra.buttons[0][0].data.clone())
    };

    let answered_toast = Arc::new(Mutex::new(None));
    let toast_clone = answered_toast.clone();
    h.link.handle_press(ChatPress {
        from: "user_1".into(),
        place: "chat_1".into(),
        message: card_msg_id,
        data: allow_data,
        answer: Arc::new(move |t| *toast_clone.lock().unwrap() = t.map(str::to_owned)),
    });
    assert_eq!(answered_toast.lock().unwrap().as_deref(), Some("Allowed once."));
    assert!(matches!(
        h.decided.lock().unwrap().as_slice(),
        [Answer::Decision { approval_id, decision: Decision::ApproveOnce, .. }] if approval_id == "appr_1"
    ));
    assert!(h.app.transport.edited.lock().unwrap().last().unwrap().text.contains("*Allowed once.*"));

    // Question card answered by option button and free reply
    let q_req = Request {
        approval_id: "appr_2".into(),
        mission_id: "m_1".into(),
        runtime: "codex".into(),
        kind: Kind::Question,
        summary: "Pick one".into(),
        detail: "".into(),
        cwd: None,
        requested_at: "2026-11-17T00:00:00Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        claude_input: None,
        questions: vec![Question {
            id: "q1".into(),
            header: Some("Mode".into()),
            question: "Which?".into(),
            options: vec![QuestionOption { label: "Fast".into(), description: None }],
            is_other: true,
            is_secret: false,
        }],
    };
    h.waiting.lock().unwrap().push((q_req.clone(), Some("t_ada".into())));
    h.link.card_raised(&q_req, Some("Ada"));
    let q_msg_id = {
        let sent = h.app.transport.sent.lock().unwrap();
        (100 + sent.len() as u64 - 1).to_string()
    };
    let mut rep = incoming("user_1", "chat_1", "21", Some("Custom answer"), now);
    rep.reply_to = Some(q_msg_id);
    h.link.handle_message(rep);
    assert!(matches!(
        h.decided.lock().unwrap().last().unwrap(),
        Answer::Answers { approval_id, answers } if approval_id == "appr_2" && answers == &[("q1".into(), vec!["Custom answer".into()])]
    ));
}

#[test]
fn unaddressed_message_asks_who_and_holds_until_chosen() {
    let h = make_harness("held", two_mates());
    pair_chat(&h);
    let now = h.clock.load(Ordering::SeqCst);

    h.link.handle_message(incoming("user_1", "chat_1", "30", Some("hello team"), now));
    assert!(h.started.lock().unwrap().is_empty());
    let (ask_id, t1_data) = {
        let sent = h.app.transport.sent.lock().unwrap();
        let last = sent.last().unwrap();
        assert_eq!(last.text, "Who is this for?");
        ((100 + sent.len() as u64 - 1).to_string(), last.extra.buttons[1][0].data.clone())
    };
    h.link.handle_press(ChatPress {
        from: "user_1".into(),
        place: "chat_1".into(),
        message: ask_id,
        data: t1_data,
        answer: Arc::new(|_| {}),
    });
    assert_eq!(h.started.lock().unwrap().as_slice(), &[("t_wren".into(), "hello team".into(), None)]);
}

#[test]
fn alerts_and_refusal_and_remove_work_as_expected() {
    let h = make_harness("alerts", two_mates());
    pair_chat(&h);
    let s = h.link.set_alerts(true);
    assert_eq!(s.alerts, Some(true));
    h.link.alert(ChatAlert {
        cards: vec![],
        lines: vec![("Ada".into(), "Needs you".into())],
    });
    assert!(h.app.transport.sent.lock().unwrap().last().unwrap().text.contains("*Ada*\nNeeds you"));

    *h.refusal.lock().unwrap() = Some("Disabled by org.".into());
    let now = h.clock.load(Ordering::SeqCst);
    h.link.handle_message(incoming("user_1", "chat_1", "40", Some("/who"), now));
    assert!(h.app.transport.sent.lock().unwrap().last().unwrap().text.contains("Disabled by org."));

    let removed = h.link.remove();
    assert!(removed.bot.is_none());
    assert!(!h.link.keeps_key());
}
