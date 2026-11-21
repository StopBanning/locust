//! The Install a NOT INSTALLED row offers while npm cannot be run (Electron Screens.tsx `npmMissing`, index.ts
//! `npmSeen`): pressed, it starts nothing, and the row says why in Electron's words, its line still there to copy. With
//! npm there, or never asked, it installs as before; and an install that finishes drops the remembered answer.
use super::*;
use locust_core::agents::{Auth, Probe};
use locust_core::npm_check::NPM_MISSING;

fn window(npm: Option<bool>) -> rune::Runtime<State> {
    NPM_STAND_IN.with(|c| c.set(npm));
    let mut s = State::default();
    s.settings_panel.agents = Some(vec![Found {
        id: "opencode".into(),
        display_name: "OpenCode".into(),
        installed: false,
        version: None,
        auth: Auth::NotApplicable,
        ready: false,
        status: Probe::Missing,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }]);
    let mut rt = rune::Runtime::new(s, |s, _| section(s, &Palette::new(true)), (1100.0, 1400.0), rune::Env::new(true, 1.0));
    settle(&mut rt);
    rt
}
fn settle(rt: &mut rune::Runtime<State>) {
    for _ in 0..40 {
        rt.frame(0.016);
    }
}
fn press(rt: &mut rune::Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}
/// Whether the row's Install can be reached (a greyed control cannot be focused).
fn install_offered(rt: &rune::Runtime<State>) -> bool {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some("agents-install-opencode")).expect("an Install on the row").focusable
}
fn words(rt: &rune::Runtime<State>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join(" | ")
}

#[test]
fn an_install_npm_cannot_run_starts_nothing_and_says_why() {
    let mut rt = window(Some(false));
    assert!(words(&rt).contains(NPM_MISSING), "{}", words(&rt));
    assert!(!install_offered(&rt), "Install is greyed");
    press(&mut rt, "agents-install-opencode");
    assert!(rt.state.agents_install.id.is_none() && !rt.state.agents_install_job.is_pending(), "nothing was started");
    assert!(words(&rt).contains("npm install -g opencode-ai"), "the line is still there to run by hand: {}", words(&rt));
}

#[test]
fn with_npm_there_or_never_asked_install_runs_and_its_end_asks_npm_again() {
    for npm in [Some(true), None] {
        let mut rt = window(npm);
        assert!(!words(&rt).contains(NPM_MISSING));
        assert!(install_offered(&rt));
        press(&mut rt, "agents-install-opencode");
        assert_eq!(rt.state.agents_install.id.as_deref(), Some("opencode"));
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while rt.state.agents_install.outcome.is_none() && std::time::Instant::now() < until {
            rt.update(|s, _| {
                poll(s);
            });
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(rt.state.agents_install.outcome.is_some(), "the install ended");
        assert_eq!(NPM_STAND_IN.with(|c| c.get()), None, "an install that finished drops what npm said");
    }
}
