//! every-settings-switch-says-what-it-is.test.ts: each Settings switch is named for what it controls (Automatic
//! replies, Urgent messages interrupt, Auto mode), never "Switch this on"; the off knob is the light lifted knob on a
//! track you can see; and the two between teammates say their name before their state. Also the Auto mode line itself
//! (Screens.tsx): its words follow the switch, a press flips it, and an organization that holds Auto off leaves it
//! drawn but deaf.
use super::*;
use crate::settings_pages::SettingsPageId;
use rune::{Env, Runtime};

struct PolicyGuard;
impl Drop for PolicyGuard {
    fn drop(&mut self) {
        locust_core::org_policy::use_policy_on_this_thread(None);
    }
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    super::view(s, &Palette::new(true))
}

fn page(id: SettingsPageId, auto: bool) -> Runtime<State> {
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(id);
    s.settings_panel.settings.auto_mode = auto;
    // Both between-teammates switches off, so each says its off sentence.
    s.settings_panel.settings.relay = false;
    s.settings_panel.settings.interrupt = false;
    let mut rt = Runtime::new(s, view, (1200.0, 4000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn switches(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| e.role == "switch" && !e.exiting).filter_map(|e| e.label).collect()
}

fn said(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    // Long enough apart that two presses are two presses, not a double-click.
    rt.frame(1.0);
}

#[test]
fn are_named_for_what_they_control() {
    let mut named = switches(&page(SettingsPageId::Relay, false));
    named.extend(switches(&page(SettingsPageId::Teammates, false)));
    for name in ["Automatic replies", "Urgent messages interrupt", "Auto mode"] {
        assert!(named.iter().any(|n| n == name), "no switch named {name} in {named:?}");
    }
    let mut every = Vec::new();
    for p in crate::settings_pages::SETTINGS_PAGES {
        every.extend(switches(&page(p.id, false)));
    }
    assert!(every.len() >= 3);
    assert!(every.iter().all(|n| !n.starts_with("Switch this")), "{every:?}");
}

#[test]
fn show_an_off_knob_you_can_see_the_lifted_knob_on_the_track_not_a_raised_surface_0_419() {
    let look = crate::look::controls::Look::new(true);
    let rt = page(SettingsPageId::Teammates, false);
    let fills: Vec<_> = rt.elements().into_iter().map(|e| e.fill).collect();
    assert!(fills.contains(&look.c("switch-off")), "the off track");
    assert!(fills.contains(&look.c("switch-knob")), "the lifted knob");
    assert_ne!(look.c("switch-knob"), look.c("switch-off"), "a knob you can see against its track");
    assert_ne!(look.c("switch-knob"), look.c("bg-raised"), "the knob, not a raised surface");
}

#[test]
fn and_the_two_between_teammates_say_their_name_before_their_state() {
    let words = said(&page(SettingsPageId::Relay, false));
    assert!(words.contains("Automatic replies: off. Messages wait for the recipient's next run."), "{words}");
    assert!(words.contains("Urgent messages interrupt: off. An urgent message still waits for the recipient to finish."), "{words}");
}

#[test]
fn auto_mode_says_what_on_and_off_mean_and_a_press_flips_it() {
    let mut rt = page(SettingsPageId::Teammates, false);
    assert!(said(&rt).contains(AUTO_OFF));
    assert!(!said(&rt).contains(AUTO_ON));
    press(&mut rt, "settings-auto-mode");
    assert!(rt.state.settings_panel.settings.auto_mode);
    assert!(said(&rt).contains(AUTO_ON));
    press(&mut rt, "settings-auto-mode");
    assert!(!rt.state.settings_panel.settings.auto_mode);
}

#[test]
fn auto_mode_held_off_by_the_organization_is_drawn_but_does_nothing() {
    let _guard = PolicyGuard;
    let mut policy = crate::managed_controls_tests::sample_policy();
    policy.policy.insert("highestMode".into(), serde_json::json!("plan"));
    locust_core::org_policy::use_policy_on_this_thread(Some(policy));
    let mut rt = page(SettingsPageId::Teammates, false);
    assert!(said(&rt).contains("Set by your organization."));
    assert!(switches(&rt).iter().any(|n| n == "Auto mode"));
    press(&mut rt, "settings-auto-mode");
    assert!(!rt.state.settings_panel.settings.auto_mode, "a held-off switch must not turn on");
}
