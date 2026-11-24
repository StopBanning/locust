//! "Use from anywhere" in Settings (Electron `RemoteMcpSettings.tsx`) and the card an app's sign-in becomes in the
//! window (Electron `RemoteConnectCard.tsx`, mounted over every screen). The logic, keys and hashes are
//! `locust_core::remote_mcp`'s; this only shows its state and passes the person's answers on.
//!
//! The relay is `StubMcpRelay` until Apple's NSURLSession transport lands, so From anywhere is never on and the switch
//! says so in Electron's own words. The "keep it running" line (Electron's `stay`) is not shown: it needs the tray and
//! login-item switches this window does not have yet.

use super::*;
use chrono::{DateTime, Local};
use locust_core::door_tools::ToolResult;
use locust_core::remote_mcp::{McpTools, Options, REMOTE_MCP_FILE, RemoteMcp, RemoteMcpApp, RemoteMcpState, StubMcpRelay};
use std::sync::{Arc, Mutex};

static SERVICE: Mutex<Option<Arc<RemoteMcp>>> = Mutex::new(None);

pub const HEADING: &str = "Use from anywhere";
pub const LEDE_ON: &str = "On. Claude on the web, Claude Duet and other apps that take a remote MCP server can use your team through Locust\u{2019}s relay, by the same rules as an app on this computer. Each one connects only after you allow it on a card here.";
pub const LEDE_OFF: &str = "Off. Turn on to use your team from Claude on the web, Claude Duet, or any app that takes a remote MCP server.";
pub const SWITCH_LABEL: &str = "Use your team from anywhere";
pub const ADDRESS_HELP: &str = "In Claude, open Settings, then Connectors, choose Add custom connector, and paste this address:";
pub const OPEN_WHILE_USED: &str = "Locust must be open on this computer while an app uses it.";
pub const RELAY_READS: &str = "The relay can read what an app asks and what Locust answers, as an app\u{2019}s own servers can; it keeps none of it, and the keys are this computer\u{2019}s.";
pub const CARD_NOTE: &str = "It can then ask your teammates and run your routines by the rules under Let your other AI apps use Locust. Remove it there at any time.";

/// Startup switches that keep Locust reachable from anywhere (Electron `RemoteMcpSettings.tsx::StayingReachable`, 0.842).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StayingReachable {
    pub keep_running: bool,
    pub open_at_login: bool,
    pub login_item_available: bool,
    /// Platform name (`"win32"`, `"darwin"`, `"linux"`), defaulting to Windows wording like Electron's `machineWords(undefined)`.
    pub platform: Option<String>,
}

impl Default for StayingReachable {
    fn default() -> Self {
        Self {
            keep_running: false,
            open_at_login: false,
            login_item_available: true,
            platform: None,
        }
    }
}

/// Whether Locust stays reachable whenever this computer is on and awake (`RemoteMcpSettings.tsx::reachable`).
pub fn reachable(stay: &StayingReachable) -> bool {
    stay.keep_running && (stay.open_at_login || !stay.login_item_available)
}

/// Turns on only the Startup switches that are off and can be on, and turns nothing off (`RemoteMcpSettings.tsx::keepItRunning`).
pub fn keep_it_running(
    stay: &mut StayingReachable,
    on_keep_running: &mut impl FnMut(bool),
    on_login_item: &mut impl FnMut(bool),
) {
    if !stay.keep_running {
        stay.keep_running = true;
        on_keep_running(true);
    }
    if stay.login_item_available && !stay.open_at_login {
        stay.open_at_login = true;
        on_login_item(true);
    }
}

fn stay_words(platform: Option<&str>) -> (&'static str, &'static str) {
    match platform {
        Some("darwin") => ("menu bar", "log in to your Mac"),
        Some("linux") => ("system tray", "sign in"),
        _ => ("tray", "sign in to Windows"),
    }
}

fn press_keep_it_running(s: &mut State) {
    if let Some(stay) = s.remote_mcp_stay.as_mut() {
        keep_it_running(stay, &mut |_| {}, &mut |_| {});
    }
}

/// The tools an app is offered reach the door's engine only once the relay's real transport brings an app's call; until
/// then nothing can arrive, and a call that did would be refused.
struct NoToolsYet;
impl McpTools for NoToolsYet {
    fn call(&self, _name: &str, _arguments: &serde_json::Value) -> Result<ToolResult, String> {
        Err("Locust's tools are not reachable through the relay yet.".into())
    }
}

/// This computer's pairing, made once. Tests never reach a relay: without a stand-in of their own there is none.
fn service(s: &State) -> Option<Arc<RemoteMcp>> {
    let mut held = SERVICE.lock().unwrap_or_else(|e| e.into_inner());
    if cfg!(test) {
        return held.clone();
    }
    if held.is_none() {
        let store = s.data.as_ref()?;
        let mut options = Options::new(store.root().join(REMOTE_MCP_FILE), Arc::new(StubMcpRelay), Arc::new(NoToolsYet));
        options.refusal = Arc::new(|| locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "phone"));
        let service = Arc::new(RemoteMcp::new(options));
        service.load();
        *held = Some(service);
    }
    held.clone()
}

#[cfg(test)]
pub(super) fn use_service(service: Arc<RemoteMcp>) {
    *SERVICE.lock().unwrap_or_else(|e| e.into_inner()) = Some(service);
}

/// Locust is quitting: a card still waiting is answered No, so no app is left holding a request.
pub(super) fn dispose() {
    // Window tests share one process and quit their own windows; a stand-in one test installed is not another test's.
    if cfg!(test) {
        return;
    }
    let held = SERVICE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(service) = held {
        service.dispose();
    }
}

/// What the pairing shows, read on every turn of the window's loop: a card must appear on any screen.
pub(super) fn poll(s: &mut State) -> bool {
    let Some(service) = service(s) else { return false };
    service.tick();
    let now = service.settings();
    if s.remote_mcp.as_ref() == Some(&now) {
        return false;
    }
    s.remote_mcp = Some(now);
    true
}

fn toggle(s: &mut State) {
    let Some(service) = service(s) else { return };
    let on = s.remote_mcp.as_ref().is_some_and(|state| state.enabled);
    // The relay is asked, which may take a moment: never on the window's thread.
    std::thread::spawn(move || {
        service.set_enabled(&serde_json::json!(!on));
    });
}

fn remove(s: &mut State, id: &str) {
    let Some(service) = service(s) else { return };
    s.remote_mcp = Some(service.remove(id));
}

fn decide(s: &mut State, request: &str, allow: bool) {
    let Some(service) = service(s) else { return };
    s.remote_mcp = Some(service.answer_sign_in(request, allow));
}

fn when(iso: &str) -> String {
    DateTime::parse_from_rfc3339(iso).map(|at| at.with_timezone(&Local).format("%b %-d").to_string()).unwrap_or_default()
}

/// `host · connected Nov 18 · last used Nov 20` (Electron's appLine).
pub fn app_line(app: &RemoteMcpApp) -> String {
    let used = app.used.as_deref().map(|used| format!(" \u{b7} last used {}", when(used))).unwrap_or_default();
    format!("{} \u{b7} connected {}{used}", app.host, when(&app.added))
}

fn button(l: &crate::look::controls::Look, label: &str, key: String, boxed: bool, disabled: bool) -> El {
    use crate::look::controls::{self, Tone};
    let b = if boxed { controls::button(l, label, Tone::Neutral, disabled) } else { controls::ghost_button(l, label) };
    b.key(key)
}

/// The section, on the General page under Your phone; nothing before the pairing has been read.
pub(super) fn section(s: &State, p: &Palette) -> Option<El> {
    let state: &RemoteMcpState = s.remote_mcp.as_ref()?;
    let l = crate::look::controls::Look::new(p.dark);
    let on = state.enabled;
    let switch = crate::look::controls::switch(&l, on, SWITCH_LABEL, false).key("remote-mcp-switch").on_press(toggle);
    let mut out = col().w_full().gap(6.0).key("remote-mcp").child(
        row().w_full().gap(16.0).padding(10.0).center_y().child(
            col()
                .grow(1.0)
                .gap(4.0)
                .child(text(HEADING).font(l.ty.control).color(l.c("text-primary")))
                .child(text(if on { LEDE_ON } else { LEDE_OFF }).font(l.ty.body).color(l.c("text-secondary")).wrap().role("status")),
        ).child(switch),
    );
    if let (true, Some(address)) = (on, &state.address) {
        let copy = address.clone();
        let mut box_col = col()
            .w_full()
            .gap(6.0)
            .px(10.0)
            .key("remote-mcp-address-box")
            .child(text(ADDRESS_HELP).font(l.ty.body).color(l.c("text-secondary")).wrap())
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(text(address.clone()).key("remote-mcp-address").font(type_::LABEL.mono()).color(l.c("text-primary")).wrap_anywhere().grow(1.0))
                    .child(button(&l, "Copy", "remote-mcp-copy".into(), true, false).on_press_tx(move |_s: &mut State, tx| tx.copy(copy.clone()))),
            );
        if let Some(stay) = &s.remote_mcp_stay {
            let (tray, sign_in) = stay_words(stay.platform.as_deref());
            if reachable(stay) {
                let starts = if stay.open_at_login {
                    format!("it starts when you {sign_in}, and ")
                } else {
                    String::new()
                };
                let line = format!(
                    "Locust stays reachable while this computer is on and awake: {starts}it keeps running in the {tray} when you close its window. Quit from the {tray}, and no app can open it again."
                );
                box_col = box_col.child(text(line).key("remote-mcp-reachable").font(l.ty.body).color(l.c("text-secondary")).wrap());
            } else {
                let also_start = if stay.login_item_available {
                    format!(", and start it when you {sign_in}")
                } else {
                    String::new()
                };
                let line = format!(
                    "An app can reach Locust only while it is running here: once it quits, nothing on this computer can open it. Keep it running in the {tray} when you close its window{also_start}."
                );
                box_col = box_col.child(
                    col()
                        .w_full()
                        .gap(6.0)
                        .key("remote-mcp-stay")
                        .child(text(line).font(l.ty.body).color(l.c("text-secondary")).wrap())
                        .child(row().child(button(&l, "Keep it running", "remote-mcp-keep-running".into(), true, false).on_press(press_keep_it_running))),
                );
            }
        } else {
            box_col = box_col.child(text(OPEN_WHILE_USED).font(l.ty.body).color(l.c("text-secondary")).wrap());
        }
        out = out.child(box_col.child(text(RELAY_READS).font(l.ty.body).color(l.c("text-muted")).wrap()));
    }
    if on && !state.apps.is_empty() {
        let mut list = col().w_full().gap(6.0).px(10.0).key("remote-mcp-apps");
        for app in &state.apps {
            let id = app.id.clone();
            list = list.child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(
                        col()
                            .grow(1.0)
                            .child(text(app.name.clone()).font(l.ty.control).color(l.c("text-primary")))
                            .child(text(app_line(app)).font(l.ty.body).color(l.c("text-secondary")).wrap()),
                    )
                    .child(button(&l, "Remove", format!("remote-mcp-remove-{}", app.id), true, false).on_press(move |s: &mut State| remove(s, &id))),
            );
        }
        out = out.child(list);
    }
    if let Some(message) = &state.message {
        out = out.child(text(message.clone()).key("remote-mcp-message").font(l.ty.body).color(l.c("text-secondary")).wrap().role("status").px(10.0));
    }
    Some(out)
}

/// The card an app's sign-in becomes, over every screen: the only place it is answered. The first one waiting is shown.
pub(super) fn card(s: &State, p: &Palette) -> Option<El> {
    let one = s.remote_mcp.as_ref()?.asking.first()?.clone();
    let l = crate::look::controls::Look::new(p.dark);
    let digits = format!("{} {}", &one.match_digits[..3.min(one.match_digits.len())], one.match_digits.get(3..).unwrap_or(""));
    let said = format!("From anywhere, through Locust\u{2019}s relay. It sends you back to {}. Allow it only if you started this, and its sign-in page shows {digits}.", one.host);
    let (allow_request, deny_request) = (one.request.clone(), one.request.clone());
    Some(
        row()
            .key("remote-connect")
            .role("alertdialog")
            .w_full()
            .gap(12.0)
            .padding(12.0)
            .center_y()
            .fill(p.raised)
            .border(1.0, p.hairline)
            .child(
                col()
                    .grow(1.0)
                    .shrink()
                    .gap(4.0)
                    .child(text(format!("{} wants to use your Locust team", one.name)).key("remote-connect-title").font(l.ty.control).color(l.c("text-primary")).wrap())
                    .child(text(said).key("remote-connect-said").font(l.ty.body).color(l.c("text-secondary")).wrap())
                    .child(text(CARD_NOTE).font(l.ty.body).color(l.c("text-muted")).wrap()),
            )
            .child(button(&l, "Allow", "remote-connect-allow".into(), true, false).on_press(move |s: &mut State| decide(s, &allow_request, true)))
            .child(button(&l, "Deny", "remote-connect-deny".into(), false, false).on_press(move |s: &mut State| decide(s, &deny_request, false))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_core::remote_mcp::{McpRelay, RemoteMcpAsk, AskKind};
    use serde_json::json;

    /// The window's pairing is one per process: tests that stand one in take turns.
    static TURN: Mutex<()> = Mutex::new(());

    struct Relay(Mutex<Option<String>>);
    impl McpRelay for Relay {
        fn address(&self) -> Option<String> {
            self.0.lock().unwrap().clone()
        }
        fn open(&self, _on: bool) -> bool {
            true
        }
    }

    fn pairing(label: &str, address: Option<&str>) -> (Arc<RemoteMcp>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("locust-remote-mcp-ui-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let options = Options::new(dir.join(REMOTE_MCP_FILE), Arc::new(Relay(Mutex::new(address.map(str::to_owned)))), Arc::new(NoToolsYet));
        (Arc::new(RemoteMcp::new(options)), dir)
    }
    fn words(s: State) -> String {
        let mut rt = rune::Runtime::new(
            s,
            |s, _| col().w_full().children(section(s, &Palette::new(true))).children(card(s, &Palette::new(true))),
            (1200.0, 1200.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
    }
    fn keys(s: State) -> Vec<String> {
        let mut rt = rune::Runtime::new(
            s,
            |s, _| col().w_full().children(section(s, &Palette::new(true))).children(card(s, &Palette::new(true))),
            (1200.0, 1200.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.elements().into_iter().filter_map(|e| e.key).collect()
    }

    #[test]
    fn off_it_says_so_and_on_without_from_anywhere_it_says_what_to_turn_on_first() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let (remote, dir) = pairing("off", None);
        use_service(remote.clone());
        let mut s = State::default();
        assert!(poll(&mut s));
        let said = words(State { remote_mcp: s.remote_mcp.clone(), ..State::default() });
        assert!(said.contains(HEADING) && said.contains(LEDE_OFF), "{said}");
        assert!(!said.contains("Copy"));
        remote.set_enabled(&json!(true));
        assert!(poll(&mut s));
        let said = words(State { remote_mcp: s.remote_mcp.clone(), ..State::default() });
        assert!(said.contains("Turn on From anywhere under Your phone first"), "{said}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn on_it_shows_the_address_with_copy_and_each_app_with_its_line_and_remove() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let (remote, dir) = pairing("on", Some("https://app.locust.lol/v1/room/r/mcp"));
        use_service(remote.clone());
        remote.set_enabled(&json!(true));
        let sign_in = {
            let remote = remote.clone();
            std::thread::spawn(move || {
                remote.answer(&RemoteMcpAsk {
                    ask: "a".into(),
                    kind: AskKind::Connect,
                    body: json!({"client_id": "c1", "client": "Claude", "redirect": "https://claude.ai/cb", "challenge": "a".repeat(43), "match": "482915"}).to_string(),
                    auth: None,
                })
            })
        };
        let mut s = State::default();
        for _ in 0..400 {
            poll(&mut s);
            if s.remote_mcp.as_ref().is_some_and(|m| !m.asking.is_empty()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let said = words(State { remote_mcp: s.remote_mcp.clone(), ..State::default() });
        assert!(said.contains(LEDE_ON) && said.contains("https://app.locust.lol/v1/room/r/mcp") && said.contains(ADDRESS_HELP) && said.contains(RELAY_READS) && said.contains(OPEN_WHILE_USED), "{said}");
        assert!(said.contains("Claude wants to use your Locust team"));
        assert!(said.contains("It sends you back to claude.ai. Allow it only if you started this, and its sign-in page shows 482 915."), "{said}");
        assert!(said.contains(CARD_NOTE));
        let shown = keys(State { remote_mcp: s.remote_mcp.clone(), ..State::default() });
        for key in ["remote-mcp-switch", "remote-mcp-copy", "remote-connect-allow", "remote-connect-deny"] {
            assert!(shown.iter().any(|k| k == key), "{key} in {shown:?}");
        }
        // Allow is the person's, here: pressing the card's Allow button lets the sign-in go on and the card goes.
        let mut rt = rune::Runtime::new(
            s,
            |s, _| col().w_full().children(section(s, &Palette::new(true))).children(card(s, &Palette::new(true))),
            (1200.0, 1200.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let (x, y) = rt.find("remote-connect-allow").expect("Allow button on screen");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.05);
        assert_eq!(sign_in.join().unwrap().status, 200);
        assert!(rt.state.remote_mcp.as_ref().unwrap().asking.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_apps_line_names_where_it_came_from_and_when_with_last_use_only_once_there_is_one() {
        let app = |used: Option<&str>| RemoteMcpApp { id: "a".into(), name: "Claude".into(), host: "claude.ai".into(), added: "2026-11-18T12:00:00.000Z".into(), used: used.map(str::to_owned) };
        assert_eq!(app_line(&app(None)), "claude.ai \u{b7} connected Nov 18");
        assert_eq!(app_line(&app(Some("2026-11-20T12:00:00.000Z"))), "claude.ai \u{b7} connected Nov 18 \u{b7} last used Nov 20");
    }

    #[test]
    // case: asks on a card naming it, where it goes back to, and the digits its page shows
    fn asks_on_a_card_naming_it_where_it_goes_back_to_and_the_digits_its_page_shows() {
        use locust_core::remote_mcp::RemoteMcpAsking;

        let mut s = State::default();
        s.remote_mcp = Some(RemoteMcpState {
            enabled: true,
            address: Some("https://app.locust.lol/v1/room/abc/mcp".into()),
            apps: Vec::new(),
            asking: vec![RemoteMcpAsking {
                request: "rq".into(),
                name: "Claude".into(),
                host: "claude.ai".into(),
                match_digits: "482913".into(),
                until: 0,
            }],
            message: None,
        });
        let mut rt = rune::Runtime::new(
            s,
            |s, _| col().w_full().children(card(s, &Palette::new(true))),
            (1200.0, 400.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let els = rt.elements();
        let dialog = els.iter().find(|e| e.key.as_deref() == Some("remote-connect")).expect("remote-connect card");
        assert_eq!(dialog.role, "alertdialog");
        let text: String = els.iter().filter_map(|e| e.text.as_deref()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Claude wants to use your Locust team"), "{text}");
        assert!(text.contains("It sends you back to claude.ai."), "{text}");
        assert!(text.contains("482 913"), "{text}");
        let allow_pos = text.find("Allow").expect("Allow button");
        let deny_pos = text.find("Deny").expect("Deny button");
        assert!(allow_pos < deny_pos, "Allow should appear before Deny in {text}");

        // Empty asking list renders no card at all.
        let mut empty = State::default();
        empty.remote_mcp = Some(RemoteMcpState {
            enabled: true,
            address: None,
            apps: Vec::new(),
            asking: Vec::new(),
            message: None,
        });
        assert!(card(&empty, &Palette::new(true)).is_none());
    }

    #[test]
    // case: is answered on every screen, and only from the window
    fn is_answered_on_every_screen_and_only_from_the_window() {
        use locust_core::remote_mcp::RemoteMcpAsking;

        let asking = vec![RemoteMcpAsking {
            request: "rq-screen".into(),
            name: "Claude".into(),
            host: "claude.ai".into(),
            match_digits: "482913".into(),
            until: 0,
        }];
        // Rendered through the real top-level window view (main.rs::view) on both Home and Settings screens.
        for on_settings in [false, true] {
            let mut s = State::default();
            if on_settings {
                s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::App);
            }
            s.remote_mcp = Some(RemoteMcpState {
                enabled: true,
                address: Some("https://app.locust.lol/v1/room/abc/mcp".into()),
                apps: Vec::new(),
                asking: asking.clone(),
                message: None,
            });
            let mut rt = rune::Runtime::new(s, crate::view, (1280.0, 900.0), rune::Env::new(true, 1.0));
            rt.frame(0.0);
            let els = rt.elements();
            assert!(
                els.iter().any(|e| e.key.as_deref() == Some("remote-connect")),
                "remote-connect card must appear on screen (on_settings={on_settings})"
            );
            assert!(els.iter().any(|e| e.key.as_deref() == Some("remote-connect-allow")));
            assert!(els.iter().any(|e| e.key.as_deref() == Some("remote-connect-deny")));
        }
    }

    #[test]
    // case: gives Settings the address for Claude, and each app with Remove
    fn gives_settings_the_address_for_claude_and_each_app_with_remove() {
        let mut s = State::default();
        s.remote_mcp = Some(RemoteMcpState {
            enabled: true,
            address: Some("https://app.locust.lol/v1/room/abc/mcp".into()),
            asking: Vec::new(),
            apps: vec![RemoteMcpApp {
                id: "g1".into(),
                name: "Claude".into(),
                host: "claude.ai".into(),
                added: "2026-10-29T10:00:00.000Z".into(),
                used: None,
            }],
            message: None,
        });
        let said = words(s);
        assert!(said.contains("Use from anywhere"), "{said}");
        assert!(
            said.contains("In Claude, open Settings, then Connectors, choose Add custom connector, and paste this address:"),
            "{said}"
        );
        assert!(said.contains("https://app.locust.lol/v1/room/abc/mcp"), "{said}");
        assert!(said.contains("Claude"), "{said}");
        assert!(said.contains("claude.ai \u{b7} connected Oct 29"), "{said}");
        assert!(said.contains("Remove"), "{said}");

        let mut off_state = State::default();
        off_state.remote_mcp = Some(RemoteMcpState {
            enabled: false,
            address: None,
            apps: Vec::new(),
            asking: Vec::new(),
            message: None,
        });
        let mut rt = rune::Runtime::new(
            off_state,
            |s, _| col().w_full().children(section(s, &Palette::new(true))),
            (1200.0, 800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let els = rt.elements();
        let sw = els.iter().find(|e| e.key.as_deref() == Some("remote-mcp-switch")).expect("switch");
        assert_ne!(sw.fill, crate::look::controls::Look::new(true).c("switch-on"));
        let off_text: String = els.iter().filter_map(|e| e.text.as_deref()).collect::<Vec<_>>().join("\n");
        assert!(!off_text.contains("app.locust.lol"), "{off_text}");
    }

    fn on_state_with_stay(stay: StayingReachable) -> State {
        let mut s = State::default();
        s.remote_mcp = Some(RemoteMcpState {
            enabled: true,
            address: Some("https://app.locust.lol/v1/room/abc/mcp".into()),
            asking: Vec::new(),
            apps: Vec::new(),
            message: None,
        });
        s.remote_mcp_stay = Some(stay);
        s
    }

    #[test]
    // case: says no app can open a Locust that quit, and offers to keep it running
    fn says_no_app_can_open_a_locust_that_quit_and_offers_to_keep_it_running() {
        let said = words(on_state_with_stay(StayingReachable::default()));
        assert!(said.contains("once it quits, nothing on this computer can open it"), "{said}");
        assert!(said.contains("and start it when you sign in to Windows."), "{said}");
        assert!(said.contains("Keep it running"), "{said}");
    }

    #[test]
    // case: says it stays reachable once both are on, with no button
    fn says_it_stays_reachable_once_both_are_on_with_no_button() {
        let both_on = words(on_state_with_stay(StayingReachable {
            keep_running: true,
            open_at_login: true,
            login_item_available: true,
            platform: None,
        }));
        assert!(
            both_on.contains("Locust stays reachable while this computer is on and awake: it starts when you sign in to Windows, and it keeps running in the tray"),
            "{both_on}"
        );
        assert!(!both_on.contains("Keep it running"), "{both_on}");

        // Where Windows sign-in cannot be set, the tray alone is enough.
        let tray_only = words(on_state_with_stay(StayingReachable {
            keep_running: true,
            open_at_login: false,
            login_item_available: false,
            platform: None,
        }));
        assert!(
            tray_only.contains("Locust stays reachable while this computer is on and awake: it keeps running"),
            "{tray_only}"
        );
        assert!(!tray_only.contains("Keep it running"), "{tray_only}");

        let login_only = words(on_state_with_stay(StayingReachable {
            keep_running: false,
            open_at_login: true,
            login_item_available: true,
            platform: None,
        }));
        assert!(login_only.contains("Keep it running"), "{login_only}");

        let keep_only_when_login_available = words(on_state_with_stay(StayingReachable {
            keep_running: true,
            open_at_login: false,
            login_item_available: true,
            platform: None,
        }));
        assert!(keep_only_when_login_available.contains("Keep it running"), "{keep_only_when_login_available}");
    }

    #[test]
    // case: turns on only what is off and can be on, and turns nothing off
    fn turns_on_only_what_is_off_and_can_be_on_and_turns_nothing_off() {
        let mut stay = StayingReachable::default();
        let mut keep_calls = Vec::new();
        let mut login_calls = Vec::new();
        keep_it_running(&mut stay, &mut |v| keep_calls.push(v), &mut |v| login_calls.push(v));
        assert_eq!(keep_calls, vec![true]);
        assert_eq!(login_calls, vec![true]);
        assert!(stay.keep_running && stay.open_at_login);

        keep_calls.clear();
        login_calls.clear();
        let mut unavailable = StayingReachable {
            keep_running: true,
            open_at_login: false,
            login_item_available: false,
            platform: None,
        };
        keep_it_running(&mut unavailable, &mut |v| keep_calls.push(v), &mut |v| login_calls.push(v));
        assert!(keep_calls.is_empty());
        assert!(login_calls.is_empty());
        assert!(unavailable.keep_running && !unavailable.open_at_login);
    }

    #[test]
    // case: is given the Startup switches themselves, and the relay says so when Locust is not running
    fn is_given_the_startup_switches_themselves_and_the_relay_says_so_when_locust_is_not_running() {
        // Clicking "Keep it running" in section() flips the Startup switches in State and hides the button.
        let s = on_state_with_stay(StayingReachable::default());
        let mut rt = rune::Runtime::new(
            s,
            |s, _| col().w_full().children(section(s, &Palette::new(true))),
            (1200.0, 900.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let (x, y) = rt.find("remote-mcp-keep-running").expect("Keep it running button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.05);
        let stay = rt.state.remote_mcp_stay.as_ref().expect("stay state");
        assert!(reachable(stay));
        assert!(stay.keep_running && stay.open_at_login);
        assert!(rt.find("remote-mcp-keep-running").is_none());
    }
}
