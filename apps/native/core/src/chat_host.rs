//! Host wiring for Discord, Slack, and Telegram chat links: connects `chat_link` to the native engine's
//! teammates, turn sender, ledger reader, approval funnels, and organization policy.

use crate::approval::{Answer, Channel, Kind, Request};
use crate::chat_apps::{ChatAppId, ChatAppSettings};
use crate::chat_discord::{DiscordOptions, create_discord_link};
use crate::chat_http::{HttpClient, HttpRequest, HttpResponse, WsCallbacks, WsClient, WsConnection};
use crate::chat_link::{ChatAlert, ChatLink, ChatLinkOptions, ChatMission, ChatStart, ChatTeammate, SettledHow, TokenInput};
use crate::chat_slack::{SlackOptions, create_slack_link};
use crate::chat_telegram::{TelegramOptions, create_telegram_link};
use crate::native::{self, Send};
use crate::phone_alerts::{AskKind, PhoneAlert, PhoneAlerts, PhoneAlertsOptions};
use crate::scout::scheduler::ScoutScheduler;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

struct NoopHttp;
impl HttpClient for NoopHttp {
    fn send(&self, _req: HttpRequest) -> Result<HttpResponse, String> {
        Err("network disabled".into())
    }
}
impl WsClient for NoopHttp {
    fn connect(&self, _url: &str, _callbacks: Arc<dyn WsCallbacks>) -> Result<Arc<dyn WsConnection>, String> {
        Err("network disabled".into())
    }
}

pub struct ChatHost {
    pub discord: Arc<ChatLink>,
    pub slack: Arc<ChatLink>,
    pub telegram: Arc<ChatLink>,
}

impl ChatHost {
    pub fn for_app(&self, app: ChatAppId) -> &Arc<ChatLink> {
        match app {
            ChatAppId::Discord => &self.discord,
            ChatAppId::Slack => &self.slack,
            ChatAppId::Telegram => &self.telegram,
        }
    }
}

static HOST: OnceLock<Mutex<Option<Arc<ChatHost>>>> = OnceLock::new();

fn build_options(dir: std::path::PathBuf) -> ChatLinkOptions {
    ChatLinkOptions {
        directory: dir,
        secrets: crate::own_models::secrets(),
        refusal: Arc::new(|| crate::org_policy::feature_refusal(Some(&crate::org_policy::current()), "chatApps")),
        computer: Arc::new(computer_name),
        team: Arc::new(read_team),
        start: Arc::new(start_turn),
        read: Arc::new(read_mission),
        live: Arc::new(is_mission_live),
        carried: Arc::new(|_| false),
        waiting: Arc::new(|| {
            native::waiting_approvals()
                .into_iter()
                .map(|r| (r, None))
                .collect()
        }),
        decide: Arc::new(|answer| match answer {
            Answer::Decision { approval_id, decision, reason } => {
                native::decide_approval(&approval_id, decision, reason).is_ok()
            }
            Answer::Answers { approval_id, answers } => {
                native::answer_approval(&approval_id, answers).is_ok()
            }
        }),
        changed: None,
        note: None,
        now: None,
        typing_ms: None,
    }
}

fn computer_name() -> String {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::NSProcessInfo;
        let name = NSProcessInfo::processInfo().hostName().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    "this computer".into()
}

fn read_team() -> Vec<ChatTeammate> {
    let Some(store) = native::store() else {
        return Vec::new();
    };
    let Ok(saved) = crate::people::team(&store) else {
        return Vec::new();
    };
    saved["teammates"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let id = t["teammateId"].as_str()?.to_owned();
                    let name = t["name"].as_str()?.to_owned();
                    let role = t["role"].as_str().unwrap_or("").to_owned();
                    Some(ChatTeammate { teammate_id: id, name, role })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn start_turn(teammate_id: &str, text: &str, follow_up_of: Option<&str>) -> ChatStart {
    let Some(store) = native::store() else {
        return ChatStart::Err { message: "Locust's data folder could not be found.".into() };
    };
    let Ok(saved) = crate::people::team(&store) else {
        return ChatStart::Err { message: "Could not read your team.".into() };
    };
    let Some(entry) = saved["teammates"].as_array().and_then(|arr| arr.iter().find(|t| t["teammateId"] == teammate_id)) else {
        return ChatStart::Err { message: "That teammate is no longer in Locust.".into() };
    };
    let teammate = crate::engine::Teammate {
        id: teammate_id.to_owned(),
        name: entry["name"].as_str().unwrap_or("").to_owned(),
        role: entry["role"].as_str().unwrap_or("").to_owned(),
        runtime: entry["route"]["runtime"].as_str().map(str::to_owned),
        model: entry["route"]["model"].as_str().map(str::to_owned),
        account: entry["route"]["account"].as_str().map(str::to_owned),
        busy: false,
        runs_in: entry["runsIn"].as_str().unwrap_or("ask").to_owned(),
        folder: entry["folder"].as_str().map(str::to_owned),
    };
    match native::send(Send {
        teammate,
        conversation: follow_up_of.map(str::to_owned),
        prompt: text.to_owned(),
        model: None,
        effort: None,
    }) {
        Ok(started) => ChatStart::Ok { mission_id: started.conversation_id },
        Err(message) => ChatStart::Err { message },
    }
}

fn read_mission(mission_id: &str) -> Option<ChatMission> {
    let store = native::store()?;
    let path = store.ledger_dir().ok()?.join(format!("{mission_id}.jsonl"));
    if !path.exists() {
        return None;
    }
    let snap = crate::ledger::read_transcript(&path);
    let events: Vec<serde_json::Value> = snap
        .items
        .iter()
        .filter_map(|item| match item {
            crate::ledger::Item::Assistant { item_id, text, r#final } => Some(serde_json::json!({
                "type": "message.delta",
                "payload": { "itemId": item_id, "operation": "replace", "text": text, "final": r#final }
            })),
            crate::ledger::Item::Note { level, text } if level == "error" => Some(serde_json::json!({
                "type": "run.failed",
                "payload": { "message": text }
            })),
            _ => None,
        })
        .collect();
    let phase = if snap.running {
        "running"
    } else if snap.failed {
        "failed"
    } else {
        "completed"
    };
    Some(ChatMission {
        mission_id: mission_id.to_owned(),
        phase: phase.to_owned(),
        events,
        started_by_kind: None,
        continues_from_mission_id: None,
        host_failure_message: None,
    })
}

fn is_mission_live(mission_id: &str) -> bool {
    native::store()
        .and_then(|s| s.ledger_dir().ok())
        .map(|d| d.join(format!("{mission_id}.jsonl")))
        .filter(|p| p.exists())
        .map(|p| crate::ledger::read_transcript(&p))
        .is_some_and(|snap| snap.running)
}

fn host() -> Option<Arc<ChatHost>> {
    if cfg!(test) {
        return None;
    }
    let lock = HOST.get_or_init(|| Mutex::new(None));
    let mut guard = crate::invariants::lock(lock);
    if let Some(ref h) = *guard {
        return Some(h.clone());
    }
    let dir = native::store()?.root().to_path_buf();
    #[cfg(target_os = "macos")]
    let (http, ws): (Arc<dyn HttpClient>, Arc<dyn WsClient>) = {
        let client = Arc::new(crate::chat_http::mac::MacUrlSessionClient::new());
        (client.clone(), client)
    };
    #[cfg(not(target_os = "macos"))]
    let (http, ws): (Arc<dyn HttpClient>, Arc<dyn WsClient>) = {
        let noop = Arc::new(NoopHttp);
        (noop.clone(), noop)
    };
    let _ = NoopHttp;

    let discord = create_discord_link(
        DiscordOptions { http: http.clone(), ws: ws.clone(), api: None, version: None },
        build_options(dir.clone()),
    );
    let slack = create_slack_link(
        SlackOptions { http: http.clone(), ws, api: None },
        build_options(dir.clone()),
    );
    let telegram = create_telegram_link(
        TelegramOptions { http, api: None, poll_seconds: None },
        build_options(dir),
    );
    discord.load();
    slack.load();
    telegram.load();
    let h = Arc::new(ChatHost { discord, slack, telegram });
    *guard = Some(h.clone());
    watch_cards(&h, store_ledger_dir());
    Some(h)
}

fn store_ledger_dir() -> Option<std::path::PathBuf> {
    native::store().and_then(|s| s.ledger_dir().ok())
}

// ---- approval cards in the chat apps (main/index.ts showApproval, answerApproval, withdrawCard, phoneAlerts' send) ----

/// How often the approval channel's cards are looked at: the window's own poll reads the same list (attention_watch).
const WATCH_MS: u64 = 500;
/// A card that left the channel while its turn goes on, with no answer in its record by then, was taken down
/// (index.ts withdrawCard: "No longer asked."). Its record is waited for first: the turn writes an answer when the
/// runtime next speaks.
const GONE_AFTER_LOOKS: u32 = 60;

/// Whether Locust's window is behind another app or minimized (index.ts phoneAlerts `away`). Nothing is said to a chat
/// app while it is in front: the window shows the card.
static WINDOW_AWAY: AtomicBool = AtomicBool::new(false);

/// The window says whether it is away (behind another app, or minimized), as phoneAlerts' `away` asks.
pub fn window_away(away: bool) {
    WINDOW_AWAY.store(away, Ordering::SeqCst);
}

/// What the chat apps hear of approval cards. A card raised goes to every link, which posts it only when its turn came
/// from that chat (index.ts:2590 `link.cardRaised`), and to the alert rules, which bring it to a chat whose switch is on
/// while the person is away (index.ts:3105). A card that leaves the channel is settled in each chat that shows it, by
/// the answer its turn recorded (index.ts:2463 `link.cardSettled`), or as taken down (index.ts:2475); one whose turn
/// ended is left to the link's own run-end words.
struct CardHook {
    links: Vec<Arc<ChatLink>>,
    alerts: Arc<PhoneAlerts>,
    seen: HashMap<String, Request>,
    leaving: HashMap<String, (Request, u32)>,
}

impl CardHook {
    fn new(links: Vec<Arc<ChatLink>>, alerts: Arc<PhoneAlerts>) -> Self {
        Self { links, alerts, seen: HashMap::new(), leaving: HashMap::new() }
    }

    fn tick(
        &mut self,
        waiting: Vec<Request>,
        name_of: &dyn Fn(&str) -> Option<String>,
        answer_of: &dyn Fn(&Request) -> Option<SettledHow>,
        live: &dyn Fn(&str) -> bool,
    ) {
        let now: HashMap<String, Request> = waiting.into_iter().map(|r| (r.approval_id.clone(), r)).collect();
        let raised: Vec<Request> = now.values().filter(|r| !self.seen.contains_key(&r.approval_id)).cloned().collect();
        for (id, card) in self.seen.drain() {
            if !now.contains_key(&id) {
                self.leaving.insert(id, (card, 0));
            }
        }
        self.seen = now;
        let mut raised = raised;
        raised.sort_by(|a, b| a.requested_at.cmp(&b.requested_at));
        for card in raised {
            let name = name_of(&card.mission_id);
            for link in &self.links {
                link.card_raised(&card, name.as_deref());
            }
            let kind = if card.kind == Kind::Question { AskKind::Question } else { AskKind::Approval };
            self.alerts.approval(name.as_deref(), kind, Some(&card.approval_id));
        }
        let mut settled: Vec<(String, SettledHow)> = Vec::new();
        self.leaving.retain(|id, (card, looks)| {
            if let Some(how) = answer_of(card) {
                settled.push((id.clone(), how));
                return false;
            }
            if !live(&card.mission_id) {
                // The turn ended: each link says so of its cards when told the run ended (chat_link run_ended).
                return false;
            }
            *looks += 1;
            if *looks >= GONE_AFTER_LOOKS {
                settled.push((id.clone(), SettledHow::Gone));
                return false;
            }
            true
        });
        for (id, how) in settled {
            for link in &self.links {
                link.card_settled(&id, how);
            }
        }
    }
}

/// One look at the approval channel: the cards waiting, each named by its turn's teammate, and how a card that left
/// was answered, read from its turn's record.
fn watch_once(
    hook: &mut CardHook,
    channel: &Channel,
    ledger_dir: Option<&Path>,
    team: &dyn Fn() -> Vec<ChatTeammate>,
    live: &dyn Fn(&str) -> bool,
) {
    let name_of = |mission_id: &str| {
        let teammate_id = channel.teammate_for(mission_id)?;
        team().into_iter().find(|m| m.teammate_id == teammate_id).map(|m| m.name)
    };
    let answer_of = |card: &Request| {
        ledger_dir.and_then(|dir| answer_in_record(&dir.join(format!("{}.jsonl", card.mission_id)), &card.approval_id))
    };
    hook.tick(channel.waiting(), &name_of, &answer_of, live);
}

/// The answer a turn's record keeps for a card (index.ts recordAnswer), as a chat app says it: an Always is said as
/// allowed, as answerApproval tells the links.
fn answer_in_record(path: &Path, approval_id: &str) -> Option<SettledHow> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().rev().filter(|line| line.contains(approval_id)).find_map(|line| {
        let record: serde_json::Value = serde_json::from_str(line).ok()?;
        if record["recordType"] != "mission.approval" || record["approval"]["approvalId"] != approval_id {
            return None;
        }
        match record["approval"]["answer"].as_str()? {
            "allowed" | "allowed-always" => Some(SettledHow::Allowed),
            "denied" => Some(SettledHow::Denied),
            "answered" => Some(SettledHow::Answered),
            _ => None,
        }
    })
}

/// The alert rules' parts as each chat whose switch is on is told them (index.ts:3105): each card still waiting, with
/// its buttons, and the other lines, who and what kind only. A card answered since it was raised is not told at all.
fn chat_alert_of(parts: &[PhoneAlert], waiting: &[Request]) -> Option<ChatAlert> {
    let cards: Vec<(Request, Option<String>)> = parts
        .iter()
        .filter_map(|part| {
            let id = part.approval_id.as_deref()?;
            waiting.iter().find(|r| r.approval_id == id).map(|r| (r.clone(), part.who.clone()))
        })
        .collect();
    let lines: Vec<(String, String)> =
        parts.iter().filter(|part| part.approval_id.is_none()).map(|part| (part.title.clone(), part.body.clone())).collect();
    (!cards.is_empty() || !lines.is_empty()).then_some(ChatAlert { cards, lines })
}

type Waiting = Arc<dyn Fn() -> Vec<Request> + std::marker::Send + Sync>;
type Away = Arc<dyn Fn() -> bool + std::marker::Send + Sync>;

/// The alert rules (phone_alerts.rs) sending to the chat links. Quiet hours are not read here yet: Scout's are not
/// kept anywhere this host can read.
fn chat_alerts(
    links: Vec<Arc<ChatLink>>,
    waiting: Waiting,
    away: Away,
    now_ms: Arc<dyn Fn() -> i64 + std::marker::Send + Sync>,
    scheduler: Arc<dyn ScoutScheduler>,
) -> Arc<PhoneAlerts> {
    PhoneAlerts::new(PhoneAlertsOptions {
        send: Arc::new(move |_merged: &PhoneAlert, parts: &[PhoneAlert]| {
            if let Some(said) = chat_alert_of(parts, &waiting()) {
                for link in &links {
                    link.alert(said.clone());
                }
            }
        }),
        away,
        quiet: Arc::new(|_| false),
        approvals_at_night: None,
        card_open: None,
        now_ms,
        scheduler,
    })
}

fn watch_cards(h: &Arc<ChatHost>, ledger_dir: Option<std::path::PathBuf>) {
    let links = vec![h.discord.clone(), h.slack.clone(), h.telegram.clone()];
    let alerts = chat_alerts(
        links.clone(),
        Arc::new(native::waiting_approvals),
        Arc::new(|| WINDOW_AWAY.load(Ordering::SeqCst)),
        Arc::new(|| chrono::Utc::now().timestamp_millis()),
        crate::scout::scheduler::RealScheduler::new(),
    );
    let mut hook = CardHook::new(links, alerts);
    let _ = std::thread::Builder::new().name("chat-cards".into()).spawn(move || {
        loop {
            watch_once(&mut hook, &native::approvals_channel(), ledger_dir.as_deref(), &read_team, &is_mission_live);
            std::thread::sleep(std::time::Duration::from_millis(WATCH_MS));
        }
    });
}

pub fn settings(app: ChatAppId) -> ChatAppSettings {
    host().map(|h| h.for_app(app).settings()).unwrap_or_default()
}

pub fn set_token(app: ChatAppId, input: TokenInput) -> ChatAppSettings {
    host().map(|h| h.for_app(app).set_token(input)).unwrap_or_default()
}

pub fn new_link(app: ChatAppId) -> ChatAppSettings {
    host().map(|h| h.for_app(app).new_link()).unwrap_or_default()
}

pub fn remove(app: ChatAppId) -> ChatAppSettings {
    host().map(|h| h.for_app(app).remove()).unwrap_or_default()
}

pub fn set_alerts(app: ChatAppId, on: bool) -> ChatAppSettings {
    host().map(|h| h.for_app(app).set_alerts(on)).unwrap_or_default()
}

pub fn managed_refusal() -> Option<String> {
    crate::org_policy::feature_refusal(Some(&crate::org_policy::current()), "chatApps")
}

pub fn on_run_ended(mission_id: &str) {
    if let Some(h) = host() {
        h.discord.on_run_ended(mission_id);
        h.slack.on_run_ended(mission_id);
        h.telegram.on_run_ended(mission_id);
    }
}

#[cfg(test)]
#[path = "chat_host_tests.rs"]
mod tests;
