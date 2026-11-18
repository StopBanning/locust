//! Host wiring for Discord, Slack, and Telegram chat links: connects `chat_link` to the native engine's
//! teammates, turn sender, ledger reader, approval funnels, and organization policy.

use crate::approval::Answer;
use crate::chat_apps::{ChatAppId, ChatAppSettings};
use crate::chat_discord::{DiscordOptions, create_discord_link};
use crate::chat_http::{HttpClient, HttpRequest, HttpResponse, WsCallbacks, WsClient, WsConnection};
use crate::chat_link::{ChatLink, ChatLinkOptions, ChatMission, ChatStart, ChatTeammate, TokenInput};
use crate::chat_slack::{SlackOptions, create_slack_link};
use crate::chat_telegram::{TelegramOptions, create_telegram_link};
use crate::native::{self, Send};
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
    Some(h)
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
