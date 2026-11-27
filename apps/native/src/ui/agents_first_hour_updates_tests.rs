use super::*;
use rune::{Env, Runtime};

// case: a copy says it leaves the machine's agents alone, instead of a switch that would not
#[test]
fn agents_a_copy_draws_the_no_updates_refusal_when_the_check_decides() {
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = crate::settings_pages::SettingsPageId::from_str("runtimes");
    s.settings_panel.agents = Some(Vec::new());
    let mut rt = Runtime::new(s, crate::view, (1420., 1600.), Env::new(true, 1.));
    rt.frame(0.05);
    assert!(rt.find("agents-held-here").is_none(), "the first check has not decided");
    rt.update(|s, _| {
        assert!(poll(s), "the actual agents poll must report the change so the window redraws");
    });
    for _ in 0..12 {
        rt.frame(0.05);
    }
    let refusal = rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some("agents-held-here")).unwrap();
    assert_eq!(refusal.text.as_deref(), Some("This copy is not the installed Locust, so it never updates the agents on this machine on its own. The installed Locust keeps them current."));
    assert!(rt.find("agents-keep-current").is_none(), "no unusable automatic-update switch");
    rt.update(|s, _| assert!(!poll_updates(s), "the held note is only a change once"));
}
