//! Electron `renderer/src/settings-in-a-browser-say-what-stays.test.tsx`:
//! Settings drawn in a browser say what stays on the computer, and which settings a browser cannot change,
//! while Settings in the window on the computer hold nothing back and say nothing about a browser.

use crate::desk::{BROWSER_SETTINGS_PAGES, WINDOW_ONLY_SAYS};
use crate::desk_host::{
    DeskAsk, DeskHandler, DeskHelloInfo, DeskHost, DeskHostDeps, DeskSender, DeskTell, DeskValue,
    DeskWindow, desk_encode_args,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn test_host() -> (DeskHost, Arc<Mutex<Vec<(String, Vec<DeskTell>)>>>) {
    let told: Arc<Mutex<Vec<(String, Vec<DeskTell>)>>> = Arc::new(Mutex::new(Vec::new()));
    let told_c = told.clone();
    let mut handlers: HashMap<String, DeskHandler> = HashMap::new();
    for ch in [
        "workspace-settings:change",
        "workspace-settings:write",
        "appearance:set",
        "phone:settings",
        "account:status",
        "discord:settings",
        "locust-mcp:set",
    ] {
        handlers.insert(ch.into(), Arc::new(|_, _| Ok(DeskValue::Json(json!({ "ok": true })))));
    }
    let host = DeskHost::new(DeskHostDeps {
        handlers: Arc::new(Mutex::new(handlers)),
        window: Arc::new(|| {
            Some(DeskWindow {
                destroyed: false,
                web_contents: DeskSender {
                    id: 1,
                    main_frame_id: 2,
                    process_id: 3,
                    destroyed: false,
                },
            })
        }),
        tell: Arc::new(move |device, items| {
            told_c
                .lock()
                .unwrap()
                .push((device.to_owned(), items.to_vec()));
            true
        }),
        cover: Arc::new(|_| {}),
        hello: Arc::new(|| {
            Ok(DeskHelloInfo {
                version: "0.849.0".into(),
                computer: "your computer".into(),
                theme: "dark".into(),
            })
        }),
        waiting: Arc::new(Vec::new),
        changed: None,
    });
    (host, told)
}

fn call_result(host: &DeskHost, told: &Arc<Mutex<Vec<(String, Vec<DeskTell>)>>>, id: &str, channel: &str) -> DeskTell {
    host.handle(
        "safari",
        &[DeskAsk::Call {
            id: id.into(),
            channel: channel.into(),
            args: desk_encode_args(&[DeskValue::Json(json!({}))]),
        }],
    );
    host.flush();
    told.lock()
        .unwrap()
        .iter()
        .flat_map(|(_, items)| items.iter())
        .find(|item| matches!(item, DeskTell::ResOk { id: i, .. } | DeskTell::ResErr { id: i, .. } if i == id))
        .cloned()
        .expect("expected response")
}

#[test]
// case: draws every page, and none of them starts by saying a setting could not be read or saved
fn draws_every_page_and_none_of_them_starts_by_saying_a_setting_could_not_be_read_or_saved() {
    let (host, _) = test_host();
    assert_eq!(BROWSER_SETTINGS_PAGES.len(), 13);
    for each in BROWSER_SETTINGS_PAGES {
        let html = host
            .settings_page(each.id, true)
            .unwrap_or_else(|| panic!("missing page {}", each.id));
        assert!(!html.is_empty(), "{}", each.id);
        let lower = html.to_ascii_lowercase();
        assert!(!lower.contains("could not be read"), "{}", each.id);
        assert!(!lower.contains("could not be saved"), "{}", each.id);
        assert!(!lower.contains("could not be changed"), "{}", each.id);
    }
}

#[test]
// case: says, in place of the sections whose every setting stays on the computer, that they work there
fn says_in_place_of_the_sections_whose_every_setting_stays_on_the_computer_that_they_work_there() {
    let (host, told) = test_host();
    let html = host.settings_page("app", true).expect("app page");
    for heading in [
        "Your phone",
        "Your account",
        "Chat apps",
        "Let your other AI apps use Locust",
    ] {
        let prefix = format!("<h2 class=\"lc-settings__heading\">{heading}</h2><p class=\"lc-settings__lede\">");
        let start = html
            .find(&prefix)
            .unwrap_or_else(|| panic!("missing heading {heading} in {html}"));
        let rest = &html[start + prefix.len()..];
        let end_lede = rest.find("</p>").expect("closing lede");
        let after = &rest[end_lede + "</p>".len()..];
        let expected_note = format!(
            "<p class=\"lc-settings__note lc-oncomputer__said\">{WINDOW_ONLY_SAYS}</p>"
        );
        assert!(
            after.starts_with(&expected_note),
            "expected computer-only note after {heading}, got: {after}"
        );
    }
    // The chat apps' rows, which read settings a browser is refused, are not drawn at all.
    assert!(!html.contains("data-chat-app="));

    // Confirm DeskHost itself refuses those sections' channels with WINDOW_ONLY_SAYS.
    host.handle("safari", &[DeskAsk::Take { id: "t".into() }]);
    for (idx, ch) in [
        "phone:settings",
        "account:status",
        "discord:settings",
        "locust-mcp:set",
    ]
    .iter()
    .enumerate()
    {
        let res = call_result(&host, &told, &format!("c{idx}"), ch);
        assert_eq!(
            res,
            DeskTell::ResErr {
                id: format!("c{idx}"),
                message: WINDOW_ONLY_SAYS.into(),
                refused: true,
            },
            "{ch}"
        );
    }
}

#[test]
// case: keeps the teammates' switches that grant something on the computer, and lets Plans change from here
fn keeps_the_teammates_switches_that_grant_something_on_the_computer_and_lets_plans_change_from_here()
{
    let (host, told) = test_host();
    let html = host.settings_page("teammates", true).expect("teammates page");
    for label in ["Swarm", "Your skills", "Auto mode"] {
        let disabled_prefix = format!(
            "<fieldset class=\"lc-oncomputer\" disabled=\"\"><div class=\"lc-settingline\"><div class=\"lc-settingline__text\"><h2 class=\"lc-settings__heading\">{label}</h2>"
        );
        assert!(
            html.contains(&disabled_prefix),
            "expected {label} to be held on the computer in: {html}"
        );
    }
    let plans_disabled = "<fieldset class=\"lc-oncomputer\" disabled=\"\"><div class=\"lc-settingline\"><div class=\"lc-settingline__text\"><h2 class=\"lc-settings__heading\">Plans</h2>";
    assert!(!html.contains(plans_disabled), "Plans should change from a browser");
    assert!(html.contains("<h2 class=\"lc-settings__heading\">Plans</h2>"));

    // Confirm DeskHost enforces the exact same split.
    host.handle("safari", &[DeskAsk::Take { id: "t".into() }]);
    assert_eq!(
        call_result(&host, &told, "w1", "workspace-settings:write"),
        DeskTell::ResErr {
            id: "w1".into(),
            message: WINDOW_ONLY_SAYS.into(),
            refused: true,
        }
    );
    assert!(matches!(
        call_result(&host, &told, "p1", "workspace-settings:change"),
        DeskTell::ResOk { .. }
    ));
}

#[test]
// case: lets the look change from here: no part of Appearance is held on the computer
fn lets_the_look_change_from_here_no_part_of_appearance_is_held_on_the_computer() {
    let (host, told) = test_host();
    let html = host.settings_page("appearance", true).expect("appearance page");
    assert!(!html.contains("lc-oncomputer"), "{html}");

    host.handle("safari", &[DeskAsk::Take { id: "t".into() }]);
    assert!(matches!(
        call_result(&host, &told, "a1", "appearance:set"),
        DeskTell::ResOk { .. }
    ));
}

#[test]
// case: keeps asking before connector calls on the computer: it is what teammates may reach
fn keeps_asking_before_connector_calls_on_the_computer_it_is_what_teammates_may_reach() {
    let (host, _) = test_host();
    let html = host.settings_page("connectors", true).expect("connectors page");
    let needle = "aria-label=\"Ask before every connector call\"";
    let pos = html.find(needle).expect("connector switch");
    let before = &html[..pos];
    let last_open = before
        .rfind("<fieldset class=\"lc-oncomputer\" disabled=\"\">")
        .expect("disabled lc-oncomputer fieldset before connector switch");
    let between = &before[last_open..];
    assert!(
        !between.contains("</fieldset>"),
        "connector switch must sit inside the disabled fieldset: {between}"
    );
}

#[test]
// case: holds nothing back and says nothing about a browser, on any page
fn holds_nothing_back_and_says_nothing_about_a_browser_on_any_page() {
    let (host, _) = test_host();
    for each in BROWSER_SETTINGS_PAGES {
        let html = host
            .settings_page(each.id, false)
            .unwrap_or_else(|| panic!("missing page {}", each.id));
        assert!(!html.contains("lc-oncomputer"), "{}", each.id);
        assert!(!html.contains(WINDOW_ONLY_SAYS), "{}", each.id);
    }
    // On the computer, Chat apps rows are drawn.
    let app_html = host.settings_page("app", false).expect("app page");
    assert!(app_html.contains("data-chat-app="));
}
