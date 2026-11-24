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
use locust_core::phone_calls::{EnginePhoneCalls, NativePhoneEngine};
use locust_core::phone_host::{PhoneHost, PhoneSettings};
use std::sync::{Arc, OnceLock};

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
}

impl PhoneAnywhereLink {
    pub(crate) fn is_live(&self) -> bool {
        self.live.is_some()
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
    pub(crate) fn set_test_open_browser(&mut self, open: Arc<dyn Fn(&str) + Send + Sync>) {
        self.open_browser = Some(open);
    }
}

impl Drop for PhoneAnywhereLink {
    fn drop(&mut self) {
        if let Some(pa) = self.live.take() {
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

fn make_anywhere(s: &mut State) -> Option<PhoneAnywhere> {
    let store = s.data.clone().or_else(locust_core::native::store)?;
    let dir = store.root().to_path_buf();
    let now: Arc<dyn Fn() -> i64 + Send + Sync> = Arc::new(now_ms);
    let engine = Arc::new(NativePhoneEngine::new(store));
    let calls = Arc::new(EnginePhoneCalls::new(engine, now.clone()));
    let (tx, rx) = std::sync::mpsc::channel::<PhoneAnywhereSettings>();
    let waker = s.phone_anywhere_link.waker.clone();
    s.phone_anywhere_link.rx = Some(rx);
    let relay_override = s.phone_anywhere_link.relay_url.clone();
    let connect = s.phone_anywhere_link.connector.clone().unwrap_or_else(|| {
        Arc::new(|_url: &str| Err("Relay WebSocket transport is not available yet.".to_owned()))
    });
    let github_user = s.phone_anywhere_link.github_user.clone();
    let computer_override = s.phone_anywhere_link.computer_name.clone();
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
        away: None,
        hooks: None,
        desk: None,
        notes: None,
        github_user,
        computer_name: Some(Arc::new(move || {
            computer_override.clone().or_else(|| {
                std::env::var("COMPUTERNAME")
                    .or_else(|_| std::env::var("HOSTNAME"))
                    .ok()
            })
        })),
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
            let next = pa.load();
            if next.enabled {
                s.phone_anywhere = Some(next);
                s.phone_anywhere_link.live = Some(pa);
            } else {
                pa.dispose();
                s.phone_anywhere_link.rx = None;
                s.phone_anywhere = Some(next);
            }
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
    s.phone_anywhere != before
}

/// Drops the live `PhoneAnywhere` cleanly when the window closes or quits.
pub(crate) fn dispose(s: &mut State) {
    if let Some(pa) = s.phone_anywhere_link.live.take() {
        pa.dispose();
    }
    s.phone_anywhere_link.rx = None;
}

fn enable_anywhere(s: &mut State) {
    if s.phone_anywhere_link.live.is_none() {
        let Some(pa) = make_anywhere(s) else {
            return;
        };
        s.phone_anywhere_link.live = Some(pa);
    }
    if let Some(pa) = s.phone_anywhere_link.live.as_ref() {
        let next = pa.set_enabled(true);
        if next.enabled {
            s.phone_anywhere = Some(next);
        } else {
            if let Some(pa) = s.phone_anywhere_link.live.take() {
                pa.dispose();
            }
            s.phone_anywhere_link.rx = None;
            s.phone_anywhere = Some(next);
        }
    }
}

fn disable_anywhere(s: &mut State) {
    if let Some(pa) = s.phone_anywhere_link.live.take() {
        let mut next = pa.set_enabled(false);
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
    }

    impl locust_core::phone_anywhere::RelaySocket for FakeRelayLine {
        fn send(&self, text: String) -> Result<(), String> {
            let val: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
            self.frames.lock().unwrap().push(val.clone());
            let kind = val.get("t").and_then(serde_json::Value::as_str).unwrap_or("");
            let id = val.get("id").and_then(serde_json::Value::as_str).map(str::to_owned);
            let reply = match kind {
                "auth" => Some(serde_json::json!({ "t": "ok", "as": "pc", "devices": [] })),
                "pair-open" | "reset" | "github-close" | "remove" => {
                    id.map(|id| serde_json::json!({ "t": "done", "id": id }))
                }
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
    }

    impl FakeRelay {
        fn new() -> Self {
            Self {
                frames: Arc::new(std::sync::Mutex::new(Vec::new())),
                connects: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                closed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                callbacks: Arc::new(std::sync::Mutex::new(None)),
                github_off: false,
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
            Arc::new(move |_url: &str| {
                connects.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(Arc::new(FakeRelayLine {
                    callbacks: Arc::clone(&callbacks),
                    frames: Arc::clone(&frames),
                    closed: Arc::clone(&closed),
                    github_off,
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
        let mut s = State::default();
        s.engine = Some(Kind::Native);
        s.data = Some(store);
        s.settings_panel.open = true;
        s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::App);
        s.phone_anywhere_link.set_test_relay(relay.connector(), "https://relay.test", None);
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
