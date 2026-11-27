//! Review change 6 (Words 1): Settings says each switch in one line, and the rules and exceptions its paragraph gave go
//! behind "More", shut until pressed. Drawn through the whole Settings window, as a person opens it: every switch that
//! used to open its words with "On." or "Off." (which the switch already shows) says what it does instead, its More
//! holds every fact the old paragraph had, and a press opens that one fold alone.
use super::*;
use crate::settings_pages::SettingsPageId;
use locust_core::scout::types::{ScoutSettings, ScoutState};
use rune::{Env, Runtime};

fn view(s: &State, _: &mut Cx<State>) -> El {
    super::view(s, &Palette::new(true))
}

fn window(mut s: State, page: SettingsPageId) -> Runtime<State> {
    s.settings_panel.open = true;
    s.settings_panel.page = Some(page);
    let mut rt = Runtime::new(s, view, (1200.0, 9000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn said(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

fn says(rt: &Runtime<State>, words: &str) -> bool {
    said(rt).iter().any(|t| t.contains(words))
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    // Long enough apart that two presses are two presses, not a double-click.
    rt.frame(1.0);
}

/// The texts drawn inside the element with this key, as a person reads it.
fn text_in(rt: &Runtime<State>, key: &str) -> String {
    let ([x, y, w, h], _) = rt.presented(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn has_key(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().into_iter().any(|e| !e.exiting && e.key.as_deref() == Some(key))
}

/// No switch's words still open with the state the switch already shows. (Boot screen is three choices, not a switch:
/// its "Off." names the choice, so it is left as it is.)
fn no_line_opens_with_its_state(rt: &Runtime<State>) {
    for t in said(rt) {
        if t.starts_with("Off. Locust goes straight to the workspace") {
            continue;
        }
        assert!(!t.starts_with("On. ") && !t.starts_with("Off. "), "a switch still opens with its state: {t}");
    }
}

fn door(enabled: bool) -> locust_core::door::State {
    locust_core::door::State {
        settings: locust_core::door::Settings {
            enabled,
            own_mode: true,
            editor_seen: true,
            editor_answers: true,
            conversations: true,
            ..Default::default()
        },
        claude_command: Some("claude mcp add example".into()),
        codex_config: None,
        openclaw_command: None,
        message: None,
    }
}

fn general_with_door(enabled: bool) -> Runtime<State> {
    let mut s = State::default();
    s.settings_panel.door = Some(door(enabled));
    window(s, SettingsPageId::App)
}

#[test]
fn the_doors_four_switches_each_say_what_they_do_in_one_line_with_the_rest_shut_under_more() {
    let rt = general_with_door(true);
    no_line_opens_with_its_state(&rt);
    for line in [DOOR_LINE, OWN_MODE_LINE, EDITOR_ANSWERS_LINE, CONVERSATIONS_LINE] {
        assert!(said(&rt).iter().any(|t| t == line), "no line {line} in {:?}", said(&rt));
    }
    for key in ["more-door", "more-door-own-mode", "more-door-editor-answers", "more-door-conversations"] {
        assert!(has_key(&rt, key), "no More for {key}");
        assert!(!has_key(&rt, &format!("{key}-text")), "{key} open before it was pressed");
    }
    // What the paragraphs said is not on the page until asked for.
    for hidden in ["Locust must be running", "Auto runs only if Auto is on in Settings", "Questions are still answered here", "cards answered only here"] {
        assert!(!says(&rt, hidden), "{hidden} shown with every More shut");
    }
}

#[test]
fn a_press_on_more_opens_that_switchs_fold_alone_with_every_fact_its_paragraph_had_and_a_second_press_shuts_it() {
    let mut rt = general_with_door(true);
    press(&mut rt, "more-door");
    let door = text_in(&rt, "more-door-text");
    for fact in [
        "Other apps on this computer can ask teammates, run your routines and read replies.",
        "New turns use each teammate's own mode, with no automatic teammate handoffs.",
        "Locust must be running; your AI account usage and monthly limits still apply.",
    ] {
        assert!(door.contains(fact), "{fact} not under the door's More: {door}");
    }
    assert!(!has_key(&rt, "more-door-own-mode-text"), "pressing one More opened another");
    press(&mut rt, "more-door-own-mode");
    let own = text_in(&rt, "more-door-own-mode-text");
    for fact in [
        "the mode and folder the app asks for, so it can edit files",
        "A folder must be one Locust knows or a worktree of one.",
        "Approval cards appear in this window, and an editor can answer the ones for turns it started.",
        "Auto runs only if Auto is on in Settings.",
    ] {
        assert!(own.contains(fact), "{fact} not under own mode's More: {own}");
    }
    press(&mut rt, "more-door-editor-answers");
    assert!(text_in(&rt, "more-door-editor-answers-text").contains(EDITOR_ANSWERS_ON));
    press(&mut rt, "more-door-conversations");
    assert!(text_in(&rt, "more-door-conversations-text").contains(CONVERSATIONS_ON));
    press(&mut rt, "more-door");
    assert!(!has_key(&rt, "more-door-text"), "a second press left it open");
    assert!(has_key(&rt, "more-door-own-mode-text"), "shutting one shut another");
}

#[test]
fn the_door_off_says_what_it_does_and_its_more_says_no_server_is_listening() {
    let mut rt = general_with_door(false);
    no_line_opens_with_its_state(&rt);
    assert!(says(&rt, DOOR_LINE));
    assert!(!says(&rt, "No local server is listening."));
    press(&mut rt, "more-door");
    assert_eq!(text_in(&rt, "more-door-text"), "No local server is listening.");
}

#[test]
fn auto_mode_says_what_it_does_and_its_more_follows_the_switch() {
    let mut rt = window(State::default(), SettingsPageId::Teammates);
    no_line_opens_with_its_state(&rt);
    assert!(says(&rt, AUTO_LINE));
    assert!(!says(&rt, AUTO_OFF), "Auto's off words shown with More shut");
    press(&mut rt, "more-auto-mode");
    assert!(text_in(&rt, "more-auto-mode-text").contains(AUTO_OFF));
    press(&mut rt, "settings-auto-mode");
    assert!(rt.state.settings_panel.settings.auto_mode);
    assert!(text_in(&rt, "more-auto-mode-text").contains(AUTO_ON), "More still says off after the switch went on");
}

#[test]
fn without_asking_says_what_it_does_its_more_says_when_and_with_no_agent_picked_it_says_why_it_cannot_be_pressed() {
    let mut s = State::default();
    s.settings_panel.plan_runs_out = Some(locust_core::carry_on::PlanRunsOut { runtime: "codex".into(), model: None, automatic: true });
    let mut rt = window(s, SettingsPageId::Runtimes);
    no_line_opens_with_its_state(&rt);
    assert!(says(&rt, WITHOUT_ASKING_LINE));
    assert!(!says(&rt, WITHOUT_ASKING_ON));
    press(&mut rt, "more-without-asking");
    assert!(text_in(&rt, "more-without-asking-text").contains(WITHOUT_ASKING_ON));

    let rt = window(State::default(), SettingsPageId::Runtimes);
    assert!(says(&rt, WITHOUT_ASKING_PICK_FIRST), "with nothing picked, why the switch is held is said");
    assert!(!has_key(&rt, "more-without-asking"));
}

fn general_with_phone_and_remote() -> Runtime<State> {
    let mut s = State::default();
    s.settings_panel.phone = Some(locust_core::phone_host::PhoneSettings::default());
    s.remote_mcp = Some(locust_core::remote_mcp::RemoteMcpState { enabled: false, address: None, apps: vec![], asking: vec![], message: None });
    window(s, SettingsPageId::App)
}

#[test]
fn your_phone_and_use_from_anywhere_each_say_what_they_do_with_their_off_words_under_more() {
    let mut rt = general_with_phone_and_remote();
    no_line_opens_with_its_state(&rt);
    assert!(says(&rt, crate::phone_section::LINE));
    assert!(says(&rt, crate::remote_mcp_card::LINE));
    assert!(!says(&rt, crate::phone_section::OFF));
    assert!(!says(&rt, crate::remote_mcp_card::LEDE_OFF));
    press(&mut rt, "more-phone");
    assert!(text_in(&rt, "more-phone-text").contains(crate::phone_section::OFF));
    assert!(!has_key(&rt, "more-remote-mcp-text"));
    press(&mut rt, "more-remote-mcp");
    assert!(text_in(&rt, "more-remote-mcp-text").contains(crate::remote_mcp_card::LEDE_OFF));
}

#[test]
fn your_phone_on_with_no_network_still_says_so_out_loud() {
    let mut s = State::default();
    s.settings_panel.phone = Some(locust_core::phone_host::PhoneSettings { enabled: true, url: None, addresses: vec![], message: None });
    let rt = window(s, SettingsPageId::App);
    assert!(says(&rt, crate::phone_section::NO_NETWORK), "a problem to fix is not folded away");
    assert!(!has_key(&rt, "more-phone"));
}

#[test]
fn scout_says_what_it_does_and_its_more_holds_what_off_used_to_say() {
    let mut s = State::default();
    s.scout.state = Some(ScoutState {
        settings: ScoutSettings::default(),
        cards: vec![],
        look_ids: vec![],
        last_look_at: None,
        model_said: None,
        today: None,
        notes: vec![],
    });
    let mut rt = window(s, SettingsPageId::App);
    no_line_opens_with_its_state(&rt);
    assert!(says(&rt, crate::scout_setting::SCOUT_LINE));
    assert!(!says(&rt, "Turn it on and Scout joins your team"));
    press(&mut rt, "more-scout");
    assert_eq!(text_in(&rt, "more-scout-text"), without_state(locust_core::scout::view::SCOUT_OFF_LEDE));
    assert!(text_in(&rt, "more-scout-text").starts_with("Turn it on and Scout joins your team"));
}

#[test]
fn your_account_off_says_what_the_switch_does_and_folds_what_off_went_on_to_say() {
    use locust_core::your_account::{AccountSyncState, SignedInAccount};
    let mut s = State::default();
    s.settings_panel.account.status.signed_in = Some(SignedInAccount { login: "colin".into() });
    s.settings_panel.account.status.sync = AccountSyncState::Off;
    s.settings_panel.account.status.has_key = true;
    let mut rt = window(s, SettingsPageId::App);
    no_line_opens_with_its_state(&rt);
    assert!(says(&rt, "Keep a copy of this Locust on Locust\u{2019}s relay."));
    assert!(!says(&rt, crate::your_account_section::SYNC_OFF));
    press(&mut rt, "more-account-sync");
    assert_eq!(text_in(&rt, "more-account-sync-text"), crate::your_account_section::SYNC_OFF);
}

#[test]
fn remote_control_on_or_off_says_what_it_does_and_folds_claude_codes_words_while_starting_says_so() {
    use locust_core::remote_control::{NEEDS_SIGN_IN, Phase, RemoteControlState};
    let mut s = State::default();
    s.remote_control = Some(RemoteControlState::default());
    let mut rt = window(s, SettingsPageId::Runtimes);
    no_line_opens_with_its_state(&rt);
    assert!(says(&rt, crate::remote_control_card::LINE));
    assert!(!says(&rt, "When on, you can start Claude Code sessions"));
    press(&mut rt, "more-remote-control");
    let more = text_in(&rt, "more-remote-control-text");
    assert!(more.contains("When on, you can start Claude Code sessions in this folder from claude.ai while Locust is open"), "{more}");
    assert!(more.contains(NEEDS_SIGN_IN), "{more}");

    let mut s = State::default();
    s.remote_control = Some(RemoteControlState { enabled: true, phase: Phase::Starting, ..RemoteControlState::default() });
    let rt = window(s, SettingsPageId::Runtimes);
    assert!(says(&rt, "Starting Claude Code\u{2026}"), "news about the switch is said, not folded");
    assert!(!has_key(&rt, "more-remote-control"));
}

#[test]
fn the_more_fold_is_a_button_that_says_more_and_whether_it_is_open() {
    let mut rt = general_with_door(true);
    let summary = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("more-door")).expect("the door's More");
    assert_eq!(summary.role, "button");
    assert_eq!(summary.label.as_deref(), Some(MORE));
    assert_eq!(summary.expanded, Some(false));
    press(&mut rt, "more-door");
    let summary = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("more-door")).expect("the door's More");
    assert_eq!(summary.expanded, Some(true));
}
