//! Settings › General › "Your phone" (Electron `components/PhoneSettings.tsx`, 0.775, 0.826, 0.838):
//! one switch, and how the phone reaches Locust — from anywhere through Locust's relay, or only on this Wi-Fi.
//! The rules and the listener are `locust_core::phone_host`, `phone_server`, and `phone_anywhere`; this file
//! opens the host once and draws its state.
//!
//! The code is `locust_core::qr`, square for square what Electron's qrcode-generator draws, dark on white whatever the
//! theme.

use super::*;
use locust_core::phone_anywhere::{PairedDeviceView, PhoneAnywhereSettings};
use locust_core::phone_host::{PhoneHost, PhoneSettings};
use std::sync::OnceLock;

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

#[cfg(not(test))]
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
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
}

fn toggle(s: &mut State) {
    let on = s.settings_panel.phone.as_ref().is_some_and(|phone| phone.enabled);
    if let Some(host) = HOST.get() {
        s.settings_panel.phone = Some(host.set_enabled(!on));
    }
}

fn new_code(s: &mut State) {
    if let Some(host) = HOST.get() {
        s.settings_panel.phone = Some(host.forget());
    }
}

fn pick_way(s: &mut State, wifi: bool) {
    s.phone_picked_wifi = wifi;
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

    if on && mode == PhoneMode::Anywhere && let Some(anywhere) = anywhere {
        let status_line = anywhere
            .message
            .as_deref()
            .unwrap_or(if anywhere.connected { CONNECTED_RELAY } else { CONNECTING_RELAY });
        let mut side = col()
            .grow(1.0)
            .gap(8.0)
            .child(text(status_line.to_owned()).key("phone-anywhere-status").role("status").font(l.ty.body).color(l.c("text-secondary")).wrap());
        if let Some(code) = &anywhere.code {
            let link = code.url.clone();
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
}
