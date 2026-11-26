//! Settings › General › Help, drawn in the real Settings pane (`settings_panel::view`) as Electron's Screens.tsx draws
//! it: the heading, the lede, and the receipt of the address and what each way carries. Electron has no test of its own
//! for this section (its address is held by `a-report-can-reach-support-by-email-or-file.test.ts`, carried in
//! core/src/report_problem.rs).
use super::*;
use crate::settings_pages::SettingsPageId;
use rune::{Env, Runtime};

fn general() -> Runtime<State> {
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(SettingsPageId::App);
    let mut rt = Runtime::new(s, |s: &State, _: &mut Cx<State>| crate::settings_panel::view(s, &Palette::new(true)), (1200.0, 9000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn words_under(rt: &Runtime<State>, key: &str) -> Vec<String> {
    let ([x, y, w, h], _) = rt.presented(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect()
}

#[test]
fn says_how_to_reach_locusts_makers_under_its_own_heading() {
    let rt = general();
    let said = words_under(&rt, "settings-help");
    assert_eq!(said.first().map(String::as_str), Some("Help"), "{said:?}");
    assert!(said.iter().any(|t| t == "Stuck, or something looks wrong? Two ways to reach the people who make Locust."), "{said:?}");
    assert!(!said.iter().any(|t| t == "runs in Locust for now"), "{said:?}");
    let heading = rt.elements().into_iter().find(|e| e.text.as_deref() == Some("Help") && e.role == "heading").expect("Help is a heading");
    assert_eq!(heading.level, Some(2));
}

#[test]
fn lists_the_private_address_who_reads_it_the_public_bug_page_and_long_reports() {
    let rt = general();
    let said = words_under(&rt, "settings-help-receipt");
    assert_eq!(
        said,
        vec![
            "Email",
            "support@locust.lol", // public-guard: allow (Locust's own published support address, shared/support.ts)
            "Who reads it",
            "Only Locust\u{2019}s makers. Send feedback above fills the email in: what you wrote, your Locust version and system, and a conversation only if you include it.",
            "Bug reports",
            "Send feedback above also opens a GitHub issue, filled in the same way. Issues are public and need a GitHub account.",
            "Long reports",
            "An email cuts off a long conversation. Save as a file keeps all of it; attach the file to the email or the issue.",
        ]
    );
}

#[test]
fn comes_after_report_a_problem_whose_send_feedback_it_names() {
    let rt = general();
    let all = rt.elements();
    let top = |words: &str| all.iter().find(|e| e.text.as_deref() == Some(words)).map(|e| e.rect[1]).unwrap_or_else(|| panic!("no {words}"));
    assert!(rt.find("settings-send-feedback").is_some(), "Send feedback is on the page");
    assert!(top("Report a problem") < top("Help"));
}
