//! The chat apps' production hook on approval cards (main/index.ts: showApproval's `link.cardRaised`, answerApproval's
//! and withdrawCard's `link.cardSettled`, and phoneAlerts' send handing `link.alert` the cards still waiting), driven
//! by the real approval channel (`approval::Channel`, the one `native::approvals_channel` holds) through
//! `watch_once`, the look the host's watch thread takes. The chat app is a stand-in transport: nothing reaches
//! Discord, Slack or Telegram. A card's answer reaches its turn's record the way the turn loop writes it
//! (`LedgerWriter::append_approval` of `Channel::take_recorded_for`, native.rs `flush_approvals`).
use super::*;
use crate::approval::Decision;
use crate::chat_apps::{ChatAppBot, ChatAppCode, ChatAppLink};
use crate::chat_link::{
    ChatApp, ChatBot, ChatCommand, ChatEvents, ChatIncoming, ChatPress, ChatSendExtra, ChatTransport, ChatWords,
    ParsedCommand, create_chat_link,
};
use crate::ledger_write::LedgerWriter;
use crate::own_models::SecretBox;
use crate::scout::scheduler::FakeScheduler;
use crate::store::Store;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

struct XorBox;
impl SecretBox for XorBox {
    fn available(&self) -> bool {
        true
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        Ok(format!("sealed:{}", text.bytes().map(|b| format!("{:02x}", b ^ 0x5a)).collect::<String>()))
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        let hex = sealed.strip_prefix("sealed:").ok_or("bad seal")?;
        let bytes: Result<Vec<u8>, _> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map(|b| b ^ 0x5a)).collect();
        String::from_utf8(bytes.map_err(|_| "bad hex")?).map_err(|_| "bad utf8".into())
    }
}

#[derive(Clone, Debug)]
struct Sent {
    text: String,
    extra: ChatSendExtra,
}

#[derive(Default)]
struct Wire {
    sent: Mutex<Vec<Sent>>,
    edited: Mutex<Vec<(String, String)>>,
    next: AtomicU64,
}

impl ChatTransport for Wire {
    fn stop(&self) {}
    fn send(&self, _place: &str, text: &str, extra: &ChatSendExtra) -> Option<String> {
        self.sent.lock().unwrap().push(Sent { text: text.to_owned(), extra: extra.clone() });
        Some(format!("msg_{}", self.next.fetch_add(1, Ordering::SeqCst)))
    }
    fn edit(&self, _place: &str, message: &str, text: &str) {
        self.edited.lock().unwrap().push((message.to_owned(), text.to_owned()));
    }
    fn commands(&self, _list: &[ChatCommand]) -> Result<(), String> {
        Ok(())
    }
}

/// A stand-in chat app: plain markup, `/name text` commands, a pairing link with `start=<code>`.
struct StandIn {
    words: ChatWords,
    wire: Arc<Wire>,
    codes: AtomicU64,
}

impl ChatApp for StandIn {
    fn id(&self) -> ChatAppId {
        ChatAppId::Telegram
    }
    fn file(&self) -> &'static str {
        "stand-in-chat.json"
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
        let rest = text.trim().strip_prefix('/')?;
        let mut parts = rest.splitn(2, char::is_whitespace);
        Some(ParsedCommand { command: parts.next()?.to_lowercase(), rest: parts.next().unwrap_or("").trim().to_owned() })
    }
    fn code_in(&self, text: &str, bot: &ChatBot) -> Option<String> {
        self.parse(text, bot).filter(|p| p.command == "start" && !p.rest.is_empty()).map(|p| p.rest)
    }
    fn fresh_code(&self) -> String {
        format!("code_{}", self.codes.fetch_add(1, Ordering::SeqCst))
    }
    fn pairing(&self, bot: &ChatBot, code: &str, until: &str) -> (Option<ChatAppLink>, Option<ChatAppCode>) {
        (Some(ChatAppLink { url: format!("https://t.me/{}?start={code}", bot.username), until: until.to_owned() }), None)
    }
    fn shown(&self, bot: &ChatBot) -> ChatAppBot {
        ChatAppBot { username: bot.username.clone(), name: bot.name.clone(), invite: None, team: None, open: None }
    }
    fn shape(&self, given: &TokenInput) -> Result<String, String> {
        match given {
            TokenInput::Single(s) if s.starts_with("stand-in-") => Ok(s.clone()),
            _ => Err("not a stand-in token".into()),
        }
    }
    fn verify(&self, _secret: &str) -> Result<ChatBot, String> {
        Ok(ChatBot { id: "7".into(), username: "tides_bot".into(), name: "Tides".into(), extra: BTreeMap::new() })
    }
    fn keeps(&self, secret: &str) -> bool {
        secret.starts_with("stand-in-")
    }
    fn connect(&self, _secret: &str, _bot: &ChatBot, events: Arc<dyn ChatEvents>) -> Arc<dyn ChatTransport> {
        events.connected();
        self.wire.clone()
    }
    fn words(&self) -> &ChatWords {
        &self.words
    }
}

fn words() -> ChatWords {
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

fn team() -> Vec<ChatTeammate> {
    vec![
        ChatTeammate { teammate_id: "mate_ada".into(), name: "Ada".into(), role: "Research & Briefs".into() },
        ChatTeammate { teammate_id: "mate_wren".into(), name: "Wren".into(), role: "Code & Migrations".into() },
    ]
}

struct Harness {
    root: PathBuf,
    store: Store,
    channel: Arc<Channel>,
    wire: Arc<Wire>,
    link: Arc<ChatLink>,
    hook: CardHook,
    live: Arc<Mutex<HashSet<String>>>,
    away: Arc<AtomicBool>,
    clock: Arc<FakeScheduler>,
    asking: Vec<JoinHandle<Result<bool, String>>>,
    writers: HashMap<String, LedgerWriter>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        for mission in self.live.lock().unwrap().iter() {
            self.channel.release(mission);
        }
        for asking in self.asking.drain(..) {
            let _ = asking.join();
        }
        self.link.dispose();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn harness(label: &str) -> Harness {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let root = std::env::temp_dir().join(format!("locust-chat-host-{label}-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
    std::fs::create_dir_all(&root).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let channel = Channel::new();
    let wire = Arc::new(Wire::default());
    let live: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let turns = Arc::new(AtomicU64::new(1));
    let app = Arc::new(StandIn { words: words(), wire: wire.clone(), codes: AtomicU64::new(1) });
    let link = create_chat_link(
        app,
        ChatLinkOptions {
            directory: root.join("chat"),
            secrets: Arc::new(XorBox),
            refusal: Arc::new(|| None),
            computer: Arc::new(|| "DESK-PC".into()),
            team: Arc::new(team),
            start: {
                let (live, turns) = (live.clone(), turns.clone());
                Arc::new(move |_teammate, _text, _follow| {
                    let mission_id = format!("mission_{}", turns.fetch_add(1, Ordering::SeqCst));
                    live.lock().unwrap().insert(mission_id.clone());
                    ChatStart::Ok { mission_id }
                })
            },
            read: Arc::new(|_| None),
            live: {
                let live = live.clone();
                Arc::new(move |id| live.lock().unwrap().contains(id))
            },
            carried: Arc::new(|_| false),
            waiting: {
                let channel = channel.clone();
                Arc::new(move || channel.waiting().into_iter().map(|r| (r, None)).collect())
            },
            // The window's one path, as build_options' `decide` reaches it through native::decide_approval.
            decide: {
                let channel = channel.clone();
                Arc::new(move |answer| channel.decide(answer).is_ok())
            },
            changed: None,
            note: None,
            now: Some(Arc::new(|| chrono::Utc::now().timestamp_millis() as u64)),
            typing_ms: Some(60_000),
        },
    );
    let away = Arc::new(AtomicBool::new(true));
    let clock = FakeScheduler::new(1_790_000_000_000);
    let alerts = chat_alerts(
        vec![link.clone()],
        {
            let channel = channel.clone();
            Arc::new(move || channel.waiting())
        },
        {
            let away = away.clone();
            Arc::new(move || away.load(Ordering::SeqCst))
        },
        {
            let clock = clock.clone();
            Arc::new(move || clock.now_ms())
        },
        clock.clone(),
    );
    let hook = CardHook::new(vec![link.clone()], alerts);
    Harness { root, store, channel, wire, link, hook, live, away, clock, asking: Vec::new(), writers: HashMap::new() }
}

impl Harness {
    /// A token kept, a link made, and the person's chat paired by it.
    fn pair(&self) {
        let settings = self.link.set_token(TokenInput::Single("stand-in-token".into()));
        let url = settings.link.expect("a pairing link").url;
        let code = url.split("start=").nth(1).unwrap().to_owned();
        self.say(&format!("/start {code}"));
        assert!(self.sent().iter().any(|m| m.text.starts_with("Paired with Locust on *DESK-PC*.")), "{:?}", self.sent());
    }

    fn say(&self, text: &str) {
        static IDS: AtomicU64 = AtomicU64::new(1);
        let id = IDS.fetch_add(1, Ordering::SeqCst).to_string();
        self.link.handle_message(ChatIncoming {
            from: "person".into(),
            bot: false,
            place: "chat_1".into(),
            direct: true,
            id: id.clone(),
            text: Some(text.into()),
            sent_at: chrono::Utc::now().timestamp_millis() as u64,
            reply_to: None,
            answer_to: Some(id),
            name: Some("Sam".into()),
            username: Some("sam_here".into()),
            command: None,
            respond: None,
        });
    }

    /// A turn asks the person, as a runtime does through the channel, and waits for the answer on its own thread.
    fn ask(&mut self, mission: &str, teammate: &str, title: &str) -> String {
        self.ask_until(mission, teammate, title, Arc::new(AtomicBool::new(false)))
    }

    fn ask_until(&mut self, mission: &str, teammate: &str, title: &str, cancel: Arc<AtomicBool>) -> String {
        self.channel.register_teammate(mission, teammate);
        // A turn that asks is running.
        self.live.lock().unwrap().insert(mission.to_owned());
        let before: HashSet<String> = self.channel.waiting().into_iter().map(|r| r.approval_id).collect();
        let (channel, mission_owned, title_owned) = (self.channel.clone(), mission.to_owned(), title.to_owned());
        self.asking.push(std::thread::spawn(move || {
            channel.ask_browser(&mission_owned, &title_owned, "https://example.invalid/billing", &|| cancel.load(Ordering::SeqCst))
        }));
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(card) = self.channel.waiting().into_iter().find(|r| r.mission_id == mission && !before.contains(&r.approval_id)) {
                return card.approval_id;
            }
            assert!(Instant::now() < until, "the card was never raised");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn look(&mut self) {
        let live = self.live.clone();
        let ledger_dir = self.store.ledger_dir().unwrap();
        watch_once(&mut self.hook, &self.channel, Some(&ledger_dir), &team, &move |id: &str| live.lock().unwrap().contains(id));
    }

    /// The turn loop writes the answers its channel recorded into the turn's record (native.rs `flush_approvals`).
    fn flush(&mut self, mission: &str) {
        let recorded = self.channel.take_recorded_for(mission);
        let store = &self.store;
        let writer = self
            .writers
            .entry(mission.to_owned())
            .or_insert_with(|| LedgerWriter::create(store, mission, "look", "gpt-5", "codex").unwrap());
        for one in recorded {
            writer.append_approval(&one.to_json()).unwrap();
        }
    }

    fn sent(&self) -> Vec<Sent> {
        self.wire.sent.lock().unwrap().clone()
    }

    fn cards_sent(&self) -> Vec<Sent> {
        self.sent().into_iter().filter(|m| m.text.contains("needs your approval")).collect()
    }

    fn edits(&self) -> Vec<(String, String)> {
        self.wire.edited.lock().unwrap().clone()
    }

    fn press(&self, message_id: &str, data: &str) -> Option<String> {
        let said = Arc::new(Mutex::new(None));
        let kept = said.clone();
        self.link.handle_press(ChatPress {
            from: "person".into(),
            place: "chat_1".into(),
            message: message_id.into(),
            data: data.into(),
            answer: Arc::new(move |t| *kept.lock().unwrap() = t.map(str::to_owned)),
        });
        said.lock().unwrap().clone()
    }
}

fn labels(card: &Sent) -> Vec<String> {
    card.extra.buttons.first().map(|row| row.iter().map(|b| b.label.clone()).collect()).unwrap_or_default()
}

// case: brings a card from its own turn with Allow once and Deny, and answers it by the window’s path
#[test]
fn a_card_raised_in_a_turn_started_in_the_chat_comes_to_the_chat_and_is_answered_through_the_channel() {
    let mut h = harness("own-turn");
    h.pair();
    h.say("/wren run the tests");
    assert!(h.live.lock().unwrap().contains("mission_1"), "the turn started");
    h.away.store(false, Ordering::SeqCst);
    let id = h.ask("mission_1", "mate_wren", "Open the billing page");
    // A card of a turn started in the window stays in the window.
    h.ask("mission_window", "mate_ada", "Open the window's page");
    h.look();
    let cards = h.cards_sent();
    assert_eq!(cards.len(), 1, "{cards:?}");
    assert!(cards[0].text.starts_with("*Wren needs your approval*\nOpen the billing page"), "{}", cards[0].text);
    assert_eq!(labels(&cards[0]), ["Allow once", "Deny"]);

    // The person's press: Allow once, never Always, answered by the channel the runtime waits on.
    let message_id = format!("msg_{}", h.sent().len() - 1);
    let allow = cards[0].extra.buttons[0][0].data.clone();
    assert_eq!(h.press(&message_id, &allow).as_deref(), Some("Allowed once."));
    let asked = h.asking.remove(0).join().unwrap();
    assert_eq!(asked, Ok(true), "the runtime was told yes");
    assert!(!h.channel.waiting().iter().any(|r| r.approval_id == id));
    assert!(h.edits().last().unwrap().1.ends_with("*Allowed once.*"), "{:?}", h.edits());

    // The answer is in the turn's record; looking again says nothing more of a card answered here.
    h.flush("mission_1");
    h.look();
    h.look();
    assert_eq!(h.edits().len(), 1, "{:?}", h.edits());
    assert_eq!(h.cards_sent().len(), 1);
}

// case: tells the person here what needs them from any run, once they turn it on (0.837)
#[test]
fn a_card_from_any_run_comes_to_a_chat_whose_switch_is_on_and_goes_when_answered_in_the_window() {
    let mut h = harness("alerts");
    h.pair();
    // Off: a window turn's card is not told here.
    h.ask("mission_window", "mate_ada", "Open the first page");
    h.look();
    assert!(h.cards_sent().is_empty(), "{:?}", h.sent());

    assert_eq!(h.link.set_alerts(true).alerts, Some(true));
    let second = h.ask("mission_window", "mate_ada", "Open the second page");
    h.look();
    // Within two minutes of the first buzz, it joins the next one (phone-alerts.ts MERGE_MS).
    assert!(h.cards_sent().is_empty(), "{:?}", h.sent());
    h.clock.advance_ms(crate::phone_alerts::MERGE_MS as u64);
    let cards = h.cards_sent();
    assert_eq!(cards.len(), 1, "only the card the buzz carried: {cards:?}");
    assert!(cards[0].text.starts_with("*Ada needs your approval*\nOpen the second page"), "{}", cards[0].text);
    assert_eq!(labels(&cards[0]), ["Allow once", "Deny"]);

    // Answered in the window: the chat's card says so once its turn has recorded the answer, and loses its buttons.
    h.channel.decide(Answer::Decision { approval_id: second, decision: Decision::ApproveOnce, reason: None }).unwrap();
    h.look();
    assert!(h.edits().is_empty(), "no answer in the record yet: {:?}", h.edits());
    h.flush("mission_window");
    h.look();
    let edits = h.edits();
    assert_eq!(edits.len(), 1, "{edits:?}");
    assert!(edits[0].1.ends_with("*Allowed in Locust.*"), "{}", edits[0].1);
    h.look();
    assert_eq!(h.edits().len(), 1, "settled once");
}

// case: stays quiet while Locust's window is in front: the window shows it
#[test]
fn no_chat_hears_of_a_card_while_the_window_is_in_front() {
    let mut h = harness("in-front");
    h.pair();
    h.link.set_alerts(true);
    h.away.store(false, Ordering::SeqCst);
    h.ask("mission_window", "mate_ada", "Open the billing page");
    h.look();
    h.clock.advance_ms(crate::phone_alerts::MERGE_MS as u64);
    assert!(h.cards_sent().is_empty(), "{:?}", h.sent());
    h.away.store(true, Ordering::SeqCst);
    h.ask("mission_window", "mate_ada", "Open the payroll page");
    h.look();
    let cards = h.cards_sent();
    assert_eq!(cards.len(), 1, "{cards:?}");
    assert!(cards[0].text.contains("Open the payroll page"));
}

// case: says in the chat what became of a card answered in the window, or whose turn ended
#[test]
fn a_card_answered_in_the_window_says_so_and_one_whose_turn_ended_says_that() {
    let mut h = harness("settled");
    h.pair();
    h.say("/ada look");
    let first = h.ask("mission_1", "mate_ada", "Open the billing page");
    h.ask("mission_1", "mate_ada", "Edit README.md");
    h.look();
    assert_eq!(h.cards_sent().len(), 2, "{:?}", h.sent());

    h.channel.decide(Answer::Decision { approval_id: first, decision: Decision::Deny, reason: None }).unwrap();
    h.flush("mission_1");
    h.look();
    let edits = h.edits();
    assert_eq!(edits.len(), 1, "{edits:?}");
    assert!(edits[0].1.contains("Open the billing page") && edits[0].1.ends_with("*Denied in Locust.*"), "{}", edits[0].1);

    // The turn ends with the other card unanswered: the watch leaves it to the run-end step, however long that takes
    // to come, and the run-end step says so.
    h.channel.release("mission_1");
    h.live.lock().unwrap().remove("mission_1");
    for _ in 0..=GONE_AFTER_LOOKS {
        h.look();
    }
    assert_eq!(h.edits().len(), 1, "{:?}", h.edits());
    h.link.on_run_ended("mission_1");
    h.look();
    let edits = h.edits();
    assert_eq!(edits.len(), 2, "{edits:?}");
    assert!(edits[1].1.contains("Edit README.md") && edits[1].1.ends_with("*The turn ended before this was answered.*"), "{}", edits[1].1);
    assert!(!edits.iter().any(|(_, t)| t.contains("No longer asked.")));
}

// follows: main/index.ts:2471-2476 withdrawCard (a card nothing waits on any more comes down in a chat app; no Electron test)
#[test]
fn a_card_taken_down_while_its_turn_goes_on_says_it_is_no_longer_asked() {
    let mut h = harness("withdrawn");
    h.pair();
    h.say("/wren run the tests");
    let cancel = Arc::new(AtomicBool::new(false));
    h.ask_until("mission_1", "mate_wren", "Open the billing page", cancel.clone());
    h.look();
    assert_eq!(h.cards_sent().len(), 1);
    cancel.store(true, Ordering::SeqCst);
    assert!(h.asking.remove(0).join().unwrap().is_err(), "the asking side stopped waiting");
    for _ in 1..GONE_AFTER_LOOKS {
        h.look();
    }
    assert!(h.edits().is_empty(), "its record is waited for first: {:?}", h.edits());
    h.look();
    let edits = h.edits();
    assert_eq!(edits.len(), 1, "{edits:?}");
    assert!(edits[0].1.ends_with("*No longer asked.*"), "{}", edits[0].1);
}

// follows: main/index.ts:3114 ("A card answered since it was raised is not told at all"; no Electron test)
#[test]
fn a_card_answered_before_its_buzz_is_sent_is_not_told() {
    let mut h = harness("answered-first");
    h.pair();
    h.link.set_alerts(true);
    h.ask("mission_window", "mate_ada", "Open the first page");
    h.look();
    assert_eq!(h.cards_sent().len(), 1, "the first buzz goes at once");
    let second = h.ask("mission_window", "mate_ada", "Open the second page");
    h.look();
    h.channel.decide(Answer::Decision { approval_id: second, decision: Decision::Deny, reason: None }).unwrap();
    h.clock.advance_ms(crate::phone_alerts::MERGE_MS as u64);
    assert_eq!(h.cards_sent().len(), 1, "{:?}", h.sent());
}

#[test]
fn a_record_names_how_a_card_was_answered_and_nothing_for_another_card() {
    let mut h = harness("record");
    let id = h.ask("mission_9", "mate_ada", "Open the billing page");
    h.channel.decide(Answer::Decision { approval_id: id.clone(), decision: Decision::ApproveOnce, reason: None }).unwrap();
    h.flush("mission_9");
    let path = h.store.ledger_dir().unwrap().join("mission_9.jsonl");
    assert_eq!(answer_in_record(&path, &id), Some(SettledHow::Allowed));
    // An Always is said as allowed (answerApproval tells the links 'allowed' for approve-always).
    let always = serde_json::json!({"approvalId": "appr_always", "kind": "command", "asked": "npm test", "answer": "allowed-always", "by": "card", "askedAt": "2026-11-27T00:00:00Z", "occurredAt": "2026-11-27T00:00:01Z"});
    h.writers.get_mut("mission_9").unwrap().append_approval(&always).unwrap();
    assert_eq!(answer_in_record(&path, "appr_always"), Some(SettledHow::Allowed));
    assert_eq!(answer_in_record(&path, "appr_other"), None);
    assert_eq!(answer_in_record(&h.root.join("missing.jsonl"), &id), None);
}
