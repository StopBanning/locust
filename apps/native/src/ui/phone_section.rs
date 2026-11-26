//! Settings › General › "Your phone" (Electron `components/PhoneSettings.tsx`, 0.775, 0.826, 0.838):
//! one switch, and how the phone reaches Locust — from anywhere through Locust's relay, or only on this Wi-Fi.
//! The rules and the listener are `locust_core::phone_host`, `phone_server`, and `phone_anywhere`; this file
//! opens the host once and draws its state.
//!
//! The code is `locust_core::qr`, square for square what Electron's qrcode-generator draws, dark on white whatever the
//! theme.

use super::*;
use locust_core::phone_anywhere::{
    DEFAULT_RELAY, PHONE_ANYWHERE_FILE, PairedDeviceView, PhoneAnywhere, PhoneAnywhereOptions,
    PhoneAnywhereSettings, RelayConnector,
};
use locust_core::chat_http::WsClient;
use locust_core::desk_host::{
    DeskHandler, DeskHelloInfo, DeskHost, DeskHostDeps, DeskSender, DeskValue, DeskWindow, desk_tell_json,
};
use locust_core::desk_snapshot::{DeskSnapshot, snapshot_key, snapshot_with};
use locust_core::phone_calls::{EnginePhoneCalls, NativePhoneEngine, PhoneNotes};
use locust_core::phone_host::{PhoneHost, PhoneSettings};
use locust_core::phone_anywhere::{DeskCallbacks, HooksCallbacks, McpCallbacks};
use locust_core::remote_mcp::{McpRelay, RemoteMcp, RemoteMcpAsk, RemoteMcpReply};
use locust_core::routine_hooks::{HookLettersPage, HookRelay, HookRoom, RoutineHooks};
use locust_core::routine_host::LiveHost;
use std::sync::{Arc, Mutex, OnceLock, Weak};

pub(crate) const HEADING: &str = "Your phone";
const SWITCH_LABEL: &str = "Use Locust from your phone";

pub(crate) const OFF: &str =
    "Off. Turn it on to check on your teammates from your phone, answer approval cards and send the next message.";
pub(crate) const ON_ANYWHERE: &str = "On, from anywhere. Your phone reaches Locust through Locust\u{2019}s relay. Every message is locked on this computer and opened on your phone, so the relay passes along what it cannot read.";
pub(crate) const NO_NETWORK: &str = "On, but this computer is not on a network Locust can find. Connect it to your Wi-Fi, then turn this off and on again.";
pub(crate) const ON: &str = "On. Scan the code with your phone\u{2019}s camera to see your conversations, answer approval cards and send the next message. Your phone must be on the same Wi-Fi as this computer.";
pub(crate) const ANYONE: &str = "Anyone who scans this code can do what you can from your phone. Lost a phone, or shared the code? Make a new one: every phone has to scan again.";
const QR_LABEL_WIFI: &str = "A code for your phone\u{2019}s camera. It opens Locust on your phone.";
const QR_LABEL_ANYWHERE: &str = "A code for your phone\u{2019}s camera. It pairs your phone with Locust.";
const CONNECTED_RELAY: &str = "Connected to Locust\u{2019}s relay.";
const CONNECTING_RELAY: &str = "Connecting to Locust\u{2019}s relay\u{2026}";
const SCAN_ANYWHERE: &str = "Scan it with your phone\u{2019}s camera. It pairs one phone, once, within ten minutes, then a new one takes its place.";
const ANOTHER_COMPUTER: &str = "On another computer, open this code as a link in its browser instead.";
const LOST_PHONE: &str = "Lost a phone? Remove it below, and it stops at once.";
const OPEN_BROWSER_OFF: &str = "Or open Locust\u{2019}s whole window in this computer\u{2019}s browser. It turns this on, from anywhere.";
const OPEN_BROWSER_ON: &str = "Or open the whole window in this computer\u{2019}s browser, paired at once.";
const OPENING_IN_BROWSER: &str = "Opening Locust in this computer\u{2019}s browser. If no tab opened, copy the link and open it in the browser.";
const GITHUB_OFF_LEDE: &str = "Sign in with GitHub, and any browser where you sign in to GitHub opens Locust at app.locust.lol/desk, with no code.";
const GITHUB_OFF_NOTE: &str = "Locust\u{2019}s relay checks who you are with GitHub, then this computer checks again with GitHub itself. Locust asks GitHub for no permissions, only who you are.";
const GITHUB_ON_NOTE: &str = "Signing out stops new sign-ins. Browsers already in stay paired until you remove them above.";
const NIGHT_APPROVALS_LEDE: &str = "Let approvals and questions buzz in quiet hours. Off, everything waits for the morning brief.";
const NIGHT_APPROVALS_LABEL: &str = "Let approvals buzz in quiet hours";
/// One square of the code, in points.
const SQUARE: f32 = 4.0;
pub(crate) const WINDOWS_ASKS: &str =
    "The first time, Windows may ask whether Locust can use your network. Choose Allow, on private networks only.";

static HOST: OnceLock<PhoneHost> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhoneMode {
    Off,
    Anywhere,
    Wifi,
}

#[derive(Default)]
pub(crate) struct PhoneAnywhereLink {
    live: Option<PhoneAnywhere>,
    rx: Option<std::sync::mpsc::Receiver<PhoneAnywhereSettings>>,
    waker: Option<Waker>,
    started: bool,
    connector: Option<RelayConnector>,
    relay_url: Option<String>,
    github_user: Option<Arc<dyn Fn(&str) -> Option<u64> + Send + Sync>>,
    computer_name: Option<String>,
    open_browser: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// A test's WebSocket client under the real line to the relay, in place of NSURLSession's.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    ws_client: Option<Arc<dyn WsClient>>,
    /// The link as the routine links (and other apps) see it.
    shared: SharedLink,
    /// Locust in a browser (index.ts `createDeskHost`): one per window, made with its first link.
    desk: Option<Arc<DeskHost>>,
    /// What this window draws, kept as a browser reads it (desk_snapshot.rs): written on the window's thread each poll,
    /// read on the line's when a browser asks.
    desk_view: Arc<Mutex<Option<DeskSnapshot>>>,
    /// The cover's turns (`cover: coverWindow`), told on the line's thread and drawn on the window's.
    desk_cover: Arc<Mutex<Vec<bool>>>,
}

/// This window's one phone link, as the routine links and other apps reach it (index.ts makes one `phoneAnywhere` and
/// sets `hookRelay = phoneAnywhere`): whichever link is live now, and the routine links a reconnect refreshes.
#[derive(Clone, Default)]
pub(crate) struct SharedLink {
    live: Arc<Mutex<Option<PhoneAnywhere>>>,
    hooks: Arc<Mutex<Option<Weak<RoutineHooks>>>>,
    mcp: Arc<Mutex<Option<Weak<RemoteMcp>>>>,
}

/// Other apps reach this computer at the live link's room (index.ts `address: () => phoneAnywhere.mcpAddress()`,
/// `open: (on) => phoneAnywhere.mcpOpen(on)`); with From anywhere off there is no address.
impl McpRelay for SharedLink {
    fn address(&self) -> Option<String> {
        self.live().and_then(|link| link.mcp_address())
    }
    fn open(&self, on: bool) -> bool {
        self.live().map_or(!on, |link| link.mcp_open(on))
    }
}

/// index.ts's `mcp: { answer, sync }`: an app's request the relay passed on is answered by this window's pairing, and on
/// each connect the relay is told again whether it may take apps.
struct ServeMcp(Arc<Mutex<Option<Weak<RemoteMcp>>>>);

impl ServeMcp {
    fn remote(&self) -> Option<Arc<RemoteMcp>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(Weak::upgrade)
    }

    /// From anywhere off forgets the room, and with it every app's address: they are let go here too (index.ts's
    /// `changed`). Never on the window's thread: the relay may be asked.
    fn follow(&self, phone: &PhoneAnywhereSettings) {
        if !phone.enabled
            && let Some(remote) = self.remote().filter(|remote| remote.settings().enabled)
        {
            std::thread::spawn(move || {
                remote.set_enabled(&serde_json::json!(false));
            });
        }
    }
}

impl McpCallbacks for ServeMcp {
    fn answer(&self, asked: RemoteMcpAsk) -> RemoteMcpReply {
        match self.remote() {
            Some(remote) => remote.answer(&asked),
            None => RemoteMcpReply { status: 503, body: serde_json::json!({ "error": "pc-offline" }).to_string() },
        }
    }
    fn sync(&self) {
        if let Some(remote) = self.remote() {
            remote.sync();
        }
    }
}

impl SharedLink {
    fn live(&self) -> Option<PhoneAnywhere> {
        self.live.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// With From anywhere off there is no room, and nothing is confirmed.
impl HookRelay for SharedLink {
    fn room(&self) -> Option<HookRoom> {
        self.live().and_then(|link| HookRelay::room(&link))
    }
    fn connected(&self) -> bool {
        self.live().is_some_and(|link| HookRelay::connected(&link))
    }
    fn hook_open(&self, hook: &str, hash: &str) -> Result<bool, String> {
        self.live().map_or(Ok(false), |link| HookRelay::hook_open(&link, hook, hash))
    }
    fn hook_close(&self, hook: &str) -> Result<bool, String> {
        self.live().map_or(Ok(false), |link| HookRelay::hook_close(&link, hook))
    }
    fn hook_letters(&self, hook: &str) -> Result<Option<HookLettersPage>, String> {
        self.live().map_or(Ok(None), |link| HookRelay::hook_letters(&link, hook))
    }
    fn hook_collected(&self, hook: &str, ids: &[String]) -> Result<bool, String> {
        self.live().map_or(Ok(false), |link| HookRelay::hook_collected(&link, hook, ids))
    }
}

/// index.ts's `hooks: { refresh: syncHooks }`: the line is back, or a link was called, so the links are synced.
struct SyncHooks(Arc<Mutex<Option<Weak<RoutineHooks>>>>);

impl HooksCallbacks for SyncHooks {
    fn refresh(&self) {
        let Some(hooks) = self.0.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(Weak::upgrade) else {
            return;
        };
        // Never on the line's own thread: a sync asks the relay and waits for its answers on this same line.
        std::thread::spawn(move || {
            let _ = hooks.sync();
        });
    }
}

/// The routines' host on this window's one phone link: its links reach the relay through it, and the link's every
/// reconnect refreshes them.
pub(crate) fn routine_host(
    link: &PhoneAnywhereLink,
    store: locust_core::store::Store,
    opened_at: chrono::DateTime<chrono::Utc>,
) -> Arc<LiveHost> {
    let host = Arc::new(LiveHost::with_hook_relay(store, opened_at, Arc::new(link.shared.clone())));
    *link.shared.hooks.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::downgrade(host.hooks()));
    host
}

impl PhoneAnywhereLink {
    pub(crate) fn is_live(&self) -> bool {
        self.live.is_some()
    }

    /// Other apps' relay: this window's one phone link.
    pub(crate) fn mcp_relay(&self) -> Arc<dyn McpRelay> {
        Arc::new(self.shared.clone())
    }

    /// The pairing that answers apps' requests the link passes on; told at once when the link is already up (index.ts:
    /// `void remoteMcp.load().then(() => { if (phoneAnywhere.connected()) void remoteMcp?.sync() })`).
    pub(crate) fn serve_mcp(&self, remote: &Arc<RemoteMcp>) {
        *self.shared.mcp.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::downgrade(remote));
        if self.shared.live().is_some_and(|link| link.connected()) {
            let remote = remote.clone();
            std::thread::spawn(move || remote.sync());
        }
    }

    /// The live link, here and where the routine links reach it; the one it replaces is handed back.
    fn set_live(&mut self, next: Option<PhoneAnywhere>) -> Option<PhoneAnywhere> {
        *self.shared.live.lock().unwrap_or_else(|e| e.into_inner()) = next.clone();
        std::mem::replace(&mut self.live, next)
    }

    #[cfg(test)]
    pub(crate) fn set_test_relay(
        &mut self,
        connector: RelayConnector,
        relay_url: impl Into<String>,
        github_user: Option<Arc<dyn Fn(&str) -> Option<u64> + Send + Sync>>,
    ) {
        self.connector = Some(connector);
        self.relay_url = Some(relay_url.into());
        self.github_user = github_user;
    }

    #[cfg(test)]
    pub(crate) fn set_test_ws_client(&mut self, client: Arc<dyn WsClient>, relay_url: impl Into<String>) {
        self.ws_client = Some(client);
        self.relay_url = Some(relay_url.into());
    }

    #[cfg(test)]
    pub(crate) fn set_test_open_browser(&mut self, open: Arc<dyn Fn(&str) + Send + Sync>) {
        self.open_browser = Some(open);
    }
}

impl Drop for PhoneAnywhereLink {
    fn drop(&mut self) {
        if let Some(pa) = self.set_live(None) {
            pa.dispose();
        }
        self.rx = None;
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn disabled_anywhere() -> PhoneAnywhereSettings {
    PhoneAnywhereSettings {
        enabled: false,
        connected: false,
        code: None,
        devices: Vec::new(),
        github_login: None,
        message: None,
        night_approvals: false,
    }
}

fn has_saved_anywhere(dir: &std::path::Path) -> bool {
    let Ok(text) = std::fs::read_to_string(dir.join(PHONE_ANYWHERE_FILE)) else {
        return false;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    val.get("v").and_then(serde_json::Value::as_u64) == Some(1)
        && val.get("closing").and_then(serde_json::Value::as_bool) != Some(true)
        && val.get("room").and_then(serde_json::Value::as_str).is_some()
}

fn relay_url_from_env() -> String {
    if let Ok(named) = std::env::var("LOCUST_RELAY") {
        let trimmed = named.trim();
        if (trimmed.starts_with("https://") || trimmed.starts_with("http://"))
            && !trimmed.chars().any(char::is_whitespace)
            && !trimmed[trimmed.find("://").map_or(0, |i| i + 3)..].contains('/')
        {
            return trimmed.to_owned();
        }
    }
    DEFAULT_RELAY.to_owned()
}

/// The line to Locust's relay when no stand-in is set (Electron's `webRelaySocket`): on macOS NSURLSession's WebSocket,
/// the client the chat apps use.
fn default_connector(link: &PhoneAnywhereLink) -> RelayConnector {
    #[cfg(target_os = "macos")]
    if let Some(client) = mac_ws_client(link) {
        return locust_core::relay_socket::relay_connector(client);
    }
    let _ = link;
    Arc::new(|_url: &str| Err(RELAY_LATER.to_owned()))
}

/// The relay's line on this platform comes later; until then From anywhere stays connecting.
const RELAY_LATER: &str = "Locust\u{2019}s relay cannot be reached from this platform yet. It comes later.";

/// A test never reaches the network: without a client of its own it has none.
#[cfg(target_os = "macos")]
fn mac_ws_client(link: &PhoneAnywhereLink) -> Option<Arc<dyn WsClient>> {
    if let Some(client) = link.ws_client.clone() {
        return Some(client);
    }
    if cfg!(test) {
        return None;
    }
    Some(Arc::new(locust_core::chat_http::mac::MacUrlSessionClient::new()))
}

/// shared/appearance.ts `APPEARANCE_GET_CHANNEL`.
const APPEARANCE_GET: &str = "appearance:get";

/// What a browser that has the window reads of it, answered from what this window draws (the reads index.ts's handlers
/// give the window: `appearanceNow()` for `appearance:get`).
const DESK_DRAWN: &[&str] = &[APPEARANCE_GET];

/// The window's appearance as `appearance:get` answers it (`AppearanceState`): no Liquid glass here.
fn appearance_now(s: &State) -> serde_json::Value {
    let theme = match s.appearance {
        Some(true) => "dark",
        Some(false) => "light",
        None => "system",
    };
    let resolved = if s.appearance.unwrap_or(true) { "dark" } else { "light" };
    serde_json::json!({ "theme": theme, "resolved": resolved, "look": "standard", "liquidOffered": false, "glassGround": "desktop" })
}

/// Keeps what this window draws where a browser's read finds it; only a change is written.
fn publish_desk_view(s: &State) {
    let key = snapshot_key(APPEARANCE_GET, &[]);
    let value = appearance_now(s);
    let mut view = s.phone_anywhere_link.desk_view.lock().unwrap_or_else(|e| e.into_inner());
    if view.as_ref().and_then(|v| v.answers.get(&key)).is_some_and(|kept| kept.value == value) {
        return;
    }
    *view = Some(snapshot_with(view.as_ref(), &key, value, now_ms().max(0) as u64));
}

/// One read a browser may make, answered from the kept view by its call and arguments.
fn drawn_read(view: Arc<Mutex<Option<DeskSnapshot>>>, channel: &'static str) -> DeskHandler {
    Arc::new(move |_, args| {
        let view = view.lock().unwrap_or_else(|e| e.into_inner());
        view.as_ref()
            .and_then(|v| v.answers.get(&snapshot_key(channel, args)))
            .map(|kept| DeskValue::Json(kept.value.clone()))
            .ok_or_else(String::new)
    })
}

/// A waiting card as the window hears it (`MissionApprovalRequest`), for a browser just given the window.
fn waiting_card(request: &locust_core::approval::Request) -> serde_json::Value {
    let mut card = serde_json::json!({
        "approvalId": request.approval_id,
        "missionId": request.mission_id,
        "runtime": request.runtime,
        "kind": request.kind.as_str(),
        "summary": request.summary,
        "detail": request.detail,
        "requestedAt": request.requested_at,
    });
    if let Some(cwd) = &request.cwd {
        card["cwd"] = serde_json::json!(cwd);
    }
    card
}

/// The window's desk host (index.ts `createDeskHost`): told through whichever link is live, its cover drawn by this
/// window, its reads answered from what this window draws.
fn desk_host(s: &mut State, computer: Arc<dyn Fn() -> Option<String> + Send + Sync>) -> Arc<DeskHost> {
    if let Some(host) = &s.phone_anywhere_link.desk {
        return host.clone();
    }
    publish_desk_view(s);
    let link = &s.phone_anywhere_link;
    let view = link.desk_view.clone();
    let handlers: std::collections::HashMap<String, DeskHandler> =
        DESK_DRAWN.iter().map(|&channel| (channel.to_owned(), drawn_read(view.clone(), channel))).collect();
    let shared = link.shared.clone();
    let covers = link.desk_cover.clone();
    let waker = link.waker.clone();
    let host = Arc::new(DeskHost::new(DeskHostDeps {
        handlers: Arc::new(Mutex::new(handlers)),
        window: Arc::new(|| {
            let sender = DeskSender { id: 1, main_frame_id: 1, process_id: std::process::id(), destroyed: false };
            Some(DeskWindow { destroyed: false, web_contents: sender })
        }),
        tell: Arc::new(move |device, items| {
            let items: Vec<serde_json::Value> = items.iter().map(desk_tell_json).collect();
            shared.live().is_some_and(|live| live.tell_desk(device, &items))
        }),
        cover: Arc::new(move |on| {
            covers.lock().unwrap_or_else(|e| e.into_inner()).push(on);
            if let Some(w) = &waker {
                w();
            }
        }),
        hello: Arc::new(move || {
            let resolved = view
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .and_then(|v| v.answers.get(&snapshot_key(APPEARANCE_GET, &[])))
                .and_then(|kept| kept.value["resolved"].as_str().map(str::to_owned))
                .unwrap_or_else(|| "dark".to_owned());
            Ok(DeskHelloInfo {
                version: env!("CARGO_PKG_VERSION").to_owned(),
                computer: computer().unwrap_or_default(),
                theme: resolved,
            })
        }),
        waiting: Arc::new(|| locust_core::engine_api::api().waiting().iter().map(waiting_card).collect()),
        changed: None,
    }));
    let back = host.clone();
    s.elsewhere_cover.on_take_back = Some(Arc::new(move || {
        back.take_back();
        back.flush();
    }));
    s.phone_anywhere_link.desk = Some(host.clone());
    host
}

/// The desk's news for this window: what it draws is kept for a browser, and the cover goes up or down.
fn poll_desk(s: &mut State) -> bool {
    publish_desk_view(s);
    let turns: Vec<bool> = std::mem::take(&mut *s.phone_anywhere_link.desk_cover.lock().unwrap_or_else(|e| e.into_inner()));
    let Some(&covered) = turns.last() else {
        return false;
    };
    let active = s.phone_anywhere_link.desk.as_ref().and_then(|host| host.active());
    let device = active.and_then(|id| {
        s.phone_anywhere.as_ref().and_then(|a| a.devices.iter().find(|d| d.id == id).map(|d| d.name.clone()))
    });
    crate::elsewhere_cover::apply_cover(s, covered, device);
    true
}

fn make_anywhere(s: &mut State) -> Option<PhoneAnywhere> {
    let store = s.data.clone().or_else(locust_core::native::store)?;
    let dir = store.root().to_path_buf();
    let now: Arc<dyn Fn() -> i64 + Send + Sync> = Arc::new(now_ms);
    let engine = Arc::new(NativePhoneEngine::new(store));
    let calls = Arc::new(EnginePhoneCalls::new(engine, now.clone()));
    // Written on a phone while this computer was off (0.837): sent now, through the phone's own path.
    let notes = Arc::new(PhoneNotes::new(calls.clone()));
    let (tx, rx) = std::sync::mpsc::channel::<PhoneAnywhereSettings>();
    let waker = s.phone_anywhere_link.waker.clone();
    s.phone_anywhere_link.rx = Some(rx);
    let relay_override = s.phone_anywhere_link.relay_url.clone();
    let connect = s.phone_anywhere_link.connector.clone().unwrap_or_else(|| default_connector(&s.phone_anywhere_link));
    let github_user = s.phone_anywhere_link.github_user.clone();
    let computer_override = s.phone_anywhere_link.computer_name.clone();
    let computer: Arc<dyn Fn() -> Option<String> + Send + Sync> = Arc::new(move || {
        computer_override.clone().or_else(|| {
            std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .ok()
        })
    });
    let desk: Arc<dyn DeskCallbacks> = desk_host(s, computer.clone());
    Some(PhoneAnywhere::new(PhoneAnywhereOptions {
        directory: dir,
        calls,
        relay: Arc::new(move || relay_override.clone().unwrap_or_else(relay_url_from_env)),
        refusal: Some(Arc::new(|| {
            locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "phone")
        })),
        changed: Some(Arc::new(move |next| {
            let _ = tx.send(next);
            if let Some(w) = &waker {
                w();
            }
        })),
        connect,
        now: Some(now),
        protect_file: None,
        retry_ms: None,
        // index.ts `away: { forPhone, refresh, letter }` waits on Scout away (scout-away.ts) in Rust: ScoutAway
        // (core/src/scout_away.rs) has no for-phone key, no catch-up of the letters left while this computer was off
        // and no told letter, its relay has no `letters`, and this app makes no ScoutAway at all.
        away: None,
        hooks: Some(Arc::new(SyncHooks(s.phone_anywhere_link.shared.hooks.clone()))),
        desk: Some(desk),
        notes: Some(notes),
        github_user,
        computer_name: Some(computer),
        mcp: Some(Arc::new(ServeMcp(s.phone_anywhere_link.shared.mcp.clone()))),
    }))
}

fn start_saved(s: &mut State) {
    if s.phone_anywhere_link.started || s.demo {
        return;
    }
    let Some(dir) = s.data.as_ref().map(|st| st.root().to_path_buf()) else {
        return;
    };
    s.phone_anywhere_link.started = true;
    if has_saved_anywhere(&dir) {
        if let Some(pa) = make_anywhere(s) {
            // Live before it connects, so the routine links its first `ok` refreshes reach it.
            s.phone_anywhere_link.set_live(Some(pa.clone()));
            let next = pa.load();
            if !next.enabled {
                s.phone_anywhere_link.set_live(None);
                pa.dispose();
                s.phone_anywhere_link.rx = None;
            }
            s.phone_anywhere = Some(next);
        }
    } else if s.phone_anywhere.is_none() {
        s.phone_anywhere = Some(disabled_anywhere());
    }
}

/// Registers the window waker and restores a saved "From anywhere" link if it was left on.
pub(crate) fn setup(s: &mut State, waker: Waker) {
    s.phone_anywhere_link.waker = Some(waker);
    start_saved(s);
}

/// Drains settings pushed by the live `PhoneAnywhere` into `s.phone_anywhere`.
pub(crate) fn poll(s: &mut State) -> bool {
    let before = s.phone_anywhere.clone();
    if !s.phone_anywhere_link.started {
        start_saved(s);
    }
    let mut latest = None;
    if let Some(rx) = &s.phone_anywhere_link.rx {
        while let Ok(next) = rx.try_recv() {
            latest = Some(next);
        }
    }
    if let Some(next) = latest {
        s.phone_anywhere = Some(next);
    }
    let covered = poll_desk(s);
    s.phone_anywhere != before || covered
}

/// Drops the live `PhoneAnywhere` cleanly when the window closes or quits.
pub(crate) fn dispose(s: &mut State) {
    if let Some(pa) = s.phone_anywhere_link.set_live(None) {
        pa.dispose();
    }
    s.phone_anywhere_link.rx = None;
}

fn enable_anywhere(s: &mut State) {
    if s.phone_anywhere_link.live.is_none() {
        let Some(pa) = make_anywhere(s) else {
            return;
        };
        s.phone_anywhere_link.set_live(Some(pa));
    }
    if let Some(pa) = s.phone_anywhere_link.live.as_ref() {
        let next = pa.set_enabled(true);
        if next.enabled {
            s.phone_anywhere = Some(next);
        } else {
            if let Some(pa) = s.phone_anywhere_link.set_live(None) {
                pa.dispose();
            }
            s.phone_anywhere_link.rx = None;
            s.phone_anywhere = Some(next);
        }
    }
}

fn disable_anywhere(s: &mut State) {
    if let Some(pa) = s.phone_anywhere_link.set_live(None) {
        let mut next = pa.set_enabled(false);
        ServeMcp(s.phone_anywhere_link.shared.mcp.clone()).follow(&next);
        pa.dispose();
        s.phone_anywhere_link.rx = None;
        next.connected = false;
        s.phone_anywhere = Some(next);
    } else if s.phone_anywhere.is_some() {
        s.phone_anywhere = Some(disabled_anywhere());
    }
}

fn current_mode(s: &State) -> PhoneMode {
    if s.phone_anywhere.as_ref().is_some_and(|a| a.enabled) {
        PhoneMode::Anywhere
    } else if s.settings_panel.phone.as_ref().is_some_and(|p| p.enabled) {
        PhoneMode::Wifi
    } else {
        PhoneMode::Off
    }
}

fn set_mode(s: &mut State, mode: PhoneMode) {
    match mode {
        PhoneMode::Anywhere => {
            if let Some(host) = HOST.get() {
                s.settings_panel.phone = Some(host.set_enabled(false));
            } else if let Some(wifi) = s.settings_panel.phone.as_mut() {
                wifi.enabled = false;
                wifi.url = None;
            }
            enable_anywhere(s);
        }
        PhoneMode::Wifi => {
            disable_anywhere(s);
            if let Some(host) = HOST.get() {
                s.settings_panel.phone = Some(host.set_enabled(true));
            }
        }
        PhoneMode::Off => {
            disable_anywhere(s);
            if let Some(host) = HOST.get() {
                s.settings_panel.phone = Some(host.set_enabled(false));
            } else if let Some(wifi) = s.settings_panel.phone.as_mut() {
                wifi.enabled = false;
                wifi.url = None;
            }
        }
    }
}

/// Opens the phone host once, on the native engine, as the person left it: listening again if it was on.
#[cfg(not(test))]
pub(crate) fn open() {
    use locust_core::phone_calls::{EnginePhoneCalls, NativePhoneEngine};
    use locust_core::phone_host::{PHONE_PORT, PhoneHostOptions, phone_addresses};
    use locust_core::phone_page::{PhonePageVia, phone_page};
    use locust_core::phone_server::{TcpPhoneListener, interfaces};
    use std::sync::Arc;

    if HOST.get().is_some() {
        return;
    }
    let Some(store) = locust_core::native::store() else {
        return;
    };
    let now: Arc<dyn Fn() -> i64 + Send + Sync> = Arc::new(now_ms);
    let engine = Arc::new(NativePhoneEngine::new(store.clone()));
    let host = PhoneHost::new(PhoneHostOptions {
        directory: store.root().to_path_buf(),
        page: Arc::new(|| phone_page(PhonePageVia::Lan, None)),
        calls: Arc::new(EnginePhoneCalls::new(engine, now.clone())),
        secrets: locust_core::own_models::secrets(),
        listener: Arc::new(TcpPhoneListener),
        refusal: Arc::new(|| {
            locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "phone")
        }),
        addresses: Arc::new(|| phone_addresses(&interfaces())),
        port: PHONE_PORT,
        now_ms: now,
    });
    if HOST.set(host).is_ok()
        && let Some(why) = HOST.get().and_then(|host| host.load().message)
    {
        eprintln!("Locust's phone: {why}");
    }
}

/// Tests never open a listener.
#[cfg(test)]
pub(crate) fn open() {}

/// What Settings shows, read when it opens.
pub(crate) fn read(s: &mut State) {
    if let Some(host) = HOST.get() {
        s.settings_panel.phone = Some(host.settings());
    }
    if let Some(pa) = s.phone_anywhere_link.live.as_ref() {
        s.phone_anywhere = Some(pa.settings());
    } else if s.phone_anywhere.is_none() && s.data.is_some() {
        s.phone_anywhere = Some(disabled_anywhere());
    }
}

fn toggle(s: &mut State) {
    let mode = current_mode(s);
    if mode != PhoneMode::Off {
        set_mode(s, PhoneMode::Off);
    } else if s.phone_picked_wifi {
        set_mode(s, PhoneMode::Wifi);
    } else {
        set_mode(s, PhoneMode::Anywhere);
    }
}

fn new_code(s: &mut State) {
    if let Some(host) = HOST.get() {
        s.settings_panel.phone = Some(host.forget());
    }
}

fn pick_way(s: &mut State, wifi: bool) {
    s.phone_picked_wifi = wifi;
    let mode = current_mode(s);
    if mode != PhoneMode::Off {
        let next = if wifi { PhoneMode::Wifi } else { PhoneMode::Anywhere };
        if mode != next {
            set_mode(s, next);
        }
    }
}

fn open_for_person(s: &State, url: &str) {
    if let Some(open) = &s.phone_anywhere_link.open_browser {
        open(url);
        return;
    }
    if let Ok(pages) = std::env::var("LOCUST_SIGN_IN_PAGES")
        && !pages.is_empty()
    {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&pages) {
            let _ = writeln!(f, "{url}");
        }
        return;
    }
    if cfg!(test) || !(url.starts_with("https://") || url.starts_with("http://")) {
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
    #[cfg(not(target_os = "macos"))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn wait_for_relay(pa: &PhoneAnywhere) -> bool {
    let max_ms = if cfg!(test) { 500 } else { 15_000 };
    let step = std::time::Duration::from_millis(10);
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
    while std::time::Instant::now() < deadline {
        if pa.connected() {
            return true;
        }
        std::thread::sleep(step);
    }
    pa.connected()
}

/// "Open in your browser" (Electron `PHONE_OPEN_BROWSER_CHANNEL`, 0.838): turns on From anywhere if off,
/// waits for the relay, asks the live `PhoneAnywhere` for `desk_code`, and opens the link in the browser.
fn open_in_browser(s: &mut State) {
    if locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "phone").is_some() {
        return;
    }
    let already_anywhere = s.phone_anywhere.as_ref().is_some_and(|a| a.enabled);
    if !already_anywhere {
        if s.settings_panel.phone.as_ref().is_some_and(|p| p.enabled) {
            let mut next = s.phone_anywhere.clone().unwrap_or_else(disabled_anywhere);
            next.message = Some("Your phone reaches Locust only on this Wi-Fi now. Choose From anywhere first.".into());
            s.phone_anywhere = Some(next);
            return;
        }
        set_mode(s, PhoneMode::Anywhere);
    }
    let Some(pa) = s.phone_anywhere_link.live.clone() else {
        return;
    };
    if !wait_for_relay(&pa) {
        let mut next = pa.settings();
        next.message = Some("Locust could not reach its relay. Check this computer\u{2019}s connection, then try again.".into());
        s.phone_anywhere = Some(next);
        return;
    }
    if let Some(url) = pa.desk_code() {
        open_for_person(s, &url);
        s.phone_anywhere = Some(pa.settings());
    } else {
        let mut next = pa.settings();
        next.message = Some("Locust\u{2019}s relay did not open a code for this browser. Try again in a moment.".into());
        s.phone_anywhere = Some(next);
    }
}

/// "Sign in with GitHub" (Electron `PHONE_GITHUB_IN_CHANNEL`, 0.838): asks the live `PhoneAnywhere` for
/// `github_link` and opens the link in the browser.
fn github_sign_in(s: &mut State) {
    let Some(pa) = s.phone_anywhere_link.live.clone() else {
        return;
    };
    if let Some(url) = pa.github_link() {
        open_for_person(s, &url);
    }
    s.phone_anywhere = Some(pa.settings());
}

/// "Sign out of GitHub" (Electron `PHONE_GITHUB_OUT_CHANNEL`, 0.838): calls `github_sign_out` on the live `PhoneAnywhere`.
fn github_sign_out(s: &mut State) {
    let Some(pa) = s.phone_anywhere_link.live.clone() else {
        return;
    };
    s.phone_anywhere = Some(pa.github_sign_out());
}

/// The words under the heading, as PhoneSettings.tsx says them for Only on this Wi-Fi.
pub(crate) fn lede(phone: &PhoneSettings, managed: bool) -> &'static str {
    if managed || !phone.enabled {
        OFF
    } else if phone.url.is_none() {
        NO_NETWORK
    } else {
        ON
    }
}

/// When a phone was last there, in the words Settings uses elsewhere (PhoneSettings.tsx `deviceState`).
pub(crate) fn device_state(device: &PairedDeviceView, now: chrono::DateTime<chrono::Utc>) -> String {
    let seen = device
        .seen
        .as_deref()
        .and_then(|iso| locust_core::home_team_state::ago_label(iso, now));
    let when = if device.online {
        "Open now".to_owned()
    } else if let Some(ago) = seen {
        format!("Last open {ago}")
    } else {
        "Paired".to_owned()
    };
    let how = if device.github {
        format!("{when} \u{b7} signed in with GitHub")
    } else {
        when
    };
    if device.alerts {
        format!("{how} \u{b7} alerts on")
    } else {
        how
    }
}

/// The code as rows of dark runs on white, with PhoneSettings.tsx's four-square margin.
pub(crate) fn qr_with_label(url: &str, label: &str) -> Option<El> {
    let code = locust_core::qr::qr_code(url)?;
    let mut rows = col().fill(Color::WHITE).padding(4.0 * SQUARE).role("img").label(label.to_owned()).key("phone-code");
    for r in 0..code.size {
        let mut line = row().h(SQUARE);
        let mut c = 0;
        while c < code.size {
            let dark = code.is_dark(r, c);
            let start = c;
            while c < code.size && code.is_dark(r, c) == dark {
                c += 1;
            }
            let run = row().w((c - start) as f32 * SQUARE).h(SQUARE);
            line = line.child(if dark { run.fill(Color::BLACK) } else { run });
        }
        rows = rows.child(line);
    }
    Some(rows)
}

pub(crate) fn section(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    if s.settings_panel.phone.is_none() && s.phone_anywhere.is_none() {
        return col()
            .w_full()
            .gap(4.0)
            .padding(10.0)
            .child(text(HEADING).role("heading").level(2).font(type_::TITLE).color(p.text))
            .child(text("Your phone opens when Locust runs on its own engine.").font(type_::LABEL).color(p.muted).wrap());
    }
    let default_wifi = PhoneSettings { enabled: false, url: None, addresses: Vec::new(), message: None };
    let phone = s.settings_panel.phone.as_ref().unwrap_or(&default_wifi);
    let anywhere = s.phone_anywhere.as_ref();
    let managed = locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "phone").is_some();
    let mode = if anywhere.is_some_and(|a| a.enabled) {
        PhoneMode::Anywhere
    } else if phone.enabled {
        PhoneMode::Wifi
    } else {
        PhoneMode::Off
    };
    let on = !managed && mode != PhoneMode::Off;
    let shown = if mode == PhoneMode::Off {
        if s.phone_picked_wifi { PhoneMode::Wifi } else { PhoneMode::Anywhere }
    } else {
        mode
    };
    let lede_text = if !on {
        OFF
    } else if mode == PhoneMode::Anywhere {
        ON_ANYWHERE
    } else {
        lede(phone, managed)
    };
    let mut words = col()
        .grow(1.0)
        .gap(4.0)
        .child(text(HEADING).role("heading").level(2).font(l.ty.control).color(l.c("text-primary")))
        .child(text(lede_text).font(l.ty.body).color(l.c("text-secondary")).wrap());
    if let Some(message) = &phone.message
        && mode != PhoneMode::Anywhere
    {
        words = words.child(text(message.clone()).key("phone-message").font(l.ty.body).color(l.c("text-secondary")).wrap());
    }
    if managed {
        words = words.child(text("Set by your organization.").font(l.ty.body).color(l.c("text-muted")));
    }
    let mut switch = crate::look::controls::switch(&l, on, SWITCH_LABEL, managed).key("phone-switch");
    if !managed {
        switch = switch.on_press(toggle);
    }

    let way_button = |label: &str, key: &str, active: bool, wifi: bool| {
        let mut btn = row()
            .key(key)
            .role("radio")
            .label(label.to_owned())
            .checked(active)
            .py(6.0)
            .px(10.0)
            .radius(l.t.px("radius-control"))
            .fill(if active { l.c("bg-selected") } else { l.c("bg-control") })
            .child(text(label.to_owned()).font(l.ty.control).color(if active { l.c("text-primary") } else { l.c("text-secondary") }));
        if !managed {
            btn = btn.on_press(move |s: &mut State| pick_way(s, wifi));
        }
        btn
    };
    let ways = row()
        .key("phone-ways")
        .role("radiogroup")
        .label("How your phone reaches Locust".to_owned())
        .gap(6.0)
        .px(10.0)
        .child(way_button("From anywhere", "phone-way-anywhere", shown == PhoneMode::Anywhere, false))
        .child(way_button("Only on this Wi-Fi", "phone-way-wifi", shown == PhoneMode::Wifi, true));

    let mut section = col()
        .key("phone-section")
        .w_full()
        .gap(8.0)
        .child(row().w_full().gap(16.0).padding(10.0).center_y().child(words).child(switch))
        .child(ways);

    if !on && !managed && shown == PhoneMode::Anywhere {
        section = section.child(
            row()
                .w_full()
                .gap(8.0)
                .px(10.0)
                .center_y()
                .child(col().grow(1.0).child(text(OPEN_BROWSER_OFF).font(l.ty.body).color(l.c("text-secondary")).wrap()))
                .child(
                    crate::look::controls::button(&l, "Open in your browser", crate::look::controls::Tone::Neutral, false)
                        .key("phone-open-browser")
                        .on_press(open_in_browser),
                ),
        );
    }

    if on && mode == PhoneMode::Anywhere && let Some(anywhere) = anywhere {
        let status_line = anywhere
            .message
            .as_deref()
            .unwrap_or(if anywhere.connected { CONNECTED_RELAY } else { CONNECTING_RELAY });
        let mut side = col()
            .grow(1.0)
            .gap(8.0)
            .child(text(status_line.to_owned()).key("phone-anywhere-status").role("status").font(l.ty.body).color(l.c("text-secondary")).wrap());
        let is_desk_code = anywhere.code.as_ref().is_some_and(|c| c.desk);
        if let Some(code) = &anywhere.code {
            let link = code.url.clone();
            if code.desk {
                side = side
                    .child(text(OPENING_IN_BROWSER).font(l.ty.body).color(l.c("text-secondary")).wrap())
                    .child(
                        row().child(
                            text("Copy the link")
                                .key("phone-copy")
                                .font(l.ty.control)
                                .color(l.c("text-primary"))
                                .py(6.0)
                                .px(10.0)
                                .radius(l.t.px("radius-control"))
                                .fill(l.c("bg-control"))
                                .role("button")
                                .on_press_tx(move |_s: &mut State, tx| crate::reply_actions::request_copy(tx, link.clone())),
                        ),
                    );
            } else {
                side = side
                    .child(text(SCAN_ANYWHERE).font(l.ty.body).color(l.c("text-secondary")).wrap())
                    .child(text(ANOTHER_COMPUTER).font(l.ty.body).color(l.c("text-secondary")).wrap())
                    .child(
                        row().child(
                            text("Copy the link")
                                .key("phone-copy")
                                .font(l.ty.control)
                                .color(l.c("text-primary"))
                                .py(6.0)
                                .px(10.0)
                                .radius(l.t.px("radius-control"))
                                .fill(l.c("bg-control"))
                                .role("button")
                                .on_press_tx(move |_s: &mut State, tx| crate::reply_actions::request_copy(tx, link.clone())),
                        ),
                    );
            }
        } else {
            side = side.child(
                row().child(
                    text("Pair a phone")
                        .key("phone-pair")
                        .font(l.ty.control)
                        .color(l.c("text-primary"))
                        .py(6.0)
                        .px(10.0)
                        .radius(l.t.px("radius-control"))
                        .fill(l.c("bg-control"))
                        .role("button"),
                ),
            );
        }
        if !is_desk_code {
            side = side.child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(col().grow(1.0).child(text(OPEN_BROWSER_ON).font(l.ty.body).color(l.c("text-secondary")).wrap()))
                    .child(
                        crate::look::controls::button(&l, "Open in your browser", crate::look::controls::Tone::Neutral, false)
                            .key("phone-open-browser")
                            .on_press(open_in_browser),
                    ),
            );
        }
        if !anywhere.devices.is_empty() {
            side = side.child(text(LOST_PHONE).font(l.ty.body).color(l.c("text-secondary")).wrap());
        }
        let mut body_row = row().w_full().gap(16.0).px(10.0);
        if let Some(code) = &anywhere.code
            && !code.desk
            && let Some(qr_el) = qr_with_label(&code.url, QR_LABEL_ANYWHERE)
        {
            body_row = body_row.child(qr_el);
        }
        section = section.child(body_row.child(side));

        if !anywhere.devices.is_empty() {
            let now = chrono::Utc::now();
            let mut list = col().w_full().gap(6.0).px(10.0).key("phone-devices").label("Paired phones".to_owned());
            for device in &anywhere.devices {
                list = list.child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .center_y()
                        .key(format!("phone-device-{}", device.id))
                        .child(
                            col()
                                .grow(1.0)
                                .child(text(device.name.clone()).font(l.ty.control).color(l.c("text-primary")))
                                .child(text(device_state(device, now)).font(l.ty.body).color(l.c("text-secondary")).wrap()),
                        )
                        .child(
                            text("Remove")
                                .key(format!("phone-remove-{}", device.id))
                                .font(l.ty.control)
                                .color(l.c("text-primary"))
                                .py(6.0)
                                .px(10.0)
                                .radius(l.t.px("radius-control"))
                                .fill(l.c("bg-control"))
                                .role("button"),
                        ),
                );
            }
            section = section.child(list);
        }

        let github_signed_in = anywhere.github_login.is_some();
        let (github_lede, github_note, github_label) = if let Some(login) = &anywhere.github_login {
            (
                format!("Any browser where you sign in to GitHub as {login} opens Locust at app.locust.lol/desk, with no code."),
                GITHUB_ON_NOTE,
                "Sign out of GitHub",
            )
        } else {
            (GITHUB_OFF_LEDE.to_owned(), GITHUB_OFF_NOTE, "Sign in with GitHub")
        };
        let mut github_btn = crate::look::controls::button(&l, github_label, crate::look::controls::Tone::Neutral, !anywhere.connected)
            .key("phone-github-btn");
        if anywhere.connected {
            github_btn = if github_signed_in {
                github_btn.on_press(github_sign_out)
            } else {
                github_btn.on_press(github_sign_in)
            };
        }
        section = section.child(
            row()
                .w_full()
                .gap(16.0)
                .padding(10.0)
                .center_y()
                .key("phone-github")
                .child(
                    col()
                        .grow(1.0)
                        .gap(4.0)
                        .child(text(github_lede).font(l.ty.body).color(l.c("text-primary")).wrap())
                        .child(text(github_note).font(l.ty.body).color(l.c("text-secondary")).wrap()),
                )
                .child(github_btn),
        );

        if anywhere.devices.iter().any(|d| d.alerts) {
            let night_switch = crate::look::controls::switch(&l, anywhere.night_approvals, NIGHT_APPROVALS_LABEL, false)
                .key("phone-night-approvals");
            section = section.child(
                row()
                    .w_full()
                    .gap(16.0)
                    .padding(10.0)
                    .center_y()
                    .child(col().grow(1.0).child(text(NIGHT_APPROVALS_LEDE).font(l.ty.body).color(l.c("text-secondary")).wrap()))
                    .child(night_switch),
            );
        }
    }

    if on && mode == PhoneMode::Wifi && let Some(url) = &phone.url {
        let link = url.clone();
        let mut side = col()
            .grow(1.0)
            .gap(8.0)
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .padding(10.0)
                    .radius(l.t.px("radius-control"))
                    .fill(l.c("bg-control"))
                    .child(col().grow(1.0).child(text(url.clone()).key("phone-link").font(l.ty.mono).color(l.c("text-primary")).wrap_anywhere()))
                    .child(
                        text("Copy the link")
                            .key("phone-copy")
                            .font(l.ty.control)
                            .color(l.c("text-secondary"))
                            .py(4.0)
                            .px(8.0)
                            .radius(l.t.px("radius-control"))
                            .hover(l.c("hover-surface"))
                            .role("button")
                            .on_press_tx(move |_s: &mut State, tx| crate::reply_actions::request_copy(tx, link.clone())),
                    ),
            );
        if cfg!(windows) {
            side = side.child(text(WINDOWS_ASKS).font(l.ty.body).color(l.c("text-secondary")).wrap());
        }
        side = side.child(text(ANYONE).font(l.ty.body).color(l.c("text-secondary")).wrap()).child(
            row().child(
                text("New code")
                    .key("phone-new-code")
                    .font(l.ty.control)
                    .color(l.c("text-primary"))
                    .py(6.0)
                    .px(10.0)
                    .radius(l.t.px("radius-control"))
                    .fill(l.c("bg-control"))
                    .role("button")
                    .on_press(new_code),
            ),
        );
        let mut code_row = row().w_full().gap(16.0).px(10.0);
        if let Some(code) = qr_with_label(url, QR_LABEL_WIFI) {
            code_row = code_row.child(code);
        }
        section = section.child(code_row.child(side));
    }
    section
}

/// General with the phone on: the link a phone on this Wi-Fi opens, and New code.
pub(super) fn scene(s: &mut State) {
    crate::settings_panel::scene(s);
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::App);
    s.settings_panel.phone = Some(PhoneSettings {
        enabled: true,
        url: Some("http://192.168.1.20:47740/#Qm9va1RoZVNlY29uZE9uZVBsZWFzZQ".into()),
        addresses: vec!["http://192.168.1.20:47740".into()],
        message: None,
    });
}

pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
    s.settings_panel.theme = crate::workspace_settings::ThemeChoice::Light;
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_core::phone_anywhere::PairingCode;

    fn settings(enabled: bool, url: Option<&str>) -> PhoneSettings {
        PhoneSettings { enabled, url: url.map(str::to_owned), addresses: Vec::new(), message: None }
    }

    fn off_wifi() -> PhoneSettings {
        settings(false, None)
    }

    fn off_anywhere() -> PhoneAnywhereSettings {
        PhoneAnywhereSettings {
            enabled: false,
            connected: false,
            code: None,
            devices: Vec::new(),
            github_login: None,
            message: None,
            night_approvals: false,
        }
    }

    fn sample_code() -> PairingCode {
        PairingCode {
            url: "https://app.locust.lol/#p=AAAAAAAAAAAAAAAAAAAAAA.BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB.CCCCCCCCCCCCCCCCCCCCCC".into(),
            until: "2099-01-01T00:00:00.000Z".into(),
            desk: false,
        }
    }

    fn render(wifi: PhoneSettings, anywhere: PhoneAnywhereSettings) -> Vec<rune::ElementInfo> {
        let mut s = State::default();
        s.settings_panel.phone = Some(wifi);
        s.phone_anywhere = Some(anywhere);
        let mut rt = rune::Runtime::new(
            s,
            |s, _| section(s, &Palette::new(true)),
            (1200.0, 1400.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.elements()
    }

    fn all_text(els: &[rune::ElementInfo]) -> String {
        els.iter().filter_map(|e| e.text.as_deref()).collect::<Vec<_>>().join("\n")
    }

    fn by_key<'a>(els: &'a [rune::ElementInfo], key: &str) -> Option<&'a rune::ElementInfo> {
        els.iter().find(|e| e.key.as_deref() == Some(key))
    }

    #[test]
    fn says_off_on_and_no_network_in_electrons_words_and_off_when_the_organization_turns_it_off() {
        assert_eq!(lede(&settings(false, None), false), OFF);
        assert_eq!(lede(&settings(true, None), false), NO_NETWORK);
        assert_eq!(lede(&settings(true, Some("http://192.168.1.20:47740/#k")), false), ON);
        assert_eq!(lede(&settings(true, Some("http://192.168.1.20:47740/#k")), true), OFF);
        assert_eq!(
            OFF,
            "Off. Turn it on to check on your teammates from your phone, answer approval cards and send the next message."
        );
        assert_eq!(
            NO_NETWORK,
            "On, but this computer is not on a network Locust can find. Connect it to your Wi-Fi, then turn this off and on again."
        );
    }

    fn switch_on(e: &rune::ElementInfo) -> bool {
        e.fill == crate::look::controls::Look::new(true).c("switch-on")
    }

    fn radio_active(e: &rune::ElementInfo) -> bool {
        e.fill == crate::look::controls::Look::new(true).c("bg-selected")
    }

    #[test]
    // case: is off until turned on, with From anywhere chosen for when it is
    fn is_off_until_turned_on_with_from_anywhere_chosen_for_when_it_is() {
        let els = render(off_wifi(), off_anywhere());
        let switch = by_key(&els, "phone-switch").expect("phone switch");
        assert_eq!(switch.role, "switch");
        assert_eq!(switch.label.as_deref(), Some("Use Locust from your phone"));
        assert!(!switch_on(switch));
        let text = all_text(&els);
        assert!(text.contains("Off. Turn it on to check on your teammates from your phone"), "{text}");
        let anywhere_radio = by_key(&els, "phone-way-anywhere").expect("From anywhere radio");
        assert_eq!(anywhere_radio.role, "radio");
        assert!(radio_active(anywhere_radio));
        let wifi_radio = by_key(&els, "phone-way-wifi").expect("Only on this Wi-Fi radio");
        assert_eq!(wifi_radio.role, "radio");
        assert!(!radio_active(wifi_radio));
        assert!(by_key(&els, "phone-code").is_none());
    }

    #[test]
    // case: shows the code to pair a phone with, and says whether this computer is connected to the relay
    fn shows_the_code_to_pair_a_phone_with_and_says_whether_this_computer_is_connected_to_the_relay() {
        let els = render(
            off_wifi(),
            PhoneAnywhereSettings { enabled: true, connected: true, code: Some(sample_code()), ..off_anywhere() },
        );
        let switch = by_key(&els, "phone-switch").expect("phone switch");
        assert!(switch_on(switch));
        let text = all_text(&els);
        assert!(text.contains("On, from anywhere."), "{text}");
        assert!(text.contains("the relay passes along what it cannot read"), "{text}");
        let qr = by_key(&els, "phone-code").expect("QR code");
        assert_eq!(qr.label.as_deref(), Some("A code for your phone\u{2019}s camera. It pairs your phone with Locust."));
        assert!(text.contains("Connected to Locust\u{2019}s relay."), "{text}");
        assert!(text.contains("It pairs one phone, once, within ten minutes"), "{text}");
        assert!(!text.contains("Lost a phone?"), "{text}");

        let connecting = all_text(&render(
            off_wifi(),
            PhoneAnywhereSettings { enabled: true, connected: false, code: Some(sample_code()), ..off_anywhere() },
        ));
        assert!(connecting.contains("Connecting to Locust\u{2019}s relay\u{2026}"), "{connecting}");

        let with_msg = all_text(&render(
            off_wifi(),
            PhoneAnywhereSettings {
                enabled: true,
                connected: false,
                message: Some("Locust\u{2019}s relay no longer knows this computer.".into()),
                ..off_anywhere()
            },
        ));
        assert!(with_msg.contains("Locust\u{2019}s relay no longer knows this computer."), "{with_msg}");
    }

    #[test]
    // case: lists each paired phone, whether it is open now, and a Remove for it
    fn lists_each_paired_phone_whether_it_is_open_now_and_a_remove_for_it() {
        let three_min_ago = (chrono::Utc::now() - chrono::Duration::minutes(3)).to_rfc3339();
        let els = render(
            off_wifi(),
            PhoneAnywhereSettings {
                enabled: true,
                connected: true,
                devices: vec![
                    PairedDeviceView {
                        id: "d1".into(),
                        name: "iPhone".into(),
                        added: "2026-10-23T00:00:00.000Z".into(),
                        seen: None,
                        online: true,
                        alerts: false,
                        desk: None,
                        github: false,
                    },
                    PairedDeviceView {
                        id: "d2".into(),
                        name: "Android phone".into(),
                        added: "2026-10-22T00:00:00.000Z".into(),
                        seen: Some(three_min_ago),
                        online: false,
                        alerts: false,
                        desk: None,
                        github: false,
                    },
                ],
                ..off_anywhere()
            },
        );
        let list = by_key(&els, "phone-devices").expect("Paired phones list");
        assert_eq!(list.label.as_deref(), Some("Paired phones"));
        let text = all_text(&els);
        assert!(text.contains("iPhone"), "{text}");
        assert!(text.contains("Open now"), "{text}");
        assert!(text.contains("Android phone"), "{text}");
        assert!(text.contains("Last open 3 minutes ago"), "{text}");
        let removes = els.iter().filter(|e| e.text.as_deref() == Some("Remove")).count();
        assert_eq!(removes, 2);
        assert!(text.contains("Lost a phone? Remove it below, and it stops at once."), "{text}");
        assert!(text.contains("Pair a phone"), "{text}");
        assert!(by_key(&els, "phone-code").is_none());
    }

    #[test]
    // case: says which phones get alerts, and offers approvals in quiet hours only once one does
    fn says_which_phones_get_alerts_and_offers_approvals_in_quiet_hours_only_once_one_does() {
        let quietly = render(
            off_wifi(),
            PhoneAnywhereSettings {
                enabled: true,
                connected: true,
                devices: vec![PairedDeviceView {
                    id: "d1".into(),
                    name: "iPhone".into(),
                    added: "2026-10-23T00:00:00.000Z".into(),
                    seen: None,
                    online: true,
                    alerts: false,
                    desk: None,
                    github: false,
                }],
                ..off_anywhere()
            },
        );
        assert!(by_key(&quietly, "phone-night-approvals").is_none());

        let with_alerts = render(
            off_wifi(),
            PhoneAnywhereSettings {
                enabled: true,
                connected: true,
                night_approvals: false,
                devices: vec![PairedDeviceView {
                    id: "d1".into(),
                    name: "iPhone".into(),
                    added: "2026-10-23T00:00:00.000Z".into(),
                    seen: None,
                    online: false,
                    alerts: true,
                    desk: None,
                    github: false,
                }],
                ..off_anywhere()
            },
        );
        let text = all_text(&with_alerts);
        assert!(text.contains("Paired \u{b7} alerts on"), "{text}");
        let night = by_key(&with_alerts, "phone-night-approvals").expect("night approvals switch");
        assert_eq!(night.label.as_deref(), Some("Let approvals buzz in quiet hours"));
        assert!(!switch_on(night));

        let with_night_on = render(
            off_wifi(),
            PhoneAnywhereSettings {
                enabled: true,
                connected: true,
                night_approvals: true,
                devices: vec![PairedDeviceView {
                    id: "d1".into(),
                    name: "iPhone".into(),
                    added: "2026-10-23T00:00:00.000Z".into(),
                    seen: None,
                    online: false,
                    alerts: true,
                    desk: None,
                    github: false,
                }],
                ..off_anywhere()
            },
        );
        let night_on = by_key(&with_night_on, "phone-night-approvals").expect("night approvals switch");
        assert!(switch_on(night_on));
    }

    #[test]
    // case: keeps the home-network code as it was when Only on this Wi-Fi is on
    fn keeps_the_home_network_code_as_it_was_when_only_on_this_wi_fi_is_on() {
        let els = render(
            PhoneSettings {
                enabled: true,
                url: Some("http://192.168.1.20:47740/#abcdefghijklmnopqrstuvwxyzABCDEF".into()),
                addresses: vec!["http://192.168.1.20:47740".into()],
                message: None,
            },
            off_anywhere(),
        );
        let wifi_radio = by_key(&els, "phone-way-wifi").expect("Only on this Wi-Fi radio");
        assert!(radio_active(wifi_radio));
        let text = all_text(&els);
        assert!(text.contains("Your phone must be on the same Wi-Fi as this computer."), "{text}");
        let qr = by_key(&els, "phone-code").expect("Wi-Fi QR code");
        assert_eq!(qr.label.as_deref(), Some("A code for your phone\u{2019}s camera. It opens Locust on your phone."));
        assert!(text.contains("New code"), "{text}");
        assert!(by_key(&els, "phone-devices").is_none());
    }

    #[test]
    // case: offers Open in your browser with phones off, saying it turns From anywhere on
    fn offers_open_in_your_browser_with_phones_off_saying_it_turns_from_anywhere_on() {
        let els = render(off_wifi(), off_anywhere());
        let text = all_text(&els);
        assert!(
            text.contains("Or open Locust\u{2019}s whole window in this computer\u{2019}s browser. It turns this on, from anywhere."),
            "{text}"
        );
        assert!(text.contains("Open in your browser"), "{text}");

        // Not while phones reach Locust only on this Wi-Fi: that way would be turned off.
        let wifi_only = all_text(&render(
            PhoneSettings { enabled: true, url: Some("http://192.168.1.2:1234/#k".into()), addresses: Vec::new(), message: None },
            off_anywhere(),
        ));
        assert!(!wifi_only.contains("Open in your browser"), "{wifi_only}");
    }

    #[test]
    // case: shows Open in your browser’s code as a link, never as a code for a phone to scan
    fn shows_open_in_your_browsers_code_as_a_link_never_as_a_code_for_a_phone_to_scan() {
        let mut desk_code = sample_code();
        desk_code.url.push_str(".d");
        desk_code.desk = true;
        let els = render(
            off_wifi(),
            PhoneAnywhereSettings { enabled: true, connected: true, code: Some(desk_code), ..off_anywhere() },
        );
        let text = all_text(&els);
        assert!(text.contains("Opening Locust in this computer\u{2019}s browser."), "{text}");
        assert!(text.contains("Copy the link"), "{text}");
        assert!(by_key(&els, "phone-code").is_none());

        let plain = render(
            off_wifi(),
            PhoneAnywhereSettings { enabled: true, connected: true, code: Some(sample_code()), ..off_anywhere() },
        );
        assert!(by_key(&plain, "phone-code").is_some());
        let plain_text = all_text(&plain);
        assert!(plain_text.contains("Or open the whole window in this computer\u{2019}s browser, paired at once."), "{plain_text}");
    }

    #[test]
    // case: offers Sign in with GitHub, saying what is checked and that GitHub is asked for nothing else
    fn offers_sign_in_with_github_saying_what_is_checked_and_that_github_is_asked_for_nothing_else() {
        let els = render(
            off_wifi(),
            PhoneAnywhereSettings { enabled: true, connected: true, ..off_anywhere() },
        );
        let text = all_text(&els);
        assert!(
            text.contains("Sign in with GitHub, and any browser where you sign in to GitHub opens Locust at app.locust.lol/desk, with no code."),
            "{text}"
        );
        assert!(
            text.contains("then this computer checks again with GitHub itself. Locust asks GitHub for no permissions, only who you are."),
            "{text}"
        );
        let btn = by_key(&els, "phone-github-btn").expect("Sign in with GitHub button");
        assert!(btn.focusable, "Sign in with GitHub should be enabled when connected");

        let disconnected = render(
            off_wifi(),
            PhoneAnywhereSettings { enabled: true, connected: false, ..off_anywhere() },
        );
        let disabled_btn = by_key(&disconnected, "phone-github-btn").expect("Sign in with GitHub button");
        assert!(!disabled_btn.focusable, "Sign in with GitHub must be disabled before the relay answers");
    }

    #[test]
    // case: once signed in, says whom it lets in, and marks the browsers that came in that way
    fn once_signed_in_says_whom_it_lets_in_and_marks_the_browsers_that_came_in_that_way() {
        let els = render(
            off_wifi(),
            PhoneAnywhereSettings {
                enabled: true,
                connected: true,
                github_login: Some("octo".into()),
                devices: vec![PairedDeviceView {
                    id: "d1".into(),
                    name: "Chrome on Windows".into(),
                    added: "2026-10-28T00:00:00.000Z".into(),
                    seen: None,
                    online: true,
                    alerts: false,
                    desk: Some(true),
                    github: true,
                }],
                ..off_anywhere()
            },
        );
        let text = all_text(&els);
        assert!(
            text.contains("Any browser where you sign in to GitHub as octo opens Locust at app.locust.lol/desk, with no code."),
            "{text}"
        );
        assert!(
            text.contains("Signing out stops new sign-ins. Browsers already in stay paired until you remove them above."),
            "{text}"
        );
        assert!(text.contains("Sign out of GitHub"), "{text}");
        assert!(text.contains("Open now \u{b7} signed in with GitHub"), "{text}");
    }

    struct FakeRelayLine {
        callbacks: Arc<std::sync::Mutex<Option<Arc<dyn locust_core::phone_anywhere::RelaySocketCallbacks>>>>,
        frames: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
        closed: Arc<std::sync::atomic::AtomicBool>,
        github_off: bool,
        notes: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    }

    impl locust_core::phone_anywhere::RelaySocket for FakeRelayLine {
        fn send(&self, text: String) -> Result<(), String> {
            let val: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
            self.frames.lock().unwrap().push(val.clone());
            let kind = val.get("t").and_then(serde_json::Value::as_str).unwrap_or("");
            let id = val.get("id").and_then(serde_json::Value::as_str).map(str::to_owned);
            let reply = match kind {
                "auth" => Some(serde_json::json!({ "t": "ok", "as": "pc", "devices": [] })),
                "pair-open" | "reset" | "github-close" | "remove" | "hook-open" | "hook-close" | "hook-collected"
                | "mcp-open" | "mcp-close" | "allow" | "pair-close" => {
                    id.map(|id| serde_json::json!({ "t": "done", "id": id }))
                }
                "hook-letters" => id.map(|id| serde_json::json!({ "t": "hook-letters", "id": id, "items": [], "lost": 0 })),
                "notes" => id.map(|id| serde_json::json!({ "t": "notes", "id": id, "items": self.notes.lock().unwrap().clone() })),
                "notes-collected" => id.map(|id| serde_json::json!({ "t": "done", "id": id })),
                "github-open" => id.map(|id| {
                    if self.github_off {
                        serde_json::json!({ "t": "err", "id": id, "code": "github-off" })
                    } else {
                        serde_json::json!({ "t": "done", "id": id })
                    }
                }),
                _ => None,
            };
            if let Some(reply) = reply
                && let Some(cb) = self.callbacks.lock().unwrap().clone()
            {
                std::thread::spawn(move || {
                    cb.on_message(reply.to_string());
                });
            }
            Ok(())
        }

        fn close(&self) {
            self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
        }

        fn set_callbacks(&self, callbacks: Arc<dyn locust_core::phone_anywhere::RelaySocketCallbacks>) {
            *self.callbacks.lock().unwrap() = Some(callbacks.clone());
            std::thread::spawn(move || {
                callbacks.on_open();
            });
        }
    }

    struct FakeRelay {
        frames: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
        connects: Arc<std::sync::atomic::AtomicUsize>,
        closed: Arc<std::sync::atomic::AtomicBool>,
        callbacks: Arc<std::sync::Mutex<Option<Arc<dyn locust_core::phone_anywhere::RelaySocketCallbacks>>>>,
        github_off: bool,
        notes: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    }

    impl FakeRelay {
        fn new() -> Self {
            Self {
                frames: Arc::new(std::sync::Mutex::new(Vec::new())),
                connects: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                closed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                callbacks: Arc::new(std::sync::Mutex::new(None)),
                github_off: false,
                notes: Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn push_frame(&self, frame: serde_json::Value) {
            if let Some(cb) = self.callbacks.lock().unwrap().clone() {
                cb.on_message(frame.to_string());
            }
        }

        fn connector(&self) -> RelayConnector {
            let frames = Arc::clone(&self.frames);
            let connects = Arc::clone(&self.connects);
            let closed = Arc::clone(&self.closed);
            let callbacks = Arc::clone(&self.callbacks);
            let github_off = self.github_off;
            let notes = Arc::clone(&self.notes);
            Arc::new(move |_url: &str| {
                connects.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(Arc::new(FakeRelayLine {
                    callbacks: Arc::clone(&callbacks),
                    frames: Arc::clone(&frames),
                    closed: Arc::clone(&closed),
                    github_off,
                    notes: Arc::clone(&notes),
                }) as Arc<dyn locust_core::phone_anywhere::RelaySocket>)
            })
        }
    }

    struct TempRoot(std::path::PathBuf);
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_store(tag: &str) -> (TempRoot, locust_core::store::Store) {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "locust-phone3-{tag}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("native")).unwrap();
        std::fs::create_dir_all(root.join("electron")).unwrap();
        let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
        (TempRoot(root), store)
    }

    fn headless_phone_window(
        store: locust_core::store::Store,
        relay: &FakeRelay,
    ) -> rune::Runtime<State> {
        let connector = relay.connector();
        headless_window(store, move |link| link.set_test_relay(connector, "https://relay.test", None))
    }

    fn headless_window(
        store: locust_core::store::Store,
        link: impl FnOnce(&mut PhoneAnywhereLink),
    ) -> rune::Runtime<State> {
        let mut s = State::default();
        s.engine = Some(Kind::Native);
        s.data = Some(store);
        s.settings_panel.open = true;
        s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::App);
        link(&mut s.phone_anywhere_link);
        let mut rt = rune::Runtime::new(
            s,
            |s, _| section(s, &Palette::new(true)),
            (1200.0, 1400.0),
            rune::Env::new(true, 1.0),
        );
        rt.update(|s, tx| {
            crate::poll(s, 0.0, tx);
        });
        rt.frame(0.0);
        rt
    }

    fn click_key(rt: &mut rune::Runtime<State>, key: &str) {
        rt.frame(0.05);
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.05);
    }

    fn pump_until(rt: &mut rune::Runtime<State>, what: &str, mut done: impl FnMut(&State) -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !done(&rt.state) {
            assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
            rt.update(|s, tx| {
                crate::poll(s, 0.0, tx);
            });
            rt.frame(0.05);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// NSURLSession's place, taken by a client that connects nowhere: it answers the PC's auth as the relay does.
    #[cfg(target_os = "macos")]
    #[derive(Default)]
    struct FakeWs {
        urls: std::sync::Mutex<Vec<String>>,
        auths: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[cfg(target_os = "macos")]
    struct FakeWsLine {
        heard: Arc<dyn locust_core::chat_http::WsCallbacks>,
        auths: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[cfg(target_os = "macos")]
    impl locust_core::chat_http::WsConnection for FakeWsLine {
        fn send_text(&self, text: &str) -> Result<(), String> {
            let frame: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
            if frame["t"] == "auth" && frame["as"] == "pc" {
                self.auths.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let heard = self.heard.clone();
                std::thread::spawn(move || heard.on_message(serde_json::json!({ "t": "ok", "as": "pc", "devices": [] }).to_string()));
            }
            Ok(())
        }
        fn close(&self, _code: u16, _reason: &str) {}
    }

    #[cfg(target_os = "macos")]
    impl WsClient for FakeWs {
        fn connect(
            &self,
            url: &str,
            heard: Arc<dyn locust_core::chat_http::WsCallbacks>,
        ) -> Result<Arc<dyn locust_core::chat_http::WsConnection>, String> {
            self.urls.lock().unwrap().push(url.to_owned());
            Ok(Arc::new(FakeWsLine { heard, auths: self.auths.clone() }))
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn on_a_mac_from_anywhere_reaches_the_relay_over_its_websocket_and_says_it_is_connected() {
        let (_root, store) = temp_store("websocket");
        let ws = Arc::new(FakeWs::default());
        let client: Arc<dyn WsClient> = ws.clone();
        let mut rt = headless_window(store, move |link| link.set_test_ws_client(client, "https://relay.test"));
        click_key(&mut rt, "phone-switch");
        pump_until(&mut rt, "connected over the WebSocket", |s| {
            s.phone_anywhere.as_ref().is_some_and(|a| a.enabled && a.connected)
        });
        let room = rt.state.phone_anywhere_link.live.as_ref().and_then(PhoneAnywhere::room).unwrap().1;
        assert_eq!(ws.urls.lock().unwrap().as_slice(), [format!("wss://relay.test/v1/room/{room}")]);
        assert_eq!(ws.auths.load(std::sync::atomic::Ordering::SeqCst), 1);
        let text = all_text(&rt.elements());
        assert!(text.contains("Connected to Locust\u{2019}s relay."), "{text}");
    }

    /// A browser paired by Open in your browser's code, over the window's own fake relay: its device and keys.
    struct DeskBrowser {
        room: String,
        device: String,
        keys: locust_core::relay_crypto::LinkKeys,
    }

    fn pair_desk_browser(rt: &mut rune::Runtime<State>, relay: &FakeRelay) -> DeskBrowser {
        use locust_core::relay_crypto::{Direction, LinkSide, aad_of, device_id_of, link_keys, new_pc_keys, pairing_proof, seal_json};
        let url = rt.state.phone_anywhere_link.live.as_ref().and_then(PhoneAnywhere::desk_code).expect("a code for the full window");
        let parts: Vec<String> = url.split("#p=").nth(1).unwrap().split('.').map(str::to_owned).collect();
        let (room, pc_key, secret) = (parts[0].clone(), parts[1].clone(), parts[2].clone());
        let mine = new_pc_keys();
        let device = device_id_of(&mine.public_key);
        let keys = link_keys(&mine.private_key, &pc_key, &room, LinkSide::Device).unwrap();
        let hello = seal_json(
            &keys.seal,
            &serde_json::json!({ "v": 1, "kind": "pair", "proof": pairing_proof(&secret), "name": "Chrome" }),
            &aad_of(&room, &device, Direction::DeviceToPc),
        )
        .unwrap();
        relay.push_frame(serde_json::json!({ "t": "msg", "from": "pairing:p1", "n": hello.n, "c": hello.c, "k": mine.public_key }));
        pump_until(rt, "the browser paired with the full window", |s| {
            s.phone_anywhere.as_ref().is_some_and(|a| a.devices.iter().any(|d| d.desk == Some(true)))
        });
        DeskBrowser { room, device, keys }
    }

    fn desk_frame(relay: &FakeRelay, browser: &DeskBrowser, q: u64, items: serde_json::Value) {
        use locust_core::relay_crypto::{Direction, aad_of, seal_json};
        let boxed = seal_json(
            &browser.keys.seal,
            &serde_json::json!({ "v": 1, "kind": "desk", "s": "browser-session", "q": q, "t": now_ms(), "items": items }),
            &aad_of(&browser.room, &browser.device, Direction::DeviceToPc),
        )
        .unwrap();
        relay.push_frame(serde_json::json!({ "t": "msg", "from": browser.device, "n": boxed.n, "c": boxed.c }));
    }

    /// The desk frames this computer sent the browser, opened, from the `from`th on.
    fn told_desk(relay: &FakeRelay, browser: &DeskBrowser, from: usize) -> Vec<serde_json::Value> {
        use locust_core::relay_crypto::{Direction, aad_of, open_json};
        let frames = relay.frames.lock().unwrap().clone();
        frames
            .iter()
            .filter(|f| f["t"] == "send" && f["to"] == browser.device.as_str())
            .skip(from)
            .filter_map(|f| {
                open_json(
                    &browser.keys.open,
                    f["n"].as_str()?,
                    f["c"].as_str()?,
                    &aad_of(&browser.room, &browser.device, Direction::PcToDevice),
                )
                .ok()
            })
            .filter(|said| said["kind"] == "desk")
            .collect()
    }

    #[test]
    // case: says what the computer is, and who has the window
    // case: lets the window go when the browser that has it may no longer have it
    fn a_browser_with_the_full_window_is_answered_by_this_windows_desk_host_and_covers_it_until_let_go() {
        let (_root, store) = temp_store("desk");
        let relay = FakeRelay::new();
        let mut rt = headless_phone_window(store, &relay);
        rt.state.appearance = Some(false);
        rt.state.phone_anywhere_link.computer_name = Some("DESK-PC".into());
        click_key(&mut rt, "phone-switch");
        pump_until(&mut rt, "connected", |s| s.phone_anywhere.as_ref().is_some_and(|a| a.enabled && a.connected));
        let browser = pair_desk_browser(&mut rt, &relay);

        desk_frame(
            &relay,
            &browser,
            1,
            serde_json::json!([
                { "kind": "hello", "id": "h1" },
                { "kind": "take", "id": "t1" },
                { "kind": "call", "id": "c1", "channel": "appearance:get", "args": "[]" }
            ]),
        );
        pump_until(&mut rt, "the browser has the window", |s| s.elsewhere_cover.covered && !told_desk(&relay, &browser, 0).is_empty());
        assert_eq!(rt.state.elsewhere_cover.device.as_deref(), Some("Chrome"));
        let told = told_desk(&relay, &browser, 0);
        assert_eq!(told.len(), 1, "{told:?}");
        let items = told[0]["items"].as_array().unwrap();
        let hello: serde_json::Value = serde_json::from_str(items[0]["value"].as_str().unwrap()).unwrap();
        assert_eq!(
            hello,
            serde_json::json!({ "version": env!("CARGO_PKG_VERSION"), "computer": "DESK-PC", "theme": "light", "active": "computer" })
        );
        assert_eq!(items[1], serde_json::json!({ "kind": "res", "id": "t1", "ok": true, "value": "true" }));
        let appearance: serde_json::Value = serde_json::from_str(items[2]["value"].as_str().unwrap()).unwrap();
        assert_eq!(appearance["theme"], "light");
        assert_eq!(appearance["resolved"], "light");

        // What the window draws is what the browser reads next.
        rt.state.appearance = Some(true);
        desk_frame(&relay, &browser, 2, serde_json::json!([{ "kind": "call", "id": "c2", "channel": "appearance:get", "args": "[]" }]));
        pump_until(&mut rt, "the second read", |_| told_desk(&relay, &browser, 1).len() == 1);
        let again = told_desk(&relay, &browser, 1);
        let appearance: serde_json::Value = serde_json::from_str(again[0]["items"][0]["value"].as_str().unwrap()).unwrap();
        assert_eq!(appearance["resolved"], "dark");

        // Full window taken away from it: the browser is told this computer has it, and the cover comes down.
        let live = rt.state.phone_anywhere_link.live.clone().unwrap();
        live.set_desk(&browser.device, false);
        pump_until(&mut rt, "the cover down", |s| !s.elsewhere_cover.covered && told_desk(&relay, &browser, 2).len() == 1);
        let gone = told_desk(&relay, &browser, 2);
        assert_eq!(gone[0]["items"], serde_json::json!([{ "kind": "taken", "by": "computer" }]));
    }

    #[test]
    // case: holds with why one whose conversation is gone, or that does not open, and leaves a busy one waiting
    fn a_note_written_while_this_computer_was_off_goes_through_the_phones_own_send_and_says_why_not() {
        use locust_core::relay_crypto::{Direction, aad_of, seal_json};
        let (_root, store) = temp_store("notes");
        let relay = FakeRelay::new();
        let mut rt = headless_phone_window(store, &relay);
        click_key(&mut rt, "phone-switch");
        pump_until(&mut rt, "connected", |s| s.phone_anywhere.as_ref().is_some_and(|a| a.enabled && a.connected));
        let phone = pair_desk_browser(&mut rt, &relay);
        let note = seal_json(
            &phone.keys.seal,
            &serde_json::json!({ "v": 1, "kind": "note", "nid": "note-written-offline-1", "missionId": "m_not_in_locust", "text": "Run the tests again", "t": now_ms() }),
            &aad_of(&phone.room, &phone.device, Direction::DeviceToPc),
        )
        .unwrap();
        relay.notes.lock().unwrap().push(serde_json::json!({
            "id": format!("{:013}-abcdefgh", now_ms()), "at": now_ms(), "device": phone.device, "n": note.n, "c": note.c
        }));
        relay.push_frame(serde_json::json!({ "t": "notes-changed" }));
        // The phone's own send reads the conversation, finds none, and starts nothing: held as failed, for the phone.
        let collected = || relay.frames.lock().unwrap().iter().find(|f| f["t"] == "notes-collected").cloned();
        pump_until(&mut rt, "the note collected", |_| collected().is_some());
        let said = collected().unwrap();
        assert_eq!(said["notes"], serde_json::json!([]));
        assert_eq!(said["held"][0]["why"], "failed");
        assert_eq!(said["held"][0]["note"], relay.notes.lock().unwrap()[0]["id"]);
    }

    /// A box that keeps a key by turning it round: never the keychain.
    struct TurnedRound;
    impl locust_core::own_models::SecretBox for TurnedRound {
        fn available(&self) -> bool {
            true
        }
        fn seal(&self, text: &str) -> Result<String, String> {
            Ok(text.chars().rev().collect())
        }
        fn open(&self, sealed: &str) -> Result<String, String> {
            Ok(sealed.chars().rev().collect())
        }
    }

    fn sent(relay: &FakeRelay, kind: &str) -> usize {
        relay.frames.lock().unwrap().iter().filter(|f| f["t"] == kind).count()
    }

    #[test]
    fn the_routines_links_reach_the_relay_through_the_phones_own_link_and_its_notice_refreshes_them() {
        let (_root, store) = temp_store("routine-links");
        locust_core::own_models::use_secrets(Arc::new(TurnedRound));
        let relay = FakeRelay::new();
        let mut rt = headless_phone_window(store, &relay);
        click_key(&mut rt, "phone-switch");
        pump_until(&mut rt, "connected", |s| s.phone_anywhere.as_ref().is_some_and(|a| a.enabled && a.connected));
        crate::routines_panel::open(&mut rt.state);
        let hooks = rt.state.routines.engine.as_ref().expect("the routines' engine").host.hooks().clone();
        let room = rt.state.phone_anywhere_link.live.as_ref().and_then(PhoneAnywhere::room).unwrap().1;
        // The room is the phone link's: a routine not called by a link is told so, never that the relay is missing.
        let state = hooks.state("rt_a", true).unwrap();
        assert_eq!(state.message.as_deref(), Some("Save this routine with When a link is called first."));
        assert_ne!(state.message.as_deref(), Some(locust_core::routine_hooks::MISSING_RELAY));
        // The relay's notice that a link was called (index.ts `hooks: { refresh: syncHooks }`) syncs the links.
        let root = rt.state.data.as_ref().unwrap().root().parent().unwrap().to_path_buf();
        std::fs::create_dir_all(root.join("work")).unwrap();
        std::fs::write(
            root.join("electron").join("teammates.json"),
            serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_a", "name": "Reader", "hue": "blue", "role": "Custom", "createdAt": "2026-11-01T00:00:00Z", "folder": root.join("work").to_string_lossy(), "route": {"runtime": "codex", "model": "account-default", "mode": "ask"}}]}).to_string(),
        )
        .unwrap();
        locust_core::routines::save(
            rt.state.data.as_ref().unwrap(),
            serde_json::json!({"name": "Event summary", "teammateId": "tm_a", "steps": ["Summarize the event."], "schedule": {"kind": "hook"}}),
        )
        .unwrap();
        let id = locust_core::routines::list(rt.state.data.as_ref().unwrap()).unwrap()["routines"][0]["routineId"].as_str().unwrap().to_owned();
        relay.push_frame(serde_json::json!({ "t": "hook-letter" }));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while sent(&relay, "hook-letters") == 0 {
            assert!(std::time::Instant::now() < deadline, "the links were never collected");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let opened = relay.frames.lock().unwrap().iter().any(|f| f["t"] == "hook-open" && f["hook"] == id.as_str());
        assert!(opened, "the routine's link was opened at the relay");
        let made = hooks.state(&id, false).unwrap();
        let prefix = format!("https://relay.test/v1/room/{room}/hook/");
        assert!(made.url.as_deref().is_some_and(|url| url.starts_with(&prefix)), "the link is this room's");
    }

    /// What the PC answered the relay for one app's request.
    fn answered(relay: &FakeRelay, ask: &str) -> serde_json::Value {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let found = relay.frames.lock().unwrap().iter().find(|f| f["t"] == "mcp-answer" && f["ask"] == ask).cloned();
            if let Some(found) = found {
                return found;
            }
            assert!(std::time::Instant::now() < deadline, "no answer to an app's request");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn an_organization_that_turns_off_other_apps_or_the_phone_keeps_from_anywhere_off_and_says_which() {
        // Lane check (26 Nov): index.ts createRemoteMcp's refusal, `otherApps` first, then `phone`.
        struct Back;
        impl Drop for Back {
            fn drop(&mut self) {
                locust_core::org_policy::use_policy_on_this_thread(None);
            }
        }
        let _back = Back;
        let off = |setting: &str| {
            locust_core::org_policy::use_policy_on_this_thread(Some(locust_core::org_policy::PolicyState {
                path: "/test/policy.json".into(),
                present: true,
                policy: [(setting.to_owned(), serde_json::json!(false))].into_iter().collect(),
                error: None,
            }));
        };
        let (_root, store) = temp_store("apps-anywhere-policy");
        let relay = FakeRelay::new();
        let mut rt = headless_phone_window(store, &relay);
        click_key(&mut rt, "phone-switch");
        pump_until(&mut rt, "connected", |s| s.phone_anywhere.as_ref().is_some_and(|a| a.enabled && a.connected));
        let remote = crate::remote_mcp_card::made(&rt.state).expect("the window's pairing");
        off("otherApps");
        let refused = remote.set_enabled(&serde_json::json!(true));
        assert!(!refused.enabled);
        assert_eq!(refused.message.as_deref(), Some("Access from other AI apps, the locust command and editors is off. Set by your organization (otherApps in /test/policy.json). Nothing was started."));
        off("phone");
        let refused = remote.set_enabled(&serde_json::json!(true));
        assert!(!refused.enabled);
        assert_eq!(refused.message.as_deref(), Some("Phone pairing is off. Set by your organization (phone in /test/policy.json). Nothing was started."));
        assert_eq!(sent(&relay, "mcp-open"), 0, "the relay is never told it may take apps");
    }

    #[test]
    fn an_app_from_anywhere_signs_in_and_uses_the_doors_own_tools_through_the_phones_link_until_from_anywhere_is_off() {
        let (_root, store) = temp_store("apps-anywhere");
        let root = store.root().parent().unwrap().to_path_buf();
        std::fs::create_dir_all(root.join("work")).unwrap();
        std::fs::write(
            root.join("electron").join("teammates.json"),
            serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_a", "name": "Reader", "hue": "blue", "role": "Custom", "createdAt": "2026-11-01T00:00:00Z", "folder": root.join("work").to_string_lossy(), "route": {"runtime": "codex", "model": "account-default", "mode": "plan"}}]}).to_string(),
        )
        .unwrap();
        let relay = FakeRelay::new();
        let mut rt = headless_phone_window(store, &relay);
        click_key(&mut rt, "phone-switch");
        pump_until(&mut rt, "connected", |s| s.phone_anywhere.as_ref().is_some_and(|a| a.enabled && a.connected));
        let remote = crate::remote_mcp_card::made(&rt.state).expect("the window's pairing");
        // On only with the phone link's room as the address, and the relay told it may take apps.
        assert!(remote.set_enabled(&serde_json::json!(true)).enabled, "turned on over the phone link");
        assert_eq!(sent(&relay, "mcp-open"), 1);
        let room = rt.state.phone_anywhere_link.live.as_ref().and_then(PhoneAnywhere::room).unwrap().1;
        assert_eq!(rt.state.phone_anywhere_link.mcp_relay().address(), Some(format!("https://relay.test/v1/room/{room}/mcp")));

        // An app signs in the way the relay passes it on, and the person allows its card.
        let redirect = "https://claude.ai/api/mcp/auth_callback";
        let verifier = locust_core::relay_crypto::random_text(32);
        let challenge = locust_core::relay_crypto::sha256_b64url(&verifier);
        let connect = serde_json::json!({"client_id": "client_1", "client": "Claude", "redirect": redirect, "challenge": challenge, "match": "123456"});
        relay.push_frame(serde_json::json!({ "t": "mcp-ask", "ask": "Ask_one-16-chars", "kind": "connect", "body": connect.to_string() }));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let card = loop {
            if let Some(card) = remote.settings().asking.first().cloned() {
                break card;
            }
            assert!(std::time::Instant::now() < deadline, "no card");
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!((card.name.as_str(), card.host.as_str()), ("Claude", "claude.ai"));
        remote.answer_sign_in(&card.request, true);
        let signed: serde_json::Value = serde_json::from_str(answered(&relay, "Ask_one-16-chars")["body"].as_str().unwrap()).unwrap();
        let code = signed["code"].as_str().expect("a code").to_owned();
        let token = serde_json::json!({"grant_type": "authorization_code", "code": code, "code_verifier": verifier, "client_id": "client_1", "redirect_uri": redirect});
        relay.push_frame(serde_json::json!({ "t": "mcp-ask", "ask": "Ask_two-16-chars", "kind": "token", "body": token.to_string() }));
        let keys = answered(&relay, "Ask_two-16-chars");
        assert_eq!(keys["status"], 200);
        let keys: serde_json::Value = serde_json::from_str(keys["body"].as_str().unwrap()).unwrap();
        let key = keys["access_token"].as_str().expect("a key").to_owned();

        // Its call is the door's own list_teammates, in Ask: each teammate's own mode is off.
        let call = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "list_teammates", "arguments": {}}});
        relay.push_frame(serde_json::json!({ "t": "mcp-ask", "ask": "Ask_three-16char", "kind": "call", "body": call.to_string(), "auth": key }));
        let reply = answered(&relay, "Ask_three-16char");
        assert_eq!(reply["status"], 200);
        let reply: serde_json::Value = serde_json::from_str(reply["body"].as_str().unwrap()).unwrap();
        let listed: serde_json::Value = serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!((listed[0]["name"].as_str(), listed[0]["runs_in"].as_str()), (Some("Reader"), Some("ask")));

        // From anywhere off turns this off too (index.ts `changed`).
        click_key(&mut rt, "phone-switch");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while remote.settings().enabled {
            assert!(std::time::Instant::now() < deadline, "still on after From anywhere was turned off");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(remote.settings().apps.is_empty());
    }

    #[test]
    fn switching_from_anywhere_on_creates_live_phone_anywhere_and_off_drops_it() {
        let (_root, store) = temp_store("toggle");
        let relay = FakeRelay::new();
        let mut rt = headless_phone_window(store, &relay);

        // Starts off: no live PhoneAnywhere, settings report disabled.
        assert!(!rt.state.phone_anywhere_link.is_live());
        assert_eq!(rt.state.phone_anywhere.as_ref().map(|a| a.enabled), Some(false));
        assert_eq!(relay.connects.load(std::sync::atomic::Ordering::SeqCst), 0);

        // Switch ON in the headless window: creates live PhoneAnywhere and pushes connected settings into State.
        click_key(&mut rt, "phone-switch");
        assert!(rt.state.phone_anywhere_link.is_live(), "PhoneAnywhere should be live after switching on");
        pump_until(&mut rt, "connected settings in State", |s| {
            s.phone_anywhere.as_ref().is_some_and(|a| a.enabled && a.connected)
        });
        assert!(rt.state.phone_anywhere.as_ref().and_then(|a| a.code.as_ref()).is_some());
        let text = all_text(&rt.elements());
        assert!(text.contains("On, from anywhere."), "{text}");
        assert!(text.contains("Connected to Locust\u{2019}s relay."), "{text}");

        // Switch OFF in the headless window: drops PhoneAnywhere and updates State to off.
        click_key(&mut rt, "phone-switch");
        assert!(!rt.state.phone_anywhere_link.is_live(), "PhoneAnywhere should be dropped after switching off");
        assert_eq!(rt.state.phone_anywhere.as_ref().map(|a| a.enabled), Some(false));
        assert!(relay.closed.load(std::sync::atomic::Ordering::SeqCst));
        let text_off = all_text(&rt.elements());
        assert!(text_off.contains("Off. Turn it on to check on your teammates from your phone"), "{text_off}");
    }

    #[test]
    fn window_restores_saved_from_anywhere_at_start_and_disposes_on_close() {
        let (_root, store) = temp_store("start");
        let relay1 = FakeRelay::new();
        let mut rt1 = headless_phone_window(store.clone(), &relay1);
        click_key(&mut rt1, "phone-switch");
        pump_until(&mut rt1, "first window connected", |s| {
            s.phone_anywhere.as_ref().is_some_and(|a| a.enabled && a.connected)
        });
        // Close window (keeping phone-relay.json on disk, as quitting does).
        rt1.update(|s, tx| crate::on_close(s, tx));
        assert!(!rt1.state.phone_anywhere_link.is_live());
        assert!(store.root().join(PHONE_ANYWHERE_FILE).exists());

        // New headless window starts with saved phone-relay.json: creates PhoneAnywhere at start and connects.
        let relay2 = FakeRelay::new();
        let mut rt2 = headless_phone_window(store, &relay2);
        assert!(rt2.state.phone_anywhere_link.is_live(), "saved From anywhere should be live at start");
        pump_until(&mut rt2, "restored window connected", |s| {
            s.phone_anywhere.as_ref().is_some_and(|a| a.enabled && a.connected)
        });
        assert_eq!(relay2.connects.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Switching to Only on this Wi-Fi while on drops PhoneAnywhere.
        click_key(&mut rt2, "phone-way-wifi");
        assert!(!rt2.state.phone_anywhere_link.is_live(), "switching to Wi-Fi should drop PhoneAnywhere");
        assert_eq!(rt2.state.phone_anywhere.as_ref().map(|a| a.enabled), Some(false));
    }

    #[test]
    fn open_in_browser_button_calls_desk_code_from_off_and_on() {
        let (_root, store) = temp_store("open-browser");
        let relay = FakeRelay::new();
        let opened: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut rt = headless_phone_window(store, &relay);
        let opened_c = Arc::clone(&opened);
        rt.state.phone_anywhere_link.set_test_open_browser(Arc::new(move |url: &str| {
            opened_c.lock().unwrap().push(url.to_owned());
        }));

        // Pressing "Open in your browser" while off turns on From anywhere, calls desk_code, and opens the .d URL.
        click_key(&mut rt, "phone-open-browser");
        assert!(rt.state.phone_anywhere_link.is_live());
        let urls = opened.lock().unwrap().clone();
        assert_eq!(urls.len(), 1, "expected one opened browser URL, got {urls:?}");
        assert!(urls[0].starts_with("https://relay.test/#p="), "unexpected URL: {}", urls[0]);
        assert!(urls[0].ends_with(".d"), "desk_code URL must end with .d: {}", urls[0]);
        let code = rt.state.phone_anywhere.as_ref().and_then(|a| a.code.as_ref()).expect("desk code in State");
        assert!(code.desk);
        assert_eq!(code.url, urls[0]);
        let text = all_text(&rt.elements());
        assert!(text.contains("Opening Locust in this computer\u{2019}s browser."), "{text}");
        assert!(by_key(&rt.elements(), "phone-code").is_none());

        // Turn off, turn on normally (plain pairing code), then click "Open in your browser" in the ON state.
        click_key(&mut rt, "phone-switch");
        assert!(!rt.state.phone_anywhere_link.is_live());
        click_key(&mut rt, "phone-switch");
        pump_until(&mut rt, "reconnected", |s| {
            s.phone_anywhere.as_ref().is_some_and(|a| a.enabled && a.connected)
        });
        assert!(rt.state.phone_anywhere.as_ref().and_then(|a| a.code.as_ref()).is_some_and(|c| !c.desk));
        click_key(&mut rt, "phone-open-browser");
        let urls2 = opened.lock().unwrap().clone();
        assert_eq!(urls2.len(), 2, "expected second opened browser URL, got {urls2:?}");
        assert!(urls2[1].ends_with(".d"), "second desk_code URL must end with .d: {}", urls2[1]);
        assert!(rt.state.phone_anywhere.as_ref().and_then(|a| a.code.as_ref()).is_some_and(|c| c.desk));
    }

    #[test]
    fn github_buttons_call_github_link_and_github_sign_out_on_live_phone_anywhere() {
        let (_root, store) = temp_store("github-btns");
        let relay = FakeRelay::new();
        let opened: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut rt = headless_phone_window(store, &relay);
        let opened_c = Arc::clone(&opened);
        rt.state.phone_anywhere_link.set_test_open_browser(Arc::new(move |url: &str| {
            opened_c.lock().unwrap().push(url.to_owned());
        }));

        // Turn From anywhere on and wait for connection.
        click_key(&mut rt, "phone-switch");
        pump_until(&mut rt, "connected", |s| {
            s.phone_anywhere.as_ref().is_some_and(|a| a.enabled && a.connected)
        });

        // Click "Sign in with GitHub": calls github_link, sends github-open, opens /v1/github/start URL.
        click_key(&mut rt, "phone-github-btn");
        let urls = opened.lock().unwrap().clone();
        assert_eq!(urls.len(), 1, "expected one github_link URL, got {urls:?}");
        assert!(urls[0].starts_with("https://relay.test/v1/github/start?room="), "unexpected URL: {}", urls[0]);
        assert!(urls[0].contains("&link="), "missing link param: {}", urls[0]);
        assert!(
            relay.frames.lock().unwrap().iter().any(|f| f.get("t").and_then(serde_json::Value::as_str) == Some("github-open")),
            "expected github-open frame sent to relay"
        );

        // Relay reports signed-in GitHub account; poll pushes github_login into State.
        relay.push_frame(serde_json::json!({ "t": "github", "account": { "user": 42, "login": "octo" } }));
        pump_until(&mut rt, "github_login in State", |s| {
            s.phone_anywhere.as_ref().and_then(|a| a.github_login.as_deref()) == Some("octo")
        });
        let signed_in_text = all_text(&rt.elements());
        assert!(signed_in_text.contains("Sign out of GitHub"), "{signed_in_text}");

        // Click "Sign out of GitHub": calls github_sign_out, sends github-close, clears github_login.
        click_key(&mut rt, "phone-github-btn");
        assert!(
            relay.frames.lock().unwrap().iter().any(|f| f.get("t").and_then(serde_json::Value::as_str) == Some("github-close")),
            "expected github-close frame sent to relay"
        );
        assert_eq!(rt.state.phone_anywhere.as_ref().and_then(|a| a.github_login.as_deref()), None);
        let signed_out_text = all_text(&rt.elements());
        assert!(signed_out_text.contains("Sign in with GitHub"), "{signed_out_text}");
    }
}
