//! Window tests for `managed-controls-say-who-set-them.test.tsx`.
use super::*;
use rune::{Env, Runtime};
use serde_json::json;
use std::sync::Mutex;

static POLICY_LOCK: Mutex<()> = Mutex::new(());

struct PolicyGuard;
impl Drop for PolicyGuard {
    fn drop(&mut self) {
        locust_core::org_policy::use_policy(None);
    }
}

pub(crate) fn sample_policy() -> locust_core::org_policy::PolicyState {
    let mut map = serde_json::Map::new();
    map.insert("allowedAgents".into(), json!(["codex"]));
    map.insert("allowedModels".into(), json!({"codex": ["safe"]}));
    map.insert("highestMode".into(), json!("plan"));
    map.insert("monthlyLimitUsd".into(), json!(10));
    map.insert("otherApps".into(), json!(false));
    map.insert("phone".into(), json!(false));
    map.insert("cloud".into(), json!(false));
    locust_core::org_policy::PolicyState {
        path: "/admin/Locust/policy.json".into(),
        present: true,
        policy: map,
        error: None,
    }
}

fn texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

// managed-controls-say-who-set-them.test.tsx
#[test]
fn finds_organization_policy_in_the_general_settings_search() {
    let general = crate::settings_pages::SETTINGS_PAGES
        .iter()
        .find(|p| p.id == crate::settings_pages::SettingsPageId::App)
        .unwrap();
    assert!(crate::settings_pages::page_matches(general, "policy"));
    assert!(crate::settings_pages::matched_headings(general, "policy").contains(&"Managed by your organization"));
}

#[test]
fn names_the_file_and_every_setting_in_settings_and_shows_broken_policy_alert() {
    let _lock = POLICY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = PolicyGuard;
    locust_core::org_policy::use_policy(Some(sample_policy()));
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::App);
    s.settings_panel.phone = Some(locust_core::phone_host::PhoneSettings::default());
    fn view(s: &State, _cx: &mut Cx<State>) -> El {
        crate::settings_panel::view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view, (900.0, 1200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = texts(&rt).join(" | ");
    assert!(all.contains("Managed by your organization"));
    assert!(all.contains("/admin/Locust/policy.json"));
    for phrase in [
        "Allowed AI agents: Codex CLI",
        "Allowed models: Codex CLI: safe",
        "Highest permission mode: Plan",
        "Maximum monthly limit: $10 a month",
        "Phone pairing: Off",
        "Cloud computers and tasks: Off",
        "Access from other AI apps: Off",
    ] {
        assert!(all.contains(phrase), "missing {phrase} in {all}");
    }
    assert!(all.matches("Set by your organization").count() >= 2, "phone and other-app switches attributed");

    // Broken policy as an alert with its path
    let mut broken = sample_policy();
    broken.error = Some("Fix /admin/Locust/policy.json. Nothing starts.".into());
    rt.update(move |_, _| {
        locust_core::org_policy::use_policy(Some(broken.clone()));
    });
    rt.frame(0.016);
    let alert = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("org-policy-alert"));
    assert!(alert.is_some());
    assert_eq!(alert.unwrap().role, "alert");
    assert!(texts(&rt).join(" ").contains("/admin/Locust/policy.json"));
}

#[test]
fn filters_the_picker_and_mode_menu_and_shows_monthly_limit_under_policy() {
    let _lock = POLICY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = PolicyGuard;
    locust_core::org_policy::use_policy(Some(sample_policy()));

    // Picker filters models and account-default fallbacks
    let mut s = State::default();
    let mate = Teammate {
        id: "tm_1".into(),
        name: "Wren".into(),
        role: "Code".into(),
        runtime: Some("codex".into()),
        model: Some("safe".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    };
    s.teammates.push(mate.clone());
    s.selected = 0;
    s.conversations.insert("tm_1".into(), Conversation::default());
    s.models.insert(
        "tm_1".into(),
        vec![
            engine::Models {
                runtime: "codex".into(),
                model: None,
                effort: None,
                teammate: None,
                models: vec![
                    engine::Model { id: "safe".into(), name: "Allowed model".into(), efforts: vec![] },
                    engine::Model { id: "blocked".into(), name: "Blocked model".into(), efforts: vec![] },
                ],
            },
            engine::Models {
                runtime: "claude".into(),
                model: None,
                effort: None,
                teammate: None,
                models: vec![engine::Model { id: "account-default".into(), name: "Account default".into(), efforts: vec![] }],
            },
        ],
    );
    s.model_menu = Some("tm_1".into());
    s.mode_menu = true;
    fn view_thread(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view_thread, (1000.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let words = texts(&rt).join(" | ");
    assert!(words.contains("Allowed model"), "got: {words}");
    assert!(!words.contains("Blocked model"));
    assert!(!words.contains("account-default"));
    // Mode menu ceiling: highestMode = plan -> auto and accept-edits are not offered
    assert!(rt.find("mode-plan").is_some());
    assert!(rt.find("mode-auto").is_none());

    // People panel new teammate dialog shows maximum monthly limit
    let mut sp = State::default();
    sp.people_panel.open = true;
    sp.people_panel.form_open = true;
    fn view_people(s: &State, _cx: &mut Cx<State>) -> El {
        col().w_full().child(crate::people_panel::edit_form(s, &Palette::new(true)))
    }
    let mut rtp = Runtime::new(sp, view_people, (900.0, 1400.0), Env::new(true, 1.0));
    rtp.frame(0.0);
    let pwords = texts(&rtp).join(" | ");
    assert!(pwords.contains("Set by your organization"), "got: {pwords}");
    assert!(pwords.contains("Maximum 10.00 USD. You can set a lower amount."), "got: {pwords}");
}

// The mode menu greys out a mode the agent cannot run, says why, and does not take it (Electron's ModeMenu,
// modeUnavailableReason). Antigravity cannot stop and ask, so Approve each is greyed with that sentence.
#[test]
fn the_mode_menu_greys_a_mode_the_agent_cannot_run_and_says_why() {
    let _lock = POLICY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    locust_core::org_policy::use_policy(None);
    let _guard = PolicyGuard;
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm_1".into(),
        name: "Wren".into(),
        role: "Code".into(),
        runtime: Some("antigravity".into()),
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    });
    s.selected = 0;
    s.conversations.insert("tm_1".into(), Conversation::default());
    s.mode_menu = true;
    fn view_thread(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view_thread, (1000.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let words = texts(&rt).join(" | ");
    assert!(words.contains("Antigravity cannot stop and ask yet."), "got: {words}");
    let (x, y) = rt.find("mode-approve-each").expect("approve-each is listed");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert_ne!(rt.state.composer_mode.as_deref(), Some("approve-each"), "a greyed mode is not taken");
}
