//! Settings › General › "Your phone" (Electron `components/PhoneSettings.tsx`, 0.775), the Wi-Fi half: one switch, the
//! link a phone on this Wi-Fi opens, and New code, which unpairs every phone. The rules and the listener are
//! `locust_core::phone_host` and `phone_server`; this file opens the host once and draws its state.
//!
//! The code is `locust_core::qr`, square for square what Electron's qrcode-generator draws, dark on white whatever the
//! theme. Not here yet: From anywhere (Locust's relay) and Electron's paired-device list, which belongs to the relay.
//! See notes/PHONE-NOTES.md.

use super::*;
use locust_core::phone_host::{PhoneHost, PhoneSettings};
use std::sync::OnceLock;

pub(crate) const HEADING: &str = "Your phone";
const SWITCH_LABEL: &str = "Use Locust from your phone";

pub(crate) const OFF: &str =
    "Off. Turn it on to check on your teammates from your phone, answer approval cards and send the next message.";
pub(crate) const NO_NETWORK: &str = "On, but this computer is not on a network Locust can find. Connect it to your Wi-Fi, then turn this off and on again.";
pub(crate) const ON: &str = "On. Scan the code with your phone’s camera to see your conversations, answer approval cards and send the next message. Your phone must be on the same Wi-Fi as this computer.";
pub(crate) const ANYONE: &str = "Anyone who scans this code can do what you can from your phone. Lost a phone, or shared the code? Make a new one: every phone has to scan again.";
const QR_LABEL: &str = "A code for your phone’s camera. It opens Locust on your phone.";
/// One square of the code, in points.
const SQUARE: f32 = 4.0;
pub(crate) const WINDOWS_ASKS: &str =
    "The first time, Windows may ask whether Locust can use your network. Choose Allow, on private networks only.";

static HOST: OnceLock<PhoneHost> = OnceLock::new();

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

/// The code as rows of dark runs on white, with PhoneSettings.tsx's four-square margin.
pub(crate) fn qr(url: &str) -> Option<El> {
    let code = locust_core::qr::qr_code(url)?;
    let mut rows = col().fill(Color::WHITE).padding(4.0 * SQUARE).role("img").label(QR_LABEL).key("phone-code");
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
    let Some(phone) = &s.settings_panel.phone else {
        return col()
            .w_full()
            .gap(4.0)
            .padding(10.0)
            .child(text(HEADING).role("heading").level(2).font(type_::TITLE).color(p.text))
            .child(text("Your phone opens when Locust runs on its own engine.").font(type_::LABEL).color(p.muted).wrap());
    };
    let managed = locust_core::org_policy::feature_refusal(Some(&locust_core::org_policy::current()), "phone").is_some();
    let on = !managed && phone.enabled;
    let mut words = col()
        .grow(1.0)
        .gap(4.0)
        .child(text(HEADING).role("heading").level(2).font(l.ty.control).color(l.c("text-primary")))
        .child(text(lede(phone, managed)).font(l.ty.body).color(l.c("text-secondary")).wrap());
    if let Some(message) = &phone.message {
        words = words.child(text(message.clone()).key("phone-message").font(l.ty.body).color(l.c("text-secondary")).wrap());
    }
    if managed {
        words = words.child(text("Set by your organization.").font(l.ty.body).color(l.c("text-muted")));
    }
    let mut switch = crate::look::controls::switch(&l, on, SWITCH_LABEL, managed).key("phone-switch");
    if !managed {
        switch = switch.on_press(toggle);
    }
    let mut section = col()
        .key("phone-section")
        .w_full()
        .gap(8.0)
        .child(row().w_full().gap(16.0).padding(10.0).center_y().child(words).child(switch));
    if on && let Some(url) = &phone.url {
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
        if let Some(code) = qr(url) {
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

    fn settings(enabled: bool, url: Option<&str>) -> PhoneSettings {
        PhoneSettings { enabled, url: url.map(str::to_owned), addresses: Vec::new(), message: None }
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
}
